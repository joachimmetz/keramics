/* Copyright 2024-2026 Joachim Metz <joachim.metz@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License. You may
 * obtain a copy of the License at https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
 * WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
 * License for the specific language governing permissions and limitations
 * under the License.
 */

use std::cmp::min;

use keramics_core::ErrorTrace;

use crate::lru_cache::LruCache;
use crate::traits::BlockReader;

use super::constants::*;
use super::file::AffFile;
use super::segment::AffSegment;

/// AFF block reader.
///
/// The data of a page is decompressed (if needed) on the first read and cached.
pub(super) struct AffBlockReader {
    /// AFF file.
    file: AffFile,

    /// Page size in bytes.
    page_size: u64,

    /// Media size in bytes.
    media_size: u64,

    /// Page cache.
    page_cache: LruCache<usize, Vec<u8>>,
}

impl AffBlockReader {
    /// Creates a new AFF block reader.
    pub fn new(aff_file: &AffFile) -> Self {
        Self {
            file: aff_file.clone(),
            page_size: aff_file.get_page_size(),
            media_size: aff_file.get_media_size(),
            page_cache: LruCache::new(8),
        }
    }

    /// Retrieves the data of a page, decompressing it when needed.
    fn read_page(&mut self, page_index: usize) -> Result<Vec<u8>, ErrorTrace> {
        if self.page_cache.contains(&page_index) {
            match self.page_cache.get(&page_index) {
                Some(page) => return Ok(page.clone()),
                None => {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Unable to retrieve data of page: {} from cache",
                        page_index
                    )));
                }
            }
        }
        // A page that is not stored in the file consists of zero bytes.
        let segment_name: String = format!("page{}", page_index);
        let segment: Option<AffSegment> = self.file.get_segment(segment_name.as_str()).cloned();
        let page: Vec<u8> = match segment {
            Some(segment) => self.read_page_data(&segment)?,
            None => vec![0; self.page_size as usize],
        };
        self.page_cache.insert(page_index, page.clone());

        Ok(page)
    }

    /// Reads the data of a page segment.
    fn read_page_data(&mut self, segment: &AffSegment) -> Result<Vec<u8>, ErrorTrace> {
        let flag: u32 = segment.get_flag();
        let page_size: usize = self.page_size as usize;

        if (flag & AFF_PAGE_FLAG_ZERO) == AFF_PAGE_FLAG_ZERO {
            // A zero page consists entirely of zero bytes; the segment data
            // only contains the number of consecutive zero bytes.
            return Ok(vec![0; page_size]);
        }
        if (flag & AFF_PAGE_FLAG_COMPRESSED) == 0 {
            let page: Vec<u8> = self.file.get_segment_data(segment.get_name())?;

            if page.len() > page_size {
                return Err(keramics_core::error_trace_new!(format!(
                    "Page data size: {} is larger than the page size: {} of segment: {}",
                    page.len(),
                    page_size,
                    segment.get_name()
                )));
            }
            return Ok(page);
        }
        let compression_algorithm: u32 = flag & AFF_PAGE_COMPRESSION_ALGORITHM_MASK;

        match compression_algorithm {
            AFF_PAGE_COMPRESSION_ALGORITHM_ZLIB => {
                let compressed_data: Vec<u8> = self.file.get_segment_data(segment.get_name())?;
                let mut page: Vec<u8> = vec![0; page_size];

                let uncompressed_data_size: usize = crate::zlib_decompress!(
                    compressed_data.as_slice(),
                    &mut page,
                    format!(
                        "Unable to decompress page data of segment: {}",
                        segment.get_name()
                    )
                );
                page.truncate(uncompressed_data_size);

                Ok(page)
            }
            AFF_PAGE_COMPRESSION_ALGORITHM_BZIP2
            | AFF_PAGE_COMPRESSION_ALGORITHM_LZMA
            | AFF_PAGE_COMPRESSION_ALGORITHM_ZERO => Err(keramics_core::error_trace_new!(format!(
                "Unsupported page compression algorithm: 0x{:04x} of segment: {}",
                compression_algorithm,
                segment.get_name()
            ))),
            _ => Err(keramics_core::error_trace_new!(format!(
                "Unknown page compression algorithm: 0x{:04x} of segment: {}",
                compression_algorithm,
                segment.get_name()
            ))),
        }
    }
}

impl BlockReader for AffBlockReader {
    /// Retrieves the size of the data.
    fn get_size(&self) -> u64 {
        self.media_size
    }

    /// Reads the data from the pages of the image.
    fn read_data_from_blocks(&mut self, data: &mut [u8], offset: u64) -> Result<usize, ErrorTrace> {
        let read_size: usize = data.len();
        let mut data_offset: usize = 0;
        let mut current_offset: u64 = offset;

        while data_offset < read_size {
            if current_offset >= self.media_size {
                break;
            }
            let page_index: usize = (current_offset / self.page_size) as usize;
            let page: Vec<u8> = match self.read_page(page_index) {
                Ok(page) => page,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!(
                            "Unable to read page data at offset: {} (0x{:08x})",
                            current_offset, current_offset
                        )
                    );
                    return Err(error);
                }
            };
            let page_relative_offset: usize = (current_offset % self.page_size) as usize;
            let page_read_size: usize = min(
                read_size - data_offset,
                page.len().saturating_sub(page_relative_offset),
            );
            if page_read_size == 0 {
                break;
            }
            data[data_offset..data_offset + page_read_size].copy_from_slice(
                &page[page_relative_offset..page_relative_offset + page_read_size],
            );
            data_offset += page_read_size;
            current_offset += page_read_size as u64;
        }
        Ok(data_offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::SeekFrom;
    use std::path::PathBuf;

    use keramics_core::{DataStreamReference, open_os_data_stream};

    use crate::file_resolver::FileResolverReference;
    use crate::os_file_resolver::OsFileResolver;
    use crate::path_component::PathComponent;
    use crate::tests::get_test_data_path;

    fn get_block_reader(file_name: &str) -> Result<AffBlockReader, ErrorTrace> {
        let mut file: AffFile = AffFile::new();

        let path_string: String = get_test_data_path("aff");
        let path_buf: PathBuf = PathBuf::from(path_string);
        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(OsFileResolver::new(path_buf)));

        let aff_file_name: PathComponent = PathComponent::from(file_name);
        file.open(&file_resolver, &aff_file_name)
            .expect("Unable to open AFF file");

        Ok(AffBlockReader::new(&file))
    }

    fn get_raw_image_data() -> Result<Vec<u8>, ErrorTrace> {
        let path_string: String = get_test_data_path("ext/ext2.raw");
        let path_buf: PathBuf = PathBuf::from(path_string);
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;

        let size: u64 = keramics_core::data_stream_get_size!(data_stream.clone());
        let mut data: Vec<u8> = vec![0; size as usize];

        keramics_core::data_stream_read_at_position!(
            data_stream.clone(),
            &mut data,
            SeekFrom::Start(0)
        );

        Ok(data)
    }

    #[test]
    fn test_get_size() -> Result<(), ErrorTrace> {
        let reader: AffBlockReader = get_block_reader("ext2.AFF")?;

        assert_eq!(reader.get_size(), 4194304);

        Ok(())
    }

    #[test]
    fn test_read_data_from_blocks() -> Result<(), ErrorTrace> {
        let mut reader: AffBlockReader = get_block_reader("ext2.AFF")?;
        let raw_image_data: Vec<u8> = get_raw_image_data()?;

        let mut data: Vec<u8> = vec![0; raw_image_data.len()];
        let read_size: usize = reader.read_data_from_blocks(&mut data, 0)?;

        assert_eq!(read_size, raw_image_data.len());
        assert_eq!(data, raw_image_data);

        Ok(())
    }

    #[test]
    fn test_read_data_from_blocks_with_partial_read() -> Result<(), ErrorTrace> {
        let mut reader: AffBlockReader = get_block_reader("ext2.AFF")?;
        let raw_image_data: Vec<u8> = get_raw_image_data()?;

        let mut data: Vec<u8> = vec![0; 512];
        let read_size: usize = reader.read_data_from_blocks(&mut data, 512)?;

        assert_eq!(read_size, 512);
        assert_eq!(data, raw_image_data[512..1024]);

        Ok(())
    }

    #[test]
    fn test_read_data_from_blocks_with_offset_past_end() -> Result<(), ErrorTrace> {
        let mut reader: AffBlockReader = get_block_reader("ext2.AFF")?;

        let size: u64 = reader.get_size();

        let mut data: Vec<u8> = vec![0; 512];
        let read_size: usize = reader.read_data_from_blocks(&mut data, size)?;

        assert_eq!(read_size, 0);

        Ok(())
    }

    #[test]
    fn test_read_data_from_blocks_with_uncompressed_data() -> Result<(), ErrorTrace> {
        let mut reader: AffBlockReader = get_block_reader("ext2_uncompressed.AFF")?;
        let raw_image_data: Vec<u8> = get_raw_image_data()?;

        let mut data: Vec<u8> = vec![0; raw_image_data.len()];
        let read_size: usize = reader.read_data_from_blocks(&mut data, 0)?;

        assert_eq!(read_size, raw_image_data.len());
        assert_eq!(data, raw_image_data);

        Ok(())
    }
}

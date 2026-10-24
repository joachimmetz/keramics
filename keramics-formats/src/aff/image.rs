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

use std::sync::{Arc, RwLock};

use keramics_core::{DataStreamReference, ErrorTrace};

use crate::fake_file_resolver::FakeFileResolver;
use crate::file_resolver::FileResolverReference;
use crate::path_component::PathComponent;

use super::block_reader::AffBlockReader;
use super::block_stream::AffBlockStream;
use super::constants::*;
use super::file::AffFile;

/// AFF image.
pub struct AffImage {
    /// File resolver.
    file_resolver: FileResolverReference,

    /// Name.
    name: String,

    /// AFF file.
    aff_file: AffFile,

    /// Number of bytes per sector.
    bytes_per_sector: u32,

    /// Page size in bytes.
    page_size: u64,

    /// Number of stored pages of the image.
    number_of_pages: usize,

    /// Media size in bytes.
    media_size: u64,
}

impl Default for AffImage {
    /// Creates a new default storage media image.
    fn default() -> Self {
        Self::new()
    }
}

impl AffImage {
    /// Creates a new storage media image.
    pub fn new() -> Self {
        Self {
            file_resolver: FileResolverReference::new(Box::new(FakeFileResolver::new())),
            name: String::new(),
            aff_file: AffFile::new(),
            bytes_per_sector: AFF_DEFAULT_BYTES_PER_SECTOR,
            page_size: AFF_DEFAULT_PAGE_SIZE,
            number_of_pages: 0,
            media_size: 0,
        }
    }

    /// Retrieves the number of bytes per sector.
    pub fn get_bytes_per_sector(&self) -> u32 {
        self.bytes_per_sector
    }

    /// Retrieves the pattern used to mark a bad sector.
    pub fn get_bad_flag(&self) -> &[u8] {
        self.aff_file.get_bad_flag()
    }

    /// Retrieves a data stream.
    pub fn get_data_stream(&self) -> DataStreamReference {
        Arc::new(RwLock::new(AffBlockStream::new(AffBlockReader::new(
            &self.aff_file,
        ))))
    }

    /// Retrieves the name.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Retrieves the media size.
    pub fn get_media_size(&self) -> u64 {
        self.media_size
    }

    /// Determines the number of bad sectors.
    pub fn get_number_of_bad_sectors(&self) -> u64 {
        self.aff_file.get_number_of_bad_sectors()
    }

    /// Determines the number of segments of the image.
    pub fn get_number_of_segments(&self) -> usize {
        self.aff_file.get_number_of_segments()
    }

    /// Determines the number of stored pages of the image.
    pub fn get_number_of_pages(&self) -> usize {
        self.number_of_pages
    }

    /// Retrieves the data of an image segment.
    pub fn get_segment_data(&self, name: &str) -> Result<Vec<u8>, ErrorTrace> {
        match self.aff_file.get_segment_data(name) {
            Ok(data) => Ok(data),
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!("Unable to read segment data of segment: {}", name)
                );
                Err(error)
            }
        }
    }

    /// Retrieves the page size.
    pub fn get_page_size(&self) -> u64 {
        self.page_size
    }

    /// Opens a storage media image.
    pub fn open(
        &mut self,
        file_resolver: &FileResolverReference,
        file_name: &PathComponent,
    ) -> Result<(), ErrorTrace> {
        let mut aff_file: AffFile = AffFile::new();

        match aff_file.open(file_resolver, file_name) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open AFF file");
                return Err(error);
            }
        }
        self.aff_file = aff_file;
        self.file_resolver = file_resolver.clone();
        self.name = file_name.to_string();
        self.bytes_per_sector = self.aff_file.get_bytes_per_sector();
        self.page_size = self.aff_file.get_page_size();
        self.number_of_pages = self.aff_file.get_number_of_pages();
        self.media_size = self.aff_file.get_media_size();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cmp::min;
    use std::io::SeekFrom;
    use std::path::PathBuf;

    use keramics_core::{DataStreamReference, open_os_data_stream};

    use crate::os_file_resolver::OsFileResolver;

    use crate::tests::get_test_data_path;

    fn get_image() -> Result<AffImage, ErrorTrace> {
        let mut image: AffImage = AffImage::new();

        let path_string: String = get_test_data_path("aff");
        let path_buf: PathBuf = PathBuf::from(path_string);
        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(OsFileResolver::new(path_buf)));

        let file_name: PathComponent = PathComponent::from("ext2.AFF");
        image.open(&file_resolver, &file_name)?;

        Ok(image)
    }

    fn get_image_from(file_name: &str) -> Result<AffImage, ErrorTrace> {
        let mut image: AffImage = AffImage::new();

        let path_string: String = get_test_data_path("aff");
        let path_buf: PathBuf = PathBuf::from(path_string);
        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(OsFileResolver::new(path_buf)));

        let file_name_component: PathComponent = PathComponent::from(file_name);
        image.open(&file_resolver, &file_name_component)?;

        Ok(image)
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
    fn test_get_bytes_per_sector() -> Result<(), ErrorTrace> {
        let image: AffImage = get_image()?;

        let bytes_per_sector: u32 = image.get_bytes_per_sector();
        assert_eq!(bytes_per_sector, 512);

        Ok(())
    }

    #[test]
    fn test_get_bad_flag() -> Result<(), ErrorTrace> {
        let image: AffImage = get_image()?;

        let bad_flag: &[u8] = image.get_bad_flag();
        assert_eq!(bad_flag.len(), 512);
        assert_eq!(&bad_flag[0..10], b"BAD SECTOR");

        Ok(())
    }

    #[test]
    fn test_get_number_of_bad_sectors() -> Result<(), ErrorTrace> {
        let image: AffImage = get_image()?;

        let number_of_bad_sectors: u64 = image.get_number_of_bad_sectors();
        assert_eq!(number_of_bad_sectors, 0);

        Ok(())
    }

    #[test]
    fn test_get_number_of_segments() -> Result<(), ErrorTrace> {
        let image: AffImage = get_image()?;

        let number_of_segments: usize = image.get_number_of_segments();
        assert_eq!(number_of_segments, 13);

        Ok(())
    }

    #[test]
    fn test_get_segment_data() -> Result<(), ErrorTrace> {
        let image: AffImage = get_image()?;

        let segment_data: Vec<u8> = image.get_segment_data("md5")?;
        assert_eq!(segment_data.len(), 16);
        assert_eq!(&segment_data[0..4], [0xb1, 0x76, 0x0d, 0x0b]);

        Ok(())
    }

    #[test]
    fn test_get_segment_data_with_missing_segment() {
        let image: AffImage = get_image().expect("Unable to get image");

        let result: Result<Vec<u8>, ErrorTrace> = image.get_segment_data("sha256");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_name() -> Result<(), ErrorTrace> {
        let image: AffImage = get_image()?;

        assert_eq!(image.get_name(), "ext2.AFF");

        Ok(())
    }

    #[test]
    fn test_get_media_size() -> Result<(), ErrorTrace> {
        let image: AffImage = get_image()?;

        let media_size: u64 = image.get_media_size();
        assert_eq!(media_size, 4194304);

        Ok(())
    }

    #[test]
    fn test_get_page_size() -> Result<(), ErrorTrace> {
        let image: AffImage = get_image()?;

        let page_size: u64 = image.get_page_size();
        assert_eq!(page_size, 16777216);

        Ok(())
    }

    #[test]
    fn test_get_number_of_pages() -> Result<(), ErrorTrace> {
        let image: AffImage = get_image()?;

        let number_of_pages: usize = image.get_number_of_pages();
        assert_eq!(number_of_pages, 1);

        Ok(())
    }

    #[test]
    fn test_get_data_stream() -> Result<(), ErrorTrace> {
        let image: AffImage = get_image()?;

        let data_stream: DataStreamReference = image.get_data_stream();
        let size: u64 = keramics_core::data_stream_get_size!(data_stream.clone());
        assert_eq!(size, 4194304);

        let raw_image_data: Vec<u8> = get_raw_image_data()?;

        let mut data: Vec<u8> = vec![0; size as usize];
        let read_size: usize = keramics_core::data_stream_read_at_position!(
            data_stream.clone(),
            &mut data,
            SeekFrom::Start(0)
        );
        assert_eq!(read_size, size as usize);
        assert_eq!(data.len(), raw_image_data.len());
        assert_eq!(data, raw_image_data);

        // Verify the stream is re-readable at an offset.
        let mut block_data: Vec<u8> = vec![0; 512];
        let block_read_size: usize = keramics_core::data_stream_read_at_position!(
            data_stream.clone(),
            &mut block_data,
            SeekFrom::Start(512)
        );
        assert_eq!(block_read_size, 512);
        assert_eq!(
            block_data,
            raw_image_data[512..min(1024, raw_image_data.len())]
        );

        Ok(())
    }

    #[test]
    fn test_get_data_stream_of_uncompressed_image() -> Result<(), ErrorTrace> {
        let image: AffImage = get_image_from("ext2_uncompressed.AFF")?;

        let data_stream: DataStreamReference = image.get_data_stream();
        let size: u64 = keramics_core::data_stream_get_size!(data_stream.clone());
        assert_eq!(size, 4194304);

        let raw_image_data: Vec<u8> = get_raw_image_data()?;

        let mut data: Vec<u8> = vec![0; size as usize];
        let read_size: usize = keramics_core::data_stream_read_at_position!(
            data_stream.clone(),
            &mut data,
            SeekFrom::Start(0)
        );
        assert_eq!(read_size, size as usize);
        assert_eq!(data.len(), raw_image_data.len());
        assert_eq!(data, raw_image_data);

        Ok(())
    }

    #[test]
    fn test_open_with_missing_file() {
        let mut image: AffImage = AffImage::new();

        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(FakeFileResolver::new()));
        let file_name: PathComponent = PathComponent::from("ext2.AFF");

        let result: Result<(), ErrorTrace> = image.open(&file_resolver, &file_name);
        assert!(result.is_err());
    }
}

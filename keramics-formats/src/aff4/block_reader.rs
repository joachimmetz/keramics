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

use crate::traits::BlockReader;
use crate::zip::{ZipEntry, ZipFile};

/// Advanced Forensic Format 4 (AFF4) block reader.
///
/// The image entry stored in the volume is decompressed (if needed) on the
/// first read and cached.
pub struct Aff4BlockReader {
    /// Zip file of the volume.
    zip_file: ZipFile,

    /// Name of the image entry stored in the volume.
    image_entry_name: String,

    /// Cached image data.
    image_data: Option<Vec<u8>>,

    /// Size of the image data.
    size: u64,
}

impl Aff4BlockReader {
    /// Creates a new block reader.
    pub fn new(zip_file: &ZipFile, image_entry_name: &str, size: u64) -> Self {
        Self {
            zip_file: zip_file.clone(),
            image_entry_name: image_entry_name.to_string(),
            image_data: None,
            size,
        }
    }
}

impl BlockReader for Aff4BlockReader {
    /// Retrieves the size of the data.
    fn get_size(&self) -> u64 {
        self.size
    }

    /// Reads the image data from the volume.
    fn read_data_from_blocks(&mut self, data: &mut [u8], offset: u64) -> Result<usize, ErrorTrace> {
        if offset >= self.size {
            return Ok(0);
        }
        let read_size: usize = min(data.len(), (self.size - offset) as usize);

        if self.image_data.is_none() {
            let entry: ZipEntry = match self.zip_file.get_entry(&self.image_entry_name) {
                Some(entry) => entry.clone(),
                None => {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Missing image entry: {} in volume",
                        self.image_entry_name
                    )));
                }
            };
            let mut image_data: Vec<u8> = vec![0; self.size as usize];

            match self.zip_file.read_entry_data(&entry, &mut image_data) {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!(
                            "Unable to read image entry data of entry: {}",
                            self.image_entry_name
                        )
                    );
                    return Err(error);
                }
            }
            self.image_data = Some(image_data);
        }
        let image_data: &Vec<u8> = match self.image_data.as_ref() {
            Some(image_data) => image_data,
            None => {
                return Err(keramics_core::error_trace_new!("Missing image data"));
            }
        };
        let offset_index: usize = offset as usize;

        data[..read_size].copy_from_slice(&image_data[offset_index..(offset_index + read_size)]);

        Ok(read_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_core::{DataStreamReference, open_os_data_stream};

    use crate::file_resolver::FileResolverReference;
    use crate::os_file_resolver::OsFileResolver;
    use crate::tests::get_test_data_path;
    use crate::zip::ZipFile;

    fn get_zip_file() -> Result<ZipFile, ErrorTrace> {
        let path_string: String = get_test_data_path("aff4");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(OsFileResolver::new(path_buf)));

        let path_components: [crate::path_component::PathComponent; 1] =
            [crate::path_component::PathComponent::from("ext2.aff4")];
        let data_stream: DataStreamReference = match file_resolver.get_data_stream(&path_components)
        {
            Ok(Some(data_stream)) => data_stream,
            Ok(None) => {
                return Err(keramics_core::error_trace_new!("Missing AFF4 file"));
            }
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open AFF4 file");
                return Err(error);
            }
        };
        let mut zip_file: ZipFile = ZipFile::new();
        zip_file.read_data_stream(&data_stream)?;

        Ok(zip_file)
    }

    fn get_raw_image_data() -> Result<Vec<u8>, ErrorTrace> {
        let path_string: String = get_test_data_path("ext/ext2.raw");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;

        let size: u64 = keramics_core::data_stream_get_size!(data_stream.clone());
        let mut data: Vec<u8> = vec![0; size as usize];

        keramics_core::data_stream_read_at_position!(
            data_stream.clone(),
            &mut data,
            std::io::SeekFrom::Start(0)
        );

        Ok(data)
    }

    #[test]
    fn test_get_size() -> Result<(), ErrorTrace> {
        let raw_image_data: Vec<u8> = get_raw_image_data()?;

        let zip_file: ZipFile = get_zip_file()?;
        let block_reader: Aff4BlockReader = Aff4BlockReader::new(
            &zip_file,
            "test_data/ext/ext2.raw",
            raw_image_data.len() as u64,
        );

        let size: u64 = block_reader.get_size();
        assert_eq!(size, raw_image_data.len() as u64);

        Ok(())
    }

    #[test]
    fn test_read_data_from_blocks() -> Result<(), ErrorTrace> {
        let raw_image_data: Vec<u8> = get_raw_image_data()?;

        let zip_file: ZipFile = get_zip_file()?;
        let mut block_reader: Aff4BlockReader = Aff4BlockReader::new(
            &zip_file,
            "test_data/ext/ext2.raw",
            raw_image_data.len() as u64,
        );

        let mut data: Vec<u8> = vec![0; raw_image_data.len()];
        let read_size: usize = block_reader.read_data_from_blocks(&mut data, 0)?;
        assert_eq!(read_size, raw_image_data.len());
        assert_eq!(data, raw_image_data);

        let mut block_data: Vec<u8> = vec![0; 1024];
        let block_read_size: usize = block_reader.read_data_from_blocks(&mut block_data, 512)?;
        assert_eq!(block_read_size, 1024);
        assert_eq!(block_data, raw_image_data[512..1536]);

        Ok(())
    }

    #[test]
    fn test_read_data_from_blocks_with_offset_beyond_size() {
        let raw_image_data: Vec<u8> = get_raw_image_data().unwrap();

        let zip_file: ZipFile = get_zip_file().unwrap();
        let mut block_reader: Aff4BlockReader = Aff4BlockReader::new(
            &zip_file,
            "test_data/ext/ext2.raw",
            raw_image_data.len() as u64,
        );

        let mut data: Vec<u8> = vec![0; 512];
        let read_size: usize = block_reader
            .read_data_from_blocks(&mut data, raw_image_data.len() as u64)
            .unwrap();
        assert_eq!(read_size, 0);
    }

    // TODO: add tests for Aff4BlockStream
}

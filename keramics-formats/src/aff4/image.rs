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

use super::block_reader::Aff4BlockReader;
use super::block_stream::Aff4BlockStream;
use super::constants::*;
use super::file::Aff4File;

/// Advanced Forensic Format 4 (AFF4) image.
pub struct Aff4Image {
    /// File resolver.
    file_resolver: FileResolverReference,

    /// Name.
    name: String,

    /// Container URI.
    container_uri: String,

    /// Volume URI.
    volume_uri: String,

    /// Version major.
    version_major: u16,

    /// Version minor.
    version_minor: u16,

    /// Tool that created the image.
    tool: String,

    /// Bytes per sector.
    bytes_per_sector: u32,

    /// Media size.
    media_size: u64,

    /// Zip file of the volume.
    zip_file: crate::zip::ZipFile,

    /// Names of the image entries stored in the volume.
    image_entry_names: Vec<String>,
}

impl Default for Aff4Image {
    /// Creates a new default storage media image.
    fn default() -> Self {
        Self::new()
    }
}

impl Aff4Image {
    /// Creates a new storage media image.
    pub fn new() -> Self {
        Self {
            file_resolver: FileResolverReference::new(Box::new(FakeFileResolver::new())),
            name: String::new(),
            container_uri: String::new(),
            volume_uri: String::new(),
            version_major: 0,
            version_minor: 0,
            tool: String::new(),
            bytes_per_sector: 0,
            media_size: 0,
            zip_file: crate::zip::ZipFile::new(),
            image_entry_names: Vec::new(),
        }
    }

    /// Retrieves the bytes per sector.
    pub fn get_bytes_per_sector(&self) -> u32 {
        self.bytes_per_sector
    }

    /// Retrieves the container URI.
    pub fn get_container_uri(&self) -> &str {
        &self.container_uri
    }

    /// Retrieves a data stream.
    pub fn get_data_stream(&self) -> DataStreamReference {
        Arc::new(RwLock::new(Aff4BlockStream::new(Aff4BlockReader::new(
            &self.zip_file,
            self.get_image_entry_name(),
            self.media_size,
        ))))
    }

    /// Retrieves the data stream of the image with the specified index.
    pub fn get_data_stream_by_index(
        &self,
        index: usize,
    ) -> Result<DataStreamReference, ErrorTrace> {
        match self.image_entry_names.get(index) {
            Some(image_entry_name) => match self.zip_file.get_entry(image_entry_name) {
                Some(entry) => Ok(Arc::new(RwLock::new(Aff4BlockStream::new(
                    Aff4BlockReader::new(
                        &self.zip_file,
                        image_entry_name.as_str(),
                        entry.uncompressed_size as u64,
                    ),
                )))),
                None => Err(keramics_core::error_trace_new!(format!(
                    "Missing image entry: {} in volume",
                    image_entry_name
                ))),
            },
            None => Err(keramics_core::error_trace_new!(format!(
                "No image entry with index: {} in volume",
                index
            ))),
        }
    }

    /// Retrieves the name.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Retrieves the media size.
    pub fn get_media_size(&self) -> u64 {
        self.media_size
    }

    /// Retrieves the volume URI.
    pub fn get_volume_uri(&self) -> &str {
        &self.volume_uri
    }

    /// Retrieves the version major.
    pub fn get_version_major(&self) -> u16 {
        self.version_major
    }

    /// Retrieves the version minor.
    pub fn get_version_minor(&self) -> u16 {
        self.version_minor
    }

    /// Retrieves the tool that created the image.
    pub fn get_tool(&self) -> &str {
        &self.tool
    }

    /// Retrieves the name of the first image entry stored in the volume.
    pub fn get_image_entry_name(&self) -> &str {
        self.image_entry_names
            .first()
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// Retrieves the name of the image entry stored in the volume with the specified index.
    pub fn get_image_entry_name_by_index(&self, index: usize) -> Option<&str> {
        self.image_entry_names.get(index).map(String::as_str)
    }

    /// Retrieves the media size of the image with the specified index.
    pub fn get_media_size_by_index(&self, index: usize) -> Option<u64> {
        self.image_entry_names
            .get(index)
            .and_then(|image_entry_name| {
                self.zip_file
                    .get_entry(image_entry_name)
                    .map(|entry| entry.uncompressed_size as u64)
            })
    }

    /// Determines the number of images stored in the volume.
    pub fn get_number_of_images(&self) -> usize {
        self.image_entry_names.len()
    }

    /// Opens a storage media image.
    pub fn open(
        &mut self,
        file_resolver: &FileResolverReference,
        file_name: &PathComponent,
    ) -> Result<(), ErrorTrace> {
        let mut aff4_file: Aff4File = Aff4File::new();

        match aff4_file.open(file_resolver, file_name) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open AFF4 file");
                return Err(error);
            }
        }
        let container_uri: String = aff4_file.get_container_description().to_string();
        let volume_uri: String = aff4_file.get_volume_uri().to_string();
        let version_major: u16 = aff4_file.get_version().major;
        let version_minor: u16 = aff4_file.get_version().minor;
        let tool: String = aff4_file.get_version().tool.clone();
        let image_entry_names: Vec<String> = aff4_file.get_image_names();
        if image_entry_names.is_empty() {
            return Err(keramics_core::error_trace_new!(
                "Missing image entry in information container"
            ));
        }
        let media_size: u64 = match aff4_file.get_zip_file().get_entry(&image_entry_names[0]) {
            Some(entry) => entry.uncompressed_size as u64,
            None => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Missing image entry: {} in volume",
                    image_entry_names[0]
                )));
            }
        };
        self.zip_file = aff4_file.take_zip_file();
        self.image_entry_names = image_entry_names;
        self.container_uri = container_uri;
        self.volume_uri = volume_uri;
        self.version_major = version_major;
        self.version_minor = version_minor;
        self.tool = tool;
        self.bytes_per_sector = AFF4_DEFAULT_BYTES_PER_SECTOR;
        self.media_size = media_size;
        self.file_resolver = file_resolver.clone();
        self.name = file_name.to_string();

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

    fn get_image() -> Result<Aff4Image, ErrorTrace> {
        let mut image: Aff4Image = Aff4Image::new();

        let path_string: String = get_test_data_path("aff4");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(OsFileResolver::new(path_buf)));

        let file_name: PathComponent = PathComponent::from("ext2.aff4");
        image.open(&file_resolver, &file_name)?;

        Ok(image)
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
            SeekFrom::Start(0)
        );

        Ok(data)
    }

    #[test]
    fn test_get_bytes_per_sector() -> Result<(), ErrorTrace> {
        let image: Aff4Image = get_image()?;

        let bytes_per_sector: u32 = image.get_bytes_per_sector();
        assert_eq!(bytes_per_sector, 512);

        Ok(())
    }

    #[test]
    fn test_get_name() -> Result<(), ErrorTrace> {
        let image: Aff4Image = get_image()?;

        assert_eq!(image.get_name(), "ext2.aff4");

        Ok(())
    }

    #[test]
    fn test_get_media_size() -> Result<(), ErrorTrace> {
        let image: Aff4Image = get_image()?;

        let media_size: u64 = image.get_media_size();
        assert_eq!(media_size, 4194304);

        Ok(())
    }

    #[test]
    fn test_get_container_uri() -> Result<(), ErrorTrace> {
        let image: Aff4Image = get_image()?;

        assert_eq!(
            image.get_container_uri(),
            "aff4://00000000-0000-1003-8000-000000000004"
        );

        Ok(())
    }

    #[test]
    fn test_get_volume_uri() -> Result<(), ErrorTrace> {
        let image: Aff4Image = get_image()?;

        assert_eq!(
            image.get_volume_uri(),
            "aff4://00000000-0000-1003-8000-000000000004"
        );

        Ok(())
    }

    #[test]
    fn test_get_version() -> Result<(), ErrorTrace> {
        let image: Aff4Image = get_image()?;

        assert_eq!(image.get_version_major(), 1);
        assert_eq!(image.get_version_minor(), 0);
        assert_eq!(image.get_tool(), "libaff4 2.0 pre-release");

        Ok(())
    }

    #[test]
    fn test_get_image_entry_name() -> Result<(), ErrorTrace> {
        let image: Aff4Image = get_image()?;

        assert_eq!(image.get_image_entry_name(), "test_data/ext/ext2.raw");

        Ok(())
    }

    #[test]
    fn test_get_number_of_images() -> Result<(), ErrorTrace> {
        let image: Aff4Image = get_image()?;

        let number_of_images: usize = image.get_number_of_images();
        assert_eq!(number_of_images, 1);

        Ok(())
    }

    #[test]
    fn test_get_image_entry_name_by_index() -> Result<(), ErrorTrace> {
        let image: Aff4Image = get_image()?;

        assert_eq!(
            image.get_image_entry_name_by_index(0),
            Some("test_data/ext/ext2.raw")
        );
        assert_eq!(image.get_image_entry_name_by_index(1), None);

        Ok(())
    }

    #[test]
    fn test_get_media_size_by_index() -> Result<(), ErrorTrace> {
        let image: Aff4Image = get_image()?;

        assert_eq!(image.get_media_size_by_index(0), Some(4194304));
        assert_eq!(image.get_media_size_by_index(1), None);

        Ok(())
    }

    #[test]
    fn test_get_data_stream_by_index() -> Result<(), ErrorTrace> {
        let image: Aff4Image = get_image()?;

        let data_stream: DataStreamReference = image.get_data_stream_by_index(0)?;
        let size: u64 = keramics_core::data_stream_get_size!(data_stream.clone());
        assert_eq!(size, 4194304);

        let result: Result<DataStreamReference, ErrorTrace> = image.get_data_stream_by_index(1);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_get_data_stream() -> Result<(), ErrorTrace> {
        let image: Aff4Image = get_image()?;

        let data_stream: DataStreamReference = image.get_data_stream();
        let size: u64 = keramics_core::data_stream_get_size!(data_stream.clone());
        assert_eq!(size, 4194304);

        let mut data: Vec<u8> = vec![0; size as usize];
        let read_size: usize = keramics_core::data_stream_read_at_position!(
            data_stream.clone(),
            &mut data,
            SeekFrom::Start(0)
        );
        assert_eq!(read_size, size as usize);

        let raw_image_data: Vec<u8> = get_raw_image_data()?;
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

    // TODO: add tests for open
}

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
use super::standard::{Aff4StandardBlockStream, Aff4StandardMedia, Aff4StandardReader};

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

    /// Media of a standard AFF4 volume (None for a legacy volume).
    ///
    /// A standard AFF4 volume (for example AFF4 Standard v1.0a) stores its
    /// image as data stream and map nodes; the media reads the image through
    /// those nodes.
    standard_media: Option<Aff4StandardMedia>,
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
            standard_media: None,
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
        match self.standard_media.as_ref() {
            Some(media) => {
                return Arc::new(RwLock::new(Aff4StandardBlockStream::new(
                    Aff4StandardReader::new(media.clone()),
                )));
            }
            None => {}
        }
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
        if let Some(media) = self.standard_media.as_ref() {
            if index > 0 {
                return Err(keramics_core::error_trace_new!(format!(
                    "No image with index: {} in volume",
                    index
                )));
            }
            return Ok(Arc::new(RwLock::new(Aff4StandardBlockStream::new(
                Aff4StandardReader::new(media.clone()),
            ))));
        }
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
        if let Some(media) = self.standard_media.as_ref() {
            return match index {
                0 => Some(media.get_media_size()),
                _ => None,
            };
        }
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

        if aff4_file.is_standard() {
            // A standard AFF4 volume (for example AFF4 Standard v1.0a)
            // stores its image as data stream and map nodes; the media is
            // read through those nodes and its companion volumes.
            let media: Aff4StandardMedia = match Aff4StandardMedia::open(file_resolver, file_name) {
                Ok(media) => media,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to open AFF4 standard media"
                    );
                    return Err(error);
                }
            };
            let media_size: u64 = media.get_media_size();
            let image_entry_names: Vec<String> = vec![media.get_image_name().to_string()];
            let bytes_per_sector: u32 = media.get_bytes_per_sector();
            self.standard_media = Some(media);
            self.image_entry_names = image_entry_names;
            self.container_uri = container_uri;
            self.volume_uri = volume_uri;
            self.version_major = version_major;
            self.version_minor = version_minor;
            self.tool = tool;
            self.bytes_per_sector = bytes_per_sector;
            self.media_size = media_size;
            self.file_resolver = file_resolver.clone();
            self.name = file_name.to_string();

            return Ok(());
        }

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

    fn get_standard_image(file_name: &str) -> Result<Aff4Image, ErrorTrace> {
        let mut image: Aff4Image = Aff4Image::new();

        let path_string: String = get_test_data_path("aff4");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(OsFileResolver::new(path_buf)));

        let file_name: PathComponent = PathComponent::from(file_name);
        image.open(&file_resolver, &file_name)?;

        Ok(image)
    }

    /// Reads the bevy data entry of the `split.aff4` volume, used as the
    /// comparison data for the standard split volume test.
    fn get_split_entry_data() -> Result<Vec<u8>, ErrorTrace> {
        use crate::zip::ZipFile;

        let path_string: String = get_test_data_path("aff4/split.aff4");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;

        let mut zip_file: ZipFile = ZipFile::new();
        zip_file.read_data_stream(&data_stream)?;

        let entry: crate::zip::ZipEntry = zip_file
            .get_entry("file%3A/tmp/keramics_aff4_split.raw/00000000")
            .cloned()
            .ok_or_else(|| {
                keramics_core::error_trace_new!("Missing bevy data entry in split volume")
            })?;
        let mut data: Vec<u8> = vec![0; entry.uncompressed_size as usize];
        zip_file.read_entry_data(&entry, &mut data)?;

        Ok(data)
    }

    #[test]
    fn test_standard_split_image() -> Result<(), ErrorTrace> {
        let image: Aff4Image = get_standard_image("split.aff4")?;

        assert_eq!(image.get_name(), "split.aff4");
        assert_eq!(
            image.get_container_uri(),
            "aff4://95580ac9-74ba-4e75-8cf4-8e83224252e8"
        );
        assert_eq!(image.get_version_major(), 1);
        assert_eq!(image.get_version_minor(), 0);
        assert_eq!(image.get_tool(), "libaff4 2.0 pre-release");
        assert_eq!(image.get_bytes_per_sector(), 512);
        assert_eq!(image.get_media_size(), 12582912);
        assert_eq!(image.get_number_of_images(), 1);
        // The media size and image node are taken from the companion
        // metadata volume (`split.aff4.A01`).
        assert_eq!(
            image.get_image_entry_name(),
            "aff4://95580ac9-74ba-4e75-8cf4-8e83224252e8/file:/tmp/keramics_aff4_split.raw"
        );

        let data_stream: DataStreamReference = image.get_data_stream_by_index(0)?;
        let size: u64 = keramics_core::data_stream_get_size!(data_stream.clone());
        assert_eq!(size, 12582912);

        // The complete media read matches the bevy data entry; the chunks
        // are stored raw, so this covers the direct stream path.
        let mut data: Vec<u8> = vec![0; size as usize];
        let read_size: usize = keramics_core::data_stream_read_at_position!(
            data_stream.clone(),
            &mut data,
            SeekFrom::Start(0)
        );
        assert_eq!(read_size, size as usize);

        let entry_data: Vec<u8> = get_split_entry_data()?;
        assert_eq!(data.len(), entry_data.len());
        assert_eq!(data, entry_data);

        Ok(())
    }
}

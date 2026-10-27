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

use keramics_core::{DataStreamReference, ErrorTrace};

use crate::file_resolver::FileResolverReference;
use crate::path_component::PathComponent;
use crate::zip::{ZipEntry, ZipFile};

use super::constants::*;
use super::information::Aff4Information;
use super::version::Aff4Version;

/// Advanced Forensic Format 4 (AFF4) file.
///
/// An AFF4 volume is a Zip (ZIP) archive containing the metadata containers
/// (version.txt, container.description and information.turtle) and one or
/// more image entries.
pub struct Aff4File {
    /// Data stream.
    data_stream: Option<DataStreamReference>,

    /// Zip file.
    zip_file: ZipFile,

    /// Version stored in the version.txt file.
    version: Aff4Version,

    /// Container URI stored in the container.description file.
    container_description: String,

    /// Volume URI stored in the @base directive of the information.turtle file.
    volume_uri: String,

    /// Names of the image entries stored in the volume.
    image_names: Vec<String>,

    /// Information container of the volume.
    information: Aff4Information,

    /// True if the volume uses the standard AFF4 (for example AFF4 Standard v1.0a) stream and map representation.
    is_standard: bool,
}

impl Aff4File {
    /// Creates a new AFF4 file.
    pub fn new() -> Self {
        Self {
            data_stream: None,
            zip_file: ZipFile::new(),
            version: Aff4Version::new(),
            container_description: String::new(),
            volume_uri: String::new(),
            image_names: Vec::new(),
            information: Aff4Information::new(),
            is_standard: false,
        }
    }

    /// Retrieves the container URI.
    pub fn get_container_description(&self) -> &str {
        &self.container_description
    }

    /// Retrieves the names of the image entries stored in the volume.
    pub fn get_image_names(&self) -> Vec<String> {
        self.image_names.clone()
    }

    /// Retrieves the version stored in the version.txt file.
    pub fn get_version(&self) -> &Aff4Version {
        &self.version
    }

    /// Retrieves the volume URI.
    pub fn get_volume_uri(&self) -> &str {
        &self.volume_uri
    }

    /// Retrieves the Zip file.
    pub fn get_zip_file(&self) -> &ZipFile {
        &self.zip_file
    }

    /// Retrieves the Zip file and moves it out.
    pub fn take_zip_file(&mut self) -> ZipFile {
        let zip_file: ZipFile = self.zip_file.clone();
        self.zip_file = ZipFile::new();

        zip_file
    }

    /// Retrieves the information container and moves it out.
    pub fn take_information(&mut self) -> Aff4Information {
        let information: Aff4Information = self.information.clone();
        self.information = Aff4Information::new();

        information
    }

    /// Determines if the volume uses the standard AFF4 representation.
    pub fn is_standard(&self) -> bool {
        self.is_standard
    }

    /// Opens the AFF4 file.
    pub fn open(
        &mut self,
        file_resolver: &FileResolverReference,
        file_name: &PathComponent,
    ) -> Result<(), ErrorTrace> {
        let path_components: [PathComponent; 1] = [file_name.clone()];

        let data_stream: DataStreamReference = match file_resolver.get_data_stream(&path_components)
        {
            Ok(Some(data_stream)) => data_stream,
            Ok(None) => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Missing AFF4 file: {}",
                    file_name
                )));
            }
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!("Unable to open file: {}", file_name)
                );
                return Err(error);
            }
        };
        match self.read_data_stream(&data_stream) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!("Unable to read AFF4 file: {}", file_name)
                );
                return Err(error);
            }
        }
        self.data_stream = Some(data_stream);

        Ok(())
    }

    /// Reads the data of an entry from the volume.
    fn read_entry_data(&mut self, file_name: &str) -> Result<Vec<u8>, ErrorTrace> {
        let entry: ZipEntry = match self.zip_file.get_entry(file_name) {
            Some(entry) => entry.clone(),
            None => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Missing entry: {} in volume",
                    file_name
                )));
            }
        };
        let mut data: Vec<u8> = vec![0; entry.uncompressed_size as usize];

        match self.zip_file.read_entry_data(&entry, &mut data) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!("Unable to read entry data of entry: {}", file_name)
                );
                return Err(error);
            }
        }
        Ok(data)
    }

    /// Reads the AFF4 file.
    pub fn read_data_stream(
        &mut self,
        data_stream: &DataStreamReference,
    ) -> Result<(), ErrorTrace> {
        match self.zip_file.read_data_stream(data_stream) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read Zip file");
                return Err(error);
            }
        }
        let version_data: Vec<u8> = match self.read_entry_data(AFF4_VERSION_FILE_NAME) {
            Ok(data) => data,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read version entry data");
                return Err(error);
            }
        };
        let mut version: Aff4Version = Aff4Version::new();

        match version.read_data(&version_data) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read version data");
                return Err(error);
            }
        }
        if !version.is_supported() {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported AFF4 version: major={} minor={}",
                version.major, version.minor
            )));
        }
        self.version = version;

        let description_data: Vec<u8> =
            match self.read_entry_data(AFF4_CONTAINER_DESCRIPTION_FILE_NAME) {
                Ok(data) => data,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to read container description entry data"
                    );
                    return Err(error);
                }
            };
        let description: String = match String::from_utf8(description_data) {
            Ok(string) => string,
            Err(error) => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unsupported encoding of container description with error: {}",
                    error
                )));
            }
        };
        self.container_description = description.trim().to_string();

        let information_data: Vec<u8> = match self.read_entry_data(AFF4_INFORMATION_FILE_NAME) {
            Ok(data) => data,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to read information entry data"
                );
                return Err(error);
            }
        };
        let mut information: Aff4Information = Aff4Information::new();

        match information.read_data(&information_data) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read information data");
                return Err(error);
            }
        }
        if information.volume_uri.is_empty() {
            return Err(keramics_core::error_trace_new!(
                "Missing volume URI in information container"
            ));
        }
        let is_standard: bool = information.is_standard();
        // Standard AFF4 volumes (for example AFF4 Standard v1.0a) store their
        // image as data stream and map nodes. The node identities are not Zip
        // entry names and are resolved from the data stream and map nodes,
        // therefore the image entry names are not validated here.
        let image_names: Vec<String> = if is_standard {
            Vec::new()
        } else {
            if information.image_names.is_empty() {
                return Err(keramics_core::error_trace_new!(
                    "Missing image entry in information container"
                ));
            }
            let image_names: Vec<String> = information
                .image_names
                .iter()
                .filter(|image_name| self.zip_file.get_entry(image_name).is_some())
                .cloned()
                .collect();
            if image_names.is_empty() {
                return Err(keramics_core::error_trace_new!(
                    "Missing image entry in volume"
                ));
            }
            image_names
        };
        self.volume_uri = information.volume_uri.clone();
        self.image_names = image_names;
        self.information = information;
        self.is_standard = is_standard;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use crate::os_file_resolver::OsFileResolver;

    use crate::tests::get_test_data_path;

    fn get_file() -> Result<Aff4File, ErrorTrace> {
        let mut file: Aff4File = Aff4File::new();

        let path_string: String = get_test_data_path("aff4");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(OsFileResolver::new(path_buf)));

        let file_name: PathComponent = PathComponent::from("ext2.aff4");
        file.open(&file_resolver, &file_name)?;

        Ok(file)
    }

    #[test]
    fn test_get_container_description() -> Result<(), ErrorTrace> {
        let file: Aff4File = get_file()?;

        assert_eq!(
            file.get_container_description(),
            "aff4://00000000-0000-1003-8000-000000000004"
        );

        Ok(())
    }

    #[test]
    fn test_get_image_names() -> Result<(), ErrorTrace> {
        let file: Aff4File = get_file()?;

        assert_eq!(
            file.get_image_names(),
            vec!["test_data/ext/ext2.raw".to_string()]
        );

        Ok(())
    }

    #[test]
    fn test_get_version() -> Result<(), ErrorTrace> {
        let file: Aff4File = get_file()?;

        let version: &Aff4Version = file.get_version();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
        assert_eq!(version.tool, "libaff4 2.0 pre-release");

        Ok(())
    }

    #[test]
    fn test_get_volume_uri() -> Result<(), ErrorTrace> {
        let file: Aff4File = get_file()?;

        assert_eq!(
            file.get_volume_uri(),
            "aff4://00000000-0000-1003-8000-000000000004"
        );

        Ok(())
    }

    #[test]
    fn test_get_zip_file() -> Result<(), ErrorTrace> {
        let file: Aff4File = get_file()?;

        let zip_file: &ZipFile = file.get_zip_file();
        assert!(zip_file.get_entry(AFF4_VERSION_FILE_NAME).is_some());
        assert!(
            zip_file
                .get_entry(AFF4_CONTAINER_DESCRIPTION_FILE_NAME)
                .is_some()
        );
        assert!(zip_file.get_entry(AFF4_INFORMATION_FILE_NAME).is_some());
        assert!(zip_file.get_entry("test_data/ext/ext2.raw").is_some());
        assert!(zip_file.get_entry("test_data/missing.raw").is_none());

        Ok(())
    }
}

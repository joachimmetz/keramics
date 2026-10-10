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

use std::collections::HashSet;
use std::sync::Arc;

use keramics_core::{DataStreamReference, ErrorTrace};
use keramics_types::ByteString;

use crate::file_resolver::FileResolverReference;
use crate::path_component::PathComponent;

use super::file_system::CdFsFileSystem;
use super::volume::CdFsVolume;

/// CD file system (CDFS) volume set.
pub struct CdFsVolumeSet {
    /// Layers.
    volumes: Vec<Arc<CdFsVolume>>,

    /// Bytes per sector.
    bytes_per_sector: u16,

    /// Volume set identifier.
    volume_set_identifier: Option<ByteString>,
}

impl CdFsVolumeSet {
    /// Creates a new volume set.
    pub fn new() -> Self {
        Self {
            volumes: Vec::new(),
            bytes_per_sector: 0,
            volume_set_identifier: None,
        }
    }

    /// Retrieves the bytes per sector.
    pub fn get_bytes_per_sector(&self) -> u16 {
        self.bytes_per_sector
    }

    /// Retrieves the number of volumes.
    pub fn get_number_of_volumes(&self) -> usize {
        self.volumes.len()
    }

    /// Retrieves a volume by index.
    pub fn get_volume_by_index(&self, volume_index: usize) -> Result<Arc<CdFsVolume>, ErrorTrace> {
        match self.volumes.get(volume_index) {
            Some(volume) => Ok(volume.clone()),
            None => Err(keramics_core::error_trace_new!(format!(
                "No volume with index: {}",
                volume_index
            ))),
        }
    }

    /// Retrieves the file system.
    pub fn get_file_system(&self) -> Result<CdFsFileSystem, ErrorTrace> {
        if self.volumes.is_empty() {
            return Err(keramics_core::error_trace_new!("Missing volumes"));
        }
        Ok(CdFsFileSystem::new(&self.volumes))
    }

    /// Opens a volume set.
    pub fn open(
        &mut self,
        file_resolver: &FileResolverReference,
        file_names: &[PathComponent],
    ) -> Result<(), ErrorTrace> {
        for file_name in file_names.iter() {
            let path_components: [PathComponent; 1] = [file_name.clone()];

            let data_stream: DataStreamReference =
                match file_resolver.get_data_stream(&path_components) {
                    Ok(Some(data_stream)) => data_stream,
                    Ok(None) => {
                        return Err(keramics_core::error_trace_new!(format!(
                            "Missing data stream: {}",
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
            let mut volume: CdFsVolume = CdFsVolume::new();

            match volume.read_data_stream(&data_stream) {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(error, "Unable to read volume");
                    return Err(error);
                }
            }
            if self.volumes.is_empty() {
                self.volume_set_identifier = volume.get_volume_set_identifier().cloned();
            } else {
                let expected: Option<&ByteString> = self.volume_set_identifier.as_ref();
                let actual: Option<&ByteString> = volume.get_volume_set_identifier();
                if expected != actual {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Inconsistent volume set identifier: {:?} expected: {:?}",
                        actual, expected
                    )));
                }
            }

            self.volumes.push(Arc::new(volume));
        }
        self.bytes_per_sector = 2048;

        if self.volumes.is_empty() {
            return Err(keramics_core::error_trace_new!("Missing volumes"));
        }
        let volumes_in_set: u16 = self.volumes[0].get_volumes_in_set();

        if volumes_in_set == 0 {
            return Err(keramics_core::error_trace_new!(
                "Unsupported number of volumes in set: 0"
            ));
        }
        let mut volume_set_indices: HashSet<u16> = HashSet::with_capacity(self.volumes.len());

        for volume in self.volumes.iter() {
            let number_of_volumes_in_set: u16 = volume.get_volumes_in_set();
            if number_of_volumes_in_set != volumes_in_set {
                return Err(keramics_core::error_trace_new!(format!(
                    "Inconsistent number of volumes in set: {} expected: {}",
                    number_of_volumes_in_set, volumes_in_set
                )));
            }
            let volume_set_index: u16 = volume.get_volume_set_index();
            if volume_set_index == 0 || volume_set_index > volumes_in_set {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unsupported volume set index: {} out of range: 1 through {}",
                    volume_set_index, volumes_in_set
                )));
            }
            if !volume_set_indices.insert(volume_set_index) {
                return Err(keramics_core::error_trace_new!(format!(
                    "Duplicate volume set index: {}",
                    volume_set_index
                )));
            }
        }
        if volume_set_indices.len() != volumes_in_set as usize {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported number of volumes: {} expected: {}",
                volume_set_indices.len(), volumes_in_set
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use crate::os_file_resolver::OsFileResolver;

    use crate::tests::get_test_data_path;

    fn get_image() -> Result<CdFsVolumeSet, ErrorTrace> {
        let mut image: CdFsVolumeSet = CdFsVolumeSet::new();

        let path_string: String = get_test_data_path("cdfs");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(OsFileResolver::new(path_buf)));
        let file_names: [PathComponent; 1] = [PathComponent::from("level3.iso")];
        image.open(&file_resolver, &file_names)?;

        Ok(image)
    }

    #[test]
    fn test_get_bytes_per_sector() -> Result<(), ErrorTrace> {
        let image: CdFsVolumeSet = get_image()?;

        let bytes_per_sector: u16 = image.get_bytes_per_sector();
        assert_eq!(bytes_per_sector, 2048);

        Ok(())
    }

    #[test]
    fn test_get_number_of_volumes() -> Result<(), ErrorTrace> {
        let image: CdFsVolumeSet = get_image()?;

        let number_of_volumes: usize = image.get_number_of_volumes();
        assert_eq!(number_of_volumes, 1);

        Ok(())
    }

    #[test]
    fn test_get_volume_by_index() -> Result<(), ErrorTrace> {
        let image: CdFsVolumeSet = get_image()?;

        let volume: Arc<CdFsVolume> = image.get_volume_by_index(0)?;

        // TODO: implement volume.size
        // assert_eq!(volume.size, 4194304);

        Ok(())
    }

    #[test]
    fn test_get_file_system() -> Result<(), ErrorTrace> {
        let image: CdFsVolumeSet = get_image()?;

        let file_system: CdFsFileSystem = image.get_file_system()?;

        Ok(())
    }

    #[test]
    fn test_open() -> Result<(), ErrorTrace> {
        let mut image: CdFsVolumeSet = CdFsVolumeSet::new();

        let path_string: String = get_test_data_path("cdfs");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(OsFileResolver::new(path_buf)));
        let file_names: [PathComponent; 1] = [PathComponent::from("level3.iso")];
        image.open(&file_resolver, &file_names)?;

        assert_eq!(image.volumes.len(), 1);
        assert_eq!(image.bytes_per_sector, 2048);

        Ok(())
    }
}

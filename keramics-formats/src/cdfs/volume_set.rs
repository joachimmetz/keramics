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

use std::sync::Arc;

use keramics_core::{DataStreamReference, ErrorTrace};

use crate::file_resolver::FileResolverReference;
use crate::path_component::PathComponent;

use super::volume::CdFsVolume;

/// CD file system (CDFS) volume set.
pub struct CdFsVolumeSet {
    /// Layers.
    volumes: Vec<Arc<CdFsVolume>>,

    /// Bytes per sector.
    bytes_per_sector: u16,
}

impl CdFsVolumeSet {
    /// Creates a new volume set.
    pub fn new() -> Self {
        Self {
            volumes: Vec::new(),
            bytes_per_sector: 0,
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
            // TODO: compare volume set identifier

            self.volumes.push(Arc::new(volume));
        }
        self.bytes_per_sector = 2048;

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

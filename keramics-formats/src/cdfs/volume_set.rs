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

    /// Retrieves the file system.
    pub fn get_file_system(&self) -> Result<CdFsFileSystem, ErrorTrace> {
        if self.volumes.is_empty() {
            return Err(keramics_core::error_trace_new!("Missing volumes"));
        }
        Ok(CdFsFileSystem::new(&self.volumes))
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
                volume_set_indices.len(),
                volumes_in_set
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;
    use std::path::PathBuf;

    use keramics_core::open_fake_data_stream;

    use crate::cdfs::constants::*;
    use crate::fake_file_resolver::FakeFileResolver;
    use crate::file_resolver::FileResolver;
    use crate::os_file_resolver::OsFileResolver;
    use crate::tests::get_test_data_path;

    struct MemoryFileResolver {
        files: HashMap<String, Vec<u8>>,
    }

    impl MemoryFileResolver {
        fn new(files: Vec<(&str, Vec<u8>)>) -> Self {
            let mut file_map: HashMap<String, Vec<u8>> = HashMap::new();
            for (name, data) in files {
                file_map.insert(name.to_string(), data);
            }
            Self { files: file_map }
        }
    }

    impl FileResolver for MemoryFileResolver {
        fn get_data_stream(
            &self,
            path_components: &[PathComponent],
        ) -> Result<Option<DataStreamReference>, ErrorTrace> {
            let path: String = path_components
                .iter()
                .map(|component| component.to_string())
                .collect::<Vec<String>>()
                .join("/");
            match self.files.get(&path) {
                Some(data) => Ok(Some(open_fake_data_stream(data))),
                None => Ok(None),
            }
        }
    }

    fn build_test_volume_data(
        volumes_in_set: u16,
        volume_set_index: u16,
        volume_set_identifier: &[u8],
    ) -> Vec<u8> {
        let mut data: Vec<u8> = vec![0; 21 * 2048];
        let pvd_offset: usize = 16 * 2048;

        data[pvd_offset] = 1;
        data[pvd_offset + 1..pvd_offset + 6].copy_from_slice(CDFS_VOLUME_DESCRIPTOR_SIGNATURE);
        data[pvd_offset + 6] = 1;
        data[pvd_offset + 80..pvd_offset + 84].copy_from_slice(&21u32.to_le_bytes());
        data[pvd_offset + 84..pvd_offset + 88].copy_from_slice(&21u32.to_be_bytes());
        data[pvd_offset + 120..pvd_offset + 122].copy_from_slice(&volumes_in_set.to_le_bytes());
        data[pvd_offset + 122..pvd_offset + 124].copy_from_slice(&volumes_in_set.to_be_bytes());
        data[pvd_offset + 124..pvd_offset + 126].copy_from_slice(&volume_set_index.to_le_bytes());
        data[pvd_offset + 126..pvd_offset + 128].copy_from_slice(&volume_set_index.to_be_bytes());
        data[pvd_offset + 128..pvd_offset + 130].copy_from_slice(&2048u16.to_le_bytes());
        data[pvd_offset + 130..pvd_offset + 132].copy_from_slice(&2048u16.to_be_bytes());
        data[pvd_offset + 132..pvd_offset + 136].copy_from_slice(&10u32.to_le_bytes());
        data[pvd_offset + 136..pvd_offset + 140].copy_from_slice(&10u32.to_be_bytes());
        data[pvd_offset + 140..pvd_offset + 144].copy_from_slice(&18u32.to_le_bytes());
        data[pvd_offset + 148..pvd_offset + 152].copy_from_slice(&19u32.to_be_bytes());

        let root_offset: usize = pvd_offset + 156;
        data[root_offset] = 34;
        data[root_offset + 2..root_offset + 6].copy_from_slice(&20u32.to_le_bytes());
        data[root_offset + 6..root_offset + 10].copy_from_slice(&20u32.to_be_bytes());
        data[root_offset + 10..root_offset + 14].copy_from_slice(&2048u32.to_le_bytes());
        data[root_offset + 14..root_offset + 18].copy_from_slice(&2048u32.to_be_bytes());
        data[root_offset + 25] = CDFS_FILE_FLAG_DIRECTORY;
        data[root_offset + 28..root_offset + 30].copy_from_slice(&volume_set_index.to_le_bytes());
        data[root_offset + 30..root_offset + 32].copy_from_slice(&volume_set_index.to_be_bytes());
        data[root_offset + 32] = 1;

        if !volume_set_identifier.is_empty() {
            data[pvd_offset + 190..pvd_offset + 190 + volume_set_identifier.len()]
                .copy_from_slice(volume_set_identifier);
        }
        let term_offset: usize = 17 * 2048;
        data[term_offset..term_offset + 6].copy_from_slice(CDFS_VOLUME_DESCRIPTOR_SET_TERMINATOR);
        data[term_offset + 6] = 1;

        let le_pt_offset: usize = 18 * 2048;
        data[le_pt_offset..le_pt_offset + 10]
            .copy_from_slice(&[0x01, 0x00, 0x14, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
        let be_pt_offset: usize = 19 * 2048;
        data[be_pt_offset..be_pt_offset + 10]
            .copy_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x14, 0x00, 0x01, 0x00, 0x00]);

        data
    }

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

        assert_eq!(volume.volume_size, 2482176);

        Ok(())
    }

    #[test]
    fn test_get_file_system() -> Result<(), ErrorTrace> {
        let image: CdFsVolumeSet = get_image()?;

        _ = image.get_file_system()?;

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

    #[test]
    fn test_get_volume_by_index_with_invalid_index() -> Result<(), ErrorTrace> {
        let image: CdFsVolumeSet = get_image()?;

        let result: Result<Arc<CdFsVolume>, ErrorTrace> = image.get_volume_by_index(1);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_get_file_system_with_empty_volumes() {
        let image: CdFsVolumeSet = CdFsVolumeSet::new();

        let result: Result<CdFsFileSystem, ErrorTrace> = image.get_file_system();
        assert!(result.is_err());
    }

    #[test]
    fn test_open_with_multi_volume_set() -> Result<(), ErrorTrace> {
        let vol1: Vec<u8> = build_test_volume_data(2, 1, b"MYSET");
        let vol2: Vec<u8> = build_test_volume_data(2, 2, b"MYSET");
        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(MemoryFileResolver::new(vec![
                ("vol1.iso", vol1),
                ("vol2.iso", vol2),
            ])));
        let file_names: [PathComponent; 2] = [
            PathComponent::from("vol1.iso"),
            PathComponent::from("vol2.iso"),
        ];

        let mut volume_set: CdFsVolumeSet = CdFsVolumeSet::new();
        volume_set.open(&file_resolver, &file_names)?;
        assert_eq!(volume_set.get_number_of_volumes(), 2);

        Ok(())
    }

    #[test]
    fn test_open_with_errors() {
        let empty_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(FakeFileResolver::new()));

        // Test with no file names.
        let mut volume_set: CdFsVolumeSet = CdFsVolumeSet::new();
        let result: Result<(), ErrorTrace> = volume_set.open(&empty_resolver, &[]);
        assert!(result.is_err());

        // Test with missing file.
        let mut volume_set: CdFsVolumeSet = CdFsVolumeSet::new();
        let result: Result<(), ErrorTrace> =
            volume_set.open(&empty_resolver, &[PathComponent::from("missing.iso")]);
        assert!(result.is_err());

        // Test with invalid volume data.
        let bad_resolver: FileResolverReference = FileResolverReference::new(Box::new(
            MemoryFileResolver::new(vec![("bad.iso", vec![0; 100])]),
        ));
        let mut volume_set: CdFsVolumeSet = CdFsVolumeSet::new();
        let result: Result<(), ErrorTrace> =
            volume_set.open(&bad_resolver, &[PathComponent::from("bad.iso")]);
        assert!(result.is_err());

        // Test with inconsistent volume set identifier.
        let resolver: FileResolverReference =
            FileResolverReference::new(Box::new(MemoryFileResolver::new(vec![
                ("v1.iso", build_test_volume_data(2, 1, b"SET_A")),
                ("v2.iso", build_test_volume_data(2, 2, b"SET_B")),
            ])));
        let mut volume_set: CdFsVolumeSet = CdFsVolumeSet::new();
        let result: Result<(), ErrorTrace> = volume_set.open(
            &resolver,
            &[PathComponent::from("v1.iso"), PathComponent::from("v2.iso")],
        );
        assert!(result.is_err());

        // Test with zero volumes in set.
        let resolver: FileResolverReference = FileResolverReference::new(Box::new(
            MemoryFileResolver::new(vec![("v0.iso", build_test_volume_data(0, 1, b"SET"))]),
        ));
        let mut volume_set: CdFsVolumeSet = CdFsVolumeSet::new();
        let result: Result<(), ErrorTrace> =
            volume_set.open(&resolver, &[PathComponent::from("v0.iso")]);
        assert!(result.is_err());

        // Test with inconsistent number of volumes in set.
        let resolver: FileResolverReference =
            FileResolverReference::new(Box::new(MemoryFileResolver::new(vec![
                ("v1.iso", build_test_volume_data(2, 1, b"SET")),
                ("v2.iso", build_test_volume_data(3, 2, b"SET")),
            ])));
        let mut volume_set: CdFsVolumeSet = CdFsVolumeSet::new();
        let result: Result<(), ErrorTrace> = volume_set.open(
            &resolver,
            &[PathComponent::from("v1.iso"), PathComponent::from("v2.iso")],
        );
        assert!(result.is_err());

        // Test with volume set index out of range (0 and > volumes_in_set).
        for bad_index in [0u16, 2u16] {
            let resolver: FileResolverReference =
                FileResolverReference::new(Box::new(MemoryFileResolver::new(vec![(
                    "v.iso",
                    build_test_volume_data(1, bad_index, b"SET"),
                )])));
            let mut volume_set: CdFsVolumeSet = CdFsVolumeSet::new();
            let result: Result<(), ErrorTrace> =
                volume_set.open(&resolver, &[PathComponent::from("v.iso")]);
            assert!(result.is_err());
        }

        // Test with duplicate volume set index.
        let resolver: FileResolverReference =
            FileResolverReference::new(Box::new(MemoryFileResolver::new(vec![
                ("v1.iso", build_test_volume_data(2, 1, b"SET")),
                ("v2.iso", build_test_volume_data(2, 1, b"SET")),
            ])));
        let mut volume_set: CdFsVolumeSet = CdFsVolumeSet::new();
        let result: Result<(), ErrorTrace> = volume_set.open(
            &resolver,
            &[PathComponent::from("v1.iso"), PathComponent::from("v2.iso")],
        );
        assert!(result.is_err());

        // Test with incomplete volume set.
        let resolver: FileResolverReference = FileResolverReference::new(Box::new(
            MemoryFileResolver::new(vec![("v1.iso", build_test_volume_data(2, 1, b"SET"))]),
        ));
        let mut volume_set: CdFsVolumeSet = CdFsVolumeSet::new();
        let result: Result<(), ErrorTrace> =
            volume_set.open(&resolver, &[PathComponent::from("v1.iso")]);
        assert!(result.is_err());
    }
}

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

use super::directory_record::CdFsDirectoryRecord;
use super::file_entry::CdFsFileEntry;
use super::volume::CdFsVolume;

/// CD file system (CDFS) file system.
pub struct CdFsFileSystem {
    /// Layers.
    volumes: Vec<Arc<CdFsVolume>>,
}

impl CdFsFileSystem {
    /// Creates a new file system.
    pub(super) fn new(volumes: &[Arc<CdFsVolume>]) -> Self {
        Self {
            volumes: volumes.to_vec(),
        }
    }

    /// Retrieves the root directory.
    pub fn get_root_directory(&self) -> Result<&CdFsDirectoryRecord, ErrorTrace> {
        match self.volumes.first() {
            Some(volume) => Ok(volume.get_root_directory()),
            None => Err(keramics_core::error_trace_new!("Missing volumes")),
        }
    }

    /// Retrieves the root file entry.
    pub fn get_root_file_entry(&self) -> Result<CdFsFileEntry, ErrorTrace> {
        match self.volumes.first() {
            Some(volume) => {
                let data_stream: &DataStreamReference = match volume.get_data_stream() {
                    Some(data_stream) => data_stream,
                    None => {
                        return Err(keramics_core::error_trace_new!("Missing data stream"));
                    }
                };
                let bytes_per_sector: u16 = volume.get_bytes_per_sector();
                let directory_record: CdFsDirectoryRecord = volume.get_root_directory().clone();

                Ok(CdFsFileEntry::new(
                    data_stream,
                    bytes_per_sector,
                    directory_record,
                ))
            }
            None => Err(keramics_core::error_trace_new!("Missing volumes")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_core::{DataStreamReference, open_os_data_stream};
    use keramics_types::ByteString;

    use crate::traits::FileEntryIterator;

    use crate::tests::get_test_data_path;

    fn get_file_system() -> Result<CdFsFileSystem, ErrorTrace> {
        let mut volume: CdFsVolume = CdFsVolume::new();

        let path_string: String = get_test_data_path("cdfs/level3.iso");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;
        volume.read_data_stream(&data_stream)?;

        Ok(CdFsFileSystem::new(&[Arc::new(volume)]))
    }

    #[test]
    fn test_get_root_directory() -> Result<(), ErrorTrace> {
        let file_system: CdFsFileSystem = get_file_system()?;

        let root_directory: &CdFsDirectoryRecord = file_system.get_root_directory()?;
        assert_eq!(root_directory.data_start_sector, 23);
        assert_eq!(root_directory.data_size, 2048);
        assert_eq!(root_directory.file_flags, 0x02);
        assert!(root_directory.name.is_empty());

        Ok(())
    }

    #[test]
    fn test_get_root_file_entry() -> Result<(), ErrorTrace> {
        let file_system: CdFsFileSystem = get_file_system()?;

        let mut root_file_entry: CdFsFileEntry = file_system.get_root_file_entry()?;
        assert!(root_file_entry.is_directory());
        assert!(root_file_entry.is_root_directory());
        assert!(root_file_entry.get_name().is_none());

        let number_of_sub_file_entries: usize = root_file_entry.get_number_of_sub_file_entries()?;
        assert_eq!(number_of_sub_file_entries, 8);

        let sub_file_entry: CdFsFileEntry = root_file_entry.get_sub_file_entry_by_index(0)?;

        let name: Option<&ByteString> = sub_file_entry.get_name();
        let name: &ByteString =
            name.ok_or_else(|| keramics_core::error_trace_new!("Missing name"))?;
        assert_eq!(name.elements, b"EMPTYFILE.;1".to_vec());

        Ok(())
    }
}

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

use crate::path::Path;
use crate::traits::FileEntryIterator;

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

    /// Retrieves the file entry for a specific path.
    pub fn get_file_entry_by_path(&self, path: &Path) -> Result<Option<CdFsFileEntry>, ErrorTrace> {
        if path.is_empty() || path.is_relative() {
            return Ok(None);
        }
        let mut file_entry: CdFsFileEntry = match self.get_root_file_entry() {
            Ok(file_entry) => file_entry,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to retrieve root file entry");
                return Err(error);
            }
        };
        for path_component in path.components[1..].iter() {
            file_entry = match file_entry.get_sub_file_entry_by_name(path_component) {
                Ok(Some(file_entry)) => file_entry,
                Ok(None) => return Ok(None),
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!("Unable to retrieve sub file entry: {}", path_component)
                    );
                    return Err(error);
                }
            };
        }
        Ok(Some(file_entry))
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
                let directory_record_offset: u64 = volume.get_root_directory_record_offset();

                Ok(CdFsFileEntry::new(
                    data_stream,
                    bytes_per_sector,
                    directory_record,
                    directory_record_offset,
                ))
            }
            None => Err(keramics_core::error_trace_new!("Missing volumes")),
        }
    }

    /// Retrieves the file entry for a specific identifier, i.e. the offset of the directory
    /// record.
    pub fn get_file_entry_by_identifier(
        &self,
        cdfs_entry_identifier: u64,
    ) -> Result<CdFsFileEntry, ErrorTrace> {
        let file_entry: CdFsFileEntry = match self.get_root_file_entry() {
            Ok(file_entry) => file_entry,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to retrieve root file entry");
                return Err(error);
            }
        };
        match Self::get_file_entry_by_identifier_recursive(file_entry, cdfs_entry_identifier) {
            Ok(Some(file_entry)) => Ok(file_entry),
            Ok(None) => Err(keramics_core::error_trace_new!(format!(
                "Missing file entry for identifier: 0x{:08x}",
                cdfs_entry_identifier
            ))),
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!(
                        "Unable to retrieve file entry: 0x{:08x}",
                        cdfs_entry_identifier
                    )
                );
                return Err(error);
            }
        }
    }

    /// Recursively searches for a file entry with a specific identifier.
    fn get_file_entry_by_identifier_recursive(
        mut file_entry: CdFsFileEntry,
        cdfs_entry_identifier: u64,
    ) -> Result<Option<CdFsFileEntry>, ErrorTrace> {
        if file_entry.get_identifier() == cdfs_entry_identifier {
            return Ok(Some(file_entry));
        }
        if file_entry.is_directory() {
            let number_of_sub_file_entries: usize =
                match file_entry.get_number_of_sub_file_entries() {
                    Ok(number) => number,
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            "Unable to retrieve number of sub file entries"
                        );
                        return Err(error);
                    }
                };
            for sub_file_entry_index in 0..number_of_sub_file_entries {
                let sub_file_entry: CdFsFileEntry =
                    match file_entry.get_sub_file_entry_by_index(sub_file_entry_index) {
                        Ok(sub_file_entry) => sub_file_entry,
                        Err(mut error) => {
                            keramics_core::error_trace_add_frame!(
                                error,
                                format!(
                                    "Unable to retrieve sub file entry: {}",
                                    sub_file_entry_index
                                )
                            );
                            return Err(error);
                        }
                    };
                match Self::get_file_entry_by_identifier_recursive(
                    sub_file_entry,
                    cdfs_entry_identifier,
                ) {
                    Ok(Some(file_entry)) => return Ok(Some(file_entry)),
                    Ok(None) => {}
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            format!("Unable to search sub file entry: {}", sub_file_entry_index)
                        );
                        return Err(error);
                    }
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_core::{DataStreamReference, open_os_data_stream};
    use keramics_types::ByteString;

    use crate::path::Path;
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
    fn test_get_file_entry_by_path() -> Result<(), ErrorTrace> {
        let file_system: CdFsFileSystem = get_file_system()?;

        let path: Path = Path::from("/");
        let file_entry: CdFsFileEntry = file_system.get_file_entry_by_path(&path)?.unwrap();
        assert!(file_entry.is_root_directory());

        let path: Path = Path::from("/emptyfile");
        let file_entry: CdFsFileEntry = file_system.get_file_entry_by_path(&path)?.unwrap();
        assert!(!file_entry.is_directory());

        let path: Path = Path::from("/testdir1/testfile1");
        let file_entry: CdFsFileEntry = file_system.get_file_entry_by_path(&path)?.unwrap();
        assert!(!file_entry.is_directory());

        let path: Path = Path::from("/nonexistent");
        let result: Option<CdFsFileEntry> = file_system.get_file_entry_by_path(&path)?;
        assert!(result.is_none());

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

    #[test]
    fn test_get_file_entry_by_identifier() -> Result<(), ErrorTrace> {
        let file_system: CdFsFileSystem = get_file_system()?;

        let file_entry: CdFsFileEntry = file_system.get_file_entry_by_identifier(32924)?;
        assert!(file_entry.is_root_directory());

        let path: Path = Path::from("/emptyfile");
        let file_entry_by_path: CdFsFileEntry = file_system.get_file_entry_by_path(&path)?.unwrap();
        let file_entry: CdFsFileEntry =
            file_system.get_file_entry_by_identifier(file_entry_by_path.get_identifier())?;
        assert_eq!(file_entry.get_name(), file_entry_by_path.get_name());

        let result: Result<CdFsFileEntry, ErrorTrace> = file_system.get_file_entry_by_identifier(1);
        assert!(result.is_err());

        Ok(())
    }
}

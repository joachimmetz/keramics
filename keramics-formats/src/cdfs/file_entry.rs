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

use std::io::SeekFrom;

use keramics_core::{DataStreamReference, ErrorTrace};
use keramics_types::ByteString;

use crate::indexed_hash_map::IndexedHashMap;
use crate::traits::FileEntryIterator;

use super::constants::*;
use super::directory_record::CdFsDirectoryRecord;

/// CD file system (CDFS) file entry.
pub struct CdFsFileEntry {
    /// The data stream.
    data_stream: DataStreamReference,

    /// Bytes per sector.
    bytes_per_sector: u16,

    /// The directory record.
    directory_record: CdFsDirectoryRecord,

    /// The sub directory records.
    sub_directory_records: IndexedHashMap<ByteString, CdFsDirectoryRecord>,

    /// Value to indicate the sub directory records were read.
    sub_directory_records_read: bool,
}

impl CdFsFileEntry {
    /// Creates a new file entry.
    pub(super) fn new(
        data_stream: &DataStreamReference,
        bytes_per_sector: u16,
        directory_record: CdFsDirectoryRecord,
    ) -> Self {
        Self {
            data_stream: data_stream.clone(),
            bytes_per_sector,
            directory_record,
            sub_directory_records: IndexedHashMap::new(),
            sub_directory_records_read: false,
        }
    }

    /// Retrieves the name.
    pub fn get_name(&self) -> Option<&ByteString> {
        if self.directory_record.name.is_empty() {
            None
        } else {
            Some(&self.directory_record.name)
        }
    }

    /// Retrieves the size.
    pub fn get_size(&self) -> u64 {
        self.directory_record.data_size as u64
    }

    /// Determines if the file entry is a directory.
    pub fn is_directory(&self) -> bool {
        self.directory_record.file_flags & CDFS_DIRECTORY_FILE_FLAG_DIRECTORY
            == CDFS_DIRECTORY_FILE_FLAG_DIRECTORY
    }

    /// Determines if the file entry is the root directory.
    pub fn is_root_directory(&self) -> bool {
        self.directory_record.name.is_empty()
    }

    /// Reads the sub directory records.
    fn read_sub_directory_records(&mut self) -> Result<(), ErrorTrace> {
        if self.directory_record.data_size == 0 {
            return Err(keramics_core::error_trace_new!(
                "Missing directory record data size"
            ));
        }
        let offset: u64 =
            (self.directory_record.data_start_sector as u64) * (self.bytes_per_sector as u64);

        let mut data: Vec<u8> = vec![0; self.directory_record.data_size as usize];

        keramics_core::data_stream_read_exact_at_position!(
            &self.data_stream,
            &mut data,
            SeekFrom::Start(offset)
        );
        keramics_core::debug_trace_data!(
            "CdFsDirectoryRecords",
            offset,
            &data,
            self.directory_record.data_size,
        );
        let mut data_offset: usize = 0;
        let data_size: usize = data.len();

        while data_offset < data_size {
            let record_size: usize = data[data_offset] as usize;

            if record_size == 0 {
                break;
            }
            let record_end_offset: usize = data_offset + record_size;

            if record_end_offset > data_size {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unsupported directory record size: {} out of bounds at offset: {}",
                    record_size, data_offset
                )));
            }
            let mut directory_record: CdFsDirectoryRecord = CdFsDirectoryRecord::new();

            match directory_record.read_data(&data[data_offset..record_end_offset]) {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!(
                            "Unable to read directory record at offset: {} (0x{:08x})",
                            data_offset, data_offset
                        )
                    );
                    return Err(error);
                }
            }
            // Skip the directory itself and the parent directory entries ".", "..".
            if !directory_record.name.is_empty() && directory_record.name.len() != 1 {
                self.sub_directory_records
                    .insert(directory_record.name.clone(), directory_record);
            }
            data_offset = record_end_offset;
        }
        self.sub_directory_records_read = true;

        Ok(())
    }
}

impl FileEntryIterator for CdFsFileEntry {
    /// Retrieves the number of sub file entries.
    fn get_number_of_sub_file_entries(&mut self) -> Result<usize, ErrorTrace> {
        if self.is_directory() && !self.sub_directory_records_read {
            match self.read_sub_directory_records() {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to read sub directory records"
                    );
                    return Err(error);
                }
            }
        }
        Ok(self.sub_directory_records.len())
    }

    /// Retrieves a specific sub file entry.
    fn get_sub_file_entry_by_index(
        &mut self,
        sub_file_entry_index: usize,
    ) -> Result<Self, ErrorTrace> {
        if self.is_directory() && !self.sub_directory_records_read {
            match self.read_sub_directory_records() {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to read sub directory records"
                    );
                    return Err(error);
                }
            }
        }
        match self
            .sub_directory_records
            .get_value_by_index(sub_file_entry_index)
        {
            Some(directory_record) => Ok(Self::new(
                &self.data_stream,
                self.bytes_per_sector,
                directory_record.clone(),
            )),
            None => Err(keramics_core::error_trace_new!(format!(
                "Unable to retrieve sub file entry: {}",
                sub_file_entry_index
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_core::open_os_data_stream;

    use crate::cdfs::volume::CdFsVolume;

    use crate::tests::get_test_data_path;

    fn get_file_entry() -> Result<CdFsFileEntry, ErrorTrace> {
        let mut volume: CdFsVolume = CdFsVolume::new();

        let path_string: String = get_test_data_path("cdfs/level3.iso");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;
        volume.read_data_stream(&data_stream)?;

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

    #[test]
    fn test_get_name() -> Result<(), ErrorTrace> {
        let mut file_entry: CdFsFileEntry = get_file_entry()?;

        let name: Option<&ByteString> = file_entry.get_name();
        assert!(name.is_none());

        let mut sub_file_entry: CdFsFileEntry = file_entry.get_sub_file_entry_by_index(0)?;

        let name: Option<&ByteString> = sub_file_entry.get_name();
        assert!(name.is_some());

        Ok(())
    }

    #[test]
    fn test_is_directory() -> Result<(), ErrorTrace> {
        let mut file_entry: CdFsFileEntry = get_file_entry()?;

        assert!(file_entry.is_directory());

        let mut sub_file_entry: CdFsFileEntry = file_entry.get_sub_file_entry_by_index(0)?;

        assert!(!sub_file_entry.is_directory());

        Ok(())
    }

    #[test]
    fn test_is_root_directory() -> Result<(), ErrorTrace> {
        let mut file_entry: CdFsFileEntry = get_file_entry()?;

        assert!(file_entry.is_root_directory());

        let mut sub_file_entry: CdFsFileEntry = file_entry.get_sub_file_entry_by_index(0)?;

        assert!(!sub_file_entry.is_root_directory());

        Ok(())
    }

    #[test]
    fn test_get_number_of_sub_file_entries() -> Result<(), ErrorTrace> {
        let mut file_entry: CdFsFileEntry = get_file_entry()?;

        let number_of_sub_file_entries: usize = file_entry.get_number_of_sub_file_entries()?;
        assert_eq!(number_of_sub_file_entries, 8);

        let mut sub_file_entry: CdFsFileEntry = file_entry.get_sub_file_entry_by_index(7)?;

        let number_of_sub_file_entries: usize = sub_file_entry.get_number_of_sub_file_entries()?;
        assert_eq!(number_of_sub_file_entries, 10);

        let mut sub_file_entry: CdFsFileEntry = file_entry.get_sub_file_entry_by_index(0)?;

        let number_of_sub_file_entries: usize = sub_file_entry.get_number_of_sub_file_entries()?;
        assert_eq!(number_of_sub_file_entries, 0);

        Ok(())
    }

    #[test]
    fn test_get_sub_file_entry_by_index() -> Result<(), ErrorTrace> {
        let mut file_entry: CdFsFileEntry = get_file_entry()?;

        let mut sub_file_entry: CdFsFileEntry = file_entry.get_sub_file_entry_by_index(0)?;
        let name: Option<&ByteString> = sub_file_entry.get_name();
        let name: &ByteString =
            name.ok_or_else(|| keramics_core::error_trace_new!("Missing name"))?;
        assert_eq!(name.elements, b"EMPTYFILE.;1".to_vec());

        let result: Result<CdFsFileEntry, ErrorTrace> = file_entry.get_sub_file_entry_by_index(8);
        assert!(result.is_err());

        Ok(())
    }
}

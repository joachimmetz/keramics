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

use keramics_core::{ByteOrder, DataStreamReference, ErrorTrace};

use super::path_table_record::CdFsPathTableRecord;
use super::path_table_record_be::CdFsPathTableRecordBigEndian;
use super::path_table_record_le::CdFsPathTableRecordLittleEndian;

/// CD file system (CDFS) path table.
pub struct CdFsPathTable {
    /// Byte order, where little-endian represents a type L path table and
    /// big-endian represents a type M path table.
    byte_order: ByteOrder,

    /// Entries.
    entries: Vec<CdFsPathTableRecord>,
}

impl CdFsPathTable {
    /// Creates a new path table with the specified byte order.
    pub fn new(byte_order: ByteOrder) -> Self {
        Self {
            byte_order,
            entries: Vec::new(),
        }
    }

    /// Retrieves the entries.
    pub fn get_entries(&self) -> &[CdFsPathTableRecord] {
        &self.entries
    }

    /// Retrieves a specific entry.
    pub fn get_entry(&self, entry_index: usize) -> Option<&CdFsPathTableRecord> {
        self.entries.get(entry_index)
    }

    /// Reads the path table from a buffer.
    fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        let data_size: usize = data.len();
        let byte_order: ByteOrder = self.byte_order;
        let mut data_offset: usize = 0;

        while data_offset < data_size {
            let mut path_table_record: CdFsPathTableRecord = CdFsPathTableRecord::new();

            let result: Result<(), ErrorTrace> = match byte_order {
                ByteOrder::BigEndian => {
                    keramics_core::debug_trace_structure!(
                        CdFsPathTableRecordBigEndian::debug_read_data(&data[data_offset..])
                    );
                    CdFsPathTableRecordBigEndian::read_data(
                        &mut path_table_record,
                        &data[data_offset..],
                    )
                }
                ByteOrder::LittleEndian => {
                    keramics_core::debug_trace_structure!(
                        CdFsPathTableRecordLittleEndian::debug_read_data(&data[data_offset..])
                    );
                    CdFsPathTableRecordLittleEndian::read_data(
                        &mut path_table_record,
                        &data[data_offset..],
                    )
                }
            };
            match result {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!(
                            "Unable to read path table record at offset: {} (0x{:08x})",
                            data_offset, data_offset
                        )
                    );
                    return Err(error);
                }
            }
            let record_data_size: usize = path_table_record.size as usize;

            if record_data_size == 0 {
                return Err(keramics_core::error_trace_new!(
                    "Unsupported path table record size"
                ));
            }
            data_offset += record_data_size;

            self.entries.push(path_table_record);
        }
        Ok(())
    }

    /// Reads the path table from a specific position in a data stream.
    pub fn read_at_position(
        &mut self,
        data_stream: &DataStreamReference,
        position: SeekFrom,
        data_size: usize,
    ) -> Result<(), ErrorTrace> {
        if data_size == 0 {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        let mut data: Vec<u8> = vec![0; data_size];

        let offset: u64 =
            keramics_core::data_stream_read_exact_at_position!(data_stream, &mut data, position);

        keramics_core::debug_trace_data!("CdFsPathTable", offset, &data, data_size);

        match self.read_data(&data) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!(
                        "Unable to read path table at offset: {} (0x{:08x})",
                        offset, offset
                    )
                );
                return Err(error);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use keramics_core::open_fake_data_stream;

    fn get_test_data_big_endian() -> Vec<u8> {
        vec![
            0x01, 0x00, 0x00, 0x00, 0x00, 0x17, 0x00, 0x01, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00,
            0x00, 0x1a, 0x00, 0x01, 0x4c, 0x4f, 0x53, 0x54, 0x5f, 0x46, 0x4f, 0x55, 0x4e, 0x44,
            0x08, 0x00, 0x00, 0x00, 0x00, 0x18, 0x00, 0x01, 0x54, 0x45, 0x53, 0x54, 0x44, 0x49,
            0x52, 0x31, 0x06, 0x00, 0x00, 0x00, 0x00, 0x19, 0x00, 0x03, 0x58, 0x41, 0x54, 0x54,
            0x52, 0x32,
        ]
    }

    fn get_test_data_little_endian() -> Vec<u8> {
        vec![
            0x01, 0x00, 0x17, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x1a, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x4c, 0x4f, 0x53, 0x54, 0x5f, 0x46, 0x4f, 0x55, 0x4e, 0x44,
            0x08, 0x00, 0x18, 0x00, 0x00, 0x00, 0x01, 0x00, 0x54, 0x45, 0x53, 0x54, 0x44, 0x49,
            0x52, 0x31, 0x06, 0x00, 0x19, 0x00, 0x00, 0x00, 0x03, 0x00, 0x58, 0x41, 0x54, 0x54,
            0x52, 0x32,
        ]
    }

    #[test]
    fn test_read_data_big_endian() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data_big_endian();

        let mut test_struct = CdFsPathTable::new(ByteOrder::BigEndian);
        test_struct.read_data(&test_data)?;

        assert_eq!(test_struct.get_entries().len(), 4);

        let entry: &CdFsPathTableRecord = test_struct.get_entry(0).unwrap();
        assert_eq!(entry.directory_start_sector, 23);
        assert_eq!(entry.parent_directory_number, 1);

        let entry: &CdFsPathTableRecord = test_struct.get_entry(1).unwrap();
        assert_eq!(entry.directory_start_sector, 26);
        assert_eq!(entry.parent_directory_number, 1);
        assert_eq!(entry.name.elements, b"LOST_FOUND".to_vec());

        let entry: &CdFsPathTableRecord = test_struct.get_entry(2).unwrap();
        assert_eq!(entry.directory_start_sector, 24);
        assert_eq!(entry.parent_directory_number, 1);
        assert_eq!(entry.name.elements, b"TESTDIR1".to_vec());

        let entry: &CdFsPathTableRecord = test_struct.get_entry(3).unwrap();
        assert_eq!(entry.directory_start_sector, 25);
        assert_eq!(entry.parent_directory_number, 3);
        assert_eq!(entry.name.elements, b"XATTR2".to_vec());

        Ok(())
    }

    #[test]
    fn test_read_data_little_endian() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data_little_endian();

        let mut test_struct = CdFsPathTable::new(ByteOrder::LittleEndian);
        test_struct.read_data(&test_data)?;

        assert_eq!(test_struct.get_entries().len(), 4);

        let entry: &CdFsPathTableRecord = test_struct.get_entry(0).unwrap();
        assert_eq!(entry.directory_start_sector, 23);
        assert_eq!(entry.parent_directory_number, 1);

        let entry: &CdFsPathTableRecord = test_struct.get_entry(1).unwrap();
        assert_eq!(entry.directory_start_sector, 26);
        assert_eq!(entry.parent_directory_number, 1);
        assert_eq!(entry.name.elements, b"LOST_FOUND".to_vec());

        let entry: &CdFsPathTableRecord = test_struct.get_entry(2).unwrap();
        assert_eq!(entry.directory_start_sector, 24);
        assert_eq!(entry.parent_directory_number, 1);
        assert_eq!(entry.name.elements, b"TESTDIR1".to_vec());

        let entry: &CdFsPathTableRecord = test_struct.get_entry(3).unwrap();
        assert_eq!(entry.directory_start_sector, 25);
        assert_eq!(entry.parent_directory_number, 3);
        assert_eq!(entry.name.elements, b"XATTR2".to_vec());

        Ok(())
    }

    #[test]
    fn test_read_data_with_unsupported_data_size() {
        let test_data: Vec<u8> = get_test_data_little_endian();

        let mut test_struct = CdFsPathTable::new(ByteOrder::LittleEndian);
        let result = test_struct.read_data(&test_data[0..7]);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_name_out_of_bounds() {
        let mut test_data: Vec<u8> = get_test_data_little_endian();
        test_data[0] = 0x80;

        let mut test_struct = CdFsPathTable::new(ByteOrder::LittleEndian);
        let result = test_struct.read_data(&test_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_at_position_big_endian() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data_big_endian();
        let data_stream: DataStreamReference = open_fake_data_stream(&test_data);

        let mut test_struct = CdFsPathTable::new(ByteOrder::BigEndian);
        test_struct.read_at_position(&data_stream, SeekFrom::Start(0), test_data.len())?;

        assert_eq!(test_struct.get_entries().len(), 4);

        Ok(())
    }

    #[test]
    fn test_read_at_position_little_endian() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data_little_endian();
        let data_stream: DataStreamReference = open_fake_data_stream(&test_data);

        let mut test_struct = CdFsPathTable::new(ByteOrder::LittleEndian);
        test_struct.read_at_position(&data_stream, SeekFrom::Start(0), test_data.len())?;

        assert_eq!(test_struct.get_entries().len(), 4);

        Ok(())
    }

    #[test]
    fn test_read_at_position_with_unsupported_data_size() {
        let test_data: Vec<u8> = get_test_data_little_endian();
        let data_stream: DataStreamReference = open_fake_data_stream(&test_data);

        let mut test_struct = CdFsPathTable::new(ByteOrder::LittleEndian);
        let result = test_struct.read_at_position(&data_stream, SeekFrom::Start(0), 0);
        assert!(result.is_err());
    }
}

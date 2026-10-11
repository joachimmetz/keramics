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

use keramics_core::ErrorTrace;
use keramics_layout_map::LayoutMap;
use keramics_types::{bytes_to_u16_le, bytes_to_u32_le};

use super::path_table_record::CdFsPathTableRecord;

#[derive(LayoutMap)]
#[layout_map(
    structure(
        byte_order = "little",
        field(name = "name_size", data_type = "u8"),
        field(name = "extended_attribute_record_size", data_type = "u8"),
        field(name = "directory_start_sector", data_type = "u32"),
        field(name = "parent_directory_number", data_type = "u16"),
    ),
    methods("debug_read_data")
)]
/// CD file system (CDFS) little-endian (type L) path table record
pub struct CdFsPathTableRecordLittleEndian {}

impl CdFsPathTableRecordLittleEndian {
    /// Reads the path table record from a buffer.
    pub fn read_data(
        path_table_record: &mut CdFsPathTableRecord,
        data: &[u8],
    ) -> Result<(), ErrorTrace> {
        let data_size: usize = data.len();

        if data_size < 8 {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        path_table_record.extended_attribute_record_size = data[1];
        path_table_record.directory_start_sector = bytes_to_u32_le!(data, 2);
        path_table_record.parent_directory_number = bytes_to_u16_le!(data, 6);

        let data_end_offset: usize = 8 + (data[0] as usize);

        if data_end_offset > data_size {
            return Err(keramics_core::error_trace_new!(
                "Unsupported file identifier size value out of bounds"
            ));
        }
        path_table_record.name.read_data(&data[8..data_end_offset]);

        path_table_record.size = data_end_offset + (data_end_offset & 1);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_data() -> Vec<u8> {
        vec![0x01, 0x00, 0x17, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]
    }

    #[test]
    fn test_read_data() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data();

        let mut test_struct = CdFsPathTableRecord::new();
        CdFsPathTableRecordLittleEndian::read_data(&mut test_struct, &test_data)?;

        assert_eq!(test_struct.extended_attribute_record_size, 0);
        assert_eq!(test_struct.directory_start_sector, 23);
        assert_eq!(test_struct.parent_directory_number, 1);
        assert_eq!(test_struct.size, 10);

        Ok(())
    }

    #[test]
    fn test_read_data_with_unsupported_data_size() {
        let test_data: Vec<u8> = vec![0x01, 0x00, 0x17];

        let mut test_struct = CdFsPathTableRecord::new();
        let result: Result<(), ErrorTrace> =
            CdFsPathTableRecordLittleEndian::read_data(&mut test_struct, &test_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_unsupported_name_size() {
        let test_data: Vec<u8> = vec![0x02, 0x00, 0x17, 0x00, 0x00, 0x00, 0x01, 0x00];

        let mut test_struct = CdFsPathTableRecord::new();
        let result: Result<(), ErrorTrace> =
            CdFsPathTableRecordLittleEndian::read_data(&mut test_struct, &test_data);
        assert!(result.is_err());
    }
}

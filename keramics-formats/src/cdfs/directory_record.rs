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
use keramics_encodings::CharacterEncoding;
use keramics_layout_map::LayoutMap;
use keramics_types::{ByteString, bytes_to_u32_be, bytes_to_u32_le};

#[derive(LayoutMap)]
#[layout_map(
    structure(
        byte_order = "big",
        field(name = "record_size", data_type = "u8"),
        field(name = "extended_attribute_record_size", data_type = "u8"),
        field(
            name = "data_start_sector_le",
            data_type = "u32",
            byte_order = "little"
        ),
        field(name = "data_start_sector_be", data_type = "u32"),
        field(name = "data_size_le", data_type = "u32", byte_order = "little"),
        field(name = "data_size_be", data_type = "u32"),
        field(name = "recording_time", data_type = "CdFsDateTime"),
        field(name = "file_flags", data_type = "u8", format = "hex"),
        field(name = "file_unit_size", data_type = "u8"),
        field(name = "interleave_gap_size", data_type = "u8"),
        field(name = "volume_set_index_le", data_type = "u16", byte_order = "little"),
        field(name = "volume_set_index_be", data_type = "u16"),
        field(name = "name_size", data_type = "u8"),
        field(name = "name", data_type = "u8"),
    ),
    methods("debug_read_data")
)]
/// CD file system (CDFS) directory record.
pub struct CdFsDirectoryRecord {
    /// Record size.
    pub record_size: u8,

    /// Data start sector.
    pub data_start_sector: u32,

    /// Data size.
    pub data_size: u32,

    /// File flags.
    pub file_flags: u8,

    /// Name.
    pub name: ByteString,
}

impl CdFsDirectoryRecord {
    /// Creates a new directory record.
    pub fn new() -> Self {
        Self {
            record_size: 0,
            data_start_sector: 0,
            data_size: 0,
            file_flags: 0,
            name: ByteString::new_with_encoding(&CharacterEncoding::Ascii),
        }
    }

    /// Reads the directory record from a buffer.
    pub fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        let data_size: usize = data.len();

        if data_size < 33 {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        self.record_size = data[0];
        self.data_start_sector = bytes_to_u32_be!(data, 6);

        let data_start_sector: u32 = bytes_to_u32_le!(data, 2);
        if self.data_start_sector != data_start_sector {
            return Err(keramics_core::error_trace_new!(
                "Mismatch between big- and little-endian data start sector"
            ));
        }
        self.data_size = bytes_to_u32_be!(data, 14);

        let data_size_value: u32 = bytes_to_u32_le!(data, 10);
        if self.data_size != data_size_value {
            return Err(keramics_core::error_trace_new!(
                "Mismatch between big- and little-endian data size"
            ));
        }
        self.file_flags = data[25];

        // TODO: check volume set index

        let data_end_offset: usize = 33 + (data[32] as usize);

        if data_end_offset > data_size {
            return Err(keramics_core::error_trace_new!(
                "Unsupported file identifier size value out of bounds"
            ));
        }
        self.name.read_data(&data[33..data_end_offset]);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_data() -> Vec<u8> {
        vec![
            0x29, 0x00, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x00, 0x08, 0x00, 0x00,
            0x00, 0x00, 0x08, 0x00, 0x1d, 0x09, 0x35, 0x1d, 0x02, 0x02, 0x00, 0x02, 0x00, 0x00,
            0x01, 0x00, 0x00, 0x01, 0x08, 0x54, 0x45, 0x53, 0x54, 0x44, 0x49, 0x52, 0x31,
        ]
    }

    #[test]
    fn test_read_data() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data();

        let mut test_struct = CdFsDirectoryRecord::new();
        test_struct.read_data(&test_data)?;

        assert_eq!(test_struct.record_size, 41);
        assert_eq!(test_struct.data_start_sector, 24);
        assert_eq!(test_struct.data_size, 2048);
        assert_eq!(test_struct.file_flags, 0x02);
        assert_eq!(
            test_struct.name,
            ByteString {
                encoding: CharacterEncoding::Ascii,
                elements: b"TESTDIR1".to_vec(),
            }
        );
        Ok(())
    }

    #[test]
    fn test_read_data_with_unsupported_data_size() {
        let mut test_struct = CdFsDirectoryRecord::new();

        let test_data: Vec<u8> = get_test_data();
        let result = test_struct.read_data(&test_data[0..32]);
        assert!(result.is_err());
    }
}

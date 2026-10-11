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
use keramics_datetime::CdFsDateTime;
use keramics_layout_map::LayoutMap;
use keramics_types::{bytes_to_u16_be, bytes_to_u16_le};

#[derive(Clone, LayoutMap)]
#[layout_map(
    structure(
        byte_order = "big",
        field(name = "owner_identifier_le", data_type = "u16", byte_order = "little"),
        field(name = "owner_identifier_be", data_type = "u16", format = "hex"),
        field(name = "group_identifier_le", data_type = "u16", byte_order = "little"),
        field(name = "group_identifier_be", data_type = "u16", format = "hex"),
        field(name = "permissions", data_type = "u16", format = "hex"),
        field(name = "creation_time", data_type = "ByteString<17>"),
        field(name = "modification_time", data_type = "ByteString<17>"),
        field(name = "expiration_time", data_type = "ByteString<17>"),
        field(name = "effective_time", data_type = "ByteString<17>"),
    ),
    methods("debug_read_data")
)]
/// CD file system (CDFS) extended attribute record.
pub struct CdFsExtendedAttributeRecord {
    /// Owner identifier.
    pub owner_identifier: u16,

    /// Group identifier.
    pub group_identifier: u16,

    /// Permissions.
    pub permissions: u16,

    /// Creation time.
    pub creation_time: CdFsDateTime,

    /// (Last) modification time.
    pub modification_time: CdFsDateTime,

    /// Expiration time.
    pub expiration_time: CdFsDateTime,

    /// Effective time.
    pub effective_time: CdFsDateTime,
}

impl CdFsExtendedAttributeRecord {
    /// Creates a new extended attribute record.
    pub fn new() -> Self {
        Self {
            owner_identifier: 0,
            group_identifier: 0,
            permissions: 0,
            creation_time: CdFsDateTime::default(),
            modification_time: CdFsDateTime::default(),
            expiration_time: CdFsDateTime::default(),
            effective_time: CdFsDateTime::default(),
        }
    }

    /// Reads the extended attribute record from a buffer.
    pub fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        if data.len() < 78 {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        self.owner_identifier = bytes_to_u16_be!(data, 2);

        let owner_identifier: u16 = bytes_to_u16_le!(data, 0);
        if self.owner_identifier != owner_identifier {
            return Err(keramics_core::error_trace_new!(
                "Mismatch between big- and little-endian owner identifier"
            ));
        }
        self.group_identifier = bytes_to_u16_be!(data, 6);

        let group_identifier: u16 = bytes_to_u16_le!(data, 4);
        if self.group_identifier != group_identifier {
            return Err(keramics_core::error_trace_new!(
                "Mismatch between big- and little-endian group identifier"
            ));
        }
        self.permissions = bytes_to_u16_be!(data, 8);

        self.creation_time = match CdFsDateTime::from_string(&data[10..27]) {
            Ok(value) => value,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read creation time");
                return Err(error);
            }
        };
        self.modification_time = match CdFsDateTime::from_string(&data[27..44]) {
            Ok(value) => value,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read modification time");
                return Err(error);
            }
        };
        self.expiration_time = match CdFsDateTime::from_string(&data[44..61]) {
            Ok(value) => value,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read expiration time");
                return Err(error);
            }
        };
        self.effective_time = match CdFsDateTime::from_string(&data[61..78]) {
            Ok(value) => value,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read effective time");
                return Err(error);
            }
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_DATA: [u8; 78] = [
        0x0a, 0x00, 0x00, 0x0a, 0x14, 0x00, 0x00, 0x14, 0x01, 0xff, 0x32, 0x30, 0x31, 0x30, 0x30,
        0x38, 0x31, 0x32, 0x32, 0x30, 0x30, 0x36, 0x33, 0x31, 0x30, 0x30, 0x38, 0x32, 0x30, 0x31,
        0x30, 0x30, 0x38, 0x31, 0x32, 0x32, 0x30, 0x30, 0x36, 0x33, 0x31, 0x30, 0x30, 0x38, 0x32,
        0x30, 0x31, 0x30, 0x30, 0x38, 0x31, 0x32, 0x32, 0x30, 0x30, 0x36, 0x33, 0x31, 0x30, 0x30,
        0x38, 0x32, 0x30, 0x31, 0x30, 0x30, 0x38, 0x31, 0x32, 0x32, 0x30, 0x30, 0x36, 0x33, 0x31,
        0x30, 0x30, 0x38,
    ];

    #[test]
    fn test_read_data() -> Result<(), ErrorTrace> {
        let test_data: &[u8] = &TEST_DATA;

        let mut test_struct = CdFsExtendedAttributeRecord::new();
        test_struct.read_data(&test_data)?;

        assert_eq!(test_struct.owner_identifier, 10);
        assert_eq!(test_struct.group_identifier, 20);
        assert_eq!(test_struct.permissions, 0x01ff);
        assert_eq!(test_struct.creation_time.year, 2010);
        assert_eq!(test_struct.creation_time.month, 8);
        assert_eq!(test_struct.creation_time.day_of_month, 12);
        assert_eq!(test_struct.creation_time.hours, 20);
        assert_eq!(test_struct.creation_time.minutes, 6);
        assert_eq!(test_struct.creation_time.seconds, 31);
        assert_eq!(test_struct.creation_time.utc_offset, 8);
        assert!(test_struct.modification_time.is_set());
        assert!(test_struct.expiration_time.is_set());
        assert!(test_struct.effective_time.is_set());

        Ok(())
    }

    #[test]
    fn test_read_data_with_unsupported_data_size() {
        let mut test_struct = CdFsExtendedAttributeRecord::new();
        let result = test_struct.read_data(&TEST_DATA[0..77]);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_owner_identifier_mismatch() {
        let mut mismatched_data: [u8; 78] = TEST_DATA;
        mismatched_data[3] = 0x0b;

        let mut test_struct = CdFsExtendedAttributeRecord::new();
        let result = test_struct.read_data(&mismatched_data);
        assert!(result.is_err());
    }
}

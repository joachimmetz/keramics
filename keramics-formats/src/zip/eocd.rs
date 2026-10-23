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
use keramics_types::bytes_to_u16_le;
use keramics_types::bytes_to_u32_le;

use super::constants::*;

#[derive(LayoutMap)]
#[layout_map(
    structure(
        byte_order = "little",
        field(name = "signature", data_type = "[u8; 4]", format = "hex"),
        field(name = "disk_number", data_type = "u16"),
        field(name = "disk_start_number", data_type = "u16"),
        field(name = "number_of_entries_disk", data_type = "u16"),
        field(name = "number_of_entries", data_type = "u16"),
        field(name = "central_directory_size", data_type = "u32"),
        field(name = "central_directory_offset", data_type = "u32"),
        field(name = "file_comment_length", data_type = "u16"),
    ),
    methods("debug_read_data", "read_at_position")
)]
/// Zip end of central directory record.
///
/// The file comment, which follows the fixed 22 byte record when present,
/// is not part of the record layout and is read separately.
pub struct ZipEndOfCentralDirectoryRecord {
    /// Number of this disk.
    pub disk_number: u16,

    /// Number of the disk with the start of the central directory.
    pub disk_start_number: u16,

    /// Total number of entries in the central directory on this disk.
    pub number_of_entries_disk: u16,

    /// Total number of entries in the central directory.
    pub number_of_entries: u16,

    /// Size of the central directory.
    pub central_directory_size: u32,

    /// Offset of the start of the central directory with respect to the starting disk number.
    pub central_directory_offset: u32,

    /// Length of the Zip file comment.
    pub file_comment_length: u16,
}

impl ZipEndOfCentralDirectoryRecord {
    /// Creates a new end of central directory record.
    pub fn new() -> Self {
        Self {
            disk_number: 0,
            disk_start_number: 0,
            number_of_entries_disk: 0,
            number_of_entries: 0,
            central_directory_size: 0,
            central_directory_offset: 0,
            file_comment_length: 0,
        }
    }

    /// Reads the end of central directory record from a buffer.
    pub fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        if data.len() < 22 {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        if &data[0..4] != ZIP_END_OF_CENTRAL_DIRECTORY_RECORD_SIGNATURE {
            return Err(keramics_core::error_trace_new!("Unsupported signature"));
        }
        self.disk_number = bytes_to_u16_le!(data, 4);
        self.disk_start_number = bytes_to_u16_le!(data, 6);
        self.number_of_entries_disk = bytes_to_u16_le!(data, 8);
        self.number_of_entries = bytes_to_u16_le!(data, 10);
        self.central_directory_size = bytes_to_u32_le!(data, 12);
        self.central_directory_offset = bytes_to_u32_le!(data, 16);
        self.file_comment_length = bytes_to_u16_le!(data, 20);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::SeekFrom;

    use keramics_core::{DataStreamReference, open_fake_data_stream};

    fn get_test_data() -> Vec<u8> {
        vec![
            0x50, 0x4b, 0x05, 0x06, // disk_number: 0.
            0x00, 0x00, // disk_start_number: 0.
            0x00, 0x00, // number_of_entries_disk: 4.
            0x04, 0x00, // number_of_entries: 4.
            0x04, 0x00, // central_directory_size: 106 (0x0000006a).
            0x6a, 0x00, 0x00, 0x00, // central_directory_offset: 12345 (0x00003039).
            0x39, 0x30, 0x00, 0x00, // file_comment_length: 0.
            0x00, 0x00,
        ]
    }

    #[test]
    fn test_read_data() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data();

        let mut test_struct = ZipEndOfCentralDirectoryRecord::new();
        test_struct.read_data(&test_data[..22])?;

        assert_eq!(test_struct.disk_number, 0);
        assert_eq!(test_struct.disk_start_number, 0);
        assert_eq!(test_struct.number_of_entries_disk, 4);
        assert_eq!(test_struct.number_of_entries, 4);
        assert_eq!(test_struct.central_directory_size, 106);
        assert_eq!(test_struct.central_directory_offset, 12345);
        assert_eq!(test_struct.file_comment_length, 0);

        Ok(())
    }

    #[test]
    fn test_read_data_with_unsupported_data_size() {
        let test_data: Vec<u8> = get_test_data();

        let mut test_struct = ZipEndOfCentralDirectoryRecord::new();
        let result = test_struct.read_data(&test_data[..21]);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_unsupported_signature() {
        let mut test_data: Vec<u8> = get_test_data();
        test_data[0] = 0xff;

        let mut test_struct = ZipEndOfCentralDirectoryRecord::new();
        let result = test_struct.read_data(&test_data[..22]);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_at_position() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data();
        let data_stream: DataStreamReference = open_fake_data_stream(&test_data);

        let mut test_struct = ZipEndOfCentralDirectoryRecord::new();
        test_struct.read_at_position(&data_stream, SeekFrom::Start(0))?;

        assert_eq!(test_struct.number_of_entries, 4);
        assert_eq!(test_struct.central_directory_size, 106);
        assert_eq!(test_struct.central_directory_offset, 12345);

        Ok(())
    }
}

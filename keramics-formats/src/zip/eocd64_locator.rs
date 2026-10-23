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
use keramics_types::bytes_to_u32_le;
use keramics_types::bytes_to_u64_le;

use super::constants::*;

#[derive(LayoutMap)]
#[layout_map(
    structure(
        byte_order = "little",
        field(name = "signature", data_type = "[u8; 4]", format = "hex"),
        field(name = "disk_number", data_type = "u32"),
        field(name = "eocd64_offset", data_type = "u64"),
        field(name = "number_of_disks", data_type = "u32"),
    ),
    methods("debug_read_data", "read_at_position")
)]
/// Zip64 end of central directory locator.
pub struct Zip64EndOfCentralDirectoryLocator {
    /// Number of the disk with the start of the zip64 end of central directory record.
    pub disk_number: u32,

    /// Relative offset of the zip64 end of central directory record.
    pub eocd64_offset: u64,

    /// Total number of disks.
    pub number_of_disks: u32,
}

impl Zip64EndOfCentralDirectoryLocator {
    /// Creates a new zip64 end of central directory locator.
    pub fn new() -> Self {
        Self {
            disk_number: 0,
            eocd64_offset: 0,
            number_of_disks: 0,
        }
    }

    /// Reads the zip64 end of central directory locator from a buffer.
    pub fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        if data.len() < 20 {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        if &data[0..4] != ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_SIGNATURE {
            return Err(keramics_core::error_trace_new!("Unsupported signature"));
        }
        self.disk_number = bytes_to_u32_le!(data, 4);
        self.eocd64_offset = bytes_to_u64_le!(data, 8);
        self.number_of_disks = bytes_to_u32_le!(data, 16);

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
            0x50, 0x4b, 0x06, 0x07, // disk_number: 0.
            0x00, 0x00, 0x00, 0x00, // eocd64_offset: 100 (0x0000000000000064).
            0x64, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // number_of_disks: 1.
            0x01, 0x00, 0x00, 0x00,
        ]
    }

    #[test]
    fn test_read_data() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data();

        let mut test_struct = Zip64EndOfCentralDirectoryLocator::new();
        test_struct.read_data(&test_data[..20])?;

        assert_eq!(test_struct.disk_number, 0);
        assert_eq!(test_struct.eocd64_offset, 100);
        assert_eq!(test_struct.number_of_disks, 1);

        Ok(())
    }

    #[test]
    fn test_read_data_with_unsupported_data_size() {
        let test_data: Vec<u8> = get_test_data();

        let mut test_struct = Zip64EndOfCentralDirectoryLocator::new();
        let result = test_struct.read_data(&test_data[..19]);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_unsupported_signature() {
        let mut test_data: Vec<u8> = get_test_data();
        test_data[0] = 0xff;

        let mut test_struct = Zip64EndOfCentralDirectoryLocator::new();
        let result = test_struct.read_data(&test_data[..20]);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_at_position() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data();
        let data_stream: DataStreamReference = open_fake_data_stream(&test_data);

        let mut test_struct = Zip64EndOfCentralDirectoryLocator::new();
        test_struct.read_at_position(&data_stream, SeekFrom::Start(0))?;

        assert_eq!(test_struct.eocd64_offset, 100);
        assert_eq!(test_struct.number_of_disks, 1);

        Ok(())
    }
}

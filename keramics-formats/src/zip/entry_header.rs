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
        field(name = "version_needed_to_extract", data_type = "u16"),
        field(name = "general_purpose_flags", data_type = "u16"),
        field(name = "compression_method", data_type = "u16"),
        field(name = "last_modification_time", data_type = "u16"),
        field(name = "last_modification_date", data_type = "u16"),
        field(name = "crc_32", data_type = "u32", format = "hex"),
        field(name = "compressed_size", data_type = "u32"),
        field(name = "uncompressed_size", data_type = "u32"),
        field(name = "file_name_length", data_type = "u16"),
        field(name = "extra_field_length", data_type = "u16"),
    ),
    methods("debug_read_data", "read_at_position")
)]
/// Zip local file header.
///
/// The file name and extra field, which follow the fixed 30 byte header when
/// present, are not part of the header layout.
pub struct ZipEntryHeader {
    /// Version needed to extract.
    pub version_needed_to_extract: u16,

    /// General purpose bit flag.
    pub general_purpose_flags: u16,

    /// Compression method.
    pub compression_method: u16,

    /// Last modification file time in DOS time format.
    pub last_modification_time: u16,

    /// Last modification file date in DOS date format.
    pub last_modification_date: u16,

    /// CRC-32 of the uncompressed file data.
    pub crc_32: u32,

    /// Compressed file size.
    pub compressed_size: u32,

    /// Uncompressed file size.
    pub uncompressed_size: u32,

    /// File name length.
    pub file_name_length: u16,

    /// Extra field length.
    pub extra_field_length: u16,
}

impl ZipEntryHeader {
    /// Creates a new local file header.
    pub fn new() -> Self {
        Self {
            version_needed_to_extract: 0,
            general_purpose_flags: 0,
            compression_method: 0,
            last_modification_time: 0,
            last_modification_date: 0,
            crc_32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            file_name_length: 0,
            extra_field_length: 0,
        }
    }

    /// Reads the local file header from a buffer.
    pub fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        if data.len() < 30 {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        if &data[0..4] != ZIP_LOCAL_FILE_HEADER_SIGNATURE {
            return Err(keramics_core::error_trace_new!("Unsupported signature"));
        }
        self.version_needed_to_extract = bytes_to_u16_le!(data, 4);
        self.general_purpose_flags = bytes_to_u16_le!(data, 6);
        self.compression_method = bytes_to_u16_le!(data, 8);
        self.last_modification_time = bytes_to_u16_le!(data, 10);
        self.last_modification_date = bytes_to_u16_le!(data, 12);
        self.crc_32 = bytes_to_u32_le!(data, 14);
        self.compressed_size = bytes_to_u32_le!(data, 18);
        self.uncompressed_size = bytes_to_u32_le!(data, 22);
        self.file_name_length = bytes_to_u16_le!(data, 26);
        self.extra_field_length = bytes_to_u16_le!(data, 28);

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
            0x50, 0x4b, 0x03, 0x04, // version_needed_to_extract: 20 (0x0014).
            0x14, 0x00, // general_purpose_flags: 0.
            0x00, 0x00, // compression_method: stored (0).
            0x00, 0x00, // last_modification_time: 0.
            0x00, 0x00, // last_modification_date: 0.
            0x00, 0x00, // crc_32: 123 (0x0000007b).
            0x7b, 0x00, 0x00, 0x00, // compressed_size: 42 (0x0000002a).
            0x2a, 0x00, 0x00, 0x00, // uncompressed_size: 42 (0x0000002a).
            0x2a, 0x00, 0x00, 0x00, // file_name_length: 11 (0x000b).
            0x0b, 0x00, // extra_field_length: 0.
            0x00, 0x00,
        ]
    }

    #[test]
    fn test_read_data() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data();

        let mut test_struct = ZipEntryHeader::new();
        test_struct.read_data(&test_data[..30])?;

        assert_eq!(test_struct.version_needed_to_extract, 20);
        assert_eq!(
            test_struct.compression_method,
            ZIP_COMPRESSION_METHOD_STORED
        );
        assert_eq!(test_struct.crc_32, 123);
        assert_eq!(test_struct.compressed_size, 42);
        assert_eq!(test_struct.uncompressed_size, 42);
        assert_eq!(test_struct.file_name_length, 11);
        assert_eq!(test_struct.extra_field_length, 0);

        Ok(())
    }

    #[test]
    fn test_read_data_with_unsupported_data_size() {
        let test_data: Vec<u8> = get_test_data();

        let mut test_struct = ZipEntryHeader::new();
        let result = test_struct.read_data(&test_data[..29]);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_unsupported_signature() {
        let mut test_data: Vec<u8> = get_test_data();
        test_data[0] = 0xff;

        let mut test_struct = ZipEntryHeader::new();
        let result = test_struct.read_data(&test_data[..30]);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_at_position() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data();
        let data_stream: DataStreamReference = open_fake_data_stream(&test_data);

        let mut test_struct = ZipEntryHeader::new();
        test_struct.read_at_position(&data_stream, SeekFrom::Start(0))?;

        assert_eq!(test_struct.compressed_size, 42);
        assert_eq!(test_struct.uncompressed_size, 42);

        Ok(())
    }
}

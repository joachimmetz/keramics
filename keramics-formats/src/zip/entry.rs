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
        field(name = "version_made_by", data_type = "u16"),
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
        field(name = "file_comment_length", data_type = "u16"),
        field(name = "disk_start_number", data_type = "u16"),
        field(name = "internal_file_attributes", data_type = "u16"),
        field(name = "external_file_attributes", data_type = "u32", format = "hex"),
        field(name = "entry_offset", data_type = "u32"),
    ),
    methods("debug_read_data", "read_at_position")
)]
/// Zip central directory header (central directory entry).
///
/// The file name, extra field and file comment, which follow the fixed 46
/// byte header when present, are not part of the header layout and are read
/// separately.
#[derive(Clone, Debug)]
pub struct ZipEntry {
    /// Version made by.
    pub version_made_by: u16,

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

    /// File comment length.
    pub file_comment_length: u16,

    /// Disk number start.
    pub disk_start_number: u16,

    /// Internal file attributes.
    pub internal_file_attributes: u16,

    /// External file attributes.
    pub external_file_attributes: u32,

    /// Relative offset of the local file header from the start of the first disk.
    pub entry_offset: u32,

    /// File name.
    pub file_name: String,
}

impl Default for ZipEntry {
    /// Creates a new default central directory entry.
    fn default() -> Self {
        Self::new()
    }
}

impl ZipEntry {
    /// Creates a new central directory entry.
    pub fn new() -> Self {
        Self {
            version_made_by: 0,
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
            file_comment_length: 0,
            disk_start_number: 0,
            internal_file_attributes: 0,
            external_file_attributes: 0,
            entry_offset: 0,
            file_name: String::new(),
        }
    }

    /// Reads the central directory entry from a buffer.
    pub fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        if data.len() < 46 {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        if &data[0..4] != ZIP_CENTRAL_DIRECTORY_HEADER_SIGNATURE {
            return Err(keramics_core::error_trace_new!("Unsupported signature"));
        }
        self.version_made_by = bytes_to_u16_le!(data, 4);
        self.version_needed_to_extract = bytes_to_u16_le!(data, 6);
        self.general_purpose_flags = bytes_to_u16_le!(data, 8);
        self.compression_method = bytes_to_u16_le!(data, 10);
        self.last_modification_time = bytes_to_u16_le!(data, 12);
        self.last_modification_date = bytes_to_u16_le!(data, 14);
        self.crc_32 = bytes_to_u32_le!(data, 16);
        self.compressed_size = bytes_to_u32_le!(data, 20);
        self.uncompressed_size = bytes_to_u32_le!(data, 24);
        self.file_name_length = bytes_to_u16_le!(data, 28);
        self.extra_field_length = bytes_to_u16_le!(data, 30);
        self.file_comment_length = bytes_to_u16_le!(data, 32);
        self.disk_start_number = bytes_to_u16_le!(data, 34);
        self.internal_file_attributes = bytes_to_u16_le!(data, 36);
        self.external_file_attributes = bytes_to_u32_le!(data, 38);
        self.entry_offset = bytes_to_u32_le!(data, 42);

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
            0x50, 0x4b, 0x01, 0x02, // version_made_by: 23 (0x0017).
            0x17, 0x00, // version_needed_to_extract: 20 (0x0014).
            0x14, 0x00, // general_purpose_flags: 0.
            0x00, 0x00, // compression_method: deflate (8).
            0x08, 0x00, // last_modification_time: 0.
            0x00, 0x00, // last_modification_date: 0.
            0x00, 0x00, // crc_32: 123 (0x0000007b).
            0x7b, 0x00, 0x00, 0x00, // compressed_size: 42 (0x0000002a).
            0x2a, 0x00, 0x00, 0x00, // uncompressed_size: 1024 (0x00000400).
            0x00, 0x04, 0x00, 0x00, // file_name_length: 11 (0x000b).
            0x0b, 0x00, // extra_field_length: 0.
            0x00, 0x00, // file_comment_length: 0.
            0x00, 0x00, // disk_start_number: 0.
            0x00, 0x00, // internal_file_attributes: 0.
            0x00, 0x00, // external_file_attributes: 0.
            0x00, 0x00, 0x00, 0x00, // entry_offset: 100 (0x00000064).
            0x64, 0x00, 0x00, 0x00,
        ]
    }

    #[test]
    fn test_read_data() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data();

        let mut test_struct = ZipEntry::new();
        test_struct.read_data(&test_data[..46])?;

        assert_eq!(test_struct.version_made_by, 23);
        assert_eq!(test_struct.version_needed_to_extract, 20);
        assert_eq!(
            test_struct.compression_method,
            ZIP_COMPRESSION_METHOD_DEFLATE
        );
        assert_eq!(test_struct.crc_32, 123);
        assert_eq!(test_struct.compressed_size, 42);
        assert_eq!(test_struct.uncompressed_size, 1024);
        assert_eq!(test_struct.file_name_length, 11);
        assert_eq!(test_struct.entry_offset, 100);

        Ok(())
    }

    #[test]
    fn test_read_data_with_unsupported_data_size() {
        let test_data: Vec<u8> = get_test_data();

        let mut test_struct = ZipEntry::new();
        let result = test_struct.read_data(&test_data[..45]);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_unsupported_signature() {
        let mut test_data: Vec<u8> = get_test_data();
        test_data[0] = 0xff;

        let mut test_struct = ZipEntry::new();
        let result = test_struct.read_data(&test_data[..46]);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_at_position() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data();
        let data_stream: DataStreamReference = open_fake_data_stream(&test_data);

        let mut test_struct = ZipEntry::new();
        test_struct.read_at_position(&data_stream, SeekFrom::Start(0))?;

        assert_eq!(test_struct.compressed_size, 42);
        assert_eq!(test_struct.uncompressed_size, 1024);
        assert_eq!(test_struct.entry_offset, 100);

        Ok(())
    }
}

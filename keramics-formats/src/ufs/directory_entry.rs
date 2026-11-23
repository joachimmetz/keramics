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
use keramics_types::ByteString;

use crate::ffs::FfsByteOrder;

use super::constants::UFS_DIRECTORY_ENTRY_TYPE_UNKNOWN;

/// Unix File System (UFS) directory entry (`struct direct`).
#[derive(Clone, Debug)]
pub struct UfsDirectoryEntry {
    /// Inode number (`d_ino`).
    pub inode_number: u32,

    /// Record length in bytes (`d_reclen`).
    pub record_length: u16,

    /// Directory entry file type (`d_type`).
    pub file_type: u8,

    /// Name (`d_name`).
    pub name: ByteString,
}

impl UfsDirectoryEntry {
    /// Creates a new directory entry.
    pub fn new(character_encoding: &CharacterEncoding) -> Self {
        Self {
            inode_number: 0,
            record_length: 0,
            file_type: UFS_DIRECTORY_ENTRY_TYPE_UNKNOWN,
            name: ByteString::new_with_encoding(character_encoding),
        }
    }

    /// Reads a directory entry from a byte buffer.
    pub fn read_data(
        &mut self,
        data: &[u8],
        byte_order: FfsByteOrder,
        has_entry_type: bool,
    ) -> Result<(), ErrorTrace> {
        let data_size: usize = data.len();

        if data_size < 8 {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        self.inode_number = byte_order.read_u32(data, 0);
        self.record_length = byte_order.read_u16(data, 4);

        let name_length: usize = if has_entry_type {
            self.file_type = data[6];
            data[7] as usize
        } else {
            self.file_type = UFS_DIRECTORY_ENTRY_TYPE_UNKNOWN;
            byte_order.read_u16(data, 6) as usize
        };
        let record_length: usize = self.record_length as usize;
        if record_length < 8 || record_length > data_size {
            return Err(keramics_core::error_trace_new!(format!(
                "Invalid directory entry record length: {} value out of bounds",
                record_length
            )));
        }
        let name_end_offset: usize = 8 + name_length;
        if name_end_offset > record_length {
            return Err(keramics_core::error_trace_new!(format!(
                "Invalid directory entry name length: {} value out of bounds",
                name_length
            )));
        }
        self.name.read_data(&data[8..name_end_offset]);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_data_44bsd_and_42bsd() -> Result<(), ErrorTrace> {
        // 4.4BSD directory entry ("testfile1", inode 257, type DT_REG=8, reclen=20).
        let mut data_44bsd: [u8; 20] = [0; 20];
        let inode_bytes: [u8; 4] = 257_u32.to_le_bytes();
        let record_length_bytes: [u8; 2] = 20_u16.to_le_bytes();
        data_44bsd[0..4].copy_from_slice(&inode_bytes);
        data_44bsd[4..6].copy_from_slice(&record_length_bytes);
        data_44bsd[6] = 8;
        data_44bsd[7] = 9;
        data_44bsd[8..17].copy_from_slice(b"testfile1");

        let mut entry: UfsDirectoryEntry = UfsDirectoryEntry::new(&CharacterEncoding::Utf8);
        entry.read_data(&data_44bsd, FfsByteOrder::LittleEndian, true)?;
        assert_eq!(entry.inode_number, 257);
        assert_eq!(entry.record_length, 20);
        assert_eq!(entry.file_type, 8);
        let expected_name: ByteString = ByteString::from("testfile1");
        assert_eq!(entry.name, expected_name);

        // 4.2BSD directory entry (16-bit name length, no d_type).
        let mut data_42bsd: [u8; 16] = [0; 16];
        let inode_bytes: [u8; 4] = 42_u32.to_be_bytes();
        let record_length_bytes: [u8; 2] = 16_u16.to_be_bytes();
        let name_length_bytes: [u8; 2] = 4_u16.to_be_bytes();
        data_42bsd[0..4].copy_from_slice(&inode_bytes);
        data_42bsd[4..6].copy_from_slice(&record_length_bytes);
        data_42bsd[6..8].copy_from_slice(&name_length_bytes);
        data_42bsd[8..12].copy_from_slice(b"file");

        let mut legacy_entry: UfsDirectoryEntry = UfsDirectoryEntry::new(&CharacterEncoding::Utf8);
        legacy_entry.read_data(&data_42bsd, FfsByteOrder::BigEndian, false)?;
        assert_eq!(legacy_entry.inode_number, 42);
        assert_eq!(legacy_entry.record_length, 16);
        assert_eq!(legacy_entry.file_type, UFS_DIRECTORY_ENTRY_TYPE_UNKNOWN);
        let expected_legacy_name: ByteString = ByteString::from("file");
        assert_eq!(legacy_entry.name, expected_legacy_name);

        Ok(())
    }
}

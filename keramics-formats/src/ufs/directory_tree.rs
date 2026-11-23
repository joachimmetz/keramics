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

use std::cmp::min;

use keramics_core::{DataStream, DataStreamReference, ErrorTrace};
use keramics_encodings::CharacterEncoding;
use keramics_types::ByteString;

use crate::ffs::{FfsBlockRange, FfsBlockReader, FfsBlockStream, FfsByteOrder};
use crate::indexed_hash_map::IndexedHashMap;

use super::constants::{
    UFS_DIRECTORY_BLOCK_SIZE, UFS_DIRECTORY_ENTRY_TYPE_WHITEOUT, UFS_WHITEOUT_INODE_NUMBER,
};
use super::directory_entry::UfsDirectoryEntry;

/// Unix File System (UFS) directory parser.
pub struct UfsDirectoryTree {
    /// Character encoding.
    character_encoding: CharacterEncoding,

    /// Byte order.
    byte_order: FfsByteOrder,

    /// Value to indicate directory entries contain `d_type`.
    has_entry_type: bool,
}

impl UfsDirectoryTree {
    /// Creates a new directory tree parser.
    pub fn new(
        character_encoding: &CharacterEncoding,
        byte_order: FfsByteOrder,
        has_entry_type: bool,
    ) -> Self {
        Self {
            character_encoding: character_encoding.clone(),
            byte_order,
            has_entry_type,
        }
    }

    /// Reads directory entries from block ranges into the entry map.
    pub fn read_block_data(
        &self,
        data_stream: &DataStreamReference,
        block_ranges: &[FfsBlockRange],
        directory_size: u64,
        entries: &mut IndexedHashMap<ByteString, UfsDirectoryEntry>,
    ) -> Result<(), ErrorTrace> {
        if directory_size == 0 {
            return Ok(());
        }
        let block_reader: FfsBlockReader =
            FfsBlockReader::new(data_stream, block_ranges, directory_size);
        let mut block_stream: FfsBlockStream = FfsBlockStream::new(block_reader);
        let mut directory_data: Vec<u8> = vec![0; directory_size as usize];

        match block_stream.read_exact(&mut directory_data) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read directory block data");
                return Err(error);
            }
        }
        self.read_data(&directory_data, entries)
    }

    /// Reads directory entries from a contiguous byte buffer.
    pub fn read_data(
        &self,
        data: &[u8],
        entries: &mut IndexedHashMap<ByteString, UfsDirectoryEntry>,
    ) -> Result<(), ErrorTrace> {
        let data_size: usize = data.len();
        let mut chunk_offset: usize = 0;

        while chunk_offset < data_size {
            let chunk_end_offset: usize = min(chunk_offset + UFS_DIRECTORY_BLOCK_SIZE, data_size);
            let mut entry_offset: usize = chunk_offset;

            while entry_offset + 8 <= chunk_end_offset {
                let mut directory_entry: UfsDirectoryEntry =
                    UfsDirectoryEntry::new(&self.character_encoding);

                match directory_entry.read_data(
                    &data[entry_offset..chunk_end_offset],
                    self.byte_order,
                    self.has_entry_type,
                ) {
                    Ok(_) => {}
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            format!(
                                "Unable to read directory entry at offset: {} (0x{:08x})",
                                entry_offset, entry_offset
                            )
                        );
                        return Err(error);
                    }
                }
                let record_length: usize = directory_entry.record_length as usize;
                let is_whiteout: bool = directory_entry.inode_number == UFS_WHITEOUT_INODE_NUMBER
                    || directory_entry.file_type == UFS_DIRECTORY_ENTRY_TYPE_WHITEOUT;
                let is_dot: bool = directory_entry.name.elements.as_slice() == b"."
                    || directory_entry.name.elements.as_slice() == b"..";

                if directory_entry.inode_number != 0
                    && !is_whiteout
                    && !is_dot
                    && !directory_entry.name.is_empty()
                {
                    let name: ByteString = directory_entry.name.clone();
                    entries.insert(name, directory_entry);
                }
                entry_offset += record_length;
            }
            chunk_offset = chunk_end_offset;
        }
        Ok(())
    }
}

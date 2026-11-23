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
use std::io::SeekFrom;

use keramics_core::{DataStreamReference, ErrorTrace};
use keramics_datetime::{DateTime, PosixTime64Ns};

use crate::ffs::{FfsBlockRange, FfsBlockRangeType, FfsByteOrder, FfsFormatVersion, FfsSuperblock};

use super::constants::*;

/// Unix File System (UFS) on-disk inode (UFS1 or UFS2).
#[derive(Clone, Debug)]
pub struct UfsInode {
    /// File mode (`di_mode`).
    pub file_mode: u16,

    /// Number of hard links (`di_nlink`).
    pub number_of_links: u16,

    /// Owner (user) identifier (`di_uid`).
    pub owner_identifier: u32,

    /// Group identifier (`di_gid`).
    pub group_identifier: u32,

    /// Optimal block size hint (`di_blksize`, UFS2).
    pub block_size_hint: u32,

    /// File data size in bytes (`di_size`).
    pub data_size: u64,

    /// Number of 512-byte blocks allocated (`di_blocks`).
    pub number_of_blocks: u64,

    /// Last access time (`di_atime`, `di_atimensec`).
    pub access_time: DateTime,

    /// Last modification time (`di_mtime`, `di_mtimensec`).
    pub modification_time: DateTime,

    /// Last inode status change time (`di_ctime`, `di_ctimensec`).
    pub change_time: DateTime,

    /// Creation (birth) time (`di_birthtime`, `di_birthnsec`, UFS2 only).
    pub creation_time: Option<DateTime>,

    /// Generation number (`di_gen`).
    pub generation_number: u32,

    /// Inode flags (`di_flags`).
    pub flags: u32,

    /// Extended attributes size in bytes (`di_extsize`, UFS2).
    pub extended_attributes_size: u32,

    /// Extended attributes fragment addresses (`di_extb`, UFS2).
    pub extended_attribute_blocks: [u64; UFS_NUMBER_OF_EXTENDED_ATTRIBUTE_BLOCKS],

    /// Direct fragment addresses (`di_db`).
    pub direct_blocks: [u64; UFS_NUMBER_OF_DIRECT_BLOCKS],

    /// Single, double, and triple indirect fragment addresses (`di_ib`).
    pub indirect_blocks: [u64; UFS_NUMBER_OF_INDIRECT_BLOCKS],

    /// Raw inline data from `di_db`/`di_ib` area (used for fast symbolic links).
    pub inline_data: Vec<u8>,

    /// Device identifier for character and block special devices (`di_db[0]`).
    pub device_identifier: Option<u32>,
}

impl UfsInode {
    /// Creates a new inode.
    pub fn new() -> Self {
        Self {
            file_mode: 0,
            number_of_links: 0,
            owner_identifier: 0,
            group_identifier: 0,
            block_size_hint: 0,
            data_size: 0,
            number_of_blocks: 0,
            access_time: DateTime::NotSet,
            modification_time: DateTime::NotSet,
            change_time: DateTime::NotSet,
            creation_time: None,
            generation_number: 0,
            flags: 0,
            extended_attributes_size: 0,
            extended_attribute_blocks: [0; UFS_NUMBER_OF_EXTENDED_ATTRIBUTE_BLOCKS],
            direct_blocks: [0; UFS_NUMBER_OF_DIRECT_BLOCKS],
            indirect_blocks: [0; UFS_NUMBER_OF_INDIRECT_BLOCKS],
            inline_data: Vec::new(),
            device_identifier: None,
        }
    }

    /// Appends a logical-to-physical range, coalescing with the previous range if contiguous.
    fn append_block_range(
        block_ranges: &mut Vec<FfsBlockRange>,
        logical_offset: u64,
        physical_offset: u64,
        size: u64,
        range_type: FfsBlockRangeType,
    ) {
        if size == 0 {
            return;
        }
        if let Some(last_range) = block_ranges.last_mut() {
            let last_logical_end: u64 = last_range.logical_offset + last_range.size;
            let last_physical_end: u64 = last_range.physical_offset + last_range.size;

            let can_coalesce: bool = match (last_range.range_type, range_type) {
                (FfsBlockRangeType::Sparse, FfsBlockRangeType::Sparse) => {
                    last_logical_end == logical_offset
                }
                (FfsBlockRangeType::InFile, FfsBlockRangeType::InFile) => {
                    last_logical_end == logical_offset && last_physical_end == physical_offset
                }
                _ => false,
            };
            if can_coalesce {
                last_range.size += size;
                return;
            }
        }
        let block_range: FfsBlockRange =
            FfsBlockRange::new(logical_offset, physical_offset, size, range_type);
        block_ranges.push(block_range);
    }

    /// Converts seconds and nanoseconds into a Keramics `DateTime`.
    fn create_date_time(seconds: i64, nanoseconds: i32) -> DateTime {
        if seconds == 0 && nanoseconds == 0 {
            DateTime::NotSet
        } else {
            let fraction: u32 = if nanoseconds < 0 {
                0
            } else {
                nanoseconds as u32
            };
            DateTime::PosixTime64Ns(PosixTime64Ns::new(seconds, fraction))
        }
    }

    /// Determines if the inode stores a fast (inline) symbolic link target.
    pub fn is_fast_symbolic_link(&self, superblock: &FfsSuperblock) -> bool {
        let is_symbolic_link: bool = (self.file_mode & 0xf000) == UFS_FILE_MODE_TYPE_SYMBOLIC_LINK;
        let max_inline_length: i32 = superblock.maximum_symbolic_link_length;

        is_symbolic_link
            && self.number_of_blocks == 0
            && max_inline_length > 0
            && self.data_size < (max_inline_length as u64)
            && self.data_size <= (self.inline_data.len() as u64)
    }

    /// Builds byte-addressed block ranges for the inode's data stream.
    pub fn read_block_ranges(
        &self,
        data_stream: &DataStreamReference,
        superblock: &FfsSuperblock,
        block_ranges: &mut Vec<FfsBlockRange>,
    ) -> Result<(), ErrorTrace> {
        block_ranges.clear();

        if self.data_size == 0 {
            return Ok(());
        }
        let block_size: u64 = superblock.block_size as u64;
        let fragment_size: u64 = superblock.fragment_size as u64;
        let direct_limit: u64 = (UFS_NUMBER_OF_DIRECT_BLOCKS as u64) * block_size;
        let allow_tail_fragment: bool = self.data_size <= direct_limit;
        let mut logical_offset: u64 = 0;

        for direct_index in 0..UFS_NUMBER_OF_DIRECT_BLOCKS {
            if logical_offset >= self.data_size {
                return Ok(());
            }
            let remaining_size: u64 = self.data_size - logical_offset;
            let range_size: u64 = if allow_tail_fragment && remaining_size < block_size {
                let fragment_count: u64 = remaining_size.div_ceil(fragment_size);
                fragment_count * fragment_size
            } else {
                block_size
            };
            let fragment_address: u64 = self.direct_blocks[direct_index];

            if fragment_address == 0 {
                Self::append_block_range(
                    block_ranges,
                    logical_offset,
                    0,
                    range_size,
                    FfsBlockRangeType::Sparse,
                );
            } else {
                let physical_offset: u64 = fragment_address * fragment_size;
                Self::append_block_range(
                    block_ranges,
                    logical_offset,
                    physical_offset,
                    range_size,
                    FfsBlockRangeType::InFile,
                );
            }
            logical_offset += range_size;
        }
        for indirect_level in 0..UFS_NUMBER_OF_INDIRECT_BLOCKS {
            if logical_offset >= self.data_size {
                return Ok(());
            }
            let indirect_fragment_address: u64 = self.indirect_blocks[indirect_level];
            let depth: u8 = (indirect_level as u8) + 1;

            match self.read_indirect_block_ranges(
                data_stream,
                superblock,
                indirect_fragment_address,
                depth,
                &mut logical_offset,
                block_ranges,
            ) {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!("Unable to read indirect block at depth: {}", depth)
                    );
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    /// Reads an inode from a byte buffer according to the superblock format version and byte order.
    pub fn read_data(
        &mut self,
        data: &[u8],
        byte_order: FfsByteOrder,
        format_version: FfsFormatVersion,
    ) -> Result<(), ErrorTrace> {
        if format_version.is_ffs2() {
            self.read_data_ufs2(data, byte_order)
        } else {
            self.read_data_ufs1(data, byte_order)
        }
    }

    /// Reads a 128-byte UFS1 on-disk inode (`struct ufs1_dinode`).
    fn read_data_ufs1(&mut self, data: &[u8], byte_order: FfsByteOrder) -> Result<(), ErrorTrace> {
        if data.len() < UFS1_INODE_SIZE {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        self.file_mode = byte_order.read_u16(data, 0);
        self.number_of_links = byte_order.read_i16(data, 2) as u16;
        self.data_size = byte_order.read_u64(data, 8);

        let access_seconds: i64 = byte_order.read_i32(data, 16) as i64;
        let access_nanoseconds: i32 = byte_order.read_i32(data, 20);
        self.access_time = Self::create_date_time(access_seconds, access_nanoseconds);

        let modification_seconds: i64 = byte_order.read_i32(data, 24) as i64;
        let modification_nanoseconds: i32 = byte_order.read_i32(data, 28);
        self.modification_time =
            Self::create_date_time(modification_seconds, modification_nanoseconds);

        let change_seconds: i64 = byte_order.read_i32(data, 32) as i64;
        let change_nanoseconds: i32 = byte_order.read_i32(data, 36);
        self.change_time = Self::create_date_time(change_seconds, change_nanoseconds);
        self.creation_time = None;

        self.inline_data = data[40..100].to_vec();

        for index in 0..UFS_NUMBER_OF_DIRECT_BLOCKS {
            let offset: usize = 40 + (index * 4);
            self.direct_blocks[index] = byte_order.read_u32(data, offset) as u64;
        }
        for index in 0..UFS_NUMBER_OF_INDIRECT_BLOCKS {
            let offset: usize = 88 + (index * 4);
            self.indirect_blocks[index] = byte_order.read_u32(data, offset) as u64;
        }
        self.flags = byte_order.read_u32(data, 100);
        self.number_of_blocks = byte_order.read_u32(data, 104) as u64;
        self.generation_number = byte_order.read_u32(data, 108);
        self.owner_identifier = byte_order.read_u32(data, 112);
        self.group_identifier = byte_order.read_u32(data, 116);
        self.block_size_hint = 0;
        self.extended_attributes_size = 0;
        self.extended_attribute_blocks = [0; UFS_NUMBER_OF_EXTENDED_ATTRIBUTE_BLOCKS];

        let file_type: u16 = self.file_mode & 0xf000;
        if file_type == UFS_FILE_MODE_TYPE_CHARACTER_DEVICE
            || file_type == UFS_FILE_MODE_TYPE_BLOCK_DEVICE
        {
            self.device_identifier = Some(self.direct_blocks[0] as u32);
        } else {
            self.device_identifier = None;
        }
        Ok(())
    }

    /// Reads a 256-byte UFS2 on-disk inode (`struct ufs2_dinode`).
    fn read_data_ufs2(&mut self, data: &[u8], byte_order: FfsByteOrder) -> Result<(), ErrorTrace> {
        if data.len() < UFS2_INODE_SIZE {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        self.file_mode = byte_order.read_u16(data, 0);
        self.number_of_links = byte_order.read_i16(data, 2) as u16;
        self.owner_identifier = byte_order.read_u32(data, 4);
        self.group_identifier = byte_order.read_u32(data, 8);
        self.block_size_hint = byte_order.read_u32(data, 12);
        self.data_size = byte_order.read_u64(data, 16);
        self.number_of_blocks = byte_order.read_u64(data, 24);

        let access_seconds: i64 = byte_order.read_i64(data, 32);
        let modification_seconds: i64 = byte_order.read_i64(data, 40);
        let change_seconds: i64 = byte_order.read_i64(data, 48);
        let birth_seconds: i64 = byte_order.read_i64(data, 56);
        let modification_nanoseconds: i32 = byte_order.read_i32(data, 64);
        let access_nanoseconds: i32 = byte_order.read_i32(data, 68);
        let change_nanoseconds: i32 = byte_order.read_i32(data, 72);
        let birth_nanoseconds: i32 = byte_order.read_i32(data, 76);

        self.access_time = Self::create_date_time(access_seconds, access_nanoseconds);
        self.modification_time =
            Self::create_date_time(modification_seconds, modification_nanoseconds);
        self.change_time = Self::create_date_time(change_seconds, change_nanoseconds);
        self.creation_time = Some(Self::create_date_time(birth_seconds, birth_nanoseconds));

        self.generation_number = byte_order.read_u32(data, 80);
        self.flags = byte_order.read_u32(data, 88);
        self.extended_attributes_size = byte_order.read_u32(data, 92);

        for index in 0..UFS_NUMBER_OF_EXTENDED_ATTRIBUTE_BLOCKS {
            let offset: usize = 96 + (index * 8);
            self.extended_attribute_blocks[index] = byte_order.read_u64(data, offset);
        }
        self.inline_data = data[112..232].to_vec();

        for index in 0..UFS_NUMBER_OF_DIRECT_BLOCKS {
            let offset: usize = 112 + (index * 8);
            self.direct_blocks[index] = byte_order.read_u64(data, offset);
        }
        for index in 0..UFS_NUMBER_OF_INDIRECT_BLOCKS {
            let offset: usize = 208 + (index * 8);
            self.indirect_blocks[index] = byte_order.read_u64(data, offset);
        }
        let file_type: u16 = self.file_mode & 0xf000;
        if file_type == UFS_FILE_MODE_TYPE_CHARACTER_DEVICE
            || file_type == UFS_FILE_MODE_TYPE_BLOCK_DEVICE
        {
            self.device_identifier = Some(self.direct_blocks[0] as u32);
        } else {
            self.device_identifier = None;
        }
        Ok(())
    }

    /// Recursively resolves an indirect block pointer tree into byte ranges.
    fn read_indirect_block_ranges(
        &self,
        data_stream: &DataStreamReference,
        superblock: &FfsSuperblock,
        fragment_address: u64,
        depth: u8,
        logical_offset: &mut u64,
        block_ranges: &mut Vec<FfsBlockRange>,
    ) -> Result<(), ErrorTrace> {
        let block_size: u64 = superblock.block_size as u64;
        let fragment_size: u64 = superblock.fragment_size as u64;
        let pointer_size: usize = if superblock.format_version.is_ffs2() {
            8
        } else {
            4
        };
        let pointers_per_block: usize = (superblock.block_size as usize) / pointer_size;

        if fragment_address == 0 {
            let mut subtree_blocks: u64 = 1;
            for _ in 0..depth {
                subtree_blocks = subtree_blocks.saturating_mul(pointers_per_block as u64);
            }
            let subtree_bytes: u64 = subtree_blocks.saturating_mul(block_size);
            let remaining_bytes: u64 = self.data_size.saturating_sub(*logical_offset);
            let sparse_bytes: u64 = min(subtree_bytes, remaining_bytes);

            Self::append_block_range(
                block_ranges,
                *logical_offset,
                0,
                sparse_bytes,
                FfsBlockRangeType::Sparse,
            );
            *logical_offset += sparse_bytes;
            return Ok(());
        }
        let block_offset: u64 = fragment_address * fragment_size;
        let mut block_data: Vec<u8> = vec![0; superblock.block_size as usize];

        keramics_core::data_stream_read_exact_at_position!(
            data_stream,
            &mut block_data,
            SeekFrom::Start(block_offset)
        );
        for index in 0..pointers_per_block {
            if *logical_offset >= self.data_size {
                return Ok(());
            }
            let entry_offset: usize = index * pointer_size;
            let child_fragment_address: u64 = if superblock.format_version.is_ffs2() {
                superblock.byte_order.read_u64(&block_data, entry_offset)
            } else {
                superblock.byte_order.read_u32(&block_data, entry_offset) as u64
            };
            if depth == 1 {
                if child_fragment_address == 0 {
                    Self::append_block_range(
                        block_ranges,
                        *logical_offset,
                        0,
                        block_size,
                        FfsBlockRangeType::Sparse,
                    );
                } else {
                    let physical_offset: u64 = child_fragment_address * fragment_size;
                    Self::append_block_range(
                        block_ranges,
                        *logical_offset,
                        physical_offset,
                        block_size,
                        FfsBlockRangeType::InFile,
                    );
                }
                *logical_offset += block_size;
            } else {
                let next_depth: u8 = depth - 1;
                match self.read_indirect_block_ranges(
                    data_stream,
                    superblock,
                    child_fragment_address,
                    next_depth,
                    logical_offset,
                    block_ranges,
                ) {
                    Ok(_) => {}
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            format!(
                                "Unable to read child indirect block at depth: {}",
                                next_depth
                            )
                        );
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }
}

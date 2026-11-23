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

use std::io::SeekFrom;

use keramics_core::{DataStreamReference, ErrorTrace};
use keramics_datetime::{DateTime, PosixTime32, PosixTime64Ns};

use super::constants::*;
use super::enums::{FfsByteOrder, FfsFormatVersion};

/// Fast File System (FFS) cylinder group header.
#[derive(Clone, Debug)]
pub struct FfsCylinderGroup {
    /// Cylinder group index (`cg_cgx`).
    pub cylinder_group_index: u32,

    /// Number of data fragments in this cylinder group (`cg_ndblk`).
    pub number_of_data_fragments: u32,

    /// Number of directories in this cylinder group (`cg_cs.cs_ndir`).
    pub number_of_directories: u32,

    /// Number of free blocks in this cylinder group (`cg_cs.cs_nbfree`).
    pub number_of_free_blocks: u32,

    /// Number of free inodes in this cylinder group (`cg_cs.cs_nifree`).
    pub number_of_free_inodes: u32,

    /// Number of free fragments in this cylinder group (`cg_cs.cs_nffree`).
    pub number_of_free_fragments: u32,

    /// Byte offset to the used inode bitmap (`cg_iusedoff`).
    pub used_inode_bitmap_offset: u32,

    /// Byte offset to the free fragment bitmap (`cg_freeoff`).
    pub free_fragment_bitmap_offset: u32,

    /// Number of inode blocks in this cylinder group (`cg_niblk` or `cg_old_niblk`).
    pub number_of_inode_blocks: u32,

    /// Number of initialized inodes in this cylinder group (`cg_initediblk`, FFS2).
    pub number_of_initialized_inodes: u32,

    /// Number of unreferenced inodes in this cylinder group (`cg_unrefs`).
    pub number_of_unreferenced_inodes: u32,

    /// Cylinder group CRC-32C check-hash (`cg_ckhash`).
    pub check_hash: u32,

    /// Last written time (`cg_time` or `cg_old_time`).
    pub last_written_time: DateTime,
}

impl FfsCylinderGroup {
    /// Creates a new cylinder group header.
    pub fn new() -> Self {
        Self {
            cylinder_group_index: 0,
            number_of_data_fragments: 0,
            number_of_directories: 0,
            number_of_free_blocks: 0,
            number_of_free_inodes: 0,
            number_of_free_fragments: 0,
            used_inode_bitmap_offset: 0,
            free_fragment_bitmap_offset: 0,
            number_of_inode_blocks: 0,
            number_of_initialized_inodes: 0,
            number_of_unreferenced_inodes: 0,
            check_hash: 0,
            last_written_time: DateTime::NotSet,
        }
    }

    /// Reads the cylinder group header from a specific position in a data stream.
    pub fn read_at_position(
        &mut self,
        data_stream: &DataStreamReference,
        position: SeekFrom,
        byte_order: FfsByteOrder,
        format_version: FfsFormatVersion,
    ) -> Result<(), ErrorTrace> {
        let mut data: [u8; FFS_CYLINDER_GROUP_HEADER_SIZE] = [0; FFS_CYLINDER_GROUP_HEADER_SIZE];

        let offset: u64 =
            keramics_core::data_stream_read_exact_at_position!(data_stream, &mut data, position);

        keramics_core::debug_trace_data!(
            "FfsCylinderGroup",
            offset,
            &data,
            FFS_CYLINDER_GROUP_HEADER_SIZE
        );
        match self.read_data(&data, byte_order, format_version) {
            Ok(_) => Ok(()),
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!(
                        "Unable to read FFS cylinder group header at offset: {} (0x{:08x})",
                        offset, offset
                    )
                );
                Err(error)
            }
        }
    }

    /// Reads the cylinder group header from a byte buffer.
    pub fn read_data(
        &mut self,
        data: &[u8],
        byte_order: FfsByteOrder,
        format_version: FfsFormatVersion,
    ) -> Result<(), ErrorTrace> {
        if data.len() < FFS_CYLINDER_GROUP_HEADER_SIZE {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        let magic: u32 = byte_order.read_u32(data, 4);

        if magic != FFS_CYLINDER_GROUP_MAGIC {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported cylinder group signature: 0x{:08x}",
                magic
            )));
        }
        self.cylinder_group_index = byte_order.read_u32(data, 12);
        let old_inode_blocks: u16 = byte_order.read_u16(data, 18);
        self.number_of_data_fragments = byte_order.read_u32(data, 20);
        self.number_of_directories = byte_order.read_u32(data, 24);
        self.number_of_free_blocks = byte_order.read_u32(data, 28);
        self.number_of_free_inodes = byte_order.read_u32(data, 32);
        self.number_of_free_fragments = byte_order.read_u32(data, 36);
        self.used_inode_bitmap_offset = byte_order.read_u32(data, 92);
        self.free_fragment_bitmap_offset = byte_order.read_u32(data, 96);
        self.number_of_unreferenced_inodes = byte_order.read_u32(data, 124);
        self.check_hash = byte_order.read_u32(data, 132);

        if format_version.is_ffs2() {
            self.number_of_inode_blocks = byte_order.read_u32(data, 116);
            self.number_of_initialized_inodes = byte_order.read_u32(data, 120);

            let timestamp: i64 = byte_order.read_i64(data, 136);
            self.last_written_time = if timestamp == 0 {
                DateTime::NotSet
            } else {
                DateTime::PosixTime64Ns(PosixTime64Ns::new(timestamp, 0))
            };
        } else {
            self.number_of_inode_blocks = old_inode_blocks as u32;
            self.number_of_initialized_inodes = 0;

            let timestamp: i32 = byte_order.read_i32(data, 8);
            self.last_written_time = if timestamp == 0 {
                DateTime::NotSet
            } else {
                DateTime::PosixTime32(PosixTime32::new(timestamp))
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_core::open_os_data_stream;

    use crate::tests::get_test_data_path;

    use super::super::superblock::FfsSuperblock;

    #[test]
    fn test_read_at_position_ufs1() -> Result<(), ErrorTrace> {
        let path_string: String = get_test_data_path("ufs/ufs1.raw");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;

        let superblock: FfsSuperblock = FfsSuperblock::probe(&data_stream)?;
        let header_offset: u64 = superblock.get_cylinder_group_header_offset(0)?;

        let mut cylinder_group: FfsCylinderGroup = FfsCylinderGroup::new();
        cylinder_group.read_at_position(
            &data_stream,
            SeekFrom::Start(header_offset),
            superblock.byte_order,
            superblock.format_version,
        )?;
        assert_eq!(cylinder_group.cylinder_group_index, 0);
        assert_eq!(cylinder_group.used_inode_bitmap_offset, 174);
        assert_eq!(cylinder_group.free_fragment_bitmap_offset, 206);

        Ok(())
    }

    #[test]
    fn test_read_at_position_ufs2() -> Result<(), ErrorTrace> {
        let path_string: String = get_test_data_path("ufs/ufs2.raw");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;

        let superblock: FfsSuperblock = FfsSuperblock::probe(&data_stream)?;
        let header_offset: u64 = superblock.get_cylinder_group_header_offset(0)?;

        let mut cylinder_group: FfsCylinderGroup = FfsCylinderGroup::new();
        cylinder_group.read_at_position(
            &data_stream,
            SeekFrom::Start(header_offset),
            superblock.byte_order,
            superblock.format_version,
        )?;
        assert_eq!(cylinder_group.cylinder_group_index, 0);
        assert_eq!(cylinder_group.number_of_initialized_inodes, 256);
        assert_eq!(cylinder_group.used_inode_bitmap_offset, 168);
        assert_eq!(cylinder_group.free_fragment_bitmap_offset, 200);

        Ok(())
    }

    #[test]
    fn test_read_data_with_errors() {
        let mut cylinder_group: FfsCylinderGroup = FfsCylinderGroup::new();

        let short_data: [u8; 16] = [0; 16];
        let result: Result<(), ErrorTrace> = cylinder_group.read_data(
            &short_data,
            FfsByteOrder::LittleEndian,
            FfsFormatVersion::Ffs2,
        );
        assert!(result.is_err());

        let invalid_magic: [u8; FFS_CYLINDER_GROUP_HEADER_SIZE] =
            [0; FFS_CYLINDER_GROUP_HEADER_SIZE];
        let result: Result<(), ErrorTrace> = cylinder_group.read_data(
            &invalid_magic,
            FfsByteOrder::LittleEndian,
            FfsFormatVersion::Ffs2,
        );
        assert!(result.is_err());
    }
}

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
use keramics_types::{ByteString, bytes_to_u32_be, bytes_to_u32_le};

use super::constants::*;
use super::enums::{FfsByteOrder, FfsFormatVersion};

/// Fast File System (FFS) superblock (`struct fs`).
#[derive(Clone, Debug)]
pub struct FfsSuperblock {
    /// Detected byte order.
    pub byte_order: FfsByteOrder,

    /// Detected format version.
    pub format_version: FfsFormatVersion,

    /// Byte offset where the superblock was read.
    pub superblock_offset: u64,

    /// Superblock fragment offset within a cylinder group (`fs_sblkno`).
    pub superblock_fragment_offset: u32,

    /// Cylinder group header fragment offset within a cylinder group (`fs_cblkno`).
    pub cylinder_group_header_fragment_offset: u32,

    /// Inode table fragment offset within a cylinder group (`fs_iblkno`).
    pub inode_table_fragment_offset: u32,

    /// First data fragment offset within a cylinder group (`fs_dblkno`).
    pub first_data_fragment_offset: u32,

    /// Legacy cylinder group offset stagger (`fs_old_cgoffset`, FFS1).
    pub old_cylinder_group_offset: u32,

    /// Legacy cylinder group stagger bitmask (`fs_old_cgmask`, FFS1).
    pub old_cylinder_group_mask: u32,

    /// Number of cylinder groups (`fs_ncg`).
    pub number_of_cylinder_groups: u32,

    /// Block size in bytes (`fs_bsize`).
    pub block_size: u32,

    /// Fragment size in bytes (`fs_fsize`).
    pub fragment_size: u32,

    /// Number of fragments per block (`fs_frag`).
    pub fragments_per_block: u32,

    /// Actual size of the superblock in bytes (`fs_sbsize`).
    pub superblock_size: u32,

    /// Number of block pointers per indirect block (`fs_nindir`).
    pub number_of_indirect_pointers: u32,

    /// Number of inodes per block (`fs_inopb`).
    pub inodes_per_block: u32,

    /// Optimization preference (`fs_optim`).
    pub optimization_preference: u32,

    /// File system identifier (`fs_id`).
    pub file_system_identifier: [u32; 2],

    /// Cylinder group header size in bytes (`fs_cgsize`).
    pub cylinder_group_size: u32,

    /// Number of inodes per cylinder group (`fs_ipg`).
    pub inodes_per_group: u32,

    /// Number of fragments per cylinder group (`fs_fpg`).
    pub fragments_per_group: u32,

    /// File system clean flag (`fs_clean`).
    pub clean_flag: u8,

    /// Last mount point path (`fs_fsmnt`).
    pub last_mount_path: ByteString,

    /// Volume name (`fs_volname`).
    pub volume_label: ByteString,

    /// Expected primary superblock byte offset (`fs_sblockloc`, FFS2).
    pub expected_superblock_offset: i64,

    /// Last written time (`fs_time` or `fs_old_time`).
    pub last_written_time: DateTime,

    /// Last mount or consistency check time (`fs_mtime`, FFS2).
    pub last_mount_time: DateTime,

    /// Total number of fragments (`fs_size` or `fs_old_size`).
    pub number_of_fragments: u64,

    /// Number of data fragments (`fs_dsize` or `fs_old_dsize`).
    pub number_of_data_fragments: u64,

    /// Superblock CRC-32C check-hash (`fs_ckhash`).
    pub check_hash: u32,

    /// Metadata check-hash flags (`fs_metackhash`).
    pub metadata_check_hash_flags: u32,

    /// File system flags (`fs_flags` or `fs_old_flags`).
    pub flags: u32,

    /// Maximum length of an inline (fast) symbolic link (`fs_maxsymlinklen`).
    pub maximum_symbolic_link_length: i32,

    /// Legacy inode format (`fs_old_inodefmt`, FFS1).
    pub old_inode_format: i32,

    /// Total number of inodes (`fs_ncg * fs_ipg`).
    pub number_of_inodes: u64,
}

impl FfsSuperblock {
    /// Creates a new superblock.
    pub fn new() -> Self {
        Self {
            byte_order: FfsByteOrder::LittleEndian,
            format_version: FfsFormatVersion::Ffs1,
            superblock_offset: 0,
            superblock_fragment_offset: 0,
            cylinder_group_header_fragment_offset: 0,
            inode_table_fragment_offset: 0,
            first_data_fragment_offset: 0,
            old_cylinder_group_offset: 0,
            old_cylinder_group_mask: 0,
            number_of_cylinder_groups: 0,
            block_size: 0,
            fragment_size: 0,
            fragments_per_block: 0,
            superblock_size: 0,
            number_of_indirect_pointers: 0,
            inodes_per_block: 0,
            optimization_preference: 0,
            file_system_identifier: [0; 2],
            cylinder_group_size: 0,
            inodes_per_group: 0,
            fragments_per_group: 0,
            clean_flag: 0,
            last_mount_path: ByteString::new(),
            volume_label: ByteString::new(),
            expected_superblock_offset: 0,
            last_written_time: DateTime::NotSet,
            last_mount_time: DateTime::NotSet,
            number_of_fragments: 0,
            number_of_data_fragments: 0,
            check_hash: 0,
            metadata_check_hash_flags: 0,
            flags: 0,
            maximum_symbolic_link_length: 0,
            old_inode_format: 0,
            number_of_inodes: 0,
        }
    }

    /// Detects the byte order and format version from the signature bytes at offset 1372.
    fn detect_signature(data: &[u8]) -> Option<(FfsByteOrder, FfsFormatVersion)> {
        if data.len() < FFS_SUPERBLOCK_SIZE {
            return None;
        }
        let magic_le: u32 = bytes_to_u32_le!(data, FFS_SUPERBLOCK_SIGNATURE_OFFSET);
        match magic_le {
            FFS1_SUPERBLOCK_MAGIC => {
                return Some((FfsByteOrder::LittleEndian, FfsFormatVersion::Ffs1));
            }
            FFS2_SUPERBLOCK_MAGIC => {
                return Some((FfsByteOrder::LittleEndian, FfsFormatVersion::Ffs2));
            }
            FFS2EA_SUPERBLOCK_MAGIC => {
                return Some((FfsByteOrder::LittleEndian, FfsFormatVersion::Ffs2Ea));
            }
            _ => {}
        }
        let magic_be: u32 = bytes_to_u32_be!(data, FFS_SUPERBLOCK_SIGNATURE_OFFSET);
        match magic_be {
            FFS1_SUPERBLOCK_MAGIC => Some((FfsByteOrder::BigEndian, FfsFormatVersion::Ffs1)),
            FFS2_SUPERBLOCK_MAGIC => Some((FfsByteOrder::BigEndian, FfsFormatVersion::Ffs2)),
            FFS2EA_SUPERBLOCK_MAGIC => Some((FfsByteOrder::BigEndian, FfsFormatVersion::Ffs2Ea)),
            _ => None,
        }
    }

    /// Retrieves the byte offset of the cylinder group header for the given cylinder group index.
    pub fn get_cylinder_group_header_offset(
        &self,
        cylinder_group_index: u32,
    ) -> Result<u64, ErrorTrace> {
        let start_fragment: u64 = match self.get_cylinder_group_start_fragment(cylinder_group_index)
        {
            Ok(fragment) => fragment,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to calculate cylinder group start fragment"
                );
                return Err(error);
            }
        };
        let header_fragment: u64 =
            start_fragment + (self.cylinder_group_header_fragment_offset as u64);
        Ok(header_fragment * (self.fragment_size as u64))
    }

    /// Retrieves the starting fragment number for the given cylinder group index.
    pub fn get_cylinder_group_start_fragment(
        &self,
        cylinder_group_index: u32,
    ) -> Result<u64, ErrorTrace> {
        if cylinder_group_index >= self.number_of_cylinder_groups {
            return Err(keramics_core::error_trace_new!(format!(
                "Invalid cylinder group index: {} value out of bounds",
                cylinder_group_index
            )));
        }
        let base_fragment: u64 = (cylinder_group_index as u64) * (self.fragments_per_group as u64);

        if self.format_version.is_ffs2() {
            Ok(base_fragment)
        } else {
            let masked_index: u64 = (cylinder_group_index & !self.old_cylinder_group_mask) as u64;
            let stagger_fragments: u64 = (self.old_cylinder_group_offset as u64) * masked_index;
            Ok(base_fragment + stagger_fragments)
        }
    }

    /// Retrieves the on-disk byte offset for a specific inode number.
    pub fn get_inode_offset(&self, inode_number: u32) -> Result<u64, ErrorTrace> {
        if (inode_number as u64) >= self.number_of_inodes {
            return Err(keramics_core::error_trace_new!(format!(
                "Invalid inode number: {} value out of bounds",
                inode_number
            )));
        }
        let cylinder_group_index: u32 = inode_number / self.inodes_per_group;
        let inode_index_in_group: u64 = (inode_number % self.inodes_per_group) as u64;
        let inode_table_offset: u64 = match self.get_inode_table_offset(cylinder_group_index) {
            Ok(offset) => offset,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to calculate inode table offset"
                );
                return Err(error);
            }
        };
        let inode_size: u64 = self.get_inode_size() as u64;
        Ok(inode_table_offset + (inode_index_in_group * inode_size))
    }

    /// Retrieves the size in bytes of an on-disk inode (128 for FFS1, 256 for FFS2).
    pub fn get_inode_size(&self) -> u16 {
        if self.format_version.is_ffs2() {
            256
        } else {
            128
        }
    }

    /// Retrieves the byte offset of the inode table for the given cylinder group index.
    pub fn get_inode_table_offset(&self, cylinder_group_index: u32) -> Result<u64, ErrorTrace> {
        let start_fragment: u64 = match self.get_cylinder_group_start_fragment(cylinder_group_index)
        {
            Ok(fragment) => fragment,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to calculate cylinder group start fragment"
                );
                return Err(error);
            }
        };
        let inode_table_fragment: u64 = start_fragment + (self.inode_table_fragment_offset as u64);
        Ok(inode_table_fragment * (self.fragment_size as u64))
    }

    /// Determines if directory entries include the 1-byte `d_type` field (4.4BSD format).
    pub fn has_directory_entry_type(&self) -> bool {
        if self.format_version.is_ffs2() {
            true
        } else {
            self.maximum_symbolic_link_length > 0 || self.old_inode_format >= FFS_INODE_FORMAT_44BSD
        }
    }

    /// Probes standard FFS superblock locations in a data stream.
    pub fn probe(data_stream: &DataStreamReference) -> Result<Self, ErrorTrace> {
        let data_stream_size: u64 = keramics_core::data_stream_get_size!(data_stream);
        let superblock_size: u64 = FFS_SUPERBLOCK_SIZE as u64;

        for offset in FFS_SUPERBLOCK_OFFSETS {
            if offset + superblock_size > data_stream_size {
                continue;
            }
            let mut superblock: FfsSuperblock = FfsSuperblock::new();
            if superblock
                .read_at_position(data_stream, SeekFrom::Start(offset))
                .is_ok()
            {
                if superblock.format_version.is_ffs2()
                    && superblock.expected_superblock_offset >= 0
                    && (superblock.expected_superblock_offset as u64) != offset
                {
                    continue;
                }
                return Ok(superblock);
            }
        }
        Err(keramics_core::error_trace_new!(
            "Unable to locate a valid FFS superblock"
        ))
    }

    /// Reads the superblock from a specific position in a data stream.
    pub fn read_at_position(
        &mut self,
        data_stream: &DataStreamReference,
        position: SeekFrom,
    ) -> Result<(), ErrorTrace> {
        let mut data: [u8; FFS_SUPERBLOCK_SIZE] = [0; FFS_SUPERBLOCK_SIZE];

        let offset: u64 =
            keramics_core::data_stream_read_exact_at_position!(data_stream, &mut data, position);

        keramics_core::debug_trace_data!("FfsSuperblock", offset, &data, FFS_SUPERBLOCK_SIZE);

        match self.read_data(&data) {
            Ok(_) => {
                self.superblock_offset = offset;
                Ok(())
            }
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!(
                        "Unable to read FFS superblock at offset: {} (0x{:08x})",
                        offset, offset
                    )
                );
                Err(error)
            }
        }
    }

    /// Reads the superblock from a byte buffer.
    pub fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        if data.len() < FFS_SUPERBLOCK_SIZE {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        let (byte_order, format_version): (FfsByteOrder, FfsFormatVersion) =
            match Self::detect_signature(data) {
                Some(detected) => detected,
                None => {
                    return Err(keramics_core::error_trace_new!("Unsupported signature"));
                }
            };
        self.byte_order = byte_order;
        self.format_version = format_version;

        self.superblock_fragment_offset = byte_order.read_u32(data, 8);
        self.cylinder_group_header_fragment_offset = byte_order.read_u32(data, 12);
        self.inode_table_fragment_offset = byte_order.read_u32(data, 16);
        self.first_data_fragment_offset = byte_order.read_u32(data, 20);
        self.old_cylinder_group_offset = byte_order.read_u32(data, 24);
        self.old_cylinder_group_mask = byte_order.read_u32(data, 28);
        let old_time: i32 = byte_order.read_i32(data, 32);
        let old_size: u32 = byte_order.read_u32(data, 36);
        let old_dsize: u32 = byte_order.read_u32(data, 40);
        self.number_of_cylinder_groups = byte_order.read_u32(data, 44);
        self.block_size = byte_order.read_u32(data, 48);
        self.fragment_size = byte_order.read_u32(data, 52);
        self.fragments_per_block = byte_order.read_u32(data, 56);
        self.superblock_size = byte_order.read_u32(data, 104);
        self.number_of_indirect_pointers = byte_order.read_u32(data, 116);
        self.inodes_per_block = byte_order.read_u32(data, 120);
        self.optimization_preference = byte_order.read_u32(data, 128);
        self.file_system_identifier = [
            byte_order.read_u32(data, 144),
            byte_order.read_u32(data, 148),
        ];
        self.cylinder_group_size = byte_order.read_u32(data, 160);
        self.inodes_per_group = byte_order.read_u32(data, 184);
        self.fragments_per_group = byte_order.read_u32(data, 188);
        self.clean_flag = data[209];
        let old_flags: u8 = data[211];

        let mut last_mount_path: ByteString = ByteString::new();
        last_mount_path.read_data(&data[212..680]);
        self.last_mount_path = last_mount_path;

        let mut volume_label: ByteString = ByteString::new();
        volume_label.read_data(&data[680..712]);
        self.volume_label = volume_label;

        self.expected_superblock_offset = byte_order.read_i64(data, 1000);
        let time_64bit: i64 = byte_order.read_i64(data, 1072);
        let size_64bit: u64 = byte_order.read_u64(data, 1080);
        let dsize_64bit: u64 = byte_order.read_u64(data, 1088);
        let mount_time_64bit: i64 = byte_order.read_i64(data, 1208);
        self.check_hash = byte_order.read_u32(data, 1304);
        self.metadata_check_hash_flags = byte_order.read_u32(data, 1308);
        let flags_32bit: u32 = byte_order.read_u32(data, 1312);
        self.maximum_symbolic_link_length = byte_order.read_i32(data, 1320);
        self.old_inode_format = byte_order.read_i32(data, 1324);

        if format_version.is_ffs2() {
            self.last_written_time = if time_64bit == 0 {
                DateTime::NotSet
            } else {
                DateTime::PosixTime64Ns(PosixTime64Ns::new(time_64bit, 0))
            };
            self.last_mount_time = if mount_time_64bit == 0 {
                DateTime::NotSet
            } else {
                DateTime::PosixTime64Ns(PosixTime64Ns::new(mount_time_64bit, 0))
            };
            self.number_of_fragments = size_64bit;
            self.number_of_data_fragments = dsize_64bit;
            self.flags = flags_32bit;
        } else {
            self.last_written_time = if old_time == 0 {
                DateTime::NotSet
            } else {
                DateTime::PosixTime32(PosixTime32::new(old_time))
            };
            self.last_mount_time = DateTime::NotSet;
            self.number_of_fragments = old_size as u64;
            self.number_of_data_fragments = old_dsize as u64;
            self.flags = if (old_flags as u32) & FFS_FLAG_FLAGS_UPDATED != 0 {
                flags_32bit
            } else {
                old_flags as u32
            };
        }
        if self.block_size < 4096 || !self.block_size.is_power_of_two() || self.block_size > 65536 {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported block size: {} value out of bounds",
                self.block_size
            )));
        }
        if self.fragment_size < 512
            || !self.fragment_size.is_power_of_two()
            || self.fragment_size > self.block_size
        {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported fragment size: {} value out of bounds",
                self.fragment_size
            )));
        }
        let expected_fragments_per_block: u32 = self.block_size / self.fragment_size;
        if self.fragments_per_block == 0
            || self.fragments_per_block > 8
            || self.fragments_per_block != expected_fragments_per_block
        {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported number of fragments per block: {} value out of bounds",
                self.fragments_per_block
            )));
        }
        if self.number_of_cylinder_groups == 0 {
            return Err(keramics_core::error_trace_new!(
                "Unsupported number of cylinder groups: 0"
            ));
        }
        if self.inodes_per_group == 0 {
            return Err(keramics_core::error_trace_new!(
                "Unsupported number of inodes per cylinder group: 0"
            ));
        }
        if self.fragments_per_group == 0 {
            return Err(keramics_core::error_trace_new!(
                "Unsupported number of fragments per cylinder group: 0"
            ));
        }
        self.number_of_inodes =
            (self.number_of_cylinder_groups as u64) * (self.inodes_per_group as u64);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_core::{open_fake_data_stream, open_os_data_stream};

    use crate::tests::get_test_data_path;

    #[test]
    fn test_probe_ufs1() -> Result<(), ErrorTrace> {
        let path_string: String = get_test_data_path("ufs/ufs1.raw");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;

        let superblock: FfsSuperblock = FfsSuperblock::probe(&data_stream)?;
        assert_eq!(superblock.byte_order, FfsByteOrder::LittleEndian);
        assert_eq!(superblock.format_version, FfsFormatVersion::Ffs1);
        assert_eq!(superblock.superblock_offset, 8192);
        assert_eq!(superblock.block_size, 32768);
        assert_eq!(superblock.fragment_size, 4096);
        assert_eq!(superblock.fragments_per_block, 8);
        assert_eq!(superblock.number_of_cylinder_groups, 2);
        assert_eq!(superblock.inodes_per_group, 256);
        assert_eq!(superblock.number_of_inodes, 512);
        let expected_label: ByteString = ByteString::from("ufs1_test");
        assert_eq!(superblock.volume_label, expected_label);

        let expected_mount_path: ByteString = ByteString::from("/mnt/keramics");
        assert_eq!(superblock.last_mount_path, expected_mount_path);
        assert_eq!(superblock.file_system_identifier, [0x6b03c7bd, 0x1a5600bd]);

        let inode_size: u16 = superblock.get_inode_size();
        assert_eq!(inode_size, 128);

        let has_entry_type: bool = superblock.has_directory_entry_type();
        assert!(has_entry_type);

        let root_offset: u64 = superblock.get_inode_offset(2)?;
        assert_eq!(root_offset, (24 * 4096) + (2 * 128));

        Ok(())
    }

    #[test]
    fn test_probe_ufs2() -> Result<(), ErrorTrace> {
        let path_string: String = get_test_data_path("ufs/ufs2.raw");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;

        let superblock: FfsSuperblock = FfsSuperblock::probe(&data_stream)?;
        assert_eq!(superblock.byte_order, FfsByteOrder::LittleEndian);
        assert_eq!(superblock.format_version, FfsFormatVersion::Ffs2);
        assert_eq!(superblock.superblock_offset, 65536);
        assert_eq!(superblock.block_size, 32768);
        assert_eq!(superblock.fragment_size, 4096);
        assert_eq!(superblock.fragments_per_block, 8);
        assert_eq!(superblock.number_of_cylinder_groups, 4);
        assert_eq!(superblock.inodes_per_group, 256);
        assert_eq!(superblock.number_of_inodes, 1024);
        let expected_label: ByteString = ByteString::from("ufs2_test");
        assert_eq!(superblock.volume_label, expected_label);

        let expected_mount_path: ByteString = ByteString::from("/mnt/keramics");
        assert_eq!(superblock.last_mount_path, expected_mount_path);
        assert_eq!(superblock.flags, 0x00000202);
        assert_eq!(superblock.file_system_identifier, [0x6b03c7bd, 0xc3b1f53b]);

        let inode_size: u16 = superblock.get_inode_size();
        assert_eq!(inode_size, 256);

        let root_offset: u64 = superblock.get_inode_offset(2)?;
        assert_eq!(root_offset, (40 * 4096) + (2 * 256));

        Ok(())
    }

    #[test]
    fn test_read_data_big_endian() -> Result<(), ErrorTrace> {
        let mut data: [u8; FFS_SUPERBLOCK_SIZE] = [0; FFS_SUPERBLOCK_SIZE];
        let cylinder_groups_bytes: [u8; 4] = 2_u32.to_be_bytes();
        let block_size_bytes: [u8; 4] = 8192_u32.to_be_bytes();
        let fragment_size_bytes: [u8; 4] = 1024_u32.to_be_bytes();
        let fragments_per_block_bytes: [u8; 4] = 8_u32.to_be_bytes();
        let inodes_per_group_bytes: [u8; 4] = 128_u32.to_be_bytes();
        let fragments_per_group_bytes: [u8; 4] = 1024_u32.to_be_bytes();

        data[44..48].copy_from_slice(&cylinder_groups_bytes);
        data[48..52].copy_from_slice(&block_size_bytes);
        data[52..56].copy_from_slice(&fragment_size_bytes);
        data[56..60].copy_from_slice(&fragments_per_block_bytes);
        data[184..188].copy_from_slice(&inodes_per_group_bytes);
        data[188..192].copy_from_slice(&fragments_per_group_bytes);
        data[1372..1376].copy_from_slice(FFS2_SUPERBLOCK_SIGNATURE_BE);

        let mut superblock: FfsSuperblock = FfsSuperblock::new();
        superblock.read_data(&data)?;

        assert_eq!(superblock.byte_order, FfsByteOrder::BigEndian);
        assert_eq!(superblock.format_version, FfsFormatVersion::Ffs2);
        assert_eq!(superblock.block_size, 8192);
        assert_eq!(superblock.fragment_size, 1024);
        assert_eq!(superblock.number_of_inodes, 256);

        Ok(())
    }

    #[test]
    fn test_probe_with_invalid_stream() {
        let empty_data: [u8; 64] = [0; 64];
        let data_stream: DataStreamReference = open_fake_data_stream(&empty_data);
        let result: Result<FfsSuperblock, ErrorTrace> = FfsSuperblock::probe(&data_stream);
        assert!(result.is_err());
    }
}

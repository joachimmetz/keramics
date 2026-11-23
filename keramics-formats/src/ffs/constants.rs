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

/// Fast File System (FFS) superblock structure size in bytes.
pub const FFS_SUPERBLOCK_SIZE: usize = 1376;

/// Byte offset of the signature within the FFS superblock.
pub const FFS_SUPERBLOCK_SIGNATURE_OFFSET: usize = 1372;

/// Primary FFS superblock probe offsets (FFS2, FFS1, floppy, piggy).
pub const FFS_SUPERBLOCK_OFFSETS: [u64; 4] = [65536, 8192, 0, 262144];

/// FFS1 superblock magic value.
pub const FFS1_SUPERBLOCK_MAGIC: u32 = 0x00011954;

/// FFS2 superblock magic value.
pub const FFS2_SUPERBLOCK_MAGIC: u32 = 0x19540119;

/// NetBSD FFS2EA superblock magic value.
pub const FFS2EA_SUPERBLOCK_MAGIC: u32 = 0x19012038;

/// Little-endian FFS1 superblock signature bytes.
pub const FFS1_SUPERBLOCK_SIGNATURE_LE: &[u8] = &[0x54, 0x19, 0x01, 0x00];

/// Big-endian FFS1 superblock signature bytes.
pub const FFS1_SUPERBLOCK_SIGNATURE_BE: &[u8] = &[0x00, 0x01, 0x19, 0x54];

/// Little-endian FFS2 superblock signature bytes.
pub const FFS2_SUPERBLOCK_SIGNATURE_LE: &[u8] = &[0x19, 0x01, 0x54, 0x19];

/// Big-endian FFS2 superblock signature bytes.
pub const FFS2_SUPERBLOCK_SIGNATURE_BE: &[u8] = &[0x19, 0x54, 0x01, 0x19];

/// Little-endian FFS2EA superblock signature bytes.
pub const FFS2EA_SUPERBLOCK_SIGNATURE_LE: &[u8] = &[0x38, 0x20, 0x01, 0x19];

/// Big-endian FFS2EA superblock signature bytes.
pub const FFS2EA_SUPERBLOCK_SIGNATURE_BE: &[u8] = &[0x19, 0x01, 0x20, 0x38];

/// FFS cylinder group header structure size in bytes.
pub const FFS_CYLINDER_GROUP_HEADER_SIZE: usize = 168;

/// FFS cylinder group header magic value (`CG_MAGIC`).
pub const FFS_CYLINDER_GROUP_MAGIC: u32 = 0x00090255;

/// Legacy 4.2BSD inode and directory format identifier (`FS_42INODEFMT`).
pub const FFS_INODE_FORMAT_42BSD: i32 = -1;

/// 4.4BSD inode and directory format identifier (`FS_44INODEFMT`).
pub const FFS_INODE_FORMAT_44BSD: i32 = 2;

/// File system flag: unclean unmount (`FS_UNCLEAN`).
pub const FFS_FLAG_UNCLEAN: u32 = 0x00000001;

/// File system flag: Soft Updates enabled (`FS_DOSOFTDEP`).
pub const FFS_FLAG_SOFT_UPDATES: u32 = 0x00000002;

/// File system flag: consistency check required (`FS_NEEDSFSCK`).
pub const FFS_FLAG_NEEDS_FSCK: u32 = 0x00000004;

/// File system flag: Soft Updates Journaling enabled (`FS_SUJ`).
pub const FFS_FLAG_SOFT_UPDATES_JOURNALING: u32 = 0x00000008;

/// File system flag: POSIX.1e ACLs enabled (`FS_ACLS`).
pub const FFS_FLAG_POSIX_ACLS: u32 = 0x00000010;

/// File system flag: MAC multi-label enabled (`FS_MULTILABEL`).
pub const FFS_FLAG_MULTILABEL: u32 = 0x00000020;

/// File system flag: GEOM journaling enabled (`FS_GJOURNAL`).
pub const FFS_FLAG_GEOM_JOURNAL: u32 = 0x00000040;

/// File system flag: flags migrated to 32-bit `fs_flags` (`FS_FLAGS_UPDATED`).
pub const FFS_FLAG_FLAGS_UPDATED: u32 = 0x00000080;

/// File system flag: NFSv4 ACLs enabled (`FS_NFS4ACLS`).
pub const FFS_FLAG_NFS4_ACLS: u32 = 0x00000100;

/// File system flag: metadata check-hashes enabled (`FS_METACKHASH`).
pub const FFS_FLAG_METADATA_CHECK_HASH: u32 = 0x00000200;

/// File system flag: automatic TRIM enabled (`FS_TRIM`).
pub const FFS_FLAG_TRIM: u32 = 0x00000400;

/// Metadata check-hash flag: cylinder group headers (`CK_CYLGRP`).
pub const FFS_METADATA_CHECK_HASH_CYLINDER_GROUP: u32 = 0x0001;

/// Metadata check-hash flag: superblock (`CK_SUPERBLOCK`).
pub const FFS_METADATA_CHECK_HASH_SUPERBLOCK: u32 = 0x0002;

/// Metadata check-hash flag: inodes (`CK_INODE`).
pub const FFS_METADATA_CHECK_HASH_INODE: u32 = 0x0004;

/// Metadata check-hash flag: indirect blocks (`CK_INDIR`).
pub const FFS_METADATA_CHECK_HASH_INDIRECT_BLOCK: u32 = 0x0008;

/// Metadata check-hash flag: directory blocks (`CK_DIR`).
pub const FFS_METADATA_CHECK_HASH_DIRECTORY: u32 = 0x0010;

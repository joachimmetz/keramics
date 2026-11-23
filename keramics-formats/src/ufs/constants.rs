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

/// UFS1 on-disk inode size in bytes.
pub const UFS1_INODE_SIZE: usize = 128;

/// UFS2 on-disk inode size in bytes.
pub const UFS2_INODE_SIZE: usize = 256;

/// Number of direct fragment addresses in an inode (`UFS_NDADDR`).
pub const UFS_NUMBER_OF_DIRECT_BLOCKS: usize = 12;

/// Number of indirect block levels in an inode (`UFS_NIADDR`).
pub const UFS_NUMBER_OF_INDIRECT_BLOCKS: usize = 3;

/// Number of extended attribute block addresses in a UFS2 inode (`UFS_NXADDR`).
pub const UFS_NUMBER_OF_EXTENDED_ATTRIBUTE_BLOCKS: usize = 2;

/// Reserved whiteout inode number (`UFS_WINO`).
pub const UFS_WHITEOUT_INODE_NUMBER: u32 = 1;

/// Reserved root directory inode number (`UFS_ROOTINO`).
pub const UFS_ROOT_DIRECTORY_IDENTIFIER: u32 = 2;

/// Directory chunk size in bytes (`DIRBLKSIZ`).
pub const UFS_DIRECTORY_BLOCK_SIZE: usize = 512;

/// File mode type: named pipe (FIFO).
pub const UFS_FILE_MODE_TYPE_FIFO: u16 = 0x1000;

/// File mode type: character device.
pub const UFS_FILE_MODE_TYPE_CHARACTER_DEVICE: u16 = 0x2000;

/// File mode type: directory.
pub const UFS_FILE_MODE_TYPE_DIRECTORY: u16 = 0x4000;

/// File mode type: block device.
pub const UFS_FILE_MODE_TYPE_BLOCK_DEVICE: u16 = 0x6000;

/// File mode type: regular file.
pub const UFS_FILE_MODE_TYPE_REGULAR_FILE: u16 = 0x8000;

/// File mode type: symbolic link.
pub const UFS_FILE_MODE_TYPE_SYMBOLIC_LINK: u16 = 0xa000;

/// File mode type: socket.
pub const UFS_FILE_MODE_TYPE_SOCKET: u16 = 0xc000;

/// File mode type: whiteout.
pub const UFS_FILE_MODE_TYPE_WHITEOUT: u16 = 0xe000;

/// Directory entry type: unknown (`DT_UNKNOWN`).
pub const UFS_DIRECTORY_ENTRY_TYPE_UNKNOWN: u8 = 0;

/// Directory entry type: named pipe (`DT_FIFO`).
pub const UFS_DIRECTORY_ENTRY_TYPE_FIFO: u8 = 1;

/// Directory entry type: character device (`DT_CHR`).
pub const UFS_DIRECTORY_ENTRY_TYPE_CHARACTER_DEVICE: u8 = 2;

/// Directory entry type: directory (`DT_DIR`).
pub const UFS_DIRECTORY_ENTRY_TYPE_DIRECTORY: u8 = 4;

/// Directory entry type: block device (`DT_BLK`).
pub const UFS_DIRECTORY_ENTRY_TYPE_BLOCK_DEVICE: u8 = 6;

/// Directory entry type: regular file (`DT_REG`).
pub const UFS_DIRECTORY_ENTRY_TYPE_REGULAR_FILE: u8 = 8;

/// Directory entry type: symbolic link (`DT_LNK`).
pub const UFS_DIRECTORY_ENTRY_TYPE_SYMBOLIC_LINK: u8 = 10;

/// Directory entry type: socket (`DT_SOCK`).
pub const UFS_DIRECTORY_ENTRY_TYPE_SOCKET: u8 = 12;

/// Directory entry type: whiteout (`DT_WHT`).
pub const UFS_DIRECTORY_ENTRY_TYPE_WHITEOUT: u8 = 14;

/// Extended attribute namespace: empty (`EXTATTR_NAMESPACE_EMPTY`).
pub const UFS_EXTENDED_ATTRIBUTE_NAMESPACE_EMPTY: u8 = 0;

/// Extended attribute namespace: user (`EXTATTR_NAMESPACE_USER`).
pub const UFS_EXTENDED_ATTRIBUTE_NAMESPACE_USER: u8 = 1;

/// Extended attribute namespace: system (`EXTATTR_NAMESPACE_SYSTEM`).
pub const UFS_EXTENDED_ATTRIBUTE_NAMESPACE_SYSTEM: u8 = 2;

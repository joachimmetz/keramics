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
use std::sync::Arc;

use keramics_core::{DataStreamReference, ErrorTrace};

use crate::ffs::{FfsCylinderGroup, FfsSuperblock};

use super::inode::UfsInode;

/// Unix File System (UFS) inode table backed by FFS cylinder groups.
pub struct UfsInodeTable {
    /// Superblock.
    pub superblock: Arc<FfsSuperblock>,

    /// Cylinder group headers.
    cylinder_groups: Vec<FfsCylinderGroup>,
}

impl UfsInodeTable {
    /// Creates a new inode table.
    pub fn new(superblock: &Arc<FfsSuperblock>, cylinder_groups: Vec<FfsCylinderGroup>) -> Self {
        Self {
            superblock: superblock.clone(),
            cylinder_groups,
        }
    }

    /// Retrieves an inode for a specific inode number.
    pub fn get_inode_by_identifier(
        &self,
        data_stream: &DataStreamReference,
        inode_number: u32,
    ) -> Result<UfsInode, ErrorTrace> {
        if (inode_number as u64) >= self.superblock.number_of_inodes {
            return Err(keramics_core::error_trace_new!(format!(
                "Invalid inode number: {} value out of bounds",
                inode_number
            )));
        }
        let cylinder_group_index: usize =
            (inode_number / self.superblock.inodes_per_group) as usize;
        let inode_index_in_group: u32 = inode_number % self.superblock.inodes_per_group;

        if self.superblock.format_version.is_ffs2() {
            if let Some(cylinder_group) = self.cylinder_groups.get(cylinder_group_index) {
                if inode_index_in_group >= cylinder_group.number_of_initialized_inodes {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Inode: {} is not initialized in cylinder group: {}",
                        inode_number, cylinder_group_index
                    )));
                }
            }
        }
        let inode_offset: u64 = match self.superblock.get_inode_offset(inode_number) {
            Ok(offset) => offset,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!("Unable to calculate offset for inode: {}", inode_number)
                );
                return Err(error);
            }
        };
        let inode_size: usize = self.superblock.get_inode_size() as usize;
        let mut data: Vec<u8> = vec![0; inode_size];

        keramics_core::data_stream_read_exact_at_position!(
            data_stream,
            &mut data,
            SeekFrom::Start(inode_offset)
        );
        keramics_core::debug_trace_data!("UfsInode", inode_offset, &data, inode_size);

        let mut inode: UfsInode = UfsInode::new();
        match inode.read_data(
            &data,
            self.superblock.byte_order,
            self.superblock.format_version,
        ) {
            Ok(_) => Ok(inode),
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!(
                        "Unable to read inode: {} at offset: {} (0x{:08x})",
                        inode_number, inode_offset, inode_offset
                    )
                );
                Err(error)
            }
        }
    }
}

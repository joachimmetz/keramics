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
use std::sync::{Arc, RwLock};

use keramics_core::{DataStream, DataStreamReference, ErrorTrace, FakeDataStream};
use keramics_datetime::DateTime;
use keramics_encodings::CharacterEncoding;
use keramics_types::ByteString;

use crate::ffs::{FfsBlockRange, FfsBlockReader, FfsBlockStream};
use crate::indexed_hash_map::IndexedHashMap;
use crate::path_component::PathComponent;
use crate::traits::{ExtendedAttributeIterator, FileEntryIterator};

use super::attributes_entry::UfsAttributesEntry;
use super::constants::*;
use super::directory_entry::UfsDirectoryEntry;
use super::directory_tree::UfsDirectoryTree;
use super::extended_attribute::UfsExtendedAttribute;
use super::extended_attributes::UfsExtendedAttributesIterator;
use super::file_entries::UfsFileEntriesIterator;
use super::inode::UfsInode;
use super::inode_table::UfsInodeTable;

/// Unix File System (UFS) file entry.
pub struct UfsFileEntry {
    /// Volume data stream.
    data_stream: DataStreamReference,

    /// Inode table.
    inode_table: Arc<UfsInodeTable>,

    /// Character encoding.
    character_encoding: CharacterEncoding,

    /// Inode number.
    inode_number: u32,

    /// Inode.
    inode: UfsInode,

    /// Name.
    name: Option<ByteString>,

    /// Resolved block ranges.
    block_ranges: Vec<FfsBlockRange>,

    /// Value to indicate block ranges were resolved.
    block_ranges_read: bool,

    /// Sub directory entries.
    sub_directory_entries: IndexedHashMap<ByteString, UfsDirectoryEntry>,

    /// Value to indicate sub directory entries were read.
    sub_directory_entries_read: bool,

    /// Symbolic link target.
    symbolic_link_target: Option<ByteString>,

    /// Extended attributes.
    attributes: IndexedHashMap<ByteString, UfsAttributesEntry>,

    /// Value to indicate extended attributes were read.
    attributes_read: bool,
}

impl UfsFileEntry {
    /// Creates a new file entry.
    pub(super) fn new(
        data_stream: &DataStreamReference,
        inode_table: &Arc<UfsInodeTable>,
        character_encoding: &CharacterEncoding,
        inode_number: u32,
        inode: UfsInode,
        name: Option<ByteString>,
    ) -> Self {
        Self {
            data_stream: data_stream.clone(),
            inode_table: inode_table.clone(),
            character_encoding: character_encoding.clone(),
            inode_number,
            inode,
            name,
            block_ranges: Vec::new(),
            block_ranges_read: false,
            sub_directory_entries: IndexedHashMap::new(),
            sub_directory_entries_read: false,
            symbolic_link_target: None,
            attributes: IndexedHashMap::new(),
            attributes_read: false,
        }
    }

    /// Retrieves an extended attributes iterator.
    pub fn extended_attributes(&mut self) -> UfsExtendedAttributesIterator<'_> {
        UfsExtendedAttributesIterator::new(self)
    }

    /// Retrieves the access time.
    pub fn get_access_time(&self) -> &DateTime {
        &self.inode.access_time
    }

    /// Retrieves the inode status change time.
    pub fn get_change_time(&self) -> &DateTime {
        &self.inode.change_time
    }

    /// Retrieves the creation (birth) time (UFS2 only).
    pub fn get_creation_time(&self) -> Option<&DateTime> {
        self.inode.creation_time.as_ref()
    }

    /// Retrieves the default data stream.
    pub fn get_data_stream(&mut self) -> Result<Option<DataStreamReference>, ErrorTrace> {
        let file_type: u16 = self.inode.file_mode & 0xf000;
        if file_type != UFS_FILE_MODE_TYPE_REGULAR_FILE {
            return Ok(None);
        }
        if !self.block_ranges_read {
            match self.inode.read_block_ranges(
                &self.data_stream,
                &self.inode_table.superblock,
                &mut self.block_ranges,
            ) {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to read block ranges from inode"
                    );
                    return Err(error);
                }
            }
            self.block_ranges_read = true;
        }
        let block_reader: FfsBlockReader =
            FfsBlockReader::new(&self.data_stream, &self.block_ranges, self.inode.data_size);
        let block_stream: FfsBlockStream = FfsBlockStream::new(block_reader);
        let rw_lock: RwLock<FfsBlockStream> = RwLock::new(block_stream);
        let data_stream: DataStreamReference = Arc::new(rw_lock);

        Ok(Some(data_stream))
    }

    /// Retrieves the device identifier.
    pub fn get_device_identifier(&self) -> Option<&u32> {
        self.inode.device_identifier.as_ref()
    }

    /// Retrieves a specific extended attribute by name.
    pub fn get_extended_attribute_by_name(
        &mut self,
        extended_attribute_name: &PathComponent,
    ) -> Result<Option<UfsExtendedAttribute>, ErrorTrace> {
        if !self.attributes_read {
            match self.read_attributes() {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(error, "Unable to read attributes");
                    return Err(error);
                }
            }
        }
        let lookup_name: ByteString =
            match extended_attribute_name.to_byte_string(&CharacterEncoding::Utf8) {
                Ok(byte_string) => byte_string,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to convert path component to byte string"
                    );
                    return Err(error);
                }
            };
        match self.attributes.get_key_value_by_key(&lookup_name) {
            Some((name, attributes_entry)) => {
                let data_size: u64 = attributes_entry.value_data.len() as u64;
                let fake_stream: FakeDataStream =
                    FakeDataStream::new(&attributes_entry.value_data, data_size);
                let rw_lock: RwLock<FakeDataStream> = RwLock::new(fake_stream);
                let data_stream: DataStreamReference = Arc::new(rw_lock);
                let extended_attribute: UfsExtendedAttribute =
                    UfsExtendedAttribute::new(name, data_stream);
                Ok(Some(extended_attribute))
            }
            None => Ok(None),
        }
    }

    /// Retrieves the file mode.
    pub fn get_file_mode(&self) -> u16 {
        self.inode.file_mode
    }

    /// Retrieves the inode flags (`di_flags`).
    pub fn get_flags(&self) -> u32 {
        self.inode.flags
    }

    /// Retrieves the group identifier.
    pub fn get_group_identifier(&self) -> u32 {
        self.inode.group_identifier
    }

    /// Retrieves the inode number.
    pub fn get_inode_number(&self) -> u32 {
        self.inode_number
    }

    /// Retrieves the modification time.
    pub fn get_modification_time(&self) -> &DateTime {
        &self.inode.modification_time
    }

    /// Retrieves the name.
    pub fn get_name(&self) -> Option<&ByteString> {
        self.name.as_ref()
    }

    /// Retrieves the number of links.
    pub fn get_number_of_links(&self) -> u16 {
        self.inode.number_of_links
    }

    /// Retrieves the owner identifier.
    pub fn get_owner_identifier(&self) -> u32 {
        self.inode.owner_identifier
    }

    /// Retrieves the size.
    pub fn get_size(&self) -> u64 {
        match self.inode.file_mode & 0xf000 {
            UFS_FILE_MODE_TYPE_REGULAR_FILE | UFS_FILE_MODE_TYPE_SYMBOLIC_LINK => {
                self.inode.data_size
            }
            _ => 0,
        }
    }

    /// Retrieves a specific sub file entry by name.
    pub fn get_sub_file_entry_by_name(
        &mut self,
        sub_file_entry_name: &PathComponent,
    ) -> Result<Option<Self>, ErrorTrace> {
        if self.is_directory() && !self.sub_directory_entries_read {
            match self.read_sub_directory_entries() {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to read sub directory entries"
                    );
                    return Err(error);
                }
            }
        }
        let lookup_name: ByteString =
            match sub_file_entry_name.to_byte_string(&self.character_encoding) {
                Ok(byte_string) => byte_string,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to convert path component to byte string"
                    );
                    return Err(error);
                }
            };
        match self
            .sub_directory_entries
            .get_key_value_by_key(&lookup_name)
        {
            Some((name, directory_entry)) => {
                let inode: UfsInode = match self
                    .inode_table
                    .get_inode_by_identifier(&self.data_stream, directory_entry.inode_number)
                {
                    Ok(inode) => inode,
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            format!("Unable to retrieve inode: {}", directory_entry.inode_number)
                        );
                        return Err(error);
                    }
                };
                let entry_name: ByteString = name.clone();
                let sub_file_entry: Self = Self::new(
                    &self.data_stream,
                    &self.inode_table,
                    &self.character_encoding,
                    directory_entry.inode_number,
                    inode,
                    Some(entry_name),
                );
                Ok(Some(sub_file_entry))
            }
            None => Ok(None),
        }
    }

    /// Retrieves the symbolic link target.
    pub fn get_symbolic_link_target(&mut self) -> Result<Option<&ByteString>, ErrorTrace> {
        if self.symbolic_link_target.is_none() && self.is_symbolic_link() {
            let block_size: u64 = self.inode_table.superblock.block_size as u64;
            if self.inode.data_size > block_size {
                return Err(keramics_core::error_trace_new!(format!(
                    "Invalid symbolic link target data size: {} value out of bounds",
                    self.inode.data_size
                )));
            }
            let mut byte_string: ByteString =
                ByteString::new_with_encoding(&self.character_encoding);

            if self
                .inode
                .is_fast_symbolic_link(&self.inode_table.superblock)
            {
                let target_size: usize = self.inode.data_size as usize;
                byte_string.read_data(&self.inode.inline_data[0..target_size]);
            } else {
                if !self.block_ranges_read {
                    match self.inode.read_block_ranges(
                        &self.data_stream,
                        &self.inode_table.superblock,
                        &mut self.block_ranges,
                    ) {
                        Ok(_) => {}
                        Err(mut error) => {
                            keramics_core::error_trace_add_frame!(
                                error,
                                "Unable to read block ranges from symbolic link inode"
                            );
                            return Err(error);
                        }
                    }
                    self.block_ranges_read = true;
                }
                let block_reader: FfsBlockReader = FfsBlockReader::new(
                    &self.data_stream,
                    &self.block_ranges,
                    self.inode.data_size,
                );
                let mut block_stream: FfsBlockStream = FfsBlockStream::new(block_reader);
                let mut data: Vec<u8> = vec![0; self.inode.data_size as usize];

                match block_stream.read_exact(&mut data) {
                    Ok(_) => {}
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            "Unable to read symbolic link target data from block stream"
                        );
                        return Err(error);
                    }
                }
                byte_string.read_data(data.as_slice());
            }
            self.symbolic_link_target = Some(byte_string);
        }
        Ok(self.symbolic_link_target.as_ref())
    }

    /// Determines if the file entry is a directory.
    pub fn is_directory(&self) -> bool {
        (self.inode.file_mode & 0xf000) == UFS_FILE_MODE_TYPE_DIRECTORY
    }

    /// Determines if the file entry is the root directory.
    pub fn is_root_directory(&self) -> bool {
        self.inode_number == UFS_ROOT_DIRECTORY_IDENTIFIER
    }

    /// Determines if the file entry is a symbolic link.
    pub fn is_symbolic_link(&self) -> bool {
        (self.inode.file_mode & 0xf000) == UFS_FILE_MODE_TYPE_SYMBOLIC_LINK
    }

    /// Reads UFS2 native extended attributes from `di_extb`.
    fn read_attributes(&mut self) -> Result<(), ErrorTrace> {
        let attributes_size: usize = self.inode.extended_attributes_size as usize;

        if attributes_size > 0 && self.inode_table.superblock.format_version.is_ffs2() {
            let block_size: usize = self.inode_table.superblock.block_size as usize;
            let maximum_size: usize = UFS_NUMBER_OF_EXTENDED_ATTRIBUTE_BLOCKS * block_size;
            if attributes_size > maximum_size {
                return Err(keramics_core::error_trace_new!(format!(
                    "Invalid extended attributes size: {} value out of bounds",
                    attributes_size
                )));
            }
            let fragment_size: u64 = self.inode_table.superblock.fragment_size as u64;
            let mut attribute_data: Vec<u8> = vec![0; attributes_size];
            let mut data_offset: usize = 0;

            for block_index in 0..UFS_NUMBER_OF_EXTENDED_ATTRIBUTE_BLOCKS {
                if data_offset >= attributes_size {
                    break;
                }
                let remaining_size: usize = attributes_size - data_offset;
                let read_size: usize = min(block_size, remaining_size);
                let data_end_offset: usize = data_offset + read_size;
                let fragment_address: u64 = self.inode.extended_attribute_blocks[block_index];

                if fragment_address != 0 {
                    let physical_offset: u64 = fragment_address * fragment_size;
                    keramics_core::data_stream_read_exact_at_position!(
                        &self.data_stream,
                        &mut attribute_data[data_offset..data_end_offset],
                        SeekFrom::Start(physical_offset)
                    );
                }
                data_offset = data_end_offset;
            }
            match UfsAttributesEntry::read_entries(
                &attribute_data,
                self.inode_table.superblock.byte_order,
                &mut self.attributes,
            ) {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to parse UFS2 extended attribute entries"
                    );
                    return Err(error);
                }
            }
        }
        self.attributes_read = true;

        Ok(())
    }

    /// Reads the sub directory entries.
    fn read_sub_directory_entries(&mut self) -> Result<(), ErrorTrace> {
        let mut block_ranges: Vec<FfsBlockRange> = Vec::new();

        match self.inode.read_block_ranges(
            &self.data_stream,
            &self.inode_table.superblock,
            &mut block_ranges,
        ) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to read block ranges from directory inode"
                );
                return Err(error);
            }
        }
        let has_entry_type: bool = self.inode_table.superblock.has_directory_entry_type();
        let directory_tree: UfsDirectoryTree = UfsDirectoryTree::new(
            &self.character_encoding,
            self.inode_table.superblock.byte_order,
            has_entry_type,
        );
        match directory_tree.read_block_data(
            &self.data_stream,
            &block_ranges,
            self.inode.data_size,
            &mut self.sub_directory_entries,
        ) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to read directory entries from block data"
                );
                return Err(error);
            }
        }
        self.sub_directory_entries_read = true;

        Ok(())
    }

    /// Retrieves a sub file entries iterator.
    pub fn sub_file_entries(&mut self) -> UfsFileEntriesIterator<'_> {
        UfsFileEntriesIterator::new(self)
    }
}

impl ExtendedAttributeIterator for UfsFileEntry {
    type ExtendedAttributeItem = UfsExtendedAttribute;

    /// Retrieves a specific extended attribute by index.
    fn get_extended_attribute_by_index(
        &mut self,
        extended_attribute_index: usize,
    ) -> Result<Self::ExtendedAttributeItem, ErrorTrace> {
        if !self.attributes_read {
            match self.read_attributes() {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(error, "Unable to read attributes");
                    return Err(error);
                }
            }
        }
        match self
            .attributes
            .get_key_value_by_index(extended_attribute_index)
        {
            Some((name, attributes_entry)) => {
                let data_size: u64 = attributes_entry.value_data.len() as u64;
                let fake_stream: FakeDataStream =
                    FakeDataStream::new(&attributes_entry.value_data, data_size);
                let rw_lock: RwLock<FakeDataStream> = RwLock::new(fake_stream);
                let data_stream: DataStreamReference = Arc::new(rw_lock);
                let extended_attribute: UfsExtendedAttribute =
                    UfsExtendedAttribute::new(name, data_stream);
                Ok(extended_attribute)
            }
            None => Err(keramics_core::error_trace_new!(format!(
                "Missing extended attribute: {}",
                extended_attribute_index
            ))),
        }
    }

    /// Retrieves the number of extended attributes.
    fn get_number_of_extended_attributes(&mut self) -> Result<usize, ErrorTrace> {
        if !self.attributes_read {
            match self.read_attributes() {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(error, "Unable to read attributes");
                    return Err(error);
                }
            }
        }
        Ok(self.attributes.len())
    }
}

impl FileEntryIterator for UfsFileEntry {
    /// Retrieves the number of sub file entries.
    fn get_number_of_sub_file_entries(&mut self) -> Result<usize, ErrorTrace> {
        if self.is_directory() && !self.sub_directory_entries_read {
            match self.read_sub_directory_entries() {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to read sub directory entries"
                    );
                    return Err(error);
                }
            }
        }
        Ok(self.sub_directory_entries.len())
    }

    /// Retrieves a specific sub file entry by index.
    fn get_sub_file_entry_by_index(
        &mut self,
        sub_file_entry_index: usize,
    ) -> Result<Self, ErrorTrace> {
        if self.is_directory() && !self.sub_directory_entries_read {
            match self.read_sub_directory_entries() {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to read sub directory entries"
                    );
                    return Err(error);
                }
            }
        }
        match self
            .sub_directory_entries
            .get_key_value_by_index(sub_file_entry_index)
        {
            Some((name, directory_entry)) => {
                let inode: UfsInode = match self
                    .inode_table
                    .get_inode_by_identifier(&self.data_stream, directory_entry.inode_number)
                {
                    Ok(inode) => inode,
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            format!("Unable to retrieve inode: {}", directory_entry.inode_number)
                        );
                        return Err(error);
                    }
                };
                let entry_name: ByteString = name.clone();
                let sub_file_entry: Self = Self::new(
                    &self.data_stream,
                    &self.inode_table,
                    &self.character_encoding,
                    directory_entry.inode_number,
                    inode,
                    Some(entry_name),
                );
                Ok(sub_file_entry)
            }
            None => Err(keramics_core::error_trace_new!(format!(
                "Missing sub file entry: {}",
                sub_file_entry_index
            ))),
        }
    }
}

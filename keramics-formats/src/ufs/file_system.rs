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
use keramics_datetime::DateTime;
use keramics_encodings::CharacterEncoding;
use keramics_types::ByteString;

use crate::ffs::{FfsByteOrder, FfsCylinderGroup, FfsFormatVersion, FfsSuperblock};
use crate::path::Path;

use super::constants::UFS_ROOT_DIRECTORY_IDENTIFIER;
use super::file_entry::UfsFileEntry;
use super::inode::UfsInode;
use super::inode_table::UfsInodeTable;

/// Unix File System (UFS) backed by the Fast File System (FFS) layout.
pub struct UfsFileSystem {
    /// Volume data stream.
    data_stream: Option<DataStreamReference>,

    /// Character encoding.
    character_encoding: CharacterEncoding,

    /// Superblock.
    superblock: Arc<FfsSuperblock>,

    /// Inode table.
    inode_table: Arc<UfsInodeTable>,
}

impl UfsFileSystem {
    /// Creates a new file system.
    pub fn new() -> Self {
        let superblock: Arc<FfsSuperblock> = Arc::new(FfsSuperblock::new());
        let inode_table: Arc<UfsInodeTable> = Arc::new(UfsInodeTable::new(&superblock, Vec::new()));

        Self {
            data_stream: None,
            character_encoding: CharacterEncoding::Utf8,
            superblock,
            inode_table,
        }
    }

    /// Retrieves the block size in bytes.
    pub fn get_block_size(&self) -> u32 {
        self.superblock.block_size
    }

    /// Retrieves the byte order.
    pub fn get_byte_order(&self) -> FfsByteOrder {
        self.superblock.byte_order
    }

    /// Retrieves the file entry for a specific identifier (inode number).
    pub fn get_file_entry_by_identifier(
        &self,
        inode_number: u32,
    ) -> Result<UfsFileEntry, ErrorTrace> {
        let data_stream: &DataStreamReference = match self.data_stream.as_ref() {
            Some(data_stream) => data_stream,
            None => {
                return Err(keramics_core::error_trace_new!("Missing data stream"));
            }
        };
        let inode: UfsInode = match self
            .inode_table
            .get_inode_by_identifier(data_stream, inode_number)
        {
            Ok(inode) => inode,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!("Unable to retrieve inode: {}", inode_number)
                );
                return Err(error);
            }
        };
        let file_entry: UfsFileEntry = UfsFileEntry::new(
            data_stream,
            &self.inode_table,
            &self.character_encoding,
            inode_number,
            inode,
            None,
        );
        Ok(file_entry)
    }

    /// Retrieves the file entry for a specific path.
    pub fn get_file_entry_by_path(&self, path: &Path) -> Result<Option<UfsFileEntry>, ErrorTrace> {
        if path.is_empty() || path.is_relative() {
            return Ok(None);
        }
        let mut file_entry: UfsFileEntry = match self.get_root_directory() {
            Ok(Some(file_entry)) => file_entry,
            Ok(None) => return Ok(None),
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to retrieve root directory");
                return Err(error);
            }
        };
        for path_component in path.components[1..].iter() {
            file_entry = match file_entry.get_sub_file_entry_by_name(path_component) {
                Ok(Some(file_entry)) => file_entry,
                Ok(None) => return Ok(None),
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!("Unable to retrieve sub file entry: {}", path_component)
                    );
                    return Err(error);
                }
            };
        }
        Ok(Some(file_entry))
    }

    /// Retrieves the file system identifier (`fs_id`).
    pub fn get_file_system_identifier(&self) -> [u32; 2] {
        self.superblock.file_system_identifier
    }

    /// Retrieves the file system flags (`fs_flags`).
    pub fn get_flags(&self) -> u32 {
        self.superblock.flags
    }

    /// Retrieves the format version.
    pub fn get_format_version(&self) -> FfsFormatVersion {
        self.superblock.format_version
    }

    /// Retrieves the fragment size in bytes.
    pub fn get_fragment_size(&self) -> u32 {
        self.superblock.fragment_size
    }

    /// Retrieves the inode size in bytes.
    pub fn get_inode_size(&self) -> u16 {
        self.superblock.get_inode_size()
    }

    /// Retrieves the last mount path.
    pub fn get_last_mount_path(&self) -> Option<&ByteString> {
        if self.superblock.last_mount_path.is_empty() {
            None
        } else {
            Some(&self.superblock.last_mount_path)
        }
    }

    /// Retrieves the last mount time.
    pub fn get_last_mount_time(&self) -> &DateTime {
        &self.superblock.last_mount_time
    }

    /// Retrieves the last written time.
    pub fn get_last_written_time(&self) -> &DateTime {
        &self.superblock.last_written_time
    }

    /// Retrieves the metadata check-hash flags (`fs_metackhash`).
    pub fn get_metadata_check_hash_flags(&self) -> u32 {
        self.superblock.metadata_check_hash_flags
    }

    /// Retrieves the number of cylinder groups.
    pub fn get_number_of_cylinder_groups(&self) -> u32 {
        self.superblock.number_of_cylinder_groups
    }

    /// Retrieves the total number of inodes.
    pub fn get_number_of_inodes(&self) -> u64 {
        self.superblock.number_of_inodes
    }

    /// Retrieves the root directory file entry.
    pub fn get_root_directory(&self) -> Result<Option<UfsFileEntry>, ErrorTrace> {
        if self.superblock.number_of_inodes <= (UFS_ROOT_DIRECTORY_IDENTIFIER as u64) {
            return Ok(None);
        }
        match self.get_file_entry_by_identifier(UFS_ROOT_DIRECTORY_IDENTIFIER) {
            Ok(file_entry) => Ok(Some(file_entry)),
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!(
                        "Unable to retrieve root directory file entry: {}",
                        UFS_ROOT_DIRECTORY_IDENTIFIER
                    )
                );
                Err(error)
            }
        }
    }

    /// Retrieves a reference to the underlying FFS superblock.
    pub fn get_superblock(&self) -> &FfsSuperblock {
        &self.superblock
    }

    /// Retrieves the volume label.
    pub fn get_volume_label(&self) -> Option<&ByteString> {
        if self.superblock.volume_label.is_empty() {
            None
        } else {
            Some(&self.superblock.volume_label)
        }
    }

    /// Reads a file system from a data stream.
    pub fn read_data_stream(
        &mut self,
        data_stream: &DataStreamReference,
    ) -> Result<(), ErrorTrace> {
        let superblock: FfsSuperblock = match FfsSuperblock::probe(data_stream) {
            Ok(superblock) => superblock,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read FFS superblock");
                return Err(error);
            }
        };
        let mut cylinder_groups: Vec<FfsCylinderGroup> =
            Vec::with_capacity(superblock.number_of_cylinder_groups as usize);

        for cylinder_group_index in 0..superblock.number_of_cylinder_groups {
            let header_offset: u64 =
                match superblock.get_cylinder_group_header_offset(cylinder_group_index) {
                    Ok(offset) => offset,
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            format!(
                                "Unable to calculate cylinder group: {} header offset",
                                cylinder_group_index
                            )
                        );
                        return Err(error);
                    }
                };
            let mut cylinder_group: FfsCylinderGroup = FfsCylinderGroup::new();

            match cylinder_group.read_at_position(
                data_stream,
                SeekFrom::Start(header_offset),
                superblock.byte_order,
                superblock.format_version,
            ) {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!(
                            "Unable to read cylinder group: {} header",
                            cylinder_group_index
                        )
                    );
                    return Err(error);
                }
            }
            cylinder_groups.push(cylinder_group);
        }
        let superblock_reference: Arc<FfsSuperblock> = Arc::new(superblock);
        let inode_table: UfsInodeTable = UfsInodeTable::new(&superblock_reference, cylinder_groups);
        let inode_table_reference: Arc<UfsInodeTable> = Arc::new(inode_table);

        self.superblock = superblock_reference;
        self.inode_table = inode_table_reference;
        self.data_stream = Some(data_stream.clone());

        Ok(())
    }

    /// Sets the character encoding.
    pub fn set_character_encoding(
        &mut self,
        character_encoding: &CharacterEncoding,
    ) -> Result<(), ErrorTrace> {
        if self.data_stream.is_some() {
            return Err(keramics_core::error_trace_new!(
                "Cannot set character encoding after opening data stream"
            ));
        }
        self.character_encoding = character_encoding.clone();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_core::open_os_data_stream;

    use crate::tests::get_test_data_path;
    use crate::traits::FileEntryIterator;

    fn open_test_file_system(relative_path: &str) -> Result<UfsFileSystem, ErrorTrace> {
        let path_string: String = get_test_data_path(relative_path);
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let data_stream: DataStreamReference = open_os_data_stream(&path_buf)?;

        let mut file_system: UfsFileSystem = UfsFileSystem::new();
        file_system.read_data_stream(&data_stream)?;

        Ok(file_system)
    }

    #[test]
    fn test_read_ufs1() -> Result<(), ErrorTrace> {
        let file_system: UfsFileSystem = open_test_file_system("ufs/ufs1.raw")?;

        let format_version: FfsFormatVersion = file_system.get_format_version();
        assert_eq!(format_version, FfsFormatVersion::Ffs1);

        let block_size: u32 = file_system.get_block_size();
        assert_eq!(block_size, 32768);

        let fragment_size: u32 = file_system.get_fragment_size();
        assert_eq!(fragment_size, 4096);

        let inode_size: u16 = file_system.get_inode_size();
        assert_eq!(inode_size, 128);

        let volume_label: Option<&ByteString> = file_system.get_volume_label();
        let expected_label: ByteString = ByteString::from("ufs1_test");
        assert_eq!(volume_label, Some(&expected_label));

        let mut root_directory: UfsFileEntry = file_system.get_root_directory()?.unwrap();
        let is_root: bool = root_directory.is_root_directory();
        assert!(is_root);

        let number_of_entries: usize = root_directory.get_number_of_sub_file_entries()?;
        assert_eq!(number_of_entries, 10);

        // Read /testdir1/testfile1 ("My file\n").
        let file1_path: Path = Path::from("/testdir1/testfile1");
        let mut file1_entry: UfsFileEntry =
            file_system.get_file_entry_by_path(&file1_path)?.unwrap();
        let file1_size: u64 = file1_entry.get_size();
        assert_eq!(file1_size, 8);

        let file1_stream: DataStreamReference = file1_entry.get_data_stream()?.unwrap();
        let mut file1_data: [u8; 8] = [0; 8];
        file1_stream.write().unwrap().read_exact(&mut file1_data)?;
        assert_eq!(&file1_data, b"My file\n");

        // Read /testdir1/TestFile2 (11358 bytes, multi-fragment tail).
        let file2_path: Path = Path::from("/testdir1/TestFile2");
        let mut file2_entry: UfsFileEntry =
            file_system.get_file_entry_by_path(&file2_path)?.unwrap();
        let file2_size: u64 = file2_entry.get_size();
        assert_eq!(file2_size, 11358);

        let file2_stream: DataStreamReference = file2_entry.get_data_stream()?.unwrap();
        let mut file2_data: Vec<u8> = vec![0; 11358];
        file2_stream.write().unwrap().read_exact(&mut file2_data)?;
        let read_size: usize = file2_data.len();
        assert_eq!(read_size, 11358);

        // Read fast symbolic link /file_symboliclink1.
        let symbolic_link_path: Path = Path::from("/file_symboliclink1");
        let mut symbolic_link_entry: UfsFileEntry = file_system
            .get_file_entry_by_path(&symbolic_link_path)?
            .unwrap();
        let target: Option<&ByteString> = symbolic_link_entry.get_symbolic_link_target()?;
        let expected_target: ByteString = ByteString::from("/mnt/keramics/testdir1/testfile1");
        assert_eq!(target, Some(&expected_target));

        // Verify hard link shares inode with /testdir1/testfile1.
        let hardlink_path: Path = Path::from("/file_hardlink1");
        let hardlink_entry: UfsFileEntry =
            file_system.get_file_entry_by_path(&hardlink_path)?.unwrap();
        let hardlink_inode: u32 = hardlink_entry.get_inode_number();
        let file1_inode: u32 = file1_entry.get_inode_number();
        assert_eq!(hardlink_inode, file1_inode);

        Ok(())
    }

    #[test]
    fn test_read_ufs2() -> Result<(), ErrorTrace> {
        let file_system: UfsFileSystem = open_test_file_system("ufs/ufs2.raw")?;

        let format_version: FfsFormatVersion = file_system.get_format_version();
        assert_eq!(format_version, FfsFormatVersion::Ffs2);

        let block_size: u32 = file_system.get_block_size();
        assert_eq!(block_size, 32768);

        let fragment_size: u32 = file_system.get_fragment_size();
        assert_eq!(fragment_size, 4096);

        let inode_size: u16 = file_system.get_inode_size();
        assert_eq!(inode_size, 256);

        let volume_label: Option<&ByteString> = file_system.get_volume_label();
        let expected_label: ByteString = ByteString::from("ufs2_test");
        assert_eq!(volume_label, Some(&expected_label));

        let mut root_directory: UfsFileEntry = file_system.get_root_directory()?.unwrap();
        let number_of_entries: usize = root_directory.get_number_of_sub_file_entries()?;
        assert_eq!(number_of_entries, 10);

        // Read /testdir1/testfile1 and verify UFS2 birth time is present.
        let file1_path: Path = Path::from("/testdir1/testfile1");
        let mut file1_entry: UfsFileEntry =
            file_system.get_file_entry_by_path(&file1_path)?.unwrap();
        let file1_size: u64 = file1_entry.get_size();
        assert_eq!(file1_size, 8);
        let creation_time: Option<&DateTime> = file1_entry.get_creation_time();
        let has_creation_time: bool = creation_time.is_some();
        assert!(has_creation_time);

        let file1_stream: DataStreamReference = file1_entry.get_data_stream()?.unwrap();
        let mut file1_data: [u8; 8] = [0; 8];
        file1_stream.write().unwrap().read_exact(&mut file1_data)?;
        assert_eq!(&file1_data, b"My file\n");

        // Read /testdir1/TestFile2 (11358 bytes) and verify identical content to UFS1.
        let file2_path: Path = Path::from("/testdir1/TestFile2");
        let mut file2_entry: UfsFileEntry =
            file_system.get_file_entry_by_path(&file2_path)?.unwrap();
        let file2_stream: DataStreamReference = file2_entry.get_data_stream()?.unwrap();
        let mut file2_data: Vec<u8> = vec![0; 11358];
        file2_stream.write().unwrap().read_exact(&mut file2_data)?;

        let ufs1_file_system: UfsFileSystem = open_test_file_system("ufs/ufs1.raw")?;
        let mut ufs1_file2: UfsFileEntry = ufs1_file_system
            .get_file_entry_by_path(&file2_path)?
            .unwrap();
        let ufs1_stream: DataStreamReference = ufs1_file2.get_data_stream()?.unwrap();
        let mut ufs1_data: Vec<u8> = vec![0; 11358];
        ufs1_stream.write().unwrap().read_exact(&mut ufs1_data)?;
        assert_eq!(file2_data, ufs1_data);

        // Read fast symbolic link /directory_symboliclink1.
        let symbolic_link_path: Path = Path::from("/directory_symboliclink1");
        let mut symbolic_link_entry: UfsFileEntry = file_system
            .get_file_entry_by_path(&symbolic_link_path)?
            .unwrap();
        let target: Option<&ByteString> = symbolic_link_entry.get_symbolic_link_target()?;
        let expected_target: ByteString = ByteString::from("/mnt/keramics/testdir1");
        assert_eq!(target, Some(&expected_target));

        // Non-existent path returns None.
        let missing_path: Path = Path::from("/does_not_exist");
        let missing_entry: Option<UfsFileEntry> =
            file_system.get_file_entry_by_path(&missing_path)?;
        let is_missing: bool = missing_entry.is_none();
        assert!(is_missing);

        Ok(())
    }
}

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

use std::fmt;

use keramics_core::{DataStreamReference, ErrorTrace};
use keramics_datetime::DateTime;
use keramics_encodings::CharacterEncoding;
use keramics_formats::ffs::constants::*;
use keramics_formats::ufs::{UfsExtendedAttribute, UfsFileEntry, UfsFileSystem};
use keramics_formats::{ExtendedAttributeIterator, FileEntryIterator, Path};
use keramics_types::ByteString;

use crate::formatters::ByteSize;

use super::constants::*;
use super::posix::PosixFileModeInfo;

/// Unix File System (UFS) date and time information.
struct UfsDateTimeInfo<'a> {
    /// Date and time value.
    date_time: &'a DateTime,
}

impl<'a> UfsDateTimeInfo<'a> {
    /// Creates new date and time information.
    fn new(date_time: &'a DateTime) -> Self {
        Self { date_time }
    }
}

impl<'a> fmt::Display for UfsDateTimeInfo<'a> {
    /// Formats the date and time information for display.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self.date_time {
            DateTime::NotSet => write!(formatter, "{}", NOT_SET_VALUE),
            DateTime::PosixTime32(posix_time32) => {
                let iso_string: String = posix_time32.to_iso8601_string();
                write!(formatter, "{}+00:00", iso_string)
            }
            DateTime::PosixTime64Ns(posix_time64ns) => {
                let iso_string: String = posix_time64ns.to_iso8601_string();
                write!(formatter, "{}+00:00", iso_string)
            }
            _ => write!(formatter, "Unsupported date time"),
        }
    }
}

/// Unix File System (UFS) file entry information.
struct UfsFileEntryInfo<'a> {
    /// File entry.
    file_entry: &'a UfsFileEntry,

    /// Symbolic link target.
    pub symbolic_link_target: Option<ByteString>,
}

impl<'a> UfsFileEntryInfo<'a> {
    /// Creates new file entry information.
    fn new(file_entry: &'a UfsFileEntry) -> Self {
        Self {
            file_entry,
            symbolic_link_target: None,
        }
    }
}

impl<'a> fmt::Display for UfsFileEntryInfo<'a> {
    /// Formats the file entry information for display.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let inode_number: u32 = self.file_entry.get_inode_number();
        writeln!(formatter, "    Inode number\t\t\t\t: {}", inode_number)?;

        if let Some(name) = self.file_entry.get_name() {
            writeln!(formatter, "    Name\t\t\t\t\t: {}", name)?;
        }
        let size: u64 = self.file_entry.get_size();
        let byte_size: ByteSize = ByteSize::new(size, 1024);
        writeln!(formatter, "    Size\t\t\t\t\t: {}", byte_size)?;

        if let Some(date_time) = self.file_entry.get_creation_time() {
            let date_time_info: UfsDateTimeInfo = UfsDateTimeInfo::new(date_time);
            writeln!(formatter, "    Creation time\t\t\t\t: {}", date_time_info)?;
        }
        let modification_time: &DateTime = self.file_entry.get_modification_time();
        let modification_time_info: UfsDateTimeInfo = UfsDateTimeInfo::new(modification_time);
        writeln!(
            formatter,
            "    Modification time\t\t\t\t: {}",
            modification_time_info
        )?;

        let access_time: &DateTime = self.file_entry.get_access_time();
        let access_time_info: UfsDateTimeInfo = UfsDateTimeInfo::new(access_time);
        writeln!(formatter, "    Access time\t\t\t\t\t: {}", access_time_info)?;

        let change_time: &DateTime = self.file_entry.get_change_time();
        let change_time_info: UfsDateTimeInfo = UfsDateTimeInfo::new(change_time);
        writeln!(
            formatter,
            "    Inode change time\t\t\t\t: {}",
            change_time_info
        )?;

        let number_of_links: u16 = self.file_entry.get_number_of_links();
        writeln!(
            formatter,
            "    Number of links\t\t\t\t: {}",
            number_of_links
        )?;

        let owner_identifier: u32 = self.file_entry.get_owner_identifier();
        writeln!(
            formatter,
            "    Owner identifier\t\t\t\t: {}",
            owner_identifier
        )?;

        let group_identifier: u32 = self.file_entry.get_group_identifier();
        writeln!(
            formatter,
            "    Group identifier\t\t\t\t: {}",
            group_identifier
        )?;

        let file_mode: u16 = self.file_entry.get_file_mode();
        let file_mode_info: PosixFileModeInfo = PosixFileModeInfo::new(file_mode);
        writeln!(formatter, "    File mode\t\t\t\t\t: {}", file_mode_info)?;

        let flags: u32 = self.file_entry.get_flags();
        if flags != 0 {
            writeln!(formatter, "    Flags\t\t\t\t\t: 0x{:08x}", flags)?;
            let flags_info: UfsInodeFlagsInfo = UfsInodeFlagsInfo::new(flags);
            write!(formatter, "{}", flags_info)?;
        }
        if let Some(device_identifier) = self.file_entry.get_device_identifier() {
            writeln!(
                formatter,
                "    Device number\t\t\t\t: 0x{:08x}",
                *device_identifier
            )?;
        }
        if let Some(symbolic_link_target) = &self.symbolic_link_target {
            writeln!(
                formatter,
                "    Symbolic link target\t\t\t: {}",
                symbolic_link_target
            )?;
        }
        writeln!(formatter)
    }
}

/// Unix File System (UFS) file system information.
struct UfsFileSystemInfo<'a> {
    /// File system.
    file_system: &'a UfsFileSystem,
}

impl<'a> UfsFileSystemInfo<'a> {
    /// Creates new file system information.
    fn new(file_system: &'a UfsFileSystem) -> Self {
        Self { file_system }
    }
}

impl<'a> fmt::Display for UfsFileSystemInfo<'a> {
    /// Formats the file system information for display.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        writeln!(formatter, "Unix File System (UFS) information:")?;

        let format_version_string: String = self.file_system.get_format_version().to_string();
        writeln!(
            formatter,
            "    Format version\t\t\t\t: {}",
            format_version_string
        )?;

        let byte_order_string: String = self.file_system.get_byte_order().to_string();
        writeln!(formatter, "    Byte order\t\t\t\t\t: {}", byte_order_string)?;

        let volume_label: String = match self.file_system.get_volume_label() {
            Some(volume_label) => volume_label.to_string(),
            None => String::new(),
        };
        writeln!(formatter, "    Volume label\t\t\t\t: {}", volume_label)?;

        let identifier: [u32; 2] = self.file_system.get_file_system_identifier();
        writeln!(
            formatter,
            "    File system identifier\t\t\t: 0x{:08x}, 0x{:08x}",
            identifier[0], identifier[1]
        )?;

        let flags: u32 = self.file_system.get_flags();
        writeln!(formatter, "    Flags\t\t\t\t\t: 0x{:08x}", flags)?;
        if flags != 0 {
            let flags_info: UfsFlagsInfo = UfsFlagsInfo::new(flags);
            writeln!(formatter, "{}", flags_info)?;
        }
        let metadata_check_hash_flags: u32 = self.file_system.get_metadata_check_hash_flags();
        if metadata_check_hash_flags != 0 {
            writeln!(
                formatter,
                "    Metadata check-hash flags\t\t\t: 0x{:08x}",
                metadata_check_hash_flags
            )?;
            let check_hash_flags_info: UfsMetadataCheckHashFlagsInfo =
                UfsMetadataCheckHashFlagsInfo::new(metadata_check_hash_flags);
            writeln!(formatter, "{}", check_hash_flags_info)?;
        }
        let block_size: u64 = self.file_system.get_block_size() as u64;
        let block_byte_size: ByteSize = ByteSize::new(block_size, 1024);
        writeln!(formatter, "    Block size\t\t\t\t\t: {}", block_byte_size)?;

        let fragment_size: u64 = self.file_system.get_fragment_size() as u64;
        let fragment_byte_size: ByteSize = ByteSize::new(fragment_size, 1024);
        writeln!(
            formatter,
            "    Fragment size\t\t\t\t: {}",
            fragment_byte_size
        )?;

        let inode_size: u64 = self.file_system.get_inode_size() as u64;
        let inode_byte_size: ByteSize = ByteSize::new(inode_size, 1024);
        writeln!(formatter, "    Inode size\t\t\t\t\t: {}", inode_byte_size)?;

        let number_of_cylinder_groups: u32 = self.file_system.get_number_of_cylinder_groups();
        writeln!(
            formatter,
            "    Number of cylinder groups\t\t\t: {}",
            number_of_cylinder_groups
        )?;

        let number_of_inodes: u64 = self.file_system.get_number_of_inodes();
        writeln!(
            formatter,
            "    Number of inodes\t\t\t\t: {}",
            number_of_inodes
        )?;

        let last_mount_path: String = match self.file_system.get_last_mount_path() {
            Some(last_mount_path) => last_mount_path.to_string(),
            None => String::new(),
        };
        writeln!(
            formatter,
            "    Last mount path\t\t\t\t: {}",
            last_mount_path
        )?;

        let last_mount_time: &DateTime = self.file_system.get_last_mount_time();
        let last_mount_time_info: UfsDateTimeInfo = UfsDateTimeInfo::new(last_mount_time);
        writeln!(
            formatter,
            "    Last mount time\t\t\t\t: {}",
            last_mount_time_info
        )?;

        let last_written_time: &DateTime = self.file_system.get_last_written_time();
        let last_written_time_info: UfsDateTimeInfo = UfsDateTimeInfo::new(last_written_time);
        writeln!(
            formatter,
            "    Last written time\t\t\t\t: {}",
            last_written_time_info
        )?;
        writeln!(formatter)
    }
}

/// Fast File System (FFS) superblock flags information.
struct UfsFlagsInfo {
    /// Flags.
    flags: u32,
}

impl UfsFlagsInfo {
    /// Creates new flags information.
    fn new(flags: u32) -> Self {
        Self { flags }
    }
}

impl fmt::Display for UfsFlagsInfo {
    /// Formats the superblock flags for display.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        if self.flags & FFS_FLAG_UNCLEAN != 0 {
            writeln!(formatter, "        0x00000001: Unclean (FS_UNCLEAN)")?;
        }
        if self.flags & FFS_FLAG_SOFT_UPDATES != 0 {
            writeln!(formatter, "        0x00000002: Soft Updates (FS_DOSOFTDEP)")?;
        }
        if self.flags & FFS_FLAG_NEEDS_FSCK != 0 {
            writeln!(
                formatter,
                "        0x00000004: Needs consistency check (FS_NEEDSFSCK)"
            )?;
        }
        if self.flags & FFS_FLAG_SOFT_UPDATES_JOURNALING != 0 {
            writeln!(
                formatter,
                "        0x00000008: Soft Updates Journaling (FS_SUJ)"
            )?;
        }
        if self.flags & FFS_FLAG_POSIX_ACLS != 0 {
            writeln!(formatter, "        0x00000010: POSIX.1e ACLs (FS_ACLS)")?;
        }
        if self.flags & FFS_FLAG_MULTILABEL != 0 {
            writeln!(
                formatter,
                "        0x00000020: MAC multi-label (FS_MULTILABEL)"
            )?;
        }
        if self.flags & FFS_FLAG_GEOM_JOURNAL != 0 {
            writeln!(
                formatter,
                "        0x00000040: GEOM journaling (FS_GJOURNAL)"
            )?;
        }
        if self.flags & FFS_FLAG_FLAGS_UPDATED != 0 {
            writeln!(
                formatter,
                "        0x00000080: Flags updated (FS_FLAGS_UPDATED)"
            )?;
        }
        if self.flags & FFS_FLAG_NFS4_ACLS != 0 {
            writeln!(formatter, "        0x00000100: NFSv4 ACLs (FS_NFS4ACLS)")?;
        }
        if self.flags & FFS_FLAG_METADATA_CHECK_HASH != 0 {
            writeln!(
                formatter,
                "        0x00000200: Metadata check-hashes (FS_METACKHASH)"
            )?;
        }
        if self.flags & FFS_FLAG_TRIM != 0 {
            writeln!(formatter, "        0x00000400: TRIM enabled (FS_TRIM)")?;
        }
        Ok(())
    }
}

/// Unix File System (UFS) inode flags information.
struct UfsInodeFlagsInfo {
    /// Flags.
    flags: u32,
}

impl UfsInodeFlagsInfo {
    /// Creates new inode flags information.
    fn new(flags: u32) -> Self {
        Self { flags }
    }
}

impl fmt::Display for UfsInodeFlagsInfo {
    /// Formats the inode flags for display.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        if self.flags & 0x00000001 != 0 {
            writeln!(formatter, "        0x00000001: Do not dump (UF_NODUMP)")?;
        }
        if self.flags & 0x00000002 != 0 {
            writeln!(formatter, "        0x00000002: Immutable (UF_IMMUTABLE)")?;
        }
        if self.flags & 0x00000004 != 0 {
            writeln!(formatter, "        0x00000004: Append-only (UF_APPEND)")?;
        }
        if self.flags & 0x00000008 != 0 {
            writeln!(formatter, "        0x00000008: Opaque (UF_OPAQUE)")?;
        }
        if self.flags & 0x00000010 != 0 {
            writeln!(formatter, "        0x00000010: Undeletable (UF_NOUNLINK)")?;
        }
        if self.flags & 0x00010000 != 0 {
            writeln!(formatter, "        0x00010000: Archived (SF_ARCHIVED)")?;
        }
        if self.flags & 0x00020000 != 0 {
            writeln!(
                formatter,
                "        0x00020000: System immutable (SF_IMMUTABLE)"
            )?;
        }
        if self.flags & 0x00040000 != 0 {
            writeln!(
                formatter,
                "        0x00040000: System append-only (SF_APPEND)"
            )?;
        }
        if self.flags & 0x00100000 != 0 {
            writeln!(
                formatter,
                "        0x00100000: System undeletable (SF_NOUNLINK)"
            )?;
        }
        if self.flags & 0x00200000 != 0 {
            writeln!(formatter, "        0x00200000: Snapshot (SF_SNAPSHOT)")?;
        }
        Ok(())
    }
}

/// Fast File System (FFS) metadata check-hash flags information.
struct UfsMetadataCheckHashFlagsInfo {
    /// Flags.
    flags: u32,
}

impl UfsMetadataCheckHashFlagsInfo {
    /// Creates new metadata check-hash flags information.
    fn new(flags: u32) -> Self {
        Self { flags }
    }
}

impl fmt::Display for UfsMetadataCheckHashFlagsInfo {
    /// Formats the metadata check-hash flags for display.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        if self.flags & FFS_METADATA_CHECK_HASH_CYLINDER_GROUP != 0 {
            writeln!(formatter, "        0x00000001: Cylinder group (CK_CYLGRP)")?;
        }
        if self.flags & FFS_METADATA_CHECK_HASH_SUPERBLOCK != 0 {
            writeln!(formatter, "        0x00000002: Superblock (CK_SUPERBLOCK)")?;
        }
        if self.flags & FFS_METADATA_CHECK_HASH_INODE != 0 {
            writeln!(formatter, "        0x00000004: Inode (CK_INODE)")?;
        }
        if self.flags & FFS_METADATA_CHECK_HASH_INDIRECT_BLOCK != 0 {
            writeln!(formatter, "        0x00000008: Indirect block (CK_INDIR)")?;
        }
        if self.flags & FFS_METADATA_CHECK_HASH_DIRECTORY != 0 {
            writeln!(formatter, "        0x00000010: Directory (CK_DIR)")?;
        }
        Ok(())
    }
}

/// Information about the Unix File System (UFS) / Fast File System (FFS) format.
pub struct UfsInfo {}

impl UfsInfo {
    /// Opens a UFS file system.
    pub fn open_file_system(
        data_stream: &DataStreamReference,
        character_encoding: Option<&CharacterEncoding>,
    ) -> Result<UfsFileSystem, ErrorTrace> {
        let mut ufs_file_system: UfsFileSystem = UfsFileSystem::new();

        if let Some(encoding) = character_encoding {
            match ufs_file_system.set_character_encoding(encoding) {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to set character encoding"
                    );
                    return Err(error);
                }
            }
        }
        match ufs_file_system.read_data_stream(data_stream) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open UFS file system");
                return Err(error);
            }
        }
        Ok(ufs_file_system)
    }

    /// Prints information about a file entry.
    fn print_file_entry(file_entry: &mut UfsFileEntry) -> Result<(), ErrorTrace> {
        let symbolic_link_target: Option<ByteString> = match file_entry.get_symbolic_link_target() {
            Ok(link_target) => link_target.cloned(),
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to retrieve symbolic link target"
                );
                return Err(error);
            }
        };
        let mut file_entry_information: UfsFileEntryInfo = UfsFileEntryInfo::new(file_entry);
        file_entry_information.symbolic_link_target = symbolic_link_target;

        print!("{}", file_entry_information);

        let number_of_attributes: usize = match file_entry.get_number_of_extended_attributes() {
            Ok(number_of_attributes) => number_of_attributes,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to retrieve number of extended attributes"
                );
                return Err(error);
            }
        };
        if number_of_attributes > 0 {
            println!("    Extended attributes:");

            for (attribute_index, result) in file_entry.extended_attributes().enumerate() {
                let ufs_extended_attribute: UfsExtendedAttribute = match result {
                    Ok(extended_attribute) => extended_attribute,
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            format!("Unable to retrieve extended attribute: {}", attribute_index)
                        );
                        return Err(error);
                    }
                };
                let attribute_name: &ByteString = ufs_extended_attribute.get_name();

                println!(
                    "        Attribute {}\t\t\t\t: {}",
                    attribute_index + 1,
                    attribute_name
                );
            }
            println!();
        }
        Ok(())
    }

    /// Prints information about a specific file entry by inode number.
    pub fn print_file_entry_by_identifier(
        data_stream: &DataStreamReference,
        ufs_entry_identifier: u64,
        character_encoding: Option<&CharacterEncoding>,
    ) -> Result<(), ErrorTrace> {
        let ufs_file_system: UfsFileSystem =
            match Self::open_file_system(data_stream, character_encoding) {
                Ok(ufs_file_system) => ufs_file_system,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(error, "Unable to open file system");
                    return Err(error);
                }
            };
        if ufs_entry_identifier > u32::MAX as u64 {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported identifier: {} value out of bounds",
                ufs_entry_identifier
            )));
        }
        let mut file_entry: UfsFileEntry =
            match ufs_file_system.get_file_entry_by_identifier(ufs_entry_identifier as u32) {
                Ok(file_entry) => file_entry,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!("Unable to retrieve file entry: {}", ufs_entry_identifier)
                    );
                    return Err(error);
                }
            };
        println!("Unix File System (UFS) file entry information:");

        match Self::print_file_entry(&mut file_entry) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!("Unable to print file entry: {}", ufs_entry_identifier)
                );
                return Err(error);
            }
        }
        Ok(())
    }

    /// Prints information about a specific file entry by path.
    pub fn print_file_entry_by_path(
        data_stream: &DataStreamReference,
        path: &Path,
        character_encoding: Option<&CharacterEncoding>,
    ) -> Result<(), ErrorTrace> {
        let ufs_file_system: UfsFileSystem =
            match Self::open_file_system(data_stream, character_encoding) {
                Ok(ufs_file_system) => ufs_file_system,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(error, "Unable to open file system");
                    return Err(error);
                }
            };
        let mut file_entry: UfsFileEntry = match ufs_file_system.get_file_entry_by_path(path) {
            Ok(Some(file_entry)) => file_entry,
            Ok(None) => {
                return Err(keramics_core::error_trace_new!("Missing file entry"));
            }
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to retrieve file entry");
                return Err(error);
            }
        };
        println!("Unix File System (UFS) file entry information:");
        println!("    Path\t\t\t\t\t: {}", path);

        match Self::print_file_entry(&mut file_entry) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to print file entry");
                return Err(error);
            }
        }
        Ok(())
    }

    /// Prints information about the file system.
    pub fn print_file_system(
        data_stream: &DataStreamReference,
        character_encoding: Option<&CharacterEncoding>,
    ) -> Result<(), ErrorTrace> {
        let ufs_file_system: UfsFileSystem =
            match Self::open_file_system(data_stream, character_encoding) {
                Ok(ufs_file_system) => ufs_file_system,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(error, "Unable to open file system");
                    return Err(error);
                }
            };
        let file_system_information: UfsFileSystemInfo = UfsFileSystemInfo::new(&ufs_file_system);

        print!("{}", file_system_information);

        Ok(())
    }

    /// Prints the file system hierarchy.
    pub fn print_hierarchy(
        data_stream: &DataStreamReference,
        character_encoding: Option<&CharacterEncoding>,
        path: Option<&String>,
    ) -> Result<(), ErrorTrace> {
        let ufs_file_system: UfsFileSystem =
            match Self::open_file_system(data_stream, character_encoding) {
                Ok(ufs_file_system) => ufs_file_system,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(error, "Unable to open file system");
                    return Err(error);
                }
            };
        println!("Unix File System (UFS) hierarchy:");

        let mut file_entry: UfsFileEntry = match path {
            Some(path_string) => {
                let lookup_path: Path = Path::from(path_string);
                match ufs_file_system.get_file_entry_by_path(&lookup_path) {
                    Ok(Some(file_entry)) => file_entry,
                    Ok(None) => {
                        return Err(keramics_core::error_trace_new!(format!(
                            "Missing file entry for path: {}",
                            path_string
                        )));
                    }
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            format!("Unable to retrieve file entry for path: {}", path_string)
                        );
                        return Err(error);
                    }
                }
            }
            None => match ufs_file_system.get_root_directory() {
                Ok(Some(file_entry)) => file_entry,
                Ok(None) => {
                    println!("No root directory found");
                    return Ok(());
                }
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to retrieve root directory"
                    );
                    return Err(error);
                }
            },
        };
        let mut path_components: Vec<String> = Vec::new();
        let mut levels: Vec<bool> = Vec::new();

        match Self::print_hierarchy_file_entry(&mut file_entry, &mut path_components, &mut levels) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    "Unable to print file entry hierarchy"
                );
                return Err(error);
            }
        }
        Ok(())
    }

    /// Prints the file entry hierarchy.
    fn print_hierarchy_file_entry(
        file_entry: &mut UfsFileEntry,
        path_components: &mut Vec<String>,
        levels: &mut Vec<bool>,
    ) -> Result<(), ErrorTrace> {
        let path: String = if file_entry.is_root_directory() {
            String::from("/")
        } else {
            let name_string: String = match file_entry.get_name() {
                Some(name) => name.to_string(),
                None => String::new(),
            };
            path_components.push(name_string);
            let joined_components: String = path_components.join("/");
            format!("/{}", joined_components)
        };
        let prefix: String = crate::hierarchy::get_hierarchy_prefix(levels);
        println!("{}{}", prefix, path);

        let number_of_sub_file_entries: usize = match file_entry.get_number_of_sub_file_entries() {
            Ok(number) => number,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!(
                        "Unable to retrieve number of sub file entries of path: {}",
                        path
                    )
                );
                return Err(error);
            }
        };
        for (sub_file_entry_index, result) in file_entry.sub_file_entries().enumerate() {
            let mut sub_file_entry: UfsFileEntry = match result {
                Ok(file_entry) => file_entry,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!(
                            "Unable to retrieve sub file entry: {} of path: {}",
                            sub_file_entry_index, path
                        )
                    );
                    return Err(error);
                }
            };
            let is_last: bool = sub_file_entry_index + 1 == number_of_sub_file_entries;
            levels.push(is_last);
            match Self::print_hierarchy_file_entry(&mut sub_file_entry, path_components, levels) {
                Ok(_) => {}
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!(
                            "Unable to print hierarchy of sub file entry: {} of path: {}",
                            sub_file_entry_index, path
                        )
                    );
                    return Err(error);
                }
            }
            levels.pop();
        }
        if !file_entry.is_root_directory() {
            path_components.pop();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_core::open_os_data_stream;

    #[test]
    fn test_file_system_info_ufs1_and_ufs2() -> Result<(), ErrorTrace> {
        let ufs1_path: PathBuf = PathBuf::from("../test_data/ufs/ufs1.raw");
        let ufs1_stream: DataStreamReference = open_os_data_stream(&ufs1_path)?;
        let ufs1_file_system: UfsFileSystem =
            UfsInfo::open_file_system(&ufs1_stream, Some(&CharacterEncoding::Utf8))?;
        let ufs1_info: UfsFileSystemInfo = UfsFileSystemInfo::new(&ufs1_file_system);
        let ufs1_text: String = ufs1_info.to_string();

        let has_ufs1_version: bool = ufs1_text.contains("FFS1 (UFS1)");
        assert!(has_ufs1_version);
        let has_ufs1_label: bool = ufs1_text.contains("ufs1_test");
        assert!(has_ufs1_label);

        let ufs2_path: PathBuf = PathBuf::from("../test_data/ufs/ufs2.raw");
        let ufs2_stream: DataStreamReference = open_os_data_stream(&ufs2_path)?;
        let ufs2_file_system: UfsFileSystem =
            UfsInfo::open_file_system(&ufs2_stream, Some(&CharacterEncoding::Utf8))?;
        let ufs2_info: UfsFileSystemInfo = UfsFileSystemInfo::new(&ufs2_file_system);
        let ufs2_text: String = ufs2_info.to_string();

        let has_ufs2_version: bool = ufs2_text.contains("FFS2 (UFS2)");
        assert!(has_ufs2_version);
        let has_ufs2_label: bool = ufs2_text.contains("ufs2_test");
        assert!(has_ufs2_label);
        let has_soft_updates: bool = ufs2_text.contains("FS_DOSOFTDEP");
        assert!(has_soft_updates);

        Ok(())
    }

    #[test]
    fn test_print_methods_ufs1_and_ufs2() -> Result<(), ErrorTrace> {
        let ufs1_path: PathBuf = PathBuf::from("../test_data/ufs/ufs1.raw");
        let ufs1_stream: DataStreamReference = open_os_data_stream(&ufs1_path)?;
        UfsInfo::print_file_system(&ufs1_stream, None)?;
        UfsInfo::print_file_entry_by_identifier(&ufs1_stream, 2, None)?;
        let file_path: Path = Path::from("/testdir1/testfile1");
        UfsInfo::print_file_entry_by_path(&ufs1_stream, &file_path, None)?;
        UfsInfo::print_hierarchy(&ufs1_stream, None, None)?;

        let ufs2_path: PathBuf = PathBuf::from("../test_data/ufs/ufs2.raw");
        let ufs2_stream: DataStreamReference = open_os_data_stream(&ufs2_path)?;
        UfsInfo::print_file_system(&ufs2_stream, None)?;
        UfsInfo::print_file_entry_by_identifier(&ufs2_stream, 769, None)?;
        let symbolic_link_path: Path = Path::from("/file_symboliclink1");
        UfsInfo::print_file_entry_by_path(&ufs2_stream, &symbolic_link_path, None)?;
        UfsInfo::print_hierarchy(&ufs2_stream, None, None)?;

        Ok(())
    }
}

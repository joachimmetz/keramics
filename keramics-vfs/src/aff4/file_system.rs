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

use std::sync::Arc;

use keramics_core::ErrorTrace;
use keramics_formats::aff4::Aff4Image;
use keramics_formats::{FileResolverReference, Path, PathComponent};

use crate::file_resolver::new_vfs_file_resolver;
use crate::location::VfsLocation;
use crate::path::VfsPath;
use crate::types::VfsFileSystemReference;

use super::file_entry::Aff4FileEntry;

/// Advanced Forensic Format 4 (AFF4) volume file system.
///
/// An AFF4 volume can contain multiple streams, each stream being a separate
/// potential disk image of the volume. The streams are conceptually not the
/// same as image layers, they are closer to volumes within a volume system.
pub struct Aff4FileSystem {
    /// Image.
    image: Arc<Aff4Image>,

    /// Number of streams.
    number_of_streams: usize,
}

impl Aff4FileSystem {
    pub const PATH_PREFIX: &'static str = "/aff4";

    /// Creates a new file system.
    pub fn new() -> Self {
        Self {
            image: Arc::new(Aff4Image::new()),
            number_of_streams: 0,
        }
    }

    /// Determines if the file entry with the specified path exists.
    pub fn file_entry_exists(&self, path: &Path) -> bool {
        if path.is_relative() {
            return false;
        }
        match path.get_component_by_index(1) {
            Some(path_component) => {
                if path.get_number_of_components() > 2 {
                    return false;
                }
                match VfsPath::get_numeric_suffix(path_component, "aff4") {
                    Some(stream_index) => {
                        stream_index >= 1 && stream_index <= self.number_of_streams
                    }
                    None => false,
                }
            }
            None => !path.is_empty(),
        }
    }

    /// Retrieves the file entry with the specific location.
    pub fn get_file_entry_by_path(&self, path: &Path) -> Result<Option<Aff4FileEntry>, ErrorTrace> {
        if path.is_relative() {
            return Ok(None);
        }
        match path.get_component_by_index(1) {
            Some(path_component) => {
                if path.get_number_of_components() > 2 {
                    return Ok(None);
                }
                let mut stream_index: usize =
                    match VfsPath::get_numeric_suffix(path_component, "aff4") {
                        Some(stream_index) => stream_index,
                        None => return Ok(None),
                    };
                if stream_index == 0 || stream_index > self.number_of_streams {
                    return Ok(None);
                }
                stream_index -= 1;
                Ok(Some(Aff4FileEntry::Stream {
                    name_index: stream_index,
                    image: self.image.clone(),
                }))
            }
            None => {
                if path.is_empty() {
                    return Ok(None);
                }
                Ok(Some(self.get_root_file_entry()))
            }
        }
    }

    /// Retrieves the root file entry.
    pub fn get_root_file_entry(&self) -> Aff4FileEntry {
        Aff4FileEntry::Root {
            image: self.image.clone(),
        }
    }

    /// Opens the file system.
    pub fn open(
        &mut self,
        parent_file_system: Option<&VfsFileSystemReference>,
        vfs_location: &VfsLocation,
    ) -> Result<(), ErrorTrace> {
        let file_system: &VfsFileSystemReference = match parent_file_system {
            Some(file_system) => file_system,
            None => {
                return Err(keramics_core::error_trace_new!(
                    "Missing parent file system"
                ));
            }
        };
        let path: &Path = vfs_location.get_path();

        match Arc::get_mut(&mut self.image) {
            Some(image) => {
                match Self::open_image(image, file_system, path) {
                    Ok(_) => {}
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(error, "Unable to open AFF4 image");
                        return Err(error);
                    }
                }
                self.number_of_streams = image.get_number_of_images();
            }
            None => {
                return Err(keramics_core::error_trace_new!(
                    "Unable to obtain mutable reference to AFF4 image"
                ));
            }
        }
        Ok(())
    }

    /// Opens an AFF4 image.
    pub(crate) fn open_image(
        image: &mut Aff4Image,
        file_system: &VfsFileSystemReference,
        path: &Path,
    ) -> Result<(), ErrorTrace> {
        let parent_path: Path = path.new_with_parent_directory();

        let file_resolver: FileResolverReference =
            match new_vfs_file_resolver(file_system, parent_path) {
                Ok(file_resolver) => file_resolver,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to create VFS file resolver"
                    );
                    return Err(error);
                }
            };
        let file_name: &PathComponent = match path.file_name() {
            Some(file_name) => file_name,
            None => {
                return Err(keramics_core::error_trace_new!(
                    "Unable to retrieve file name"
                ));
            }
        };
        match image.open(&file_resolver, file_name) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open AFF4 image");
                return Err(error);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use keramics_formats::PathComponent;

    use crate::enums::{VfsFileType, VfsType};
    use crate::file_system::VfsFileSystem;

    use crate::tests::get_test_data_path;

    fn get_file_system() -> Result<Aff4FileSystem, ErrorTrace> {
        let mut aff4_file_system: Aff4FileSystem = Aff4FileSystem::new();

        let parent_file_system: VfsFileSystemReference =
            VfsFileSystemReference::new(VfsFileSystem::new(&VfsType::Os));
        let path_string: String = get_test_data_path("aff4/ext2.aff4");
        let parent_vfs_location: VfsLocation = VfsLocation::from(&path_string);
        aff4_file_system.open(Some(&parent_file_system), &parent_vfs_location)?;

        Ok(aff4_file_system)
    }

    #[test]
    fn test_file_entry_exists() -> Result<(), ErrorTrace> {
        let aff4_file_system: Aff4FileSystem = get_file_system()?;

        let path: Path = Path::from("/");
        let result: bool = aff4_file_system.file_entry_exists(&path);
        assert_eq!(result, true);

        let path: Path = Path::from("/aff41");
        let result: bool = aff4_file_system.file_entry_exists(&path);
        assert_eq!(result, true);

        let path: Path = Path::from("/aff499");
        let result: bool = aff4_file_system.file_entry_exists(&path);
        assert_eq!(result, false);

        let path: Path = Path::from("aff41");
        let result: bool = aff4_file_system.file_entry_exists(&path);
        assert_eq!(result, false);

        let path: Path = Path::from("/bogus");
        let result: bool = aff4_file_system.file_entry_exists(&path);
        assert_eq!(result, false);

        let path: Path = Path::from("/aff41/bogus1");
        let result: bool = aff4_file_system.file_entry_exists(&path);
        assert_eq!(result, false);

        Ok(())
    }

    #[test]
    fn test_get_file_entry_by_path() -> Result<(), ErrorTrace> {
        let aff4_file_system: Aff4FileSystem = get_file_system()?;

        let path: Path = Path::from("/");
        let result: Option<Aff4FileEntry> = aff4_file_system.get_file_entry_by_path(&path)?;
        assert!(result.is_some());

        let aff4_file_entry: Aff4FileEntry = result.unwrap();

        let name: PathComponent = aff4_file_entry.get_name();
        assert_eq!(name, PathComponent::Root);

        let file_type: VfsFileType = aff4_file_entry.get_file_type();
        assert_eq!(file_type, VfsFileType::Directory);

        let path: Path = Path::from("/aff41");
        let result: Option<Aff4FileEntry> = aff4_file_system.get_file_entry_by_path(&path)?;
        assert!(result.is_some());

        let aff4_file_entry: Aff4FileEntry = result.unwrap();

        let name: PathComponent = aff4_file_entry.get_name();
        assert_eq!(name, PathComponent::from("aff41"));

        let file_type: VfsFileType = aff4_file_entry.get_file_type();
        assert_eq!(file_type, VfsFileType::File);

        let path: Path = Path::from("/bogus");
        let result: Option<Aff4FileEntry> = aff4_file_system.get_file_entry_by_path(&path)?;
        assert!(result.is_none());

        Ok(())
    }

    #[test]
    fn test_get_root_file_entry() -> Result<(), ErrorTrace> {
        let aff4_file_system: Aff4FileSystem = get_file_system()?;

        let aff4_file_entry: Aff4FileEntry = aff4_file_system.get_root_file_entry();
        assert!(matches!(aff4_file_entry, Aff4FileEntry::Root { .. }));

        Ok(())
    }

    #[test]
    fn test_open() -> Result<(), ErrorTrace> {
        let mut aff4_file_system: Aff4FileSystem = Aff4FileSystem::new();

        let parent_file_system: VfsFileSystemReference =
            VfsFileSystemReference::new(VfsFileSystem::new(&VfsType::Os));
        let path_string: String = get_test_data_path("aff4/ext2.aff4");
        let parent_vfs_location: VfsLocation = VfsLocation::from(&path_string);
        aff4_file_system.open(Some(&parent_file_system), &parent_vfs_location)?;

        assert_eq!(aff4_file_system.number_of_streams, 1);

        Ok(())
    }
}

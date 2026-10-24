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

use keramics_core::{DataStreamReference, ErrorTrace};
use keramics_formats::PathComponent;
use keramics_formats::aff4::Aff4Image;

use crate::enums::VfsFileType;

/// Advanced Forensic Format 4 (AFF4) file entry.
///
/// An AFF4 volume can contain multiple streams, each stream being a separate
/// potential disk image of the volume. The streams are conceptually not the
/// same as image layers, they are closer to volumes within a volume system.
pub enum Aff4FileEntry {
    /// Root file entry.
    Root {
        /// Image.
        image: Arc<Aff4Image>,
    },

    /// Stream file entry.
    Stream {
        /// File name index.
        name_index: usize,

        /// Image.
        image: Arc<Aff4Image>,
    },
}

impl Aff4FileEntry {
    /// Retrieves the default data stream.
    pub fn get_data_stream(&self) -> Result<Option<DataStreamReference>, ErrorTrace> {
        match self {
            Aff4FileEntry::Root { .. } => Ok(None),
            Aff4FileEntry::Stream {
                name_index, image, ..
            } => match image.get_data_stream_by_index(*name_index) {
                Ok(data_stream) => Ok(Some(data_stream)),
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        "Unable to retrieve AFF4 stream data stream"
                    );
                    Err(error)
                }
            },
        }
    }

    /// Retrieves the file type.
    pub fn get_file_type(&self) -> VfsFileType {
        match self {
            Aff4FileEntry::Root { .. } => VfsFileType::Directory,
            Aff4FileEntry::Stream { .. } => VfsFileType::File,
        }
    }

    /// Retrieves the name.
    pub fn get_name(&self) -> PathComponent {
        match self {
            Aff4FileEntry::Root { .. } => PathComponent::Root,
            Aff4FileEntry::Stream { name_index, .. } => {
                PathComponent::from(format!("aff4.s{}", name_index + 1))
            }
        }
    }

    /// Retrieves the number of sub file entries.
    pub fn get_number_of_sub_file_entries(&self) -> usize {
        match self {
            Aff4FileEntry::Root { image } => image.get_number_of_images(),
            Aff4FileEntry::Stream { .. } => 0,
        }
    }

    /// Determines if the file entry is the root file entry.
    pub fn is_root_file_entry(&self) -> bool {
        match self {
            Aff4FileEntry::Root { .. } => true,
            Aff4FileEntry::Stream { .. } => false,
        }
    }

    /// Retrieves a specific sub file entry.
    pub fn get_sub_file_entry_by_index(
        &self,
        sub_file_entry_index: usize,
    ) -> Result<Aff4FileEntry, ErrorTrace> {
        match self {
            Aff4FileEntry::Root { image } => {
                if sub_file_entry_index >= image.get_number_of_images() {
                    return Err(keramics_core::error_trace_new!(format!(
                        "No sub file entry with index: {}",
                        sub_file_entry_index
                    )));
                }
                Ok(Aff4FileEntry::Stream {
                    name_index: sub_file_entry_index,
                    image: image.clone(),
                })
            }
            Aff4FileEntry::Stream { .. } => {
                Err(keramics_core::error_trace_new!("No sub file entries"))
            }
        }
    }

    /// Retrieves the size.
    pub fn get_size(&self) -> u64 {
        match self {
            Aff4FileEntry::Root { .. } => 0,
            Aff4FileEntry::Stream { name_index, image } => {
                image.get_media_size_by_index(*name_index).unwrap_or(0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_formats::{FileResolverReference, OsFileResolver, PathComponent};

    use crate::tests::get_test_data_path;

    fn get_image() -> Result<Arc<Aff4Image>, ErrorTrace> {
        let mut image: Aff4Image = Aff4Image::new();

        let path_string: String = get_test_data_path("aff4");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(OsFileResolver::new(path_buf)));
        let file_name: PathComponent = PathComponent::from("ext2.aff4");
        image.open(&file_resolver, &file_name)?;

        Ok(Arc::new(image))
    }

    fn get_root_file_entry(aff4_image: &Arc<Aff4Image>) -> Aff4FileEntry {
        Aff4FileEntry::Root {
            image: aff4_image.clone(),
        }
    }

    fn get_stream_file_entry(aff4_image: &Arc<Aff4Image>, name_index: usize) -> Aff4FileEntry {
        Aff4FileEntry::Stream {
            name_index,
            image: aff4_image.clone(),
        }
    }

    #[test]
    fn test_get_data_stream() -> Result<(), ErrorTrace> {
        let test_image: Arc<Aff4Image> = get_image()?;

        let file_entry: Aff4FileEntry = get_root_file_entry(&test_image);

        let data_stream: Option<DataStreamReference> = file_entry.get_data_stream()?;
        assert!(data_stream.is_none());

        let file_entry: Aff4FileEntry = get_stream_file_entry(&test_image, 0);

        let data_stream: Option<DataStreamReference> = file_entry.get_data_stream()?;
        assert!(data_stream.is_some());
        assert_eq!(
            keramics_core::data_stream_get_size!(data_stream.unwrap()),
            4194304
        );

        Ok(())
    }

    #[test]
    fn test_get_file_type() -> Result<(), ErrorTrace> {
        let test_image: Arc<Aff4Image> = get_image()?;

        let file_entry: Aff4FileEntry = get_root_file_entry(&test_image);

        let file_type: VfsFileType = file_entry.get_file_type();
        assert_eq!(file_type, VfsFileType::Directory);

        let file_entry: Aff4FileEntry = get_stream_file_entry(&test_image, 0);

        let file_type: VfsFileType = file_entry.get_file_type();
        assert_eq!(file_type, VfsFileType::File);

        Ok(())
    }

    #[test]
    fn test_get_name() -> Result<(), ErrorTrace> {
        let test_image: Arc<Aff4Image> = get_image()?;

        let file_entry: Aff4FileEntry = get_root_file_entry(&test_image);

        let name: PathComponent = file_entry.get_name();
        assert_eq!(name, PathComponent::Root);

        let file_entry: Aff4FileEntry = get_stream_file_entry(&test_image, 0);

        let name: PathComponent = file_entry.get_name();
        assert_eq!(name, PathComponent::from("aff4.s1"));

        Ok(())
    }

    #[test]
    fn test_get_size() -> Result<(), ErrorTrace> {
        let test_image: Arc<Aff4Image> = get_image()?;

        let file_entry: Aff4FileEntry = get_root_file_entry(&test_image);

        let size: u64 = file_entry.get_size();
        assert_eq!(size, 0);

        let file_entry: Aff4FileEntry = get_stream_file_entry(&test_image, 0);

        let size: u64 = file_entry.get_size();
        assert_eq!(size, 4194304);

        Ok(())
    }

    #[test]
    fn test_get_number_of_sub_file_entries() -> Result<(), ErrorTrace> {
        let test_image: Arc<Aff4Image> = get_image()?;

        let file_entry: Aff4FileEntry = get_root_file_entry(&test_image);

        let number_of_sub_file_entries: usize = file_entry.get_number_of_sub_file_entries();
        assert_eq!(number_of_sub_file_entries, 1);

        let file_entry: Aff4FileEntry = get_stream_file_entry(&test_image, 0);

        let number_of_sub_file_entries: usize = file_entry.get_number_of_sub_file_entries();
        assert_eq!(number_of_sub_file_entries, 0);

        Ok(())
    }

    #[test]
    fn test_get_sub_file_entry_by_index() -> Result<(), ErrorTrace> {
        let test_image: Arc<Aff4Image> = get_image()?;

        let file_entry: Aff4FileEntry = get_root_file_entry(&test_image);

        let sub_file_entry: Aff4FileEntry = file_entry.get_sub_file_entry_by_index(0)?;

        let name: PathComponent = sub_file_entry.get_name();
        assert_eq!(name, PathComponent::from("aff4.s1"));

        let result: Result<Aff4FileEntry, ErrorTrace> = file_entry.get_sub_file_entry_by_index(99);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_is_root_file_entry() -> Result<(), ErrorTrace> {
        let test_image: Arc<Aff4Image> = get_image()?;

        let file_entry: Aff4FileEntry = get_root_file_entry(&test_image);
        assert_eq!(file_entry.is_root_file_entry(), true);

        let file_entry: Aff4FileEntry = get_stream_file_entry(&test_image, 0);
        assert_eq!(file_entry.is_root_file_entry(), false);

        Ok(())
    }
}

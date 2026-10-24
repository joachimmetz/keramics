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
use keramics_formats::aff::AffImage;

use crate::enums::VfsFileType;

/// AFF (Advanced Forensic Format) storage media image file entry.
pub enum AffFileEntry {
    /// Layer file entry.
    Layer {
        /// Image.
        image: Arc<AffImage>,
    },

    /// Root file entry.
    Root {
        /// Image.
        image: Arc<AffImage>,
    },
}

impl AffFileEntry {
    /// Retrieves the default data stream.
    pub fn get_data_stream(&self) -> Result<Option<DataStreamReference>, ErrorTrace> {
        match self {
            AffFileEntry::Layer { image, .. } => Ok(Some(image.get_data_stream())),
            AffFileEntry::Root { .. } => Ok(None),
        }
    }

    /// Retrieves the file type.
    pub fn get_file_type(&self) -> VfsFileType {
        match self {
            AffFileEntry::Layer { .. } => VfsFileType::File,
            AffFileEntry::Root { .. } => VfsFileType::Directory,
        }
    }

    /// Retrieves the name.
    pub fn get_name(&self) -> PathComponent {
        match self {
            AffFileEntry::Layer { .. } => PathComponent::from("aff1"),
            AffFileEntry::Root { .. } => PathComponent::Root,
        }
    }

    /// Retrieves the size.
    pub fn get_size(&self) -> u64 {
        match self {
            AffFileEntry::Layer { image, .. } => image.get_media_size(),
            AffFileEntry::Root { .. } => 0,
        }
    }

    /// Retrieves the number of sub file entries.
    pub fn get_number_of_sub_file_entries(&self) -> usize {
        match self {
            AffFileEntry::Layer { .. } => 0,
            AffFileEntry::Root { .. } => 1,
        }
    }

    /// Retrieves a specific sub file entry.
    pub fn get_sub_file_entry_by_index(
        &self,
        sub_file_entry_index: usize,
    ) -> Result<AffFileEntry, ErrorTrace> {
        match self {
            AffFileEntry::Layer { .. } => {
                Err(keramics_core::error_trace_new!("No sub file entries"))
            }
            AffFileEntry::Root { image } => {
                if sub_file_entry_index != 0 {
                    return Err(keramics_core::error_trace_new!(format!(
                        "No sub file entry with index: {}",
                        sub_file_entry_index
                    )));
                }
                Ok(AffFileEntry::Layer {
                    image: image.clone(),
                })
            }
        }
    }

    /// Determines if the file entry is the root file entry.
    pub fn is_root_file_entry(&self) -> bool {
        match self {
            AffFileEntry::Layer { .. } => false,
            AffFileEntry::Root { .. } => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use keramics_formats::{FileResolverReference, OsFileResolver, PathComponent};

    use crate::tests::get_test_data_path;

    fn get_image() -> Result<Arc<AffImage>, ErrorTrace> {
        let mut image: AffImage = AffImage::new();

        let path_string: String = get_test_data_path("aff");
        let path_buf: PathBuf = PathBuf::from(path_string.as_str());
        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(OsFileResolver::new(path_buf)));
        let file_name: PathComponent = PathComponent::from("ext2.AFF");
        image.open(&file_resolver, &file_name)?;

        Ok(Arc::new(image))
    }

    // TODO: implement get_layer_file_entry
    // TODO: implement get_root_file_entry

    fn get_layer_file_entry(aff_image: &Arc<AffImage>) -> Result<AffFileEntry, ErrorTrace> {
        Ok(AffFileEntry::Layer {
            image: aff_image.clone(),
        })
    }

    fn get_root_file_entry(aff_image: &Arc<AffImage>) -> AffFileEntry {
        AffFileEntry::Root {
            image: aff_image.clone(),
        }
    }

    // TODO: add tests for get_data_stream

    #[test]
    fn test_get_file_type() -> Result<(), ErrorTrace> {
        let test_image: Arc<AffImage> = get_image()?;

        let file_entry: AffFileEntry = get_root_file_entry(&test_image);

        let file_type: VfsFileType = file_entry.get_file_type();
        assert_eq!(file_type, VfsFileType::Directory);

        let file_entry: AffFileEntry = get_layer_file_entry(&test_image)?;

        let file_type: VfsFileType = file_entry.get_file_type();
        assert_eq!(file_type, VfsFileType::File);

        Ok(())
    }

    #[test]
    fn test_get_name() -> Result<(), ErrorTrace> {
        let test_image: Arc<AffImage> = get_image()?;

        let file_entry: AffFileEntry = get_root_file_entry(&test_image);

        let name: PathComponent = file_entry.get_name();
        assert_eq!(name, PathComponent::Root);

        let file_entry: AffFileEntry = get_layer_file_entry(&test_image)?;

        let name: PathComponent = file_entry.get_name();
        assert_eq!(name, PathComponent::from("aff1"));

        Ok(())
    }

    #[test]
    fn test_get_size() -> Result<(), ErrorTrace> {
        let test_image: Arc<AffImage> = get_image()?;

        let file_entry: AffFileEntry = get_root_file_entry(&test_image);

        let size: u64 = file_entry.get_size();
        assert_eq!(size, 0);

        let file_entry: AffFileEntry = get_layer_file_entry(&test_image)?;

        let size: u64 = file_entry.get_size();
        assert_eq!(size, 4194304);

        Ok(())
    }

    #[test]
    fn test_get_number_of_sub_file_entries() -> Result<(), ErrorTrace> {
        let test_image: Arc<AffImage> = get_image()?;

        let file_entry: AffFileEntry = get_root_file_entry(&test_image);

        let number_of_sub_file_entries: usize = file_entry.get_number_of_sub_file_entries();
        assert_eq!(number_of_sub_file_entries, 1);

        let file_entry: AffFileEntry = get_layer_file_entry(&test_image)?;

        let number_of_sub_file_entries: usize = file_entry.get_number_of_sub_file_entries();
        assert_eq!(number_of_sub_file_entries, 0);

        Ok(())
    }

    #[test]
    fn test_get_sub_file_entry_by_index() -> Result<(), ErrorTrace> {
        let test_image: Arc<AffImage> = get_image()?;

        let file_entry: AffFileEntry = get_root_file_entry(&test_image);

        let sub_file_entry: AffFileEntry = file_entry.get_sub_file_entry_by_index(0)?;

        let name: PathComponent = sub_file_entry.get_name();
        assert_eq!(name, PathComponent::from("aff1"));

        let result: Result<AffFileEntry, ErrorTrace> = file_entry.get_sub_file_entry_by_index(99);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_is_root_file_entry() -> Result<(), ErrorTrace> {
        let test_image: Arc<AffImage> = get_image()?;

        let file_entry: AffFileEntry = get_root_file_entry(&test_image);
        assert_eq!(file_entry.is_root_file_entry(), true);

        let file_entry: AffFileEntry = get_layer_file_entry(&test_image)?;
        assert_eq!(file_entry.is_root_file_entry(), false);

        Ok(())
    }
}

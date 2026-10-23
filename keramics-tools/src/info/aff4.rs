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
use std::path::PathBuf;

use keramics_core::ErrorTrace;
use keramics_formats::aff4::Aff4Image;
use keramics_formats::{FileResolverReference, OsFileResolver, PathComponent};

use crate::formatters::ByteSize;

/// Information about an Advanced Forensic Format 4 (AFF4) image.
struct Aff4ImageInfo<'a> {
    /// Image.
    image: &'a Aff4Image,
}

impl<'a> Aff4ImageInfo<'a> {
    /// Create new image information.
    pub fn new(image: &'a Aff4Image) -> Self {
        Self { image }
    }
}

impl<'a> fmt::Display for Aff4ImageInfo<'a> {
    /// Formats image information for display.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        writeln!(formatter, "Advanced Forensic Format 4 (AFF4) information:")?;

        writeln!(formatter, "    Name\t\t\t\t\t: {}", self.image.get_name())?;
        writeln!(
            formatter,
            "    Format version\t\t\t\t: {}.{}",
            self.image.get_version_major(),
            self.image.get_version_minor()
        )?;
        let tool: &str = self.image.get_tool();

        if !tool.is_empty() {
            writeln!(formatter, "    Tool\t\t\t\t\t: {}", tool)?;
        }
        let container_uri: &str = self.image.get_container_uri();

        if !container_uri.is_empty() {
            writeln!(formatter, "    Container URI\t\t\t\t: {}", container_uri)?;
        }
        let volume_uri: &str = self.image.get_volume_uri();

        if !volume_uri.is_empty() {
            writeln!(formatter, "    Volume URI\t\t\t\t\t: {}", volume_uri)?;
        }
        let image_entry_name: &str = self.image.get_image_entry_name();

        if !image_entry_name.is_empty() {
            writeln!(formatter, "    Image entry\t\t\t\t\t: {}", image_entry_name)?;
        }
        writeln!(formatter)?;

        writeln!(formatter, "    Media information:")?;
        let byte_size: ByteSize = ByteSize::new(self.image.get_media_size(), 1024);
        writeln!(formatter, "        Media size\t\t\t\t: {}", byte_size)?;
        writeln!(
            formatter,
            "        Bytes per sector\t\t\t: {}",
            self.image.get_bytes_per_sector()
        )?;

        writeln!(formatter)
    }
}

/// Information about Advanced Forensic Format 4 (AFF4).
pub struct Aff4Info {}

impl Aff4Info {
    /// Opens an image.
    fn open_image(path_buf: &PathBuf) -> Result<Aff4Image, ErrorTrace> {
        let mut base_path: PathBuf = path_buf.clone();
        base_path.pop();

        let file_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(OsFileResolver::new(base_path)));
        let file_name: PathComponent = match path_buf.file_name() {
            Some(file_name) => match file_name.to_str() {
                Some(file_name) => PathComponent::from(file_name),
                None => {
                    return Err(keramics_core::error_trace_new!("Unsupported file name"));
                }
            },
            None => {
                return Err(keramics_core::error_trace_new!("Missing file name"));
            }
        };
        let mut aff4_image: Aff4Image = Aff4Image::new();

        match aff4_image.open(&file_resolver, &file_name) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open AFF4 image");
                return Err(error);
            }
        }
        Ok(aff4_image)
    }

    /// Prints information about an image.
    pub fn print_image(path_buf: &PathBuf) -> Result<(), ErrorTrace> {
        let aff4_image: Aff4Image = match Self::open_image(path_buf) {
            Ok(aff4_image) => aff4_image,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open image");
                return Err(error);
            }
        };
        let image_information: Aff4ImageInfo = Aff4ImageInfo::new(&aff4_image);

        print!("{}", image_information);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::assert_lines_eq;

    fn get_image() -> Result<Aff4Image, ErrorTrace> {
        let path_buf: PathBuf = PathBuf::from("../test_data/aff4/ext2.aff4");
        Aff4Info::open_image(&path_buf)
    }

    #[test]
    fn test_image_information_fmt() -> Result<(), ErrorTrace> {
        let aff4_image: Aff4Image = get_image()?;

        let test_struct: Aff4ImageInfo = Aff4ImageInfo::new(&aff4_image);

        let expected_string: &str = concat!(
            "Advanced Forensic Format 4 (AFF4) information:\n",
            "    Name\t\t\t\t\t: ext2.aff4\n",
            "    Format version\t\t\t\t: 1.0\n",
            "    Tool\t\t\t\t\t: libaff4 2.0 pre-release\n",
            "    Container URI\t\t\t\t: aff4://00000000-0000-1003-8000-000000000004\n",
            "    Volume URI\t\t\t\t\t: aff4://00000000-0000-1003-8000-000000000004\n",
            "    Image entry\t\t\t\t\t: test_data/ext/ext2.raw\n",
            "\n",
            "    Media information:\n",
            "        Media size\t\t\t\t: 4.0 MiB (4194304 bytes)\n",
            "        Bytes per sector\t\t\t: 512\n",
            "\n"
        );
        let string: String = test_struct.to_string();
        assert_lines_eq!(string.as_str(), expected_string);

        Ok(())
    }

    // TODO: add tests for open_image
    // TODO: add tests for print_image
}

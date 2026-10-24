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
use keramics_core::formatters::format_as_string;
use keramics_formats::aff::AffImage;
use keramics_formats::{FileResolverReference, OsFileResolver, PathComponent};

use crate::formatters::ByteSize;

/// Information about an Advanced Forensic Format (AFF) image.
struct AffImageInfo<'a> {
    /// Image.
    image: &'a AffImage,
}

impl<'a> AffImageInfo<'a> {
    /// Create new image information.
    pub fn new(image: &'a AffImage) -> Self {
        Self { image }
    }

    /// Retrieves the AFF library version from the "afflib_version" segment.
    fn get_afflib_version(&self) -> Option<String> {
        let segment_data: Vec<u8> = self.image.get_segment_data("afflib_version").ok()?;

        let string: String = String::from_utf8_lossy(&segment_data).to_string();
        let string: String = string.trim_matches(['\n', '\r', '\0']).to_string();

        // The AFF library writes the version between quotes.
        let string: String = string.trim_matches('"').to_string();

        if string.is_empty() {
            None
        } else {
            Some(string)
        }
    }

    /// Retrieves a digest hash from a segment.
    fn get_segment_digest(&self, name: &str) -> Option<String> {
        let segment_data: Vec<u8> = self.image.get_segment_data(name).ok()?;

        if segment_data.is_empty() {
            None
        } else {
            Some(format_as_string(&segment_data))
        }
    }
}

impl<'a> fmt::Display for AffImageInfo<'a> {
    /// Formats image information for display.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        writeln!(formatter, "Advanced Forensic Format (AFF) information:")?;

        writeln!(formatter, "    Name\t\t\t\t\t: {}", self.image.get_name())?;
        if let Some(version) = self.get_afflib_version() {
            writeln!(formatter, "    AFF library version\t\t\t: {}", version)?;
        }

        writeln!(formatter)?;

        writeln!(formatter, "    Media information:")?;
        let byte_size: ByteSize = ByteSize::new(self.image.get_media_size(), 1024);
        writeln!(formatter, "        Media size\t\t\t\t: {}", byte_size)?;
        writeln!(
            formatter,
            "        Number of pages\t\t\t: {}",
            self.image.get_number_of_pages()
        )?;
        let page_byte_size: ByteSize = ByteSize::new(self.image.get_page_size(), 1024);
        writeln!(formatter, "        Page size\t\t\t\t: {}", page_byte_size)?;
        writeln!(
            formatter,
            "        Bytes per sector\t\t\t: {}",
            self.image.get_bytes_per_sector()
        )?;

        let number_of_bad_sectors: u64 = self.image.get_number_of_bad_sectors();

        if number_of_bad_sectors > 0 {
            writeln!(
                formatter,
                "        Number of bad sectors\t\t: {}",
                number_of_bad_sectors
            )?;
        }
        if let Some(md5_hash) = self.get_segment_digest("md5") {
            writeln!(formatter, "        MD5\t\t\t\t\t: {}", md5_hash)?;
        }
        if let Some(sha1_hash) = self.get_segment_digest("sha1") {
            writeln!(formatter, "        SHA1\t\t\t\t\t: {}", sha1_hash)?;
        }

        writeln!(formatter)
    }
}

/// Information about Advanced Forensic Format (AFF).
pub struct AffInfo {}

impl AffInfo {
    /// Opens an image.
    fn open_image(path_buf: &PathBuf) -> Result<AffImage, ErrorTrace> {
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
        let mut aff_image: AffImage = AffImage::new();

        match aff_image.open(&file_resolver, &file_name) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open AFF image");
                return Err(error);
            }
        }
        Ok(aff_image)
    }

    /// Prints information about an image.
    pub fn print_image(path_buf: &PathBuf) -> Result<(), ErrorTrace> {
        let aff_image: AffImage = match Self::open_image(path_buf) {
            Ok(aff_image) => aff_image,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to open image");
                return Err(error);
            }
        };
        let image_information: AffImageInfo = AffImageInfo::new(&aff_image);

        print!("{}", image_information);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::assert_lines_eq;

    fn get_image() -> Result<AffImage, ErrorTrace> {
        let path_buf: PathBuf = PathBuf::from("../test_data/aff/ext2.AFF");
        AffInfo::open_image(&path_buf)
    }

    #[test]
    fn test_image_information_fmt() -> Result<(), ErrorTrace> {
        let aff_image: AffImage = get_image()?;

        let test_struct: AffImageInfo = AffImageInfo::new(&aff_image);

        let expected_string: &str = concat!(
            "Advanced Forensic Format (AFF) information:\n",
            "    Name\t\t\t\t\t: ext2.AFF\n",
            "    AFF library version\t\t\t: 3.7.22\n",
            "\n",
            "    Media information:\n",
            "        Media size\t\t\t\t: 4.0 MiB (4194304 bytes)\n",
            "        Number of pages\t\t\t: 1\n",
            "        Page size\t\t\t\t: 16.0 MiB (16777216 bytes)\n",
            "        Bytes per sector\t\t\t: 512\n",
            "        MD5\t\t\t\t\t: b1760d0b35a512ef56970df4e6f8c5d6\n",
            "        SHA1\t\t\t\t\t: 68aa20d46e6160d952d228775390404938b5414a\n",
            "\n"
        );
        let string: String = test_struct.to_string();
        assert_lines_eq!(string.as_str(), expected_string);

        Ok(())
    }

    // TODO: add tests for open_image
    // TODO: add tests for print_image
}

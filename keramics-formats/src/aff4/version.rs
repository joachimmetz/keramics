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

use keramics_core::ErrorTrace;

use super::constants::*;

/// Advanced Forensic Format 4 (AFF4) file version, as stored in the "version.txt" file.
///
/// The file consists of variable length "name=value" lines and therefore has
/// no fixed layout; the fields are parsed in `read_data`.
pub struct Aff4Version {
    /// Major version.
    pub major: u16,

    /// Minor version.
    pub minor: u16,

    /// Tool that created the image.
    pub tool: String,
}

impl Aff4Version {
    /// Creates a new image version.
    pub fn new() -> Self {
        Self {
            major: 0,
            minor: 0,
            tool: String::new(),
        }
    }

    /// Determines if the image version is supported.
    pub fn is_supported(&self) -> bool {
        self.major == AFF4_SUPPORTED_MAJOR_VERSION
    }

    /// Reads the image version from a buffer.
    pub fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        let content: String = match String::from_utf8(data.to_vec()) {
            Ok(string) => string,
            Err(error) => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unsupported encoding of version data with error: {}",
                    error
                )));
            }
        };
        for line in content.lines() {
            let line: &str = line.trim();

            if line.is_empty() {
                continue;
            }
            let (name, value): (&str, &str) = match line.split_once('=') {
                Some((name, value)) => (name.trim(), value.trim()),
                None => {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Unsupported version data line: {}",
                        line
                    )));
                }
            };
            match name {
                "major" => {
                    self.major = match value.parse::<u16>() {
                        Ok(major) => major,
                        Err(error) => {
                            return Err(keramics_core::error_trace_new!(format!(
                                "Unsupported major version value: {} with error: {}",
                                value, error
                            )));
                        }
                    }
                }
                "minor" => {
                    self.minor = match value.parse::<u16>() {
                        Ok(minor) => minor,
                        Err(error) => {
                            return Err(keramics_core::error_trace_new!(format!(
                                "Unsupported minor version value: {} with error: {}",
                                value, error
                            )));
                        }
                    }
                }
                "tool" => self.tool = value.to_string(),
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_data() -> Result<(), ErrorTrace> {
        let data: &[u8] = b"major=1\nminor=0\ntool=libaff4 2.0 pre-release\n";

        let mut test_struct = Aff4Version::new();
        test_struct.read_data(data)?;

        assert_eq!(test_struct.major, 1);
        assert_eq!(test_struct.minor, 0);
        assert_eq!(test_struct.tool, "libaff4 2.0 pre-release");

        Ok(())
    }

    #[test]
    fn test_is_supported() {
        let mut test_struct = Aff4Version::new();
        let data: &[u8] = b"major=1\nminor=2\ntool=libaff4\n";
        test_struct.read_data(data).unwrap();
        assert!(test_struct.is_supported());

        let mut test_struct = Aff4Version::new();
        let data: &[u8] = b"major=2\nminor=0\ntool=libaff4\n";
        test_struct.read_data(data).unwrap();
        assert!(!test_struct.is_supported());
    }

    #[test]
    fn test_read_data_with_unsupported_encoding() {
        let data: &[u8] = &[0xff, 0xfe, 0xfd, 0xfc];

        let mut test_struct = Aff4Version::new();
        let result = test_struct.read_data(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_unsupported_line() {
        let data: &[u8] = b"major\n";

        let mut test_struct = Aff4Version::new();
        let result = test_struct.read_data(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_unsupported_major_version_value() {
        let data: &[u8] = b"major=not_a_number\n";

        let mut test_struct = Aff4Version::new();
        let result = test_struct.read_data(data);
        assert!(result.is_err());
    }
}

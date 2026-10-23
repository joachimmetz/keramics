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

/// Advanced Forensic Format 4 (AFF4) information container, as stored in the "information.turtle" file.
///
/// The file is a subset of the Turtle (RDF) syntax. The supported subset
/// consists of the `@base` directive, `@prefix` directives and simple image
/// statements with an IRI subject and an `aff4:Image` (or a derived image
/// type) predicate. The size of the file varies per volume and therefore has
/// no fixed layout; the fields are parsed in `read_data`.
pub struct Aff4Information {
    /// Volume URI as set by the `@base` directive.
    pub volume_uri: String,

    /// Names of the image entries stored in the volume.
    pub image_names: Vec<String>,
}

impl Aff4Information {
    /// Creates a new information container.
    pub fn new() -> Self {
        Self {
            volume_uri: String::new(),
            image_names: Vec::new(),
        }
    }

    /// Determines if the line is an image predicate.
    fn is_image_predicate(line: &str) -> bool {
        line.contains("a aff4:Image")
            || line.contains("a aff4:DiskImage")
            || line.contains("a aff4:ContiguousDiskImage")
    }

    /// Retrieves the first IRI in the data.
    fn extract_iri(data: &str) -> Option<&str> {
        let start: usize = data.find('<')?;
        let rest: &str = &data[start + 1..];
        let end: usize = rest.find('>')?;

        Some(&rest[..end])
    }

    /// Reads the information container from a buffer.
    pub fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        let content: String = match String::from_utf8(data.to_vec()) {
            Ok(string) => string,
            Err(error) => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unsupported encoding of information data with error: {}",
                    error
                )));
            }
        };
        let mut current_subject: Option<String> = None;

        for line in content.lines() {
            let line: &str = line.trim();

            if line.is_empty() {
                continue;
            }
            if line.starts_with("@") {
                if line.starts_with("@base") {
                    let rest: &str = line.trim_start_matches("@base").trim();
                    let base: &str = match Self::extract_iri(rest) {
                        Some(base) => base,
                        None => {
                            return Err(keramics_core::error_trace_new!(format!(
                                "Unsupported @base directive: {}",
                                line
                            )));
                        }
                    };
                    self.volume_uri = base.to_string();
                }
                // @prefix and other directives are not used by this support.
                current_subject = None;
                continue;
            }
            if Self::is_image_predicate(line) {
                let candidate: Option<&str> = match Self::extract_iri(line) {
                    Some(iri) => Some(iri),
                    None => current_subject.as_deref(),
                };
                if let Some(candidate) = candidate {
                    if candidate.contains("://") {
                        continue;
                    }
                    // Image entry names are volume relative URLs and have their
                    // leading slash removed when stored as Zip entry names.
                    let name: &str = candidate.strip_prefix('/').unwrap_or(candidate);
                    if !name.is_empty() {
                        let image_name: String = name.to_string();

                        if !self.image_names.contains(&image_name) {
                            self.image_names.push(image_name);
                        }
                    }
                }
                continue;
            }
            if line.starts_with("<") && line.ends_with('>') {
                current_subject = Self::extract_iri(line).map(|subject| subject.to_string());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_data() -> Vec<u8> {
        b"@base <aff4://00000000-0000-1003-8000-000000000004> .\n@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n@prefix aff4: <http://aff4.org/Schema#> .\n@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n\n</test_data/ext/ext2.raw>\n    aff4:stored <> ;\n    a aff4:Image .\n"
            .to_vec()
    }

    #[test]
    fn test_read_data() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data();

        let mut test_struct = Aff4Information::new();
        test_struct.read_data(&test_data)?;

        assert_eq!(
            test_struct.volume_uri,
            "aff4://00000000-0000-1003-8000-000000000004"
        );
        assert_eq!(
            test_struct.image_names,
            vec!["test_data/ext/ext2.raw".to_string()]
        );
        Ok(())
    }

    #[test]
    fn test_read_data_with_single_line_statement() -> Result<(), ErrorTrace> {
        let data: &[u8] = b"@base <aff4://00000000-0000-1003-8000-000000000004> .\n<test.ext2.raw> a aff4:DiskImage .\n";

        let mut test_struct = Aff4Information::new();
        test_struct.read_data(data)?;

        assert_eq!(test_struct.image_names, vec!["test.ext2.raw".to_string()]);

        Ok(())
    }

    #[test]
    fn test_read_data_with_multiple_images() -> Result<(), ErrorTrace> {
        let data: &[u8] = b"@base <aff4://00000000-0000-1003-8000-000000000004> .\n</test1.raw> a aff4:Image .\n</test2.raw> a aff4:DiskImage .\n</test1.raw> a aff4:Image .\n";

        let mut test_struct = Aff4Information::new();
        test_struct.read_data(data)?;

        assert_eq!(
            test_struct.image_names,
            vec!["test1.raw".to_string(), "test2.raw".to_string()]
        );

        Ok(())
    }

    #[test]
    fn test_read_data_with_image_iri_subject() -> Result<(), ErrorTrace> {
        // Subjects with an absolute IRI are not entry names.
        let data: &[u8] = b"@base <aff4://00000000-0000-1003-8000-000000000004> .\n<aff4://00000000-0000-1003-8000-000000000004/test.raw> a aff4:Image .\n";

        let mut test_struct = Aff4Information::new();
        test_struct.read_data(data)?;

        assert!(test_struct.image_names.is_empty());

        Ok(())
    }

    #[test]
    fn test_read_data_with_missing_base() -> Result<(), ErrorTrace> {
        let data: &[u8] = b"@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n";

        let mut test_struct = Aff4Information::new();
        test_struct.read_data(data)?;

        assert!(test_struct.volume_uri.is_empty());
        assert!(test_struct.image_names.is_empty());

        Ok(())
    }

    #[test]
    fn test_read_data_with_unsupported_encoding() {
        let data: &[u8] = &[0xff, 0xfe, 0xfd, 0xfc];

        let mut test_struct = Aff4Information::new();
        let result = test_struct.read_data(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_data_with_unsupported_base_directive() {
        let data: &[u8] = b"@base not_an_iri .\n";

        let mut test_struct = Aff4Information::new();
        let result = test_struct.read_data(data);
        assert!(result.is_err());
    }
}

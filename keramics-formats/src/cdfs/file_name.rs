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

use keramics_types::ByteString;

use crate::path_component::PathComponent;

/// A CDFS file entry name.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CdFsFileName {
    name: ByteString,
}

impl CdFsFileName {
    /// Creates a new CDFS file entry name.
    pub fn new(name: ByteString) -> Self {
        Self { name }
    }

    /// Determines if a sub file entry name matches the entry name.
    pub fn compare(&self, sub_file_entry_name: &PathComponent) -> bool {
        match sub_file_entry_name {
            PathComponent::Root | PathComponent::Parent | PathComponent::Current => false,
            _ => {
                let sub_file_entry_name_string: String =
                    sub_file_entry_name.to_string().to_ascii_lowercase();
                let entry_name_string: String = self.name.to_string().to_ascii_lowercase();

                // CDFS entry names consist of a base name, an extension part and
                // a version number, e.g. EMPTYFILE.;1 or EMPTYFILE.TXT;1.
                let entry_name_stem: &str = entry_name_string
                    .split(';')
                    .next()
                    .unwrap_or(entry_name_string.as_str());
                let entry_name_stem: &str =
                    entry_name_stem.strip_suffix('.').unwrap_or(entry_name_stem);

                sub_file_entry_name_string == entry_name_stem
                    || sub_file_entry_name_string == entry_name_string
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use keramics_encodings::CharacterEncoding;

    use crate::path_component::PathComponent;

    #[test]
    fn test_compare() {
        let entry_name: CdFsFileName = CdFsFileName::new(ByteString {
            encoding: CharacterEncoding::Ascii,
            elements: b"EMPTYFILE.;1".to_vec(),
        });

        let result: bool = entry_name.compare(&PathComponent::from("EMPTYFILE"));
        assert!(result);

        let result: bool = entry_name.compare(&PathComponent::from("EMPTYFILE.;1"));
        assert!(result);

        let result: bool = entry_name.compare(&PathComponent::from("emptyfile"));
        assert!(result);

        let result: bool = entry_name.compare(&PathComponent::Root);
        assert!(!result);

        let result: bool = entry_name.compare(&PathComponent::Parent);
        assert!(!result);

        let result: bool = entry_name.compare(&PathComponent::Current);
        assert!(!result);

        let result: bool = entry_name.compare(&PathComponent::from("NONEXISTENT"));
        assert!(!result);
    }
}

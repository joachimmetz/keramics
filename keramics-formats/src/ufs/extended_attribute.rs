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

use keramics_core::DataStreamReference;
use keramics_types::ByteString;

/// Unix File System (UFS) extended attribute.
pub struct UfsExtendedAttribute {
    /// Attribute name.
    name: ByteString,

    /// Attribute data stream.
    data_stream: DataStreamReference,
}

impl UfsExtendedAttribute {
    /// Creates a new extended attribute.
    pub(super) fn new(name: &ByteString, data_stream: DataStreamReference) -> Self {
        Self {
            name: name.clone(),
            data_stream,
        }
    }

    /// Retrieves the default data stream.
    pub fn get_data_stream(&self) -> &DataStreamReference {
        &self.data_stream
    }

    /// Retrieves the name.
    pub fn get_name(&self) -> &ByteString {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use keramics_core::open_fake_data_stream;

    #[test]
    fn test_get_data_stream_and_name() {
        let test_name: ByteString = ByteString::from("user.test");
        let test_data: &[u8] = b"value";
        let data_stream: DataStreamReference = open_fake_data_stream(test_data);
        let test_struct: UfsExtendedAttribute = UfsExtendedAttribute::new(&test_name, data_stream);

        _ = test_struct.get_data_stream();

        let name: &ByteString = test_struct.get_name();
        assert_eq!(name, &test_name);
    }
}

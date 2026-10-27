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

/// Advanced Forensic Format 4 (AFF4) information statement node.
///
/// A node represents a single set of Turtle (RDF) statements with a common
/// subject, for example the image, map or image stream node of a standard
/// AFF4 volume. The subject may be an absolute IRI (for example
/// `<aff4://00000000-0000-1003-8000-000000000001>`) or the default subject
/// `:` that refers to the volume itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Aff4Node {
    /// Subject of the node (`:` for the volume subject).
    pub subject: String,

    /// Types of the node as declared by the `a` (i.e. `rdf:type`) predicate,
    /// stored as prefix local names (for example `Image`).
    pub types: Vec<String>,

    /// Non-type properties of the node in order of appearance.
    pub properties: Vec<Aff4NodeProperty>,
}

/// Property values of a single statement node property.
#[derive(Clone, Debug, PartialEq)]
pub struct Aff4NodeProperty {
    /// Local name of the property (for example `chunkSize`).
    pub name: String,

    /// Values of the property in order of appearance.
    ///
    /// IRIs are stored without their enclosing angle brackets, string
    /// literals are stored without their enclosing quotes and datatype
    /// annotations (for example `^^xsd:long`) are removed.
    pub values: Vec<String>,
}

impl Aff4NodeProperty {
    /// Creates a new property.
    fn new(name: String, values: Vec<String>) -> Self {
        Self { name, values }
    }

    /// Retrieves the first value of the property.
    pub fn get_value(&self) -> Option<&str> {
        self.values.first().map(String::as_str)
    }

    /// Retrieves the value of the property with the specified index.
    pub fn get_value_by_index(&self, index: usize) -> Option<&str> {
        self.values.get(index).map(String::as_str)
    }
}

impl Aff4Node {
    /// Determines if the node has the type with the specified prefix local name (for example `Image`).
    pub fn is_type(&self, type_name: &str) -> bool {
        self.types.iter().any(|node_type| node_type == type_name)
    }

    /// Determines if the subject of the node is an absolute IRI.
    pub fn has_iri_subject(&self) -> bool {
        self.subject.contains("://")
    }

    /// Retrieves the property with the specified local name.
    pub fn get_property(&self, name: &str) -> Option<&Aff4NodeProperty> {
        self.properties
            .iter()
            .find(|property| property.name == name)
    }

    /// Retrieves the values of the property with the specified local name.
    pub fn get_property_values(&self, name: &str) -> Option<&[String]> {
        match self.get_property(name) {
            Some(property) => Some(property.values.as_slice()),
            None => None,
        }
    }

    /// Retrieves the first value of the property with the specified local name.
    pub fn get_property_value(&self, name: &str) -> Option<&str> {
        self.get_property(name)
            .and_then(|property| property.get_value())
    }

    /// Retrieves an integer property value (for example `aff4:size`).
    pub fn get_u64_property(&self, name: &str) -> Option<u64> {
        self.get_property_value(name)
            .map(|value| value.trim())
            .and_then(|value| value.parse::<u64>().ok())
    }
}

/// Advanced Forensic Format 4 (AFF4) information container, as stored in the "information.turtle" file.
///
/// The file is a subset of the Turtle (RDF) syntax. The supported subset
/// consists of the `@base` directive, `@prefix` directives (including the
/// `@prefix :` volume directive used by standard AFF4 volumes) and statement
/// nodes with an IRI (or volume) subject, `a` type predicates and property
/// predicates with IRI, prefixed IRI, literal or integer values. The size of
/// the file varies per volume and therefore has no fixed layout; the fields
/// are parsed in `read_data`.
#[derive(Clone)]
pub struct Aff4Information {
    /// Volume URI as set by the `@base` or `@prefix :` directive.
    pub volume_uri: String,

    /// Names of the image entries stored in the volume.
    pub image_names: Vec<String>,

    /// Statement nodes of the information container.
    pub nodes: Vec<Aff4Node>,
}

impl Aff4Information {
    /// Creates a new information container.
    pub fn new() -> Self {
        Self {
            volume_uri: String::new(),
            image_names: Vec::new(),
            nodes: Vec::new(),
        }
    }

    /// Determines if the information container describes a standard AFF4 (for example AFF4 Standard v1.0a) volume.
    ///
    /// A standard volume is indicated by the presence of `Map` or
    /// `ImageStream` nodes or by an `Image` node with an absolute IRI
    /// subject.
    pub fn is_standard(&self) -> bool {
        self.nodes
            .iter()
            .any(|node| node.is_type("Map") || node.is_type("ImageStream"))
            || self
                .nodes
                .iter()
                .any(|node| node.is_type("Image") && node.has_iri_subject())
    }

    /// Retrieves the first IRI in the data.
    fn extract_iri(data: &str) -> Option<&str> {
        let start: usize = data.find('<')?;
        let rest: &str = &data[start + 1..];
        let end: usize = rest.find('>')?;

        Some(&rest[..end])
    }

    /// Tokenizes the statement body into parse tokens.
    ///
    /// IRIs are returned with their enclosing angle brackets, string literals
    /// without their enclosing quotes and datatype annotations, and the
    /// statement punctuation (`;`, `.`, `,`, `(`, `)`) as single tokens.
    fn tokenize(data: &str) -> Vec<String> {
        let mut tokens: Vec<String> = Vec::new();
        let chars: Vec<char> = data.chars().collect();
        let mut index: usize = 0;

        while index < chars.len() {
            let ch: char = chars[index];

            if ch.is_whitespace() {
                index += 1;
                continue;
            }
            match ch {
                '<' => {
                    let start: usize = index;
                    index += 1;
                    while index < chars.len() && chars[index] != '>' {
                        index += 1;
                    }
                    if index >= chars.len() {
                        return tokens;
                    }
                    let iri: String = chars[(start + 1)..index].iter().collect();
                    index += 1;
                    tokens.push(format!("<{}>", iri));
                }
                '"' => {
                    let start: usize = index;
                    index += 1;
                    while index < chars.len() && chars[index] != '"' {
                        index += 1;
                    }
                    if index >= chars.len() {
                        return tokens;
                    }
                    let value: String = chars[start + 1..index].iter().collect();
                    index += 1;
                    // Remove an attached datatype annotation such as
                    // `^^xsd:long` or `^^aff4:SHA1`.
                    if index + 1 < chars.len() && chars[index] == '^' && chars[index + 1] == '^' {
                        index += 2;
                        while index < chars.len()
                            && (chars[index].is_ascii_alphanumeric()
                                || chars[index] == '_'
                                || chars[index] == '-'
                                || chars[index] == ':'
                                || chars[index] == '.')
                        {
                            index += 1;
                        }
                    }
                    tokens.push(value);
                }
                ';' | '.' | ',' | '(' | ')' => {
                    tokens.push(ch.to_string());
                    index += 1;
                }
                _ => {
                    let mut token: String = String::new();
                    while index < chars.len()
                        && !chars[index].is_whitespace()
                        && !matches!(chars[index], '<' | '"' | ';' | '.' | ',' | '(' | ')')
                    {
                        token.push(chars[index]);
                        index += 1;
                    }
                    if token.is_empty() {
                        index += 1;
                    } else {
                        tokens.push(token);
                    }
                }
            }
        }
        tokens
    }

    /// Retrieves the prefix local name of a predicate token.
    fn predicate_local_name(predicate: &str) -> String {
        if predicate.starts_with('<') && predicate.ends_with('>') {
            let iri: &str = &predicate[1..predicate.len() - 1];
            if let Some(hash_pos) = iri.rfind('#') {
                return iri[hash_pos + 1..].to_string();
            }
            if let Some(slash_pos) = iri.rfind('/') {
                return iri[slash_pos + 1..].to_string();
            }
            return iri.to_string();
        }
        if let Some(colon_pos) = predicate.rfind(':') {
            return predicate[colon_pos + 1..].to_string();
        }
        predicate.to_string()
    }

    /// Retrieves the value of an object token.
    fn object_value(object: &str) -> String {
        if object.starts_with('<') && object.ends_with('>') {
            return object[1..object.len() - 1].to_string();
        }
        object.to_string()
    }

    /// Parses the statement nodes from a token stream.
    fn parse_statements(tokens: &[String]) -> Vec<Aff4Node> {
        let mut nodes: Vec<Aff4Node> = Vec::new();
        let mut index: usize = 0;

        while index < tokens.len() {
            let subject: String = Self::object_value(tokens[index].as_str());
            index += 1;

            let mut node: Aff4Node = Aff4Node {
                subject,
                types: Vec::new(),
                properties: Vec::new(),
            };
            loop {
                if index >= tokens.len() {
                    break;
                }
                match tokens[index].as_str() {
                    "." => {
                        index += 1;
                        break;
                    }
                    ";" => {
                        index += 1;
                        continue;
                    }
                    _ => {}
                }
                let predicate: String = tokens[index].clone();
                let name: String = Self::predicate_local_name(predicate.as_str());
                index += 1;

                let mut values: Vec<String> = Vec::new();
                loop {
                    if index >= tokens.len() {
                        break;
                    }
                    match tokens[index].as_str() {
                        "," => {
                            index += 1;
                            continue;
                        }
                        // The statement separator (`;`) and the end-of-
                        // statement (`.`) are handled by the outer loop.
                        ";" | "." => break,
                        "(" => {
                            index += 1;
                            while index < tokens.len() && tokens[index].as_str() != ")" {
                                if tokens[index].as_str() == "," {
                                    index += 1;
                                    continue;
                                }
                                values.push(Self::object_value(tokens[index].as_str()));
                                index += 1;
                            }
                            if index < tokens.len() {
                                index += 1;
                            }
                            break;
                        }
                        _ => {
                            values.push(Self::object_value(tokens[index].as_str()));
                            index += 1;
                        }
                    }
                }
                if values.is_empty() {
                    continue;
                }
                // `a` is the Turtle alias of the RDF type predicate.
                if name == "type" || predicate == "a" {
                    // Types are stored as prefix local names (for example
                    // `ImageStream` for `aff4:ImageStream`).
                    for value in values.drain(..) {
                        node.types.push(Self::predicate_local_name(value.as_str()));
                    }
                } else {
                    node.properties.push(Aff4NodeProperty::new(name, values));
                }
            }
            nodes.push(node);
        }
        nodes
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
        let mut body: Vec<String> = Vec::new();

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
                } else if line.starts_with("@prefix") {
                    // The `@prefix :` directive sets the volume URI of
                    // standard AFF4 volumes; other prefixes are not used by
                    // this support.
                    let rest: &str = line.trim_start_matches("@prefix").trim();
                    let parts: Vec<&str> = rest.split_whitespace().collect();
                    if parts.len() >= 2 && parts[0] == ":" {
                        if let Some(volume_iri) = Self::extract_iri(parts[1]) {
                            if self.volume_uri.is_empty() {
                                self.volume_uri = volume_iri.to_string();
                            }
                        }
                    }
                }
                continue;
            }
            body.push(line.to_string());
        }
        let nodes: Vec<Aff4Node> = Self::parse_statements(&Self::tokenize(&body.join("\n")));

        // Derive the legacy image entry names from the `Image` typed nodes
        // with a volume relative subject. Their leading slash is removed when
        // stored as Zip entry names.
        let mut image_names: Vec<String> = Vec::new();
        for node in nodes.iter() {
            let is_image: bool = node.is_type("Image")
                || node.is_type("DiskImage")
                || node.is_type("ContiguousDiskImage");
            if is_image && !node.has_iri_subject() && node.subject != ":" {
                let name: String = node.subject.trim_start_matches('/').to_string();
                if !name.is_empty() && !image_names.contains(&name) {
                    image_names.push(name);
                }
            }
        }
        self.nodes = nodes;
        self.image_names = image_names;

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

    fn get_standard_data() -> Vec<u8> {
        b"@prefix :      <aff4://00000000-0000-1003-8000-000000000004> .\n@prefix rdf:   <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n@prefix xsd:   <http://www.w3.org/2001/XMLSchema#> .\n@prefix aff4:  <http://aff4.org/Schema#> .\n\n<aff4://00000000-0000-1003-8000-000000000001>\n        a                          aff4:ImageStream ;\n        aff4:chunkSize             \"32768\"^^xsd:int ;\n        aff4:chunksInSegment       \"2048\"^^xsd:int ;\n        aff4:compressionMethod     <http://code.google.com/p/snappy/> ;\n        aff4:hash                  \"fbac22cca549310bc5df03b7560afcf490995fbb\"^^aff4:SHA1 , \"d5825dc1152a42958c8219ff11ed01a3\"^^aff4:MD5 ;\n        aff4:size                  \"3964928\"^^xsd:long ;\n        aff4:stored                : ;\n        aff4:target                <aff4://00000000-0000-1003-8000-000000000002> ;\n        aff4:version               \"1\"^^xsd:int .\n\n:       a                  aff4:ZipVolume ;\n        aff4:contains      <aff4://00000000-0000-1003-8000-000000000001> , <aff4://00000000-0000-1003-8000-000000000002> ;\n        aff4:stored        \"Base-Linear.aff4\" .\n\n<aff4://00000000-0000-1003-8000-000000000003>\n        a                            aff4:ContiguousImage , aff4:DiskImage , aff4:Image ;\n        aff4:acquisitionCompletionState\n                \"Completed Normally\" ;\n        aff4:blockSize               \"512\"^^xsd:int ;\n        aff4:dataStream              <aff4://00000000-0000-1003-8000-000000000002> ;\n        aff4:sectorCount             \"524288\"^^xsd:long ;\n        aff4:size                    \"268435456\"^^xsd:long ;\n        aff4:stored                  : .\n\n<aff4://00000000-0000-1003-8000-000000000002>\n        a                         aff4:Map ;\n        aff4:dependentStream      <aff4://00000000-0000-1003-8000-000000000001> ;\n        aff4:mapGapDefaultStream  aff4:Zero ;\n        aff4:size                 \"268435456\"^^xsd:long ;\n        aff4:stored               : .\n"
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

    #[test]
    fn test_read_data_with_standard_volume() -> Result<(), ErrorTrace> {
        let data: Vec<u8> = get_standard_data();

        let mut test_struct = Aff4Information::new();
        test_struct.read_data(&data)?;

        assert_eq!(
            test_struct.volume_uri,
            "aff4://00000000-0000-1003-8000-000000000004"
        );
        assert!(test_struct.image_names.is_empty());
        assert!(test_struct.is_standard());

        let stream_node: Option<&Aff4Node> = test_struct
            .nodes
            .iter()
            .find(|node| node.subject == "aff4://00000000-0000-1003-8000-000000000001");
        assert!(stream_node.is_some());
        let stream_node: &Aff4Node = stream_node.unwrap();
        assert!(stream_node.is_type("ImageStream"));
        assert_eq!(stream_node.get_u64_property("chunkSize"), Some(32768));
        assert_eq!(
            stream_node.get_property_value("compressionMethod"),
            Some("http://code.google.com/p/snappy/")
        );
        assert_eq!(stream_node.get_u64_property("size"), Some(3964928));
        assert_eq!(
            stream_node.get_property_value("hash"),
            Some("fbac22cca549310bc5df03b7560afcf490995fbb")
        );
        assert_eq!(stream_node.get_property_values("hash").unwrap().len(), 2);

        let map_node: Option<&Aff4Node> = test_struct
            .nodes
            .iter()
            .find(|node| node.subject == "aff4://00000000-0000-1003-8000-000000000002");
        assert!(map_node.is_some());
        let map_node: &Aff4Node = map_node.unwrap();
        assert!(map_node.is_type("Map"));
        assert_eq!(
            map_node.get_property_values("dependentStream").unwrap(),
            &vec!["aff4://00000000-0000-1003-8000-000000000001".to_string()]
        );
        assert_eq!(
            map_node.get_property_value("mapGapDefaultStream"),
            Some("aff4:Zero")
        );

        let image_node: Option<&Aff4Node> = test_struct
            .nodes
            .iter()
            .find(|node| node.subject == "aff4://00000000-0000-1003-8000-000000000003");
        assert!(image_node.is_some());
        let image_node: &Aff4Node = image_node.unwrap();
        assert!(image_node.is_type("Image"));
        assert!(image_node.is_type("ContiguousImage"));
        assert!(image_node.is_type("DiskImage"));
        assert_eq!(
            image_node.get_property_value("dataStream"),
            Some("aff4://00000000-0000-1003-8000-000000000002")
        );
        assert_eq!(image_node.get_u64_property("size"), Some(268435456));
        assert_eq!(
            image_node.get_property_value("acquisitionCompletionState"),
            Some("Completed Normally")
        );

        let volume_node: Option<&Aff4Node> =
            test_struct.nodes.iter().find(|node| node.subject == ":");
        assert!(volume_node.is_some());
        let volume_node: &Aff4Node = volume_node.unwrap();
        assert!(volume_node.is_type("ZipVolume"));

        Ok(())
    }

    #[test]
    fn test_read_data_with_split_volume() -> Result<(), ErrorTrace> {
        let data: &[u8] = b"@base <aff4://00000000-0000-1003-8000-000000000004> .\n@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n@prefix aff4: <http://aff4.org/Schema#> .\n@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n\n</file:/tmp/keramics_aff4_split.raw>\n    aff4:size 0 ;\n    aff4:stored <> ;\n    a aff4:ImageStream .\n";

        let mut test_struct = Aff4Information::new();
        test_struct.read_data(data)?;

        // An `ImageStream` typed node is not included in the legacy image
        // entry names.
        assert!(test_struct.image_names.is_empty());
        assert!(test_struct.is_standard());

        Ok(())
    }

    #[test]
    fn test_is_standard() -> Result<(), ErrorTrace> {
        let test_data: Vec<u8> = get_test_data();

        let mut test_struct = Aff4Information::new();
        test_struct.read_data(&test_data)?;

        assert!(!test_struct.is_standard());

        let standard_data: Vec<u8> = get_standard_data();
        let mut standard_test_struct = Aff4Information::new();
        standard_test_struct.read_data(&standard_data)?;

        assert!(standard_test_struct.is_standard());

        Ok(())
    }
}

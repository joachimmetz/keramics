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

use std::cmp::min;

use keramics_compression::{DeflateContext, SnappyContext, ZlibContext};
use keramics_core::ErrorTrace;

use crate::file_resolver::FileResolverReference;
use crate::lru_cache::LruCache;
use crate::path_component::PathComponent;
use crate::traits::BlockReader;
use crate::zip::{ZipEntry, ZipFile};

use super::constants::AFF4_DEFAULT_BYTES_PER_SECTOR;
use super::file::Aff4File;
use super::information::{Aff4Information, Aff4Node};

/// Size of an AFF4 Standard map entry in bytes.
const AFF4_STANDARD_MAP_ENTRY_SIZE: usize = 28;

/// Size of an AFF4 Standard bevy index entry in bytes.
const AFF4_STANDARD_BEVY_INDEX_ENTRY_SIZE: usize = 12;

/// Maximum number of decoded chunks kept in the chunk cache.
const AFF4_STANDARD_CHUNK_CACHE_SIZE: usize = 64;

/// Maximum number of bevy segments probed for streams without a declared size.
const AFF4_STANDARD_MAX_SEGMENTS: usize = 16384;

/// Default number of chunks per bevy segment.
const AFF4_STANDARD_DEFAULT_CHUNKS_IN_SEGMENT: usize = 1024;

/// Compression method of the chunks of an AFF4 Standard data stream.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CompressionMethod {
    /// Stored (uncompressed) chunks.
    Stored,

    /// Snappy compressed chunks.
    Snappy,

    /// Zlib (RFC 1950) compressed chunks.
    Zlib,

    /// Deflate (RFC 1951) compressed chunks.
    Deflate,
}

impl CompressionMethod {
    /// Determines the compression method from a compression method IRI.
    fn from_iri(iri: &str) -> CompressionMethod {
        let lowercased: String = iri.to_ascii_lowercase();

        if lowercased.contains("snappy") {
            CompressionMethod::Snappy
        } else if lowercased.contains("rfc1950") || lowercased.contains("zlib") {
            CompressionMethod::Zlib
        } else if lowercased.contains("rfc1951") || lowercased.contains("deflate") {
            CompressionMethod::Deflate
        } else {
            CompressionMethod::Stored
        }
    }
}

/// Target of an AFF4 Standard map entry.
#[derive(Clone, Copy, Debug, PartialEq)]
enum MapTarget {
    /// Data stream with the specified index.
    Stream(usize),

    /// Zero filled gap.
    Gap,

    /// Symbolic stream fill byte.
    Symbolic(u8),
}

/// Data of an AFF4 Standard bevy index entry.
#[derive(Clone, Debug)]
struct BevyIndexEntry {
    /// Offset of the chunk in the bevy data entry.
    bevvy_offset: u64,

    /// Stored (possibly compressed) size of the chunk in bytes.
    stored_size: u32,
}

/// Resolved bevy segment of an AFF4 Standard data stream.
#[derive(Clone, Debug)]
struct ResolvedSegment {
    /// Index of the first chunk in the segment.
    first_chunk_index: usize,

    /// Name of the bevy data entry.
    data_entry_name: String,

    /// Bevy index entries in order of the chunks of the segment.
    index_entries: Vec<BevyIndexEntry>,
}

/// Resolved bevy segment of an AFF4 Standard data stream.
///
/// The segment data is read from the volume that stores the stream data.
#[derive(Clone)]
struct ResolvedStream {
    /// Identity of the stream.
    name: String,

    /// Size of a single chunk in bytes.
    chunk_size: usize,

    /// Number of chunks per bevy segment.
    chunks_in_segment: usize,

    /// Compression method of the chunks.
    compression_method: CompressionMethod,

    /// Total size of the stream data in bytes (0 if not declared).
    size: u64,

    /// Zip file of the volume that stores the stream data.
    zip_file: ZipFile,

    /// Prefix of the stream data entries in the volume.
    entry_prefix: String,

    /// Resolved bevy segments.
    segments: Vec<ResolvedSegment>,
}

impl ResolvedStream {
    /// Retrieves the number of chunks of the stream.
    fn get_number_of_chunks(&self) -> usize {
        self.segments
            .iter()
            .map(|segment| segment.index_entries.len())
            .sum()
    }

    /// Retrieves the bevy segment that contains the chunk with the specified index.
    fn get_segment_for_chunk(
        &self,
        chunk_index: usize,
    ) -> Option<(usize, &ResolvedSegment, &BevyIndexEntry)> {
        for (segment_index, segment) in self.segments.iter().enumerate() {
            if chunk_index < segment.first_chunk_index {
                continue;
            }
            let local_index: usize = chunk_index - segment.first_chunk_index;
            if let Some(index_entry) = segment.index_entries.get(local_index) {
                return Some((segment_index, segment, index_entry));
            }
        }
        None
    }
}

/// Resolved AFF4 Standard map entry.
#[derive(Clone, Debug)]
struct ResolvedMapEntry {
    /// Offset in the media data the entry maps.
    mapped_offset: u64,

    /// Size of the mapped region in bytes.
    length: u64,

    /// Offset in the target data stream the region is read from.
    target_offset: u64,

    /// Target of the entry.
    target: MapTarget,
}

/// Resolved AFF4 Standard map.
#[derive(Clone, Debug)]
struct ResolvedMap {
    /// Map entries in order of their mapped offset.
    entries: Vec<ResolvedMapEntry>,

    /// Default target of the map used to fill gaps.
    gap_default: MapTarget,
}

/// AFF4 Standard data volume.
#[derive(Clone)]
struct DataVolume {
    /// File name of the volume container.
    file_name: String,

    /// Container URI of the volume.
    container_uri: String,

    /// Volume URI of the volume.
    volume_uri: String,

    /// Information container of the volume.
    information: Aff4Information,

    /// Zip file of the volume container.
    zip_file: ZipFile,
}

impl DataVolume {
    /// Opens the data volume with the specified file name.
    fn open(
        file_resolver: &FileResolverReference,
        file_name: &PathComponent,
    ) -> Result<DataVolume, ErrorTrace> {
        let mut aff4_file: Aff4File = Aff4File::new();

        match aff4_file.open(file_resolver, file_name) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!("Unable to open AFF4 volume: {}", file_name)
                );
                return Err(error);
            }
        }
        Ok(DataVolume {
            file_name: file_name.to_string(),
            container_uri: aff4_file.get_container_description().to_string(),
            volume_uri: aff4_file.get_volume_uri().to_string(),
            information: aff4_file.take_information(),
            zip_file: aff4_file.take_zip_file(),
        })
    }

    /// Determines if the volume contains an entry with the specified name.
    fn has_entry(&self, file_name: &str) -> bool {
        self.zip_file.get_entry(file_name).is_some()
    }

    /// Reads an entry with the specified name.
    fn read_entry_data(&self, file_name: &str) -> Result<Vec<u8>, ErrorTrace> {
        let entry: ZipEntry = match self.zip_file.get_entry(file_name) {
            Some(entry) => entry.clone(),
            None => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Missing entry: {} in volume: {}",
                    file_name, self.file_name
                )));
            }
        };
        let mut data: Vec<u8> = vec![0; entry.uncompressed_size as usize];
        let mut zip_file: ZipFile = self.zip_file.clone();

        match zip_file.read_entry_data(&entry, &mut data) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!(
                        "Unable to read entry data of entry: {} in volume: {}",
                        file_name, self.file_name
                    )
                );
                return Err(error);
            }
        }
        Ok(data)
    }

    /// Retrieves the size of an entry with the specified name.
    fn get_entry_size(&self, file_name: &str) -> Option<u64> {
        self.zip_file
            .get_entry(file_name)
            .map(|entry| entry.uncompressed_size as u64)
    }

    /// Resolves the prefix of the entries stored in the volume for a data
    /// stream or map node name.
    ///
    /// The entry names are percent encoded variants of the node name; the
    /// supported encodings are the fully percent encoded name (for example
    /// `aff4%3A%2F%2F<uuid>`) and the percent encoded name with path
    /// separators preserved (for example `file%3A/tmp/file.raw`).
    fn get_entry_prefix_for_node_name(
        &self,
        node_name: &str,
        suffix: &str,
        index_suffix: Option<&str>,
    ) -> Option<String> {
        for prefix_candidate in Self::entry_prefix_candidates(node_name) {
            let entry_name: String = format!("{}/{}", prefix_candidate, suffix);

            if !self.has_entry(entry_name.as_str()) {
                continue;
            }
            if let Some(index_suffix) = index_suffix {
                let index_entry_name: String = format!("{}{}", entry_name, index_suffix);
                if !self.has_entry(index_entry_name.as_str()) {
                    continue;
                }
            }
            return Some(prefix_candidate);
        }
        None
    }

    /// Retrieves the candidate entry prefixes for a node name.
    fn entry_prefix_candidates(node_name: &str) -> Vec<String> {
        let mut candidates: Vec<String> = Vec::new();
        let mut candidate: String = node_name.to_string();

        candidates.push(candidate.clone());
        candidate = Self::percent_encode(node_name, false);
        if !candidates.contains(&candidate) {
            candidates.push(candidate.clone());
        }
        candidate = Self::percent_encode(node_name, true);
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }

        candidates
    }

    /// Percent encodes a value.
    ///
    /// If `preserve_path_separators` is set the `/` and `\` characters are
    /// not encoded.
    fn percent_encode(value: &str, preserve_path_separators: bool) -> String {
        let mut encoded: String = String::new();

        for byte in value.bytes() {
            if byte.is_ascii_alphanumeric()
                || byte == b'-'
                || byte == b'.'
                || byte == b'_'
                || byte == b'~'
                || (preserve_path_separators && (byte == b'/' || byte == b'\\'))
            {
                encoded.push(byte as char);
            } else {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
        encoded
    }
}

/// AFF4 Standard media.
///
/// The media is the resolved image of a standard AFF4 (for example AFF4
/// Standard v1.0a) volume: the data streams (with their bevy data and index
/// entries), the map (if present) and the media (image) size.
#[derive(Clone)]
pub struct Aff4StandardMedia {
    /// Data volumes (the volume and its companion volumes).
    volumes: Vec<DataVolume>,

    /// Resolved data streams.
    streams: Vec<ResolvedStream>,

    /// Resolved map; None for images that read directly from their data stream.
    map: Option<ResolvedMap>,

    /// Media (image) size in bytes.
    media_size: u64,

    /// Name of the image node (or the primary data stream).
    image_name: String,

    /// Bytes per sector.
    bytes_per_sector: u32,
}

impl Aff4StandardMedia {
    /// Opens the AFF4 Standard media stored in the volume with the specified file name.
    ///
    /// Companion volumes (for example the `split.aff4.A01` companion
    /// container and other AFF4 containers in the same directory) are
    /// resolved and their data streams and maps are available.
    pub fn open(
        file_resolver: &FileResolverReference,
        file_name: &PathComponent,
    ) -> Result<Aff4StandardMedia, ErrorTrace> {
        let mut volumes: Vec<DataVolume> = Vec::new();
        let primary_volume: DataVolume = DataVolume::open(file_resolver, file_name)?;

        if !primary_volume.information.is_standard() {
            return Err(keramics_core::error_trace_new!(format!(
                "Volume: {} does not contain standard AFF4 nodes",
                primary_volume.file_name
            )));
        }
        volumes.push(primary_volume);
        Self::add_companion_volumes(file_resolver, &mut volumes)?;

        // Group the data stream nodes by their identity.
        let mut stream_identities: Vec<String> = Vec::new();
        let mut stream_identity_groups: Vec<Vec<(usize, Aff4Node)>> = Vec::new();
        for (volume_index, volume) in volumes.iter().enumerate() {
            for node in volume.information.nodes.iter() {
                if !node.is_type("ImageStream") {
                    continue;
                }
                let identity: String = Self::stream_identity(node.subject.as_str());
                if identity.is_empty() {
                    continue;
                }
                let mut group_index: Option<usize> = None;
                for (index, group) in stream_identity_groups.iter().enumerate() {
                    let group_identity: String = Self::stream_identity(group[0].1.subject.as_str());
                    if group_identity == identity {
                        group_index = Some(index);
                        break;
                    }
                }
                match group_index {
                    Some(index) => {
                        stream_identity_groups[index].push((volume_index, node.clone()));
                    }
                    None => {
                        if !stream_identities.contains(&identity) {
                            stream_identities.push(identity);
                        }
                        stream_identity_groups.push(vec![(volume_index, node.clone())]);
                    }
                }
            }
        }
        if stream_identity_groups.is_empty() {
            return Err(keramics_core::error_trace_new!(
                "Missing data stream node in information container"
            ));
        }

        // Resolve the data streams; those without data entries in any of the
        // volumes are only kept when required by the map.
        let mut streams: Vec<ResolvedStream> = Vec::new();
        for (index, group) in stream_identity_groups.iter().enumerate() {
            let identity: String = stream_identities[index].clone();
            if let Some(stream) = Self::resolve_stream(&volumes, &identity, group) {
                streams.push(stream);
            }
        }
        if streams.is_empty() {
            return Err(keramics_core::error_trace_new!(
                "Missing data entry for image stream in volume"
            ));
        }

        // Resolve the image node; when the image points to a map node the
        // media is read through the map.
        let image_node: Option<&Aff4Node> = {
            let mut sized_image_node: Option<&Aff4Node> = None;
            let mut image_node: Option<&Aff4Node> = None;

            for volume in volumes.iter() {
                for node in volume.information.nodes.iter() {
                    if !node.is_type("Image") {
                        continue;
                    }
                    if node.get_u64_property("size").is_some() {
                        if sized_image_node.is_none() {
                            sized_image_node = Some(node);
                        }
                    } else if image_node.is_none() {
                        image_node = Some(node);
                    }
                }
            }
            match sized_image_node {
                Some(image_node) => Some(image_node),
                None => image_node,
            }
        };

        let bytes_per_sector: u32 =
            match image_node.and_then(|node| node.get_u64_property("blockSize")) {
                Some(block_size) => block_size as u32,
                None => AFF4_DEFAULT_BYTES_PER_SECTOR,
            };

        let map: Option<ResolvedMap> =
            match image_node.and_then(|node| node.get_property_value("dataStream")) {
                Some(map_iri) => match Self::resolve_map(&volumes, &streams, map_iri) {
                    Ok(map) => Some(map),
                    Err(error) => {
                        // When the data stream reference points to a map
                        // node the map is required; reading the media data
                        // directly from the data streams would silently
                        // deliver the wrong data.
                        let references_map_node: bool = volumes.iter().any(|volume| {
                            volume
                                .information
                                .nodes
                                .iter()
                                .any(|node| node.is_type("Map") && node.subject == map_iri)
                        });
                        if references_map_node {
                            return Err(error);
                        }
                        None
                    }
                },
                None => None,
            };

        // Determine the media size.
        let media_size: u64 = match image_node.and_then(|node| node.get_u64_property("size")) {
            Some(media_size) => media_size,
            None => match map.as_ref() {
                Some(map) => map
                    .entries
                    .iter()
                    .map(|entry| entry.mapped_offset + entry.length)
                    .max()
                    .unwrap_or(0),
                None => {
                    let stream_size: u64 =
                        streams.iter().map(|stream| stream.size).max().unwrap_or(0);
                    if stream_size > 0 {
                        stream_size
                    } else {
                        streams[0].get_number_of_chunks() as u64 * streams[0].chunk_size as u64
                    }
                }
            },
        };
        let image_name: String = match image_node {
            Some(node) if node.subject != ":" => node.subject.clone(),
            _ => streams[0].name.clone(),
        };

        Ok(Aff4StandardMedia {
            volumes,
            streams,
            map,
            media_size,
            image_name,
            bytes_per_sector,
        })
    }

    /// Retrieves the bytes per sector.
    pub fn get_bytes_per_sector(&self) -> u32 {
        self.bytes_per_sector
    }

    /// Retrieves the media size in bytes.
    pub fn get_media_size(&self) -> u64 {
        self.media_size
    }

    /// Retrieves the name of the image.
    pub fn get_image_name(&self) -> &str {
        &self.image_name
    }

    /// Determines if the media is read through a map.
    pub fn is_mapped(&self) -> bool {
        self.map.is_some()
    }

    /// Adds the companion volumes to the list of volumes.
    ///
    /// The companion volumes are the `<name>.A01` to `<name>.A32` split
    /// companion containers and the AFF4 containers stored in the same
    /// directory that the data stream nodes store as `aff4:stored`.
    fn add_companion_volumes(
        file_resolver: &FileResolverReference,
        volumes: &mut Vec<DataVolume>,
    ) -> Result<(), ErrorTrace> {
        let primary_file_name: String = volumes[0].file_name.clone();
        let mut candidate_names: Vec<String> = Vec::new();

        // Split the file name into a directory and a base file name, for
        // example `Striped/Base-Linear_1.aff4` results in the directory
        // `Striped` and the base file name `Base-Linear_1.aff4`.
        let directory: String = match primary_file_name.rfind('/') {
            Some(pos) if pos > 0 => primary_file_name[..pos].to_string(),
            _ => String::new(),
        };
        let base_file_name: String = primary_file_name
            .rsplit_once('/')
            .map(|(_, file_name)| file_name.to_string())
            .unwrap_or(primary_file_name.clone());

        // Split volume companions are derived from the container file name.
        for index in 1..=32 {
            candidate_names.push(format!("{directory}{base_file_name}.A{index:02}"));
        }

        // The data stream nodes referenced as stored in another volume by an
        // absolute volume URI; the companion volumes are the containers
        // storing those streams.
        let stored_volume_uris: Vec<String> = volumes[0]
            .information
            .nodes
            .iter()
            .filter(|node| node.is_type("ImageStream"))
            .filter_map(|node| node.get_property_value("stored"))
            .filter(|stored| stored.contains("://") && *stored != volumes[0].volume_uri)
            .map(String::from)
            .collect();

        // List the base directory and the directory containing the primary
        // volume (when different).
        let mut directory_paths: Vec<Vec<PathComponent>> = Vec::new();
        directory_paths.push(Vec::new());
        if !directory.is_empty() {
            let mut directory_path: Vec<PathComponent> = Vec::new();
            for directory_part in directory.split('/') {
                if !directory_part.is_empty() {
                    directory_path.push(PathComponent::from(directory_part));
                }
            }
            if !directory_path.is_empty() {
                directory_paths.push(directory_path);
            }
        }
        for directory_path in directory_paths.iter() {
            match file_resolver.list_file_names(directory_path.as_slice()) {
                Ok(listed_file_names) => {
                    for listed_file_name in listed_file_names {
                        let lowercased: String = listed_file_name.to_ascii_lowercase();
                        if !lowercased.ends_with(".aff4") && !lowercased.ends_with(".af4") {
                            continue;
                        }
                        if listed_file_name == base_file_name && directory_path.is_empty() {
                            continue;
                        }
                        let candidate_name: String = if directory.is_empty() {
                            listed_file_name.clone()
                        } else {
                            format!("{}/{}", directory, listed_file_name)
                        };
                        if stored_volume_uris.is_empty()
                            && !Self::is_split_companion_name(
                                candidate_name.as_str(),
                                primary_file_name.as_str(),
                            )
                        {
                            // Without a volume reference a listed container
                            // is only a split companion candidate.
                            continue;
                        }
                        if !candidate_names.contains(&candidate_name) {
                            candidate_names.push(candidate_name);
                        }
                    }
                }
                Err(_) => {
                    // A directory listing is not required; only the named
                    // companion containers are resolved when it is not
                    // available.
                }
            }
        }

        // Open the candidate containers; a listed container is only
        // accepted as a companion volume when a data stream node references
        // it or it is a split companion of the primary container.
        for candidate_name in candidate_names.iter() {
            if *candidate_name == primary_file_name {
                continue;
            }
            if volumes
                .iter()
                .any(|existing| existing.file_name == *candidate_name)
            {
                continue;
            }
            let path_component: PathComponent = PathComponent::from(candidate_name.as_str());

            match DataVolume::open(file_resolver, &path_component) {
                Ok(volume) => {
                    if !volume.information.is_standard() {
                        // Non-standard or unreadable containers are not
                        // companion volumes.
                        continue;
                    }
                    if Self::is_split_companion_name(
                        candidate_name.as_str(),
                        primary_file_name.as_str(),
                    ) || stored_volume_uris
                        .iter()
                        .any(|stored_uri| volume.volume_uri == *stored_uri)
                    {
                        volumes.push(volume);
                    }
                }
                _ => {
                    // Unreadable containers are not companion volumes.
                }
            }
        }
        Ok(())
    }

    /// Determines if the candidate file name is a split companion container of the primary container (for example `image.aff4.A01`).
    fn is_split_companion_name(candidate_file_name: &str, primary_file_name: &str) -> bool {
        let suffix: &str = match candidate_file_name.strip_prefix(primary_file_name) {
            Some(suffix) if suffix.len() >= 4 && suffix.starts_with(".A") => &suffix[2..],
            _ => return false,
        };
        match suffix.parse::<u8>().ok() {
            Some(index) => (1..=32).contains(&index),
            None => false,
        }
    }

    /// Retrieves the identity of a data stream node.
    ///
    /// The identity is the node subject with a leading `aff4://` scheme and a
    /// volume identifier path segment removed, so that the same stream
    /// declared in different companion volumes has the same identity.
    fn stream_identity(subject: &str) -> String {
        let subject: &str = subject.trim();

        if subject.is_empty() || subject == ":" {
            return String::new();
        }
        if let Some(rest) = subject.strip_prefix("aff4://") {
            if let Some(slash_pos) = rest.find('/') {
                let prefix: &str = &rest[..slash_pos];
                if prefix.len() == 36 {
                    return rest[slash_pos + 1..].to_string();
                }
            }
            return rest.to_string();
        }
        subject.to_string()
    }

    /// Retrieves the candidate names of a data stream node.
    fn stream_name_candidates(subject: &str) -> Vec<String> {
        let mut candidates: Vec<String> = Vec::new();
        let subject: &str = subject.trim();

        if subject.is_empty() || subject == ":" {
            return candidates;
        }
        candidates.push(subject.to_string());
        if let Some(rest) = subject.strip_prefix("aff4://") {
            let mut rest: String = rest.to_string();
            if !candidates.contains(&rest) {
                candidates.push(rest.clone());
            }
            if let Some(slash_pos) = rest.find('/') {
                if rest[..slash_pos].len() == 36 {
                    rest = rest[slash_pos + 1..].to_string();
                    if !candidates.contains(&rest) {
                        candidates.push(rest);
                    }
                }
            }
        }
        candidates
    }

    /// Resolves the data stream with the specified identity.
    ///
    /// The node metadata (chunk size, number of chunks per segment,
    /// compression method and size) is merged from the volumes that declare
    /// the stream; the data entries are read from the volume that stores the
    /// stream data.
    fn resolve_stream(
        volumes: &[DataVolume],
        identity: &str,
        group: &[(usize, Aff4Node)],
    ) -> Option<ResolvedStream> {
        let mut chunk_size: usize = 0;
        let chunks_in_segment: usize = {
            let mut chunks_in_segment: usize = AFF4_STANDARD_DEFAULT_CHUNKS_IN_SEGMENT;

            for (_volume_index, node) in group.iter() {
                if let Some(node_chunks_in_segment) = node.get_u64_property("chunksInSegment") {
                    if node_chunks_in_segment > 0 {
                        chunks_in_segment = node_chunks_in_segment as usize;
                        break;
                    }
                }
            }
            chunks_in_segment
        };
        let compression_method: CompressionMethod = {
            let mut compression_method: CompressionMethod = CompressionMethod::Stored;

            for (_volume_index, node) in group.iter() {
                if let Some(compression_iri) = node.get_property_value("compressionMethod") {
                    compression_method = CompressionMethod::from_iri(compression_iri);
                    break;
                }
            }
            compression_method
        };
        let size: u64 = group
            .iter()
            .filter_map(|(_volume_index, node)| node.get_u64_property("size"))
            .max()
            .unwrap_or(0);
        let mut name_candidates: Vec<String> = Vec::new();

        for (_volume_index, node) in group.iter() {
            if chunk_size == 0 {
                if let Some(node_chunk_size) = node.get_u64_property("chunkSize") {
                    chunk_size = node_chunk_size as usize;
                }
            }
            for name_candidate in Self::stream_name_candidates(node.subject.as_str()) {
                if !name_candidates.contains(&name_candidate) {
                    name_candidates.push(name_candidate);
                }
            }
        }
        if chunk_size == 0 || name_candidates.is_empty() {
            return None;
        }

        // Locate the volume that stores the data entries of the stream and
        // read its bevy segments.
        for name_candidate in name_candidates.iter() {
            for volume in volumes.iter() {
                if let Some(entry_prefix) = volume.get_entry_prefix_for_node_name(
                    name_candidate,
                    "00000000",
                    Some(".index"),
                ) {
                    let expected_number_of_chunks: usize = if size > 0 {
                        (size / chunk_size as u64) as usize
                    } else {
                        0
                    };
                    let mut segments: Vec<ResolvedSegment> = Vec::new();

                    let mut segments_read: usize = 0;
                    loop {
                        if segments_read >= AFF4_STANDARD_MAX_SEGMENTS {
                            break;
                        }
                        let first_chunk_index: usize = segments_read * chunks_in_segment;
                        let segment: String = format!("{:08x}", first_chunk_index);
                        let segment_data_entry_name: String =
                            format!("{}/{}", entry_prefix, segment);
                        let segment_index_entry_name: String =
                            format!("{}/{}.index", entry_prefix, segment);
                        if !volume.has_entry(segment_data_entry_name.as_str())
                            || !volume.has_entry(segment_index_entry_name.as_str())
                        {
                            break;
                        }
                        let index_data: Vec<u8> =
                            match volume.read_entry_data(segment_index_entry_name.as_str()) {
                                Ok(index_data) => index_data,
                                Err(_) => {
                                    break;
                                }
                            };
                        if index_data.len() % AFF4_STANDARD_BEVY_INDEX_ENTRY_SIZE != 0 {
                            break;
                        }
                        let mut index_entries: Vec<BevyIndexEntry> = Vec::new();
                        for entry_index in
                            0..(index_data.len() / AFF4_STANDARD_BEVY_INDEX_ENTRY_SIZE)
                        {
                            let offset: usize = entry_index * AFF4_STANDARD_BEVY_INDEX_ENTRY_SIZE;
                            let bevvy_offset: u64 = u64::from_le_bytes(
                                index_data[offset..(offset + 8)].try_into().unwrap(),
                            );
                            let stored_size: u32 = u32::from_le_bytes(
                                index_data[(offset + 8)..(offset + 12)].try_into().unwrap(),
                            );
                            index_entries.push(BevyIndexEntry {
                                bevvy_offset,
                                stored_size,
                            });
                        }
                        if index_entries.is_empty() {
                            break;
                        }
                        segments.push(ResolvedSegment {
                            first_chunk_index,
                            data_entry_name: segment_data_entry_name,
                            index_entries,
                        });
                        segments_read += 1;
                        if expected_number_of_chunks > 0
                            && segments
                                .iter()
                                .map(|segment| segment.index_entries.len())
                                .sum::<usize>()
                                >= expected_number_of_chunks
                        {
                            break;
                        }
                    }
                    if segments.is_empty() {
                        continue;
                    }
                    return Some(ResolvedStream {
                        name: identity.to_string(),
                        chunk_size,
                        chunks_in_segment,
                        compression_method,
                        size,
                        zip_file: volume.zip_file.clone(),
                        entry_prefix,
                        segments,
                    });
                }
            }
        }
        None
    }

    /// Resolves the map node referenced by the image data stream.
    fn resolve_map(
        volumes: &[DataVolume],
        streams: &[ResolvedStream],
        map_iri: &str,
    ) -> Result<ResolvedMap, ErrorTrace> {
        let mut map_volume: Option<&DataVolume> = None;
        let mut map_node: Option<&Aff4Node> = None;
        for volume in volumes.iter() {
            for node in volume.information.nodes.iter() {
                if node.is_type("Map") && node.subject == map_iri {
                    map_volume = Some(volume);
                    map_node = Some(node);
                    break;
                }
            }
            if map_node.is_some() {
                break;
            }
        }
        let (map_volume, map_node) = match (map_volume, map_node) {
            (Some(map_volume), Some(map_node)) => (map_volume, map_node),
            _ => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Missing map node: {} in volume",
                    map_iri
                )));
            }
        };

        // The map target table is stored as a sibling entry of the map data
        // entry (`<prefix>/map` and `<prefix>/idx`).
        let entry_prefix: Option<String> =
            map_volume.get_entry_prefix_for_node_name(map_iri, "map", None);
        let entry_prefix: String = match entry_prefix {
            Some(entry_prefix) if map_volume.has_entry(&format!("{}/idx", entry_prefix)) => {
                entry_prefix
            }
            _ => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Missing map entries for node: {}",
                    map_iri
                )));
            }
        };

        // Read the target identities of the map.
        let idx_data: Vec<u8> = map_volume.read_entry_data(&format!("{}/idx", entry_prefix))?;
        let target_identities: Vec<String> = match String::from_utf8(idx_data) {
            Ok(idx_data) => idx_data
                .split(['\n', '\r', '\0'])
                .map(|target_identity| target_identity.trim().to_string())
                .filter(|target_identity| !target_identity.is_empty())
                .collect(),
            Err(error) => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unsupported encoding of map target identities with error: {}",
                    error
                )));
            }
        };

        // Resolve the streams referenced by the map target identities.
        let stream_identities: Vec<String> =
            streams.iter().map(|stream| stream.name.clone()).collect();

        // Resolve the default gap target.
        let gap_iri: &str = map_node
            .get_property_value("mapGapDefaultStream")
            .unwrap_or("aff4:Zero");
        let gap_default: MapTarget =
            Self::resolve_map_target(gap_iri, &stream_identities).unwrap_or(MapTarget::Gap);

        // Read and resolve the map entries.
        let map_data: Vec<u8> = map_volume.read_entry_data(&format!("{}/map", entry_prefix))?;
        if map_data.len() % AFF4_STANDARD_MAP_ENTRY_SIZE != 0 {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported map data size: {}",
                map_data.len()
            )));
        }
        let mut entries: Vec<ResolvedMapEntry> = Vec::new();
        for entry_index in 0..(map_data.len() / AFF4_STANDARD_MAP_ENTRY_SIZE) {
            let offset: usize = entry_index * AFF4_STANDARD_MAP_ENTRY_SIZE;
            let mapped_offset: u64 =
                u64::from_le_bytes(map_data[offset..(offset + 8)].try_into().unwrap());
            let length: u64 =
                u64::from_le_bytes(map_data[(offset + 8)..(offset + 16)].try_into().unwrap());
            let target_offset: u64 =
                u64::from_le_bytes(map_data[(offset + 16)..(offset + 24)].try_into().unwrap());
            let target_index: i32 =
                i32::from_le_bytes(map_data[(offset + 24)..(offset + 28)].try_into().unwrap());
            let target_identity: Option<&str> = target_identities
                .get(target_index as usize)
                .map(String::as_str);
            let target: MapTarget = match target_identity {
                Some(target_identity) => {
                    Self::resolve_map_target(target_identity, &stream_identities)
                        .unwrap_or(MapTarget::Gap)
                }
                None => MapTarget::Gap,
            };
            entries.push(ResolvedMapEntry {
                mapped_offset,
                length,
                target_offset,
                target,
            });
        }
        entries.sort_by_key(|entry| entry.mapped_offset);

        Ok(ResolvedMap {
            entries,
            gap_default,
        })
    }

    /// Resolves the target of a map entry.
    ///
    /// Zero (and the `UnknownData` and `UnreadableData` targets) are zero
    /// filled gaps, `SymbolicStreamXX` targets are filled with the `XX` byte
    /// and data stream IRIs resolve to the data stream with the matching
    /// identity.
    fn resolve_map_target(target: &str, stream_identities: &[String]) -> Option<MapTarget> {
        let lowercased: String = target.to_ascii_lowercase();

        if lowercased == "zero"
            || lowercased == "aff4:zero"
            || lowercased == "aff4://zero"
            || lowercased.ends_with("#zero")
            || lowercased.ends_with("#unknowndata")
            || lowercased.ends_with("#unreadabledata")
        {
            return Some(MapTarget::Gap);
        }
        if let Some(symbolic_pos) = lowercased.find("symbolicstream") {
            let symbolic_value: &str = lowercased[(symbolic_pos + "symbolicstream".len())..].trim();
            if !symbolic_value.is_empty() {
                if let Some(byte) = u8::from_str_radix(symbolic_value, 16).ok() {
                    return Some(MapTarget::Symbolic(byte));
                }
            }
        }
        let target_identity: String = Self::stream_identity(target);
        if !target_identity.is_empty() {
            if let Some(position) = stream_identities
                .iter()
                .position(|stream_identity| stream_identity == &target_identity)
            {
                return Some(MapTarget::Stream(position));
            }
        }
        None
    }
}

/// Advanced Forensic Format 4 (AFF4) Standard media block reader.
///
/// Reads the media data by resolving the region in the map (or in the data
/// stream for a non-mapped image), reading the chunk from the bevy data
/// entry, decompressing it (if stored compressed) and caching the decoded
/// chunks.
pub struct Aff4StandardReader {
    /// Media.
    media: Aff4StandardMedia,

    /// Decoded chunk cache keyed by stream and chunk index.
    chunk_cache: LruCache<u32, Vec<u8>>,

    /// Bevy data cache keyed by stream and segment index.
    bevy_cache: LruCache<u32, Vec<u8>>,
}

impl Aff4StandardReader {
    /// Creates a new block reader.
    pub fn new(media: Aff4StandardMedia) -> Self {
        Self {
            media,
            chunk_cache: LruCache::new(AFF4_STANDARD_CHUNK_CACHE_SIZE),
            bevy_cache: LruCache::new(16),
        }
    }

    /// Reads the mapped media data into the buffer.
    fn read_mapped_data(
        &mut self,
        map: &ResolvedMap,
        data: &mut [u8],
        offset: u64,
        read_size: usize,
    ) -> Result<usize, ErrorTrace> {
        let mut offset_in_read: usize = 0;
        let mut position: u64 = offset;

        while offset_in_read < read_size {
            match self.get_map_entry(position) {
                Some((entry, offset_in_entry)) => {
                    let available_length: usize = min(
                        entry.length as usize - offset_in_entry,
                        read_size - offset_in_read,
                    );
                    match entry.target {
                        MapTarget::Gap | MapTarget::Symbolic(_) => {
                            let fill_byte: u8 = match entry.target {
                                MapTarget::Symbolic(fill_byte) => fill_byte,
                                _ => 0,
                            };
                            data[offset_in_read..(offset_in_read + available_length)]
                                .fill(fill_byte);
                        }
                        MapTarget::Stream(stream_index) => {
                            let target_position: u64 = entry.target_offset + offset_in_entry as u64;
                            self.read_stream_data(
                                stream_index,
                                target_position,
                                &mut data[offset_in_read..(offset_in_read + available_length)],
                            )?;
                        }
                    }
                    offset_in_read += available_length;
                    position += available_length as u64;
                }
                None => {
                    // The offset is not covered by a map entry; the region is
                    // filled with the default gap target of the map.
                    match map.gap_default {
                        MapTarget::Gap | MapTarget::Symbolic(_) => {
                            let fill_byte: u8 = match map.gap_default {
                                MapTarget::Symbolic(fill_byte) => fill_byte,
                                _ => 0,
                            };
                            data[offset_in_read..read_size].fill(fill_byte);
                            offset_in_read = read_size;
                            position = u64::MAX;
                        }
                        MapTarget::Stream(stream_index) => {
                            // A gap default data stream is read from its
                            // corresponding media offset.
                            let read_length: usize = read_size - offset_in_read;
                            self.read_stream_data(
                                stream_index,
                                position,
                                &mut data[offset_in_read..read_size],
                            )?;
                            offset_in_read += read_length;
                            position += read_length as u64;
                        }
                    }
                }
            }
        }
        Ok(read_size)
    }

    /// Retrieves the map entry that covers the specified offset.
    fn get_map_entry(&self, position: u64) -> Option<(ResolvedMapEntry, usize)> {
        let map: &ResolvedMap = self.media.map.as_ref()?;
        let mut lower: usize = 0;
        let mut upper: usize = map.entries.len();

        while lower < upper {
            let offset: usize = lower + (upper - lower) / 2;
            if map.entries[offset].mapped_offset <= position {
                lower = offset + 1;
            } else {
                upper = offset;
            }
        }
        let index: usize = lower.saturating_sub(1);

        if index >= map.entries.len() {
            return None;
        }
        let entry: &ResolvedMapEntry = &map.entries[index];
        if position >= entry.mapped_offset && position < entry.mapped_offset + entry.length {
            return Some((entry.clone(), (position - entry.mapped_offset) as usize));
        }
        None
    }

    /// Reads the data stream data at the specified offset into the buffer.
    fn read_stream_data(
        &mut self,
        stream_index: usize,
        position: u64,
        data: &mut [u8],
    ) -> Result<(), ErrorTrace> {
        let chunk_size: u64 = match self.media.streams.get(stream_index) {
            Some(stream) => stream.chunk_size as u64,
            None => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Missing data stream with index: {}",
                    stream_index
                )));
            }
        };
        if chunk_size == 0 {
            return Err(keramics_core::error_trace_new!("Unsupported chunk size"));
        }
        let mut offset: usize = 0;
        let mut read_position: u64 = position;

        while offset < data.len() {
            let chunk_index: usize = (read_position / chunk_size) as usize;
            let chunk: Vec<u8> = self.get_chunk(stream_index, chunk_index)?;
            let offset_in_chunk: usize = (read_position % chunk_size) as usize;
            if offset_in_chunk >= chunk.len() {
                read_position += chunk_size;
                continue;
            }
            let read_size: usize = min(data.len() - offset, chunk.len() - offset_in_chunk);
            data[offset..(offset + read_size)]
                .copy_from_slice(&chunk[offset_in_chunk..(offset_in_chunk + read_size)]);
            offset += read_size;
            read_position += read_size as u64;
        }
        Ok(())
    }

    /// Retrieves the decoded chunk of the data stream with the specified index.
    fn get_chunk(
        &mut self,
        stream_index: usize,
        chunk_index: usize,
    ) -> Result<Vec<u8>, ErrorTrace> {
        let cache_key: u32 = (stream_index as u32) << 24 | (chunk_index as u32);

        if let Some(cached_chunk) = self.chunk_cache.get(&cache_key) {
            return Ok(cached_chunk.clone());
        }
        // Collect the chunk metadata first; the borrow of the media must end
        // before the chunk data is read (which uses an interior copy of the
        // Zip file and updates the caches).
        let (
            stream_name,
            chunk_size,
            compression_method,
            segment_index,
            bevvy_offset,
            stored_size,
        ): (String, usize, CompressionMethod, usize, u64, u32) = {
            let stream: &ResolvedStream = match self.media.streams.get(stream_index) {
                Some(stream) => stream,
                None => {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Missing data stream with index: {}",
                        stream_index
                    )))
                }
            };
            let (segment_index, _segment, index_entry): (usize, &ResolvedSegment, &BevyIndexEntry) =
                match stream.get_segment_for_chunk(chunk_index) {
                    Some(segment) => segment,
                    None => {
                        return Err(keramics_core::error_trace_new!(format!(
                            "Missing chunk: {} of data stream: {}",
                            chunk_index, stream.name
                        )))
                    }
                };
            (
                stream.name.clone(),
                stream.chunk_size,
                stream.compression_method,
                segment_index,
                index_entry.bevvy_offset,
                index_entry.stored_size,
            )
        };
        let mut chunk: Vec<u8> = vec![0; chunk_size];

        if stored_size == 0 {
            // A stored size of 0 indicates a chunk consisting only of zero
            // bytes.
        } else if stored_size as usize == chunk_size {
            // A stored size equal to the chunk size indicates a chunk that is
            // stored without compression.
            let bevy_data: Vec<u8> = self.get_bevy_data(stream_index, segment_index)?;
            let bebvy_offset_pos: usize = bevvy_offset as usize;
            if bebvy_offset_pos + stored_size as usize > bevy_data.len() {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unsupported bevy offset: {} for chunk: {} of data stream: {}",
                    bevvy_offset, chunk_index, stream_name
                )));
            }
            chunk.copy_from_slice(
                &bevy_data[bebvy_offset_pos..(bebvy_offset_pos + stored_size as usize)],
            );
        } else {
            let bevy_data: Vec<u8> = self.get_bevy_data(stream_index, segment_index)?;
            let bevvy_offset_pos: usize = bevvy_offset as usize;
            if bevvy_offset_pos + stored_size as usize > bevy_data.len() {
                return Err(keramics_core::error_trace_new!(format!(
                    "Unsupported bevy offset: {} for chunk: {} of data stream: {}",
                    bevvy_offset, chunk_index, stream_name
                )));
            }
            let compressed_data: &[u8] =
                &bevy_data[bevvy_offset_pos..(bevvy_offset_pos + stored_size as usize)];
            match compression_method {
                CompressionMethod::Snappy => {
                    let mut snappy_context: SnappyContext = SnappyContext::new();
                    match snappy_context.decompress(compressed_data, &mut chunk) {
                        Ok(_) => {}
                        Err(mut error) => {
                            keramics_core::error_trace_add_frame!(
                                error,
                                format!(
                                    "Unable to decompress Snappy chunk: {} of data stream: {}",
                                    chunk_index, stream_name
                                )
                            );
                            return Err(error);
                        }
                    }
                }
                CompressionMethod::Zlib => {
                    let mut zlib_context: ZlibContext = ZlibContext::new();
                    match zlib_context.decompress(compressed_data, &mut chunk) {
                        Ok(_) => {}
                        Err(mut error) => {
                            keramics_core::error_trace_add_frame!(
                                error,
                                format!(
                                    "Unable to decompress Zlib chunk: {} of data stream: {}",
                                    chunk_index, stream_name
                                )
                            );
                            return Err(error);
                        }
                    }
                }
                CompressionMethod::Deflate => {
                    let mut deflate_context: DeflateContext = DeflateContext::new();
                    match deflate_context.decompress(compressed_data, &mut chunk) {
                        Ok(_) => {}
                        Err(mut error) => {
                            keramics_core::error_trace_add_frame!(
                                error,
                                format!(
                                    "Unable to decompress Deflate chunk: {} of data stream: {}",
                                    chunk_index, stream_name
                                )
                            );
                            return Err(error);
                        }
                    }
                }
                CompressionMethod::Stored => {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Unsupported stored size: {} for chunk: {} of data stream: {}",
                        stored_size, chunk_index, stream_name
                    )));
                }
            }
        }
        self.chunk_cache.insert(cache_key, chunk.clone());

        Ok(chunk)
    }

    /// Retrieves the bevy data of the segment of the data stream with the specified index.
    fn get_bevy_data(
        &mut self,
        stream_index: usize,
        segment_index: usize,
    ) -> Result<Vec<u8>, ErrorTrace> {
        let cache_key: u32 = (stream_index as u32) << 8 | (segment_index as u32);

        if let Some(cached_bevy) = self.bevy_cache.get(&cache_key) {
            return Ok(cached_bevy.clone());
        }
        let (stream_name, data_entry_name, zip_file): (String, String, ZipFile) = {
            let stream: &ResolvedStream = match self.media.streams.get(stream_index) {
                Some(stream) => stream,
                None => {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Missing data stream with index: {}",
                        stream_index
                    )));
                }
            };
            let segment: &ResolvedSegment = match stream.segments.get(segment_index) {
                Some(segment) => segment,
                None => {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Missing segment with index: {} of data stream: {}",
                        segment_index, stream.name
                    )));
                }
            };
            (
                stream.name.clone(),
                segment.data_entry_name.clone(),
                stream.zip_file.clone(),
            )
        };
        let entry: ZipEntry = match self.media.streams[stream_index]
            .zip_file
            .get_entry(data_entry_name.as_str())
        {
            Some(entry) => entry.clone(),
            None => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Missing bevy data entry: {} of data stream: {}",
                    data_entry_name, stream_name
                )));
            }
        };
        let mut bevy_data: Vec<u8> = vec![0; entry.uncompressed_size as usize];
        let mut zip_file: ZipFile = zip_file;

        match zip_file.read_entry_data(&entry, &mut bevy_data) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!(
                        "Unable to read bevy data of entry: {} of data stream: {}",
                        data_entry_name, stream_name
                    )
                );
                return Err(error);
            }
        }
        self.bevy_cache.insert(cache_key, bevy_data.clone());

        Ok(bevy_data)
    }
}

impl BlockReader for Aff4StandardReader {
    /// Retrieves the media size.
    fn get_size(&self) -> u64 {
        self.media.media_size
    }

    /// Reads the media data from the blocks.
    fn read_data_from_blocks(&mut self, data: &mut [u8], offset: u64) -> Result<usize, ErrorTrace> {
        let media_size: u64 = self.get_size();

        if data.is_empty() || offset >= media_size {
            return Ok(0);
        }
        let read_size: usize = min(data.len(), (media_size - offset) as usize);

        if let Some(map) = self.media.map.clone() {
            return self.read_mapped_data(&map, data, offset, read_size);
        }
        if self.media.streams.is_empty() {
            return Err(keramics_core::error_trace_new!(
                "Missing data stream in volume"
            ));
        }
        // A non-mapped image (for example a split volume) reads its data
        // directly from the first data stream.
        self.read_stream_data(0, offset, &mut data[..read_size])?;

        Ok(read_size)
    }
}

/// Advanced Forensic Format 4 (AFF4) Standard media block stream.
pub type Aff4StandardBlockStream = crate::block_stream::BlockStream<Aff4StandardReader>;

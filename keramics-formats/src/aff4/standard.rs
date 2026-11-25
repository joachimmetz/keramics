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

use keramics_compression::SnappyContext;
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
const AFF4_STANDARD_MAXIMUM_NUMBER_OF_SEGMENTS: usize = 16384;

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
    bevy_offset: u64,

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

    /// Compression method of the chunks.
    compression_method: CompressionMethod,

    /// Total size of the stream data in bytes (0 if not declared).
    size: u64,

    /// Zip file of the volume that stores the stream data.
    zip_file: ZipFile,

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

    /// Volume URI of the volume.
    volume_uri: String,

    /// Information container of the volume.
    information: Aff4Information,

    /// Zip file of the volume container.
    zip_file: ZipFile,
}

impl DataVolume {
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
            let has_index: bool = index_suffix
                .is_none_or(|suffix| self.has_entry(&format!("{}{}", entry_name, suffix)));

            if self.has_entry(entry_name.as_str()) && has_index {
                return Some(prefix_candidate);
            }
        }
        None
    }

    /// Retrieves the size of an entry with the specified name.
    #[cfg(test)]
    // Used for testing.
    fn get_entry_size(&self, file_name: &str) -> Option<u64> {
        self.zip_file
            .get_entry(file_name)
            .map(|entry| entry.uncompressed_size as u64)
    }

    /// Determines if the volume contains an entry with the specified name.
    fn has_entry(&self, file_name: &str) -> bool {
        self.zip_file.get_entry(file_name).is_some()
    }

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
            volume_uri: aff4_file.get_volume_uri().to_string(),
            information: aff4_file.take_information(),
            zip_file: aff4_file.take_zip_file(),
        })
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
}

/// AFF4 Standard media.
///
/// The media is the resolved image of a standard AFF4 (for example AFF4
/// Standard v1.0a) volume: the data streams (with their bevy data and index
/// entries), the map (if present) and the media (image) size.
#[derive(Clone)]
pub struct Aff4StandardMedia {
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
            Some(slash_index) if slash_index > 0 => primary_file_name[..slash_index].to_string(),
            _ => String::new(),
        };
        let base_file_name: String = primary_file_name
            .rsplit_once('/')
            .map(|(_, file_name)| file_name.to_string())
            .unwrap_or_else(|| primary_file_name.clone());

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

        Self::collect_listed_companion_candidates(
            file_resolver,
            directory.as_str(),
            base_file_name.as_str(),
            primary_file_name.as_str(),
            !stored_volume_uris.is_empty(),
            &mut candidate_names,
        );
        // Open the candidate containers; a listed container is only
        // accepted as a companion volume when a data stream node references
        // it or it is a split companion of the primary container.
        for candidate_name in candidate_names.iter() {
            if *candidate_name == primary_file_name
                || volumes
                    .iter()
                    .any(|existing| existing.file_name == *candidate_name)
            {
                continue;
            }
            if let Some(volume) = Self::try_open_companion_volume(
                file_resolver,
                candidate_name.as_str(),
                primary_file_name.as_str(),
                &stored_volume_uris,
            ) {
                volumes.push(volume);
            }
        }
        Ok(())
    }

    /// Groups `ImageStream` nodes from a single volume into identity groups.
    fn add_volume_stream_nodes(
        volume_index: usize,
        volume: &DataVolume,
        stream_identities: &mut Vec<String>,
        stream_identity_groups: &mut Vec<Vec<(usize, Aff4Node)>>,
    ) {
        for node in volume.information.nodes.iter() {
            if !node.is_type("ImageStream") {
                continue;
            }
            let identity: String = Self::stream_identity(node.subject.as_str());
            if identity.is_empty() {
                continue;
            }
            match stream_identities
                .iter()
                .position(|existing| existing == &identity)
            {
                Some(index) => {
                    stream_identity_groups[index].push((volume_index, node.clone()));
                }
                None => {
                    stream_identities.push(identity);
                    stream_identity_groups.push(vec![(volume_index, node.clone())]);
                }
            }
        }
    }

    /// Appends valid companion file name candidates from a directory listing.
    fn append_listed_candidates(
        listed_file_names: Vec<String>,
        directory: &str,
        base_file_name: &str,
        primary_file_name: &str,
        has_stored_volume_uris: bool,
        is_base_directory: bool,
        candidate_names: &mut Vec<String>,
    ) {
        for listed_file_name in listed_file_names {
            let candidate_name: String = if directory.is_empty() {
                listed_file_name.clone()
            } else {
                format!("{}/{}", directory, listed_file_name)
            };
            if Self::is_valid_listed_candidate(
                listed_file_name.as_str(),
                candidate_name.as_str(),
                base_file_name,
                primary_file_name,
                has_stored_volume_uris,
                is_base_directory,
            ) && !candidate_names.contains(&candidate_name)
            {
                candidate_names.push(candidate_name);
            }
        }
    }

    /// Appends unique string items into `target` while preserving order.
    fn append_unique_strings(target: &mut Vec<String>, items: Vec<String>) {
        for item in items {
            if !target.contains(&item) {
                target.push(item);
            }
        }
    }

    /// Builds the directory paths to scan for companion volumes.
    fn build_directory_paths(directory: &str) -> Vec<Vec<PathComponent>> {
        let mut directory_paths: Vec<Vec<PathComponent>> = vec![Vec::new()];

        if !directory.is_empty() {
            let directory_path: Vec<PathComponent> = directory
                .split('/')
                .filter(|directory_part| !directory_part.is_empty())
                .map(PathComponent::from)
                .collect();
            if !directory_path.is_empty() {
                directory_paths.push(directory_path);
            }
        }
        directory_paths
    }

    /// Collects companion volume candidate file names from directory listings.
    fn collect_listed_companion_candidates(
        file_resolver: &FileResolverReference,
        directory: &str,
        base_file_name: &str,
        primary_file_name: &str,
        has_stored_volume_uris: bool,
        candidate_names: &mut Vec<String>,
    ) {
        let directory_paths: Vec<Vec<PathComponent>> = Self::build_directory_paths(directory);

        for directory_path in directory_paths.iter() {
            // A directory listing is not required; only the named companion
            // containers are resolved when it is not available.
            let Ok(listed_file_names) = file_resolver.list_file_names(directory_path.as_slice())
            else {
                continue;
            };
            Self::append_listed_candidates(
                listed_file_names,
                directory,
                base_file_name,
                primary_file_name,
                has_stored_volume_uris,
                directory_path.is_empty(),
                candidate_names,
            );
        }
    }

    /// Determines the media size from the image node, map, or streams.
    fn determine_media_size(
        image_node: Option<&Aff4Node>,
        map: Option<&ResolvedMap>,
        streams: &[ResolvedStream],
    ) -> u64 {
        if let Some(media_size) = image_node.and_then(|node| node.get_u64_property("size")) {
            return media_size;
        }
        if let Some(map) = map {
            return map
                .entries
                .iter()
                .map(|entry| entry.mapped_offset + entry.length)
                .max()
                .unwrap_or(0);
        }
        let stream_size: u64 = streams.iter().map(|stream| stream.size).max().unwrap_or(0);
        if stream_size > 0 {
            stream_size
        } else {
            streams[0].get_number_of_chunks() as u64 * streams[0].chunk_size as u64
        }
    }

    /// Extracts merged stream metadata and candidate names from a stream node group.
    fn extract_stream_metadata(
        group: &[(usize, Aff4Node)],
    ) -> (usize, usize, CompressionMethod, u64, Vec<String>) {
        let chunk_size: usize = group
            .iter()
            .filter_map(|(_volume_index, node)| node.get_u64_property("chunkSize"))
            .find(|&value| value > 0)
            .unwrap_or(0) as usize;
        let chunks_in_segment: usize = group
            .iter()
            .filter_map(|(_volume_index, node)| node.get_u64_property("chunksInSegment"))
            .find(|&value| value > 0)
            .map_or(AFF4_STANDARD_DEFAULT_CHUNKS_IN_SEGMENT, |value| {
                value as usize
            });
        let compression_method: CompressionMethod = group
            .iter()
            .filter_map(|(_volume_index, node)| node.get_property_value("compressionMethod"))
            .next()
            .map_or(CompressionMethod::Stored, CompressionMethod::from_iri);
        let size: u64 = group
            .iter()
            .filter_map(|(_volume_index, node)| node.get_u64_property("size"))
            .max()
            .unwrap_or(0);

        let mut name_candidates: Vec<String> = Vec::new();
        for (_volume_index, node) in group.iter() {
            Self::append_unique_strings(
                &mut name_candidates,
                Self::stream_name_candidates(node.subject.as_str()),
            );
        }
        (
            chunk_size,
            chunks_in_segment,
            compression_method,
            size,
            name_candidates,
        )
    }

    /// Finds the preferred `Image` node across all loaded volumes.
    fn find_image_node(volumes: &[DataVolume]) -> Option<&Aff4Node> {
        let mut fallback_image_node: Option<&Aff4Node> = None;

        for node in volumes
            .iter()
            .flat_map(|volume| volume.information.nodes.iter())
            .filter(|node| node.is_type("Image"))
        {
            if node.get_u64_property("size").is_some() {
                return Some(node);
            }
            if fallback_image_node.is_none() {
                fallback_image_node = Some(node);
            }
        }
        fallback_image_node
    }

    /// Finds the volume and `Map` node with the specified subject IRI.
    fn find_map_node<'a>(
        volumes: &'a [DataVolume],
        map_iri: &str,
    ) -> Option<(&'a DataVolume, &'a Aff4Node)> {
        volumes.iter().find_map(|volume| {
            volume
                .information
                .nodes
                .iter()
                .find(|node| node.is_type("Map") && node.subject == map_iri)
                .map(|node| (volume, node))
        })
    }

    /// Searches loaded volumes for the bevy segments of a single stream name candidate.
    fn find_stream_for_candidate<'a>(
        volumes: &'a [DataVolume],
        name_candidate: &str,
        chunks_in_segment: usize,
        expected_number_of_chunks: usize,
    ) -> Option<(&'a DataVolume, Vec<ResolvedSegment>)> {
        for volume in volumes.iter() {
            let Some(entry_prefix) =
                volume.get_entry_prefix_for_node_name(name_candidate, "00000000", Some(".index"))
            else {
                continue;
            };
            let segments: Vec<ResolvedSegment> = Self::read_volume_stream_segments(
                volume,
                entry_prefix.as_str(),
                chunks_in_segment,
                expected_number_of_chunks,
            );
            if !segments.is_empty() {
                return Some((volume, segments));
            }
        }
        None
    }

    /// Searches loaded volumes for the bevy segments of any of the stream name candidates.
    fn find_stream_in_volumes<'a>(
        volumes: &'a [DataVolume],
        name_candidates: &[String],
        chunks_in_segment: usize,
        expected_number_of_chunks: usize,
    ) -> Option<(&'a DataVolume, Vec<ResolvedSegment>)> {
        for name_candidate in name_candidates.iter() {
            if let Some(found) = Self::find_stream_for_candidate(
                volumes,
                name_candidate.as_str(),
                chunks_in_segment,
                expected_number_of_chunks,
            ) {
                return Some(found);
            }
        }
        None
    }

    /// Retrieves the bytes per sector.
    pub fn get_bytes_per_sector(&self) -> u32 {
        self.bytes_per_sector
    }

    /// Retrieves the name of the image.
    pub fn get_image_name(&self) -> &str {
        &self.image_name
    }

    /// Retrieves the media size in bytes.
    pub fn get_media_size(&self) -> u64 {
        self.media_size
    }

    /// Groups all `ImageStream` nodes across the loaded volumes by stream identity.
    fn group_image_stream_nodes(
        volumes: &[DataVolume],
    ) -> (Vec<String>, Vec<Vec<(usize, Aff4Node)>>) {
        let mut stream_identities: Vec<String> = Vec::new();
        let mut stream_identity_groups: Vec<Vec<(usize, Aff4Node)>> = Vec::new();

        for (volume_index, volume) in volumes.iter().enumerate() {
            Self::add_volume_stream_nodes(
                volume_index,
                volume,
                &mut stream_identities,
                &mut stream_identity_groups,
            );
        }
        (stream_identities, stream_identity_groups)
    }

    /// Determines if a lowercased target IRI represents a zero-filled gap.
    fn is_gap_target(lowercased: &str) -> bool {
        matches!(lowercased, "zero" | "aff4:zero" | "aff4://zero")
            || lowercased.ends_with("#zero")
            || lowercased.ends_with("#unknowndata")
            || lowercased.ends_with("#unreadabledata")
    }

    /// Determines if the media is read through a map.
    pub fn is_mapped(&self) -> bool {
        self.map.is_some()
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

    /// Determines if a listed file name is a valid companion container candidate.
    fn is_valid_listed_candidate(
        listed_file_name: &str,
        candidate_name: &str,
        base_file_name: &str,
        primary_file_name: &str,
        has_stored_volume_uris: bool,
        is_base_directory: bool,
    ) -> bool {
        let lowercased: String = listed_file_name.to_ascii_lowercase();
        if !lowercased.ends_with(".aff4") && !lowercased.ends_with(".af4") {
            return false;
        }
        if listed_file_name == base_file_name && is_base_directory {
            return false;
        }
        // Without a volume reference a listed container is only a split
        // companion candidate.
        has_stored_volume_uris || Self::is_split_companion_name(candidate_name, primary_file_name)
    }

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
        let primary_volume: DataVolume = match DataVolume::open(file_resolver, file_name) {
            Ok(primary_volume) => primary_volume,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(
                    error,
                    format!("Unable to open primary volume: {}", file_name)
                );
                return Err(error);
            }
        };
        if !primary_volume.information.is_standard() {
            return Err(keramics_core::error_trace_new!(format!(
                "Volume: {} does not contain standard AFF4 nodes",
                primary_volume.file_name
            )));
        }
        volumes.push(primary_volume);
        match Self::add_companion_volumes(file_resolver, &mut volumes) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to add companion volumes");
                return Err(error);
            }
        }
        let (stream_identities, stream_identity_groups): (
            Vec<String>,
            Vec<Vec<(usize, Aff4Node)>>,
        ) = Self::group_image_stream_nodes(&volumes);

        if stream_identity_groups.is_empty() {
            return Err(keramics_core::error_trace_new!(
                "Missing data stream node in information container"
            ));
        }
        // Resolve the data streams; those without data entries in any of the
        // volumes are only kept when required by the map.
        let streams: Vec<ResolvedStream> = stream_identity_groups
            .iter()
            .enumerate()
            .filter_map(|(index, group)| {
                Self::resolve_stream(&volumes, stream_identities[index].as_str(), group)
            })
            .collect();
        if streams.is_empty() {
            return Err(keramics_core::error_trace_new!(
                "Missing data entry for image stream in volume"
            ));
        }
        let image_node: Option<&Aff4Node> = Self::find_image_node(&volumes);
        let bytes_per_sector: u32 = image_node
            .and_then(|node| node.get_u64_property("blockSize"))
            .map_or(AFF4_DEFAULT_BYTES_PER_SECTOR, |block_size| {
                block_size as u32
            });
        let map: Option<ResolvedMap> = Self::resolve_optional_map(&volumes, &streams, image_node)?;
        let media_size: u64 = Self::determine_media_size(image_node, map.as_ref(), &streams);
        let image_name: String = match image_node {
            Some(node) if node.subject != ":" => node.subject.clone(),
            _ => streams[0].name.clone(),
        };
        Ok(Aff4StandardMedia {
            streams,
            map,
            media_size,
            image_name,
            bytes_per_sector,
        })
    }

    /// Parses bevy index entries from raw index data.
    fn parse_bevy_index_entries(index_data: &[u8]) -> Option<Vec<BevyIndexEntry>> {
        if index_data.is_empty()
            || !index_data
                .len()
                .is_multiple_of(AFF4_STANDARD_BEVY_INDEX_ENTRY_SIZE)
        {
            return None;
        }
        let mut index_entries: Vec<BevyIndexEntry> = Vec::new();

        for chunk in index_data.chunks_exact(AFF4_STANDARD_BEVY_INDEX_ENTRY_SIZE) {
            let bevy_offset: u64 = u64::from_le_bytes(chunk[0..8].try_into().unwrap());
            let stored_size: u32 = u32::from_le_bytes(chunk[8..12].try_into().unwrap());
            index_entries.push(BevyIndexEntry {
                bevy_offset,
                stored_size,
            });
        }
        Some(index_entries)
    }

    /// Parses and sorts resolved map entries from raw binary map data.
    fn parse_map_entries(
        map_data: &[u8],
        target_identities: &[String],
        stream_identities: &[String],
    ) -> Result<Vec<ResolvedMapEntry>, ErrorTrace> {
        if !map_data.len().is_multiple_of(AFF4_STANDARD_MAP_ENTRY_SIZE) {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported map data size: {}",
                map_data.len()
            )));
        }
        let mut entries: Vec<ResolvedMapEntry> = Vec::new();

        for chunk in map_data.chunks_exact(AFF4_STANDARD_MAP_ENTRY_SIZE) {
            let mapped_offset: u64 = u64::from_le_bytes(chunk[0..8].try_into().unwrap());
            let length: u64 = u64::from_le_bytes(chunk[8..16].try_into().unwrap());
            let target_offset: u64 = u64::from_le_bytes(chunk[16..24].try_into().unwrap());
            let target_index: i32 = i32::from_le_bytes(chunk[24..28].try_into().unwrap());
            let target: MapTarget = target_identities
                .get(target_index as usize)
                .and_then(|identity| Self::resolve_map_target(identity.as_str(), stream_identities))
                .unwrap_or(MapTarget::Gap);
            entries.push(ResolvedMapEntry {
                mapped_offset,
                length,
                target_offset,
                target,
            });
        }
        entries.sort_by_key(|entry| entry.mapped_offset);
        Ok(entries)
    }

    /// Reads and parses the map target identities table (`<prefix>/idx`).
    fn parse_map_target_identities(
        map_volume: &DataVolume,
        entry_prefix: &str,
    ) -> Result<Vec<String>, ErrorTrace> {
        let index_data: Vec<u8> = match map_volume.read_entry_data(&format!("{}/idx", entry_prefix))
        {
            Ok(index_data) => index_data,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read map index entry data");
                return Err(error);
            }
        };
        match String::from_utf8(index_data) {
            Ok(index_data) => Ok(index_data
                .split(['\n', '\r', '\0'])
                .map(|target_identity| target_identity.trim().to_string())
                .filter(|target_identity| !target_identity.is_empty())
                .collect()),
            Err(error) => Err(keramics_core::error_trace_new!(format!(
                "Unsupported encoding of map target identities with error: {}",
                error
            ))),
        }
    }

    /// Reads a single bevy segment (data entry name and parsed index entries) from a volume.
    fn read_bevy_segment(
        volume: &DataVolume,
        entry_prefix: &str,
        first_chunk_index: usize,
    ) -> Option<ResolvedSegment> {
        let segment: String = format!("{:08x}", first_chunk_index);
        let segment_data_entry_name: String = format!("{}/{}", entry_prefix, segment);
        let segment_index_entry_name: String = format!("{}/{}.index", entry_prefix, segment);

        if !volume.has_entry(segment_data_entry_name.as_str())
            || !volume.has_entry(segment_index_entry_name.as_str())
        {
            return None;
        }
        let index_data: Vec<u8> = volume
            .read_entry_data(segment_index_entry_name.as_str())
            .ok()?;
        let index_entries: Vec<BevyIndexEntry> = Self::parse_bevy_index_entries(&index_data)?;

        Some(ResolvedSegment {
            first_chunk_index,
            data_entry_name: segment_data_entry_name,
            index_entries,
        })
    }

    /// Reads all consecutive bevy segments for a stream from a volume.
    fn read_volume_stream_segments(
        volume: &DataVolume,
        entry_prefix: &str,
        chunks_in_segment: usize,
        expected_number_of_chunks: usize,
    ) -> Vec<ResolvedSegment> {
        let mut segments: Vec<ResolvedSegment> = Vec::new();
        let mut total_chunks: usize = 0;

        for segments_read in 0..AFF4_STANDARD_MAXIMUM_NUMBER_OF_SEGMENTS {
            let first_chunk_index: usize = segments_read * chunks_in_segment;
            let Some(segment) = Self::read_bevy_segment(volume, entry_prefix, first_chunk_index)
            else {
                break;
            };
            total_chunks += segment.index_entries.len();
            segments.push(segment);

            if expected_number_of_chunks > 0 && total_chunks >= expected_number_of_chunks {
                break;
            }
        }
        segments
    }

    /// Resolves the map node referenced by the image data stream.
    fn resolve_map(
        volumes: &[DataVolume],
        streams: &[ResolvedStream],
        map_iri: &str,
    ) -> Result<ResolvedMap, ErrorTrace> {
        let (map_volume, map_node): (&DataVolume, &Aff4Node) =
            match Self::find_map_node(volumes, map_iri) {
                Some(result) => result,
                None => {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Missing map node: {} in volume",
                        map_iri
                    )));
                }
            };
        // The map target table is stored as a sibling entry of the map data
        // entry (`<prefix>/map` and `<prefix>/idx`).
        let entry_prefix: String =
            match map_volume.get_entry_prefix_for_node_name(map_iri, "map", None) {
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
        let target_identities: Vec<String> =
            Self::parse_map_target_identities(map_volume, entry_prefix.as_str())?;
        let stream_identities: Vec<String> =
            streams.iter().map(|stream| stream.name.clone()).collect();

        // Resolve the default gap target.
        let gap_iri: &str = map_node
            .get_property_value("mapGapDefaultStream")
            .unwrap_or("aff4:Zero");
        let gap_default: MapTarget =
            Self::resolve_map_target(gap_iri, &stream_identities).unwrap_or(MapTarget::Gap);

        // Read and resolve the map entries.
        let map_data: Vec<u8> = match map_volume.read_entry_data(&format!("{}/map", entry_prefix)) {
            Ok(map_data) => map_data,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read map entry data");
                return Err(error);
            }
        };
        let entries: Vec<ResolvedMapEntry> =
            Self::parse_map_entries(&map_data, &target_identities, &stream_identities)?;

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

        if Self::is_gap_target(lowercased.as_str()) {
            return Some(MapTarget::Gap);
        }
        let symbolic_byte: Option<u8> = lowercased.find("symbolicstream").and_then(|index| {
            let symbolic_value: &str = lowercased[(index + "symbolicstream".len())..].trim();
            u8::from_str_radix(symbolic_value, 16).ok()
        });
        if let Some(byte) = symbolic_byte {
            return Some(MapTarget::Symbolic(byte));
        }
        let target_identity: String = Self::stream_identity(target);
        if target_identity.is_empty() {
            return None;
        }
        stream_identities
            .iter()
            .position(|stream_identity| stream_identity == &target_identity)
            .map(MapTarget::Stream)
    }

    /// Resolves the optional map referenced by an image node, returning an error if a declared map fails to resolve.
    fn resolve_optional_map(
        volumes: &[DataVolume],
        streams: &[ResolvedStream],
        image_node: Option<&Aff4Node>,
    ) -> Result<Option<ResolvedMap>, ErrorTrace> {
        let Some(map_iri) = image_node.and_then(|node| node.get_property_value("dataStream"))
        else {
            return Ok(None);
        };
        match Self::resolve_map(volumes, streams, map_iri) {
            Ok(map) => Ok(Some(map)),
            Err(error) => {
                // When the data stream reference points to a map node the map
                // is required; reading the media data directly from the data
                // streams would silently deliver the wrong data.
                if Self::find_map_node(volumes, map_iri).is_some() {
                    return Err(error);
                }
                Ok(None)
            }
        }
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
        let (chunk_size, chunks_in_segment, compression_method, size, name_candidates): (
            usize,
            usize,
            CompressionMethod,
            u64,
            Vec<String>,
        ) = Self::extract_stream_metadata(group);

        if chunk_size == 0 || name_candidates.is_empty() {
            return None;
        }
        let expected_number_of_chunks: usize = if size > 0 {
            (size / chunk_size as u64) as usize
        } else {
            0
        };
        let (volume, segments): (&DataVolume, Vec<ResolvedSegment>) = Self::find_stream_in_volumes(
            volumes,
            &name_candidates,
            chunks_in_segment,
            expected_number_of_chunks,
        )?;

        Some(ResolvedStream {
            name: identity.to_string(),
            chunk_size,
            compression_method,
            size,
            zip_file: volume.zip_file.clone(),
            segments,
        })
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
        let Some(rest) = subject.strip_prefix("aff4://") else {
            return subject.to_string();
        };
        if let Some((prefix, tail)) = rest.split_once('/')
            && prefix.len() == 36
        {
            return tail.to_string();
        }
        rest.to_string()
    }

    /// Retrieves the candidate names of a data stream node.
    fn stream_name_candidates(subject: &str) -> Vec<String> {
        let mut candidates: Vec<String> = Vec::new();
        let subject: &str = subject.trim();

        if subject.is_empty() || subject == ":" {
            return candidates;
        }
        candidates.push(subject.to_string());
        let Some(rest) = subject.strip_prefix("aff4://") else {
            return candidates;
        };
        let rest_string: String = rest.to_string();
        if !candidates.contains(&rest_string) {
            candidates.push(rest_string);
        }
        let Some((prefix, tail)) = rest.split_once('/') else {
            return candidates;
        };
        let tail_string: String = tail.to_string();
        if prefix.len() == 36 && !candidates.contains(&tail_string) {
            candidates.push(tail_string);
        }
        candidates
    }

    /// Attempts to open a candidate container and returns it if it is a valid companion volume.
    fn try_open_companion_volume(
        file_resolver: &FileResolverReference,
        candidate_name: &str,
        primary_file_name: &str,
        stored_volume_uris: &[String],
    ) -> Option<DataVolume> {
        let path_component: PathComponent = PathComponent::from(candidate_name);
        let volume: DataVolume = DataVolume::open(file_resolver, &path_component).ok()?;

        if !volume.information.is_standard() {
            return None;
        }
        if Self::is_split_companion_name(candidate_name, primary_file_name)
            || stored_volume_uris.contains(&volume.volume_uri)
        {
            return Some(volume);
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

    /// Decompresses a compressed chunk according to its compression method.
    fn decompress_chunk(
        compression_method: CompressionMethod,
        compressed_data: &[u8],
        chunk: &mut [u8],
        stream_name: &str,
        chunk_index: usize,
        stored_size: u32,
    ) -> Result<(), ErrorTrace> {
        match compression_method {
            CompressionMethod::Snappy => {
                let mut snappy_context: SnappyContext = SnappyContext::new();
                match snappy_context.decompress(compressed_data, chunk) {
                    Ok(_) => Ok(()),
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(
                            error,
                            format!(
                                "Unable to decompress Snappy chunk: {} of data stream: {}",
                                chunk_index, stream_name
                            )
                        );
                        Err(error)
                    }
                }
            }
            CompressionMethod::Zlib => {
                _ = crate::zlib_decompress!(
                    compressed_data,
                    chunk,
                    format!(
                        "Unable to decompress Zlib chunk: {} of data stream: {}",
                        chunk_index, stream_name
                    )
                );
                Ok(())
            }
            CompressionMethod::Deflate => {
                _ = crate::deflate_decompress!(
                    compressed_data,
                    chunk,
                    format!(
                        "Unable to decompress Deflate chunk: {} of data stream: {}",
                        chunk_index, stream_name
                    )
                );
                Ok(())
            }
            CompressionMethod::Stored => Err(keramics_core::error_trace_new!(format!(
                "Unsupported stored size: {} for chunk: {} of data stream: {}",
                stored_size, chunk_index, stream_name
            ))),
        }
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
            bevy_offset,
            stored_size,
        ): (String, usize, CompressionMethod, usize, u64, u32) =
            self.get_chunk_metadata(stream_index, chunk_index)?;
        let mut chunk: Vec<u8> = vec![0; chunk_size];

        if stored_size == 0 {
            // A stored size of 0 indicates a chunk consisting only of zero bytes.
            self.chunk_cache.insert(cache_key, chunk.clone());
            return Ok(chunk);
        }
        let bevy_data: Vec<u8> = match self.get_bevy_data(stream_index, segment_index) {
            Ok(bevy_data) => bevy_data,
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to retrieve bevy data");
                return Err(error);
            }
        };
        let bevy_data_offset: usize = bevy_offset as usize;
        let bevy_data_end_offset: usize = bevy_data_offset + stored_size as usize;

        if bevy_data_end_offset > bevy_data.len() {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported bevy offset: {} for chunk: {} of data stream: {}",
                bevy_offset, chunk_index, stream_name
            )));
        }
        let stored_data: &[u8] = &bevy_data[bevy_data_offset..bevy_data_end_offset];

        if stored_size as usize == chunk_size {
            // A stored size equal to the chunk size indicates a chunk that is
            // stored without compression.
            chunk.copy_from_slice(stored_data);
        } else {
            Self::decompress_chunk(
                compression_method,
                stored_data,
                &mut chunk,
                stream_name.as_str(),
                chunk_index,
                stored_size,
            )?;
        }
        self.chunk_cache.insert(cache_key, chunk.clone());

        Ok(chunk)
    }

    /// Retrieves metadata for the specified stream chunk.
    fn get_chunk_metadata(
        &self,
        stream_index: usize,
        chunk_index: usize,
    ) -> Result<(String, usize, CompressionMethod, usize, u64, u32), ErrorTrace> {
        let stream: &ResolvedStream = match self.media.streams.get(stream_index) {
            Some(stream) => stream,
            None => {
                return Err(keramics_core::error_trace_new!(format!(
                    "Missing data stream with index: {}",
                    stream_index
                )));
            }
        };
        let (segment_index, _segment, index_entry): (usize, &ResolvedSegment, &BevyIndexEntry) =
            match stream.get_segment_for_chunk(chunk_index) {
                Some(segment) => segment,
                None => {
                    return Err(keramics_core::error_trace_new!(format!(
                        "Missing chunk: {} of data stream: {}",
                        chunk_index, stream.name
                    )));
                }
            };
        Ok((
            stream.name.clone(),
            stream.chunk_size,
            stream.compression_method,
            segment_index,
            index_entry.bevy_offset,
            index_entry.stored_size,
        ))
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
                    let target_position: u64 = entry.target_offset + offset_in_entry as u64;
                    self.read_target_data(
                        entry.target,
                        target_position,
                        &mut data[offset_in_read..(offset_in_read + available_length)],
                    )?;
                    offset_in_read += available_length;
                    position += available_length as u64;
                }
                None => {
                    // The offset is not covered by a map entry; the region is
                    // filled with the default gap target of the map.
                    self.read_target_data(
                        map.gap_default,
                        position,
                        &mut data[offset_in_read..read_size],
                    )?;
                    offset_in_read = read_size;
                }
            }
        }
        Ok(read_size)
    }

    /// Reads data from a map target (gap, symbolic fill byte, or data stream) into `buffer`.
    fn read_target_data(
        &mut self,
        target: MapTarget,
        target_position: u64,
        buffer: &mut [u8],
    ) -> Result<(), ErrorTrace> {
        match target {
            MapTarget::Gap => {
                buffer.fill(0);
            }
            MapTarget::Symbolic(fill_byte) => {
                buffer.fill(fill_byte);
            }
            MapTarget::Stream(stream_index) => {
                match self.read_stream_data(stream_index, target_position, buffer) {
                    Ok(_) => {}
                    Err(mut error) => {
                        keramics_core::error_trace_add_frame!(error, "Unable to read stream data");
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
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
            let chunk: Vec<u8> = match self.get_chunk(stream_index, chunk_index) {
                Ok(chunk) => chunk,
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(
                        error,
                        format!("Unable to retrieve chunk: {}", chunk_index)
                    );
                    return Err(error);
                }
            };
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
        match self.read_stream_data(0, offset, &mut data[..read_size]) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read stream data");
                return Err(error);
            }
        }

        Ok(read_size)
    }
}

/// Advanced Forensic Format 4 (AFF4) Standard media block stream.
pub type Aff4StandardBlockStream = crate::block_stream::BlockStream<Aff4StandardReader>;

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use keramics_checksums::ReversedCrc32Context;
    use keramics_core::{DataStreamReference, open_fake_data_stream};

    use crate::file_resolver::FileResolver;

    struct MemoryFileResolver {
        files: HashMap<String, Vec<u8>>,
    }

    impl MemoryFileResolver {
        fn new(files: Vec<(&str, Vec<u8>)>) -> Self {
            let mut file_map: HashMap<String, Vec<u8>> = HashMap::new();
            for (name, data) in files {
                file_map.insert(name.to_string(), data);
            }
            Self { files: file_map }
        }
    }

    impl FileResolver for MemoryFileResolver {
        fn get_data_stream(
            &self,
            path_components: &[PathComponent],
        ) -> Result<Option<DataStreamReference>, ErrorTrace> {
            let path: String = path_components
                .iter()
                .map(|component| component.to_string())
                .collect::<Vec<String>>()
                .join("/");
            match self.files.get(&path) {
                Some(data) => Ok(Some(open_fake_data_stream(data))),
                None => Ok(None),
            }
        }

        fn list_file_names(
            &self,
            path_components: &[PathComponent],
        ) -> Result<Vec<String>, ErrorTrace> {
            let prefix: String = if path_components.is_empty() {
                String::new()
            } else {
                format!(
                    "{}/",
                    path_components
                        .iter()
                        .map(|component| component.to_string())
                        .collect::<Vec<String>>()
                        .join("/")
                )
            };
            let mut file_names: Vec<String> = Vec::new();
            for key in self.files.keys() {
                let Some(rest) = key.strip_prefix(&prefix) else {
                    continue;
                };
                if !rest.contains('/') {
                    file_names.push(rest.to_string());
                }
            }
            file_names.sort();
            Ok(file_names)
        }
    }

    fn build_bevy_index(entries: &[(u64, u32)]) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        for (bevy_offset, stored_size) in entries {
            data.extend_from_slice(&bevy_offset.to_le_bytes());
            data.extend_from_slice(&stored_size.to_le_bytes());
        }
        data
    }

    fn build_map_entries(entries: &[(u64, u64, u64, i32)]) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        for (mapped_offset, length, target_offset, target_index) in entries {
            data.extend_from_slice(&mapped_offset.to_le_bytes());
            data.extend_from_slice(&length.to_le_bytes());
            data.extend_from_slice(&target_offset.to_le_bytes());
            data.extend_from_slice(&target_index.to_le_bytes());
        }
        data
    }

    fn build_test_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut archive: Vec<u8> = Vec::new();
        let mut central_directory: Vec<u8> = Vec::new();

        for (file_name, data) in entries {
            let local_header_offset: u32 = archive.len() as u32;
            let file_name_bytes: &[u8] = file_name.as_bytes();
            let mut crc32_context: ReversedCrc32Context = ReversedCrc32Context::new(0xedb88320, 0);
            crc32_context.update(data);
            let crc_32: u32 = crc32_context.finalize();

            archive.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
            archive.extend_from_slice(&20_u16.to_le_bytes());
            archive.extend_from_slice(&0_u16.to_le_bytes());
            archive.extend_from_slice(&0_u16.to_le_bytes());
            archive.extend_from_slice(&0_u16.to_le_bytes());
            archive.extend_from_slice(&0_u16.to_le_bytes());
            archive.extend_from_slice(&crc_32.to_le_bytes());
            archive.extend_from_slice(&(data.len() as u32).to_le_bytes());
            archive.extend_from_slice(&(data.len() as u32).to_le_bytes());
            archive.extend_from_slice(&(file_name_bytes.len() as u16).to_le_bytes());
            archive.extend_from_slice(&0_u16.to_le_bytes());
            archive.extend_from_slice(file_name_bytes);
            archive.extend_from_slice(data);

            central_directory.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
            central_directory.extend_from_slice(&20_u16.to_le_bytes());
            central_directory.extend_from_slice(&20_u16.to_le_bytes());
            central_directory.extend_from_slice(&0_u16.to_le_bytes());
            central_directory.extend_from_slice(&0_u16.to_le_bytes());
            central_directory.extend_from_slice(&0_u16.to_le_bytes());
            central_directory.extend_from_slice(&0_u16.to_le_bytes());
            central_directory.extend_from_slice(&crc_32.to_le_bytes());
            central_directory.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central_directory.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central_directory.extend_from_slice(&(file_name_bytes.len() as u16).to_le_bytes());
            central_directory.extend_from_slice(&0_u16.to_le_bytes());
            central_directory.extend_from_slice(&0_u16.to_le_bytes());
            central_directory.extend_from_slice(&0_u16.to_le_bytes());
            central_directory.extend_from_slice(&0_u16.to_le_bytes());
            central_directory.extend_from_slice(&0_u32.to_le_bytes());
            central_directory.extend_from_slice(&local_header_offset.to_le_bytes());
            central_directory.extend_from_slice(file_name_bytes);
        }
        let central_directory_offset: u32 = archive.len() as u32;
        let central_directory_size: u32 = central_directory.len() as u32;
        archive.extend_from_slice(&central_directory);

        archive.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        archive.extend_from_slice(&0_u16.to_le_bytes());
        archive.extend_from_slice(&0_u16.to_le_bytes());
        archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        archive.extend_from_slice(&central_directory_size.to_le_bytes());
        archive.extend_from_slice(&central_directory_offset.to_le_bytes());
        archive.extend_from_slice(&0_u16.to_le_bytes());

        archive
    }

    fn get_deflate_chunk_16(fill_byte: u8) -> Vec<u8> {
        let mut data: Vec<u8> = vec![0x01, 0x10, 0x00, 0xef, 0xff];
        data.extend_from_slice(&[fill_byte; 16]);
        data
    }

    fn get_snappy_chunk_16(fill_byte: u8) -> Vec<u8> {
        // Uncompressed size = 16, 1-byte literal of fill_byte, copy2 of 15 bytes at offset 1.
        vec![0x10, 0x00, fill_byte, 0x3a, 0x01, 0x00]
    }

    fn get_zlib_chunk_16_0x44() -> Vec<u8> {
        let mut data: Vec<u8> = vec![0x78, 0x01];
        data.extend_from_slice(&get_deflate_chunk_16(0x44));
        // Adler-32 of 16 bytes of 0x44 (s1 = 0x0441, s2 = 0x2430).
        data.extend_from_slice(&[0x24, 0x30, 0x04, 0x41]);
        data
    }

    #[test]
    fn test_compression_method_from_iri() {
        assert_eq!(
            CompressionMethod::from_iri("http://code.google.com/p/snappy/"),
            CompressionMethod::Snappy
        );
        assert_eq!(
            CompressionMethod::from_iri("https://www.ietf.org/rfc/rfc1950.txt"),
            CompressionMethod::Zlib
        );
        assert_eq!(
            CompressionMethod::from_iri("http://aff4.org/Schema#zlib"),
            CompressionMethod::Zlib
        );
        assert_eq!(
            CompressionMethod::from_iri("https://www.ietf.org/rfc/rfc1951.txt"),
            CompressionMethod::Deflate
        );
        assert_eq!(
            CompressionMethod::from_iri("http://aff4.org/Schema#deflate"),
            CompressionMethod::Deflate
        );
        assert_eq!(
            CompressionMethod::from_iri("http://aff4.org/Schema#NullCompressor"),
            CompressionMethod::Stored
        );
    }

    #[test]
    fn test_is_split_companion_name() {
        assert!(Aff4StandardMedia::is_split_companion_name(
            "split.aff4.A01",
            "split.aff4"
        ));
        assert!(Aff4StandardMedia::is_split_companion_name(
            "split.aff4.A32",
            "split.aff4"
        ));
        assert!(!Aff4StandardMedia::is_split_companion_name(
            "split.aff4.A00",
            "split.aff4"
        ));
        assert!(!Aff4StandardMedia::is_split_companion_name(
            "split.aff4.A33",
            "split.aff4"
        ));
        assert!(!Aff4StandardMedia::is_split_companion_name(
            "split.aff4.Axx",
            "split.aff4"
        ));
        assert!(!Aff4StandardMedia::is_split_companion_name(
            "other.aff4.A01",
            "split.aff4"
        ));
    }

    #[test]
    fn test_stream_identity_and_name_candidates() {
        assert_eq!(Aff4StandardMedia::stream_identity(""), "");
        assert_eq!(Aff4StandardMedia::stream_identity(":"), "");
        assert_eq!(
            Aff4StandardMedia::stream_identity(
                "aff4://00000000-0000-1003-8000-000000000001/stream.raw"
            ),
            "stream.raw"
        );
        assert_eq!(
            Aff4StandardMedia::stream_identity("aff4://00000000-0000-1003-8000-000000000001"),
            "00000000-0000-1003-8000-000000000001"
        );
        assert_eq!(
            Aff4StandardMedia::stream_identity("/local/stream"),
            "/local/stream"
        );

        assert!(Aff4StandardMedia::stream_name_candidates("").is_empty());
        assert!(Aff4StandardMedia::stream_name_candidates(":").is_empty());
        let candidates: Vec<String> = Aff4StandardMedia::stream_name_candidates(
            "aff4://00000000-0000-1003-8000-000000000001/stream.raw",
        );
        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[2], "stream.raw");
    }

    #[test]
    fn test_resolve_map_target() {
        let stream_identities: Vec<String> =
            vec!["00000000-0000-1003-8000-000000000001".to_string()];

        assert_eq!(
            Aff4StandardMedia::resolve_map_target("Zero", &stream_identities),
            Some(MapTarget::Gap)
        );
        assert_eq!(
            Aff4StandardMedia::resolve_map_target("aff4:Zero", &stream_identities),
            Some(MapTarget::Gap)
        );
        assert_eq!(
            Aff4StandardMedia::resolve_map_target("aff4://Zero", &stream_identities),
            Some(MapTarget::Gap)
        );
        assert_eq!(
            Aff4StandardMedia::resolve_map_target(
                "http://aff4.org/Schema#UnknownData",
                &stream_identities
            ),
            Some(MapTarget::Gap)
        );
        assert_eq!(
            Aff4StandardMedia::resolve_map_target(
                "http://aff4.org/Schema#UnreadableData",
                &stream_identities
            ),
            Some(MapTarget::Gap)
        );
        assert_eq!(
            Aff4StandardMedia::resolve_map_target(
                "http://aff4.org/Schema#SymbolicStreamAA",
                &stream_identities
            ),
            Some(MapTarget::Symbolic(0xaa))
        );
        assert_eq!(
            Aff4StandardMedia::resolve_map_target(
                "http://aff4.org/Schema#SymbolicStreamZZ",
                &stream_identities
            ),
            None
        );
        assert_eq!(
            Aff4StandardMedia::resolve_map_target(
                "aff4://00000000-0000-1003-8000-000000000001",
                &stream_identities
            ),
            Some(MapTarget::Stream(0))
        );
        assert_eq!(
            Aff4StandardMedia::resolve_map_target("aff4://unknown", &stream_identities),
            None
        );
    }

    #[test]
    fn test_data_volume_helpers() -> Result<(), ErrorTrace> {
        assert_eq!(
            DataVolume::percent_encode("aff4://a/b", false),
            "aff4%3A%2F%2Fa%2Fb"
        );
        assert_eq!(
            DataVolume::percent_encode("file:/tmp/a.raw", true),
            "file%3A/tmp/a.raw"
        );

        let candidates: Vec<String> = DataVolume::entry_prefix_candidates("aff4://a/b");
        assert_eq!(candidates.len(), 3);

        let turtle: &[u8] = b"@prefix : <aff4://vol> .\n<aff4://stream> a aff4:ImageStream .\n";
        let archive: Vec<u8> = build_test_zip(&[
            ("version.txt", b"major=1\nminor=0\n"),
            ("container.description", b"aff4://vol\n"),
            ("information.turtle", turtle),
            ("sample.bin", b"12345"),
        ]);
        let resolver: FileResolverReference = FileResolverReference::new(Box::new(
            MemoryFileResolver::new(vec![("vol.aff4", archive)]),
        ));
        let volume: DataVolume = DataVolume::open(&resolver, &PathComponent::from("vol.aff4"))?;

        assert_eq!(volume.get_entry_size("sample.bin"), Some(5));
        assert_eq!(volume.get_entry_size("missing.bin"), None);
        assert!(volume.read_entry_data("missing.bin").is_err());

        Ok(())
    }

    #[test]
    fn test_open_and_read_mapped_snappy_image() -> Result<(), ErrorTrace> {
        let stream_iri: &str = "aff4://00000000-0000-1003-8000-000000000001";
        let map_iri: &str = "aff4://00000000-0000-1003-8000-000000000002";
        let image_iri: &str = "aff4://00000000-0000-1003-8000-000000000003";
        let volume_iri: &str = "aff4://00000000-0000-1003-8000-000000000004";

        let snappy_chunk: Vec<u8> = get_snappy_chunk_16(0x42);
        let raw_chunk: Vec<u8> = vec![0x45; 16];
        let mut bevy_data: Vec<u8> = Vec::new();
        bevy_data.extend_from_slice(&snappy_chunk);
        let raw_offset: u64 = bevy_data.len() as u64;
        bevy_data.extend_from_slice(&raw_chunk);

        // Chunk 0 = Snappy (16 x 0x42), chunk 1 = zero (stored_size = 0), chunk 2 = raw (16 x 0x45).
        let bevy_index: Vec<u8> = build_bevy_index(&[
            (0, snappy_chunk.len() as u32),
            (0, 0),
            (raw_offset, raw_chunk.len() as u32),
        ]);
        let map_idx: String = format!(
            "{}\naff4:Zero\nhttp://aff4.org/Schema#SymbolicStream5A\n",
            stream_iri
        );
        // Media layout (96 bytes):
        //   [0..48]:  Stream(0) target_offset 0 (chunks 0, 1, 2)
        //   [48..64]: Gap (zeros)
        //   [64..80]: Symbolic(0x5a)
        //   [80..96]: unmapped -> mapGapDefaultStream SymbolicStreamFF
        let map_data: Vec<u8> = build_map_entries(&[(0, 48, 0, 0), (48, 16, 0, 1), (64, 16, 0, 2)]);
        let turtle: String = format!(
            concat!(
                "@prefix : <{vol}> .\n",
                "@prefix aff4: <http://aff4.org/Schema#> .\n",
                "<{stream}> a aff4:ImageStream ;\n",
                "    aff4:chunkSize \"16\"^^xsd:int ;\n",
                "    aff4:chunksInSegment \"4\"^^xsd:int ;\n",
                "    aff4:compressionMethod <http://code.google.com/p/snappy/> ;\n",
                "    aff4:size \"48\"^^xsd:long .\n",
                "<{map}> a aff4:Map ;\n",
                "    aff4:mapGapDefaultStream <http://aff4.org/Schema#SymbolicStreamFF> .\n",
                "<{img}> a aff4:Image ;\n",
                "    aff4:blockSize \"512\"^^xsd:int ;\n",
                "    aff4:dataStream <{map}> ;\n",
                "    aff4:size \"96\"^^xsd:long .\n"
            ),
            vol = volume_iri,
            stream = stream_iri,
            map = map_iri,
            img = image_iri
        );

        let stream_prefix: String = DataVolume::percent_encode(stream_iri, false);
        let map_prefix: String = DataVolume::percent_encode(map_iri, false);
        let bevy_name: String = format!("{}/00000000", stream_prefix);
        let bevy_index_name: String = format!("{}/00000000.index", stream_prefix);
        let map_name: String = format!("{}/map", map_prefix);
        let map_idx_name: String = format!("{}/idx", map_prefix);

        let archive: Vec<u8> = build_test_zip(&[
            ("version.txt", b"major=1\nminor=0\n"),
            ("container.description", volume_iri.as_bytes()),
            ("information.turtle", turtle.as_bytes()),
            (bevy_name.as_str(), &bevy_data),
            (bevy_index_name.as_str(), &bevy_index),
            (map_name.as_str(), &map_data),
            (map_idx_name.as_str(), map_idx.as_bytes()),
        ]);

        let resolver: FileResolverReference = FileResolverReference::new(Box::new(
            MemoryFileResolver::new(vec![("mapped.aff4", archive)]),
        ));
        let media: Aff4StandardMedia =
            Aff4StandardMedia::open(&resolver, &PathComponent::from("mapped.aff4"))?;

        assert!(media.is_mapped());
        assert_eq!(media.get_media_size(), 96);
        assert_eq!(media.get_bytes_per_sector(), 512);
        assert_eq!(media.get_image_name(), image_iri);
        assert_eq!(media.streams[0].get_number_of_chunks(), 3);
        assert!(media.streams[0].get_segment_for_chunk(99).is_none());

        let mut reader: Aff4StandardReader = Aff4StandardReader::new(media);
        assert_eq!(reader.get_size(), 96);

        let mut output: Vec<u8> = vec![0; 96];
        let read_size: usize = reader.read_data_from_blocks(&mut output, 0)?;
        assert_eq!(read_size, 96);

        let mut expected: Vec<u8> = Vec::new();
        expected.extend_from_slice(&[0x42; 16]);
        expected.extend_from_slice(&[0x00; 16]);
        expected.extend_from_slice(&[0x45; 16]);
        expected.extend_from_slice(&[0x00; 16]);
        expected.extend_from_slice(&[0x5a; 16]);
        expected.extend_from_slice(&[0xff; 16]);
        assert_eq!(output, expected);

        // Second read exercises chunk cache and bevy cache.
        let mut cached_output: Vec<u8> = vec![0; 16];
        let cached_read_size: usize = reader.read_data_from_blocks(&mut cached_output, 0)?;
        assert_eq!(cached_read_size, 16);
        assert_eq!(cached_output, vec![0x42; 16]);

        // Reading at end of media and with empty buffer returns 0.
        assert_eq!(reader.read_data_from_blocks(&mut cached_output, 96)?, 0);
        assert_eq!(reader.read_data_from_blocks(&mut [], 0)?, 0);

        Ok(())
    }

    #[test]
    fn test_open_and_read_companion_zlib_deflate_and_stream_gap_default() -> Result<(), ErrorTrace>
    {
        let zlib_stream_iri: &str = "aff4://00000000-0000-1003-8000-000000000011";
        let deflate_stream_iri: &str = "aff4://00000000-0000-1003-8000-000000000012";
        let map_iri: &str = "aff4://00000000-0000-1003-8000-000000000013";
        let primary_vol_iri: &str = "aff4://00000000-0000-1003-8000-000000000014";
        let companion_vol_iri: &str = "aff4://00000000-0000-1003-8000-000000000015";

        let zlib_chunk: Vec<u8> = get_zlib_chunk_16_0x44();
        let zlib_index: Vec<u8> = build_bevy_index(&[(0, zlib_chunk.len() as u32)]);
        let deflate_chunk: Vec<u8> = get_deflate_chunk_16(0x43);

        // Map entry [0..16] uses zlib stream; gap [16..32] falls through to deflate stream.
        let map_idx: String = format!("{}\n", zlib_stream_iri);
        let map_data: Vec<u8> = build_map_entries(&[(0, 16, 0, 0)]);

        let primary_turtle: String = format!(
            concat!(
                "@prefix : <{vol}> .\n",
                "@prefix aff4: <http://aff4.org/Schema#> .\n",
                "<{zstream}> a aff4:ImageStream ;\n",
                "    aff4:chunkSize \"16\"^^xsd:int ;\n",
                "    aff4:compressionMethod <https://www.ietf.org/rfc/rfc1950.txt> ;\n",
                "    aff4:size \"16\"^^xsd:long .\n",
                "<{dstream}> a aff4:ImageStream ;\n",
                "    aff4:chunkSize \"16\"^^xsd:int ;\n",
                "    aff4:compressionMethod <https://www.ietf.org/rfc/rfc1951.txt> ;\n",
                "    aff4:stored <{cvol}> .\n",
                "<{map}> a aff4:Map ;\n",
                "    aff4:mapGapDefaultStream <{dstream}> .\n",
                "<aff4://img> a aff4:Image ;\n",
                "    aff4:dataStream <{map}> ;\n",
                "    aff4:size \"32\"^^xsd:long .\n"
            ),
            vol = primary_vol_iri,
            cvol = companion_vol_iri,
            zstream = zlib_stream_iri,
            dstream = deflate_stream_iri,
            map = map_iri
        );
        let companion_turtle: String = format!(
            concat!(
                "@prefix : <{cvol}> .\n",
                "@prefix aff4: <http://aff4.org/Schema#> .\n",
                "<{dstream}> a aff4:ImageStream ;\n",
                "    aff4:chunkSize \"16\"^^xsd:int ;\n",
                "    aff4:compressionMethod <https://www.ietf.org/rfc/rfc1951.txt> ;\n",
                "    aff4:size \"32\"^^xsd:long .\n"
            ),
            cvol = companion_vol_iri,
            dstream = deflate_stream_iri
        );

        let zlib_prefix: String = DataVolume::percent_encode(zlib_stream_iri, false);
        let deflate_prefix: String = DataVolume::percent_encode(deflate_stream_iri, false);
        let map_prefix: String = DataVolume::percent_encode(map_iri, false);

        let primary_zip: Vec<u8> = build_test_zip(&[
            ("version.txt", b"major=1\nminor=0\n"),
            ("container.description", primary_vol_iri.as_bytes()),
            ("information.turtle", primary_turtle.as_bytes()),
            (&format!("{}/00000000", zlib_prefix), &zlib_chunk),
            (&format!("{}/00000000.index", zlib_prefix), &zlib_index),
            (&format!("{}/map", map_prefix), &map_data),
            (&format!("{}/idx", map_prefix), map_idx.as_bytes()),
        ]);
        // Companion provides two chunks of 0x43 so offset 16 reads chunk 1.
        let mut companion_bevy: Vec<u8> = Vec::new();
        companion_bevy.extend_from_slice(&deflate_chunk);
        let second_offset: u64 = companion_bevy.len() as u64;
        companion_bevy.extend_from_slice(&deflate_chunk);
        let companion_index: Vec<u8> = build_bevy_index(&[
            (0, deflate_chunk.len() as u32),
            (second_offset, deflate_chunk.len() as u32),
        ]);
        let companion_zip: Vec<u8> = build_test_zip(&[
            ("version.txt", b"major=1\nminor=0\n"),
            ("container.description", companion_vol_iri.as_bytes()),
            ("information.turtle", companion_turtle.as_bytes()),
            (&format!("{}/00000000", deflate_prefix), &companion_bevy),
            (
                &format!("{}/00000000.index", deflate_prefix),
                &companion_index,
            ),
        ]);

        let resolver: FileResolverReference =
            FileResolverReference::new(Box::new(MemoryFileResolver::new(vec![
                ("sub/primary.aff4", primary_zip),
                ("sub/companion.aff4", companion_zip),
                ("sub/unrelated.txt", b"ignored".to_vec()),
            ])));
        let media: Aff4StandardMedia =
            Aff4StandardMedia::open(&resolver, &PathComponent::from("sub/primary.aff4"))?;
        assert!(media.is_mapped());
        assert_eq!(media.get_media_size(), 32);

        let mut reader: Aff4StandardReader = Aff4StandardReader::new(media);
        let mut output: Vec<u8> = vec![0; 32];
        let read_size: usize = reader.read_data_from_blocks(&mut output, 0)?;
        assert_eq!(read_size, 32);
        assert_eq!(&output[0..16], &[0x44; 16]);
        assert_eq!(&output[16..32], &[0x43; 16]);

        Ok(())
    }

    #[test]
    fn test_open_with_error_conditions() {
        // Missing primary volume.
        let empty_resolver: FileResolverReference =
            FileResolverReference::new(Box::new(MemoryFileResolver::new(vec![])));
        assert!(
            Aff4StandardMedia::open(&empty_resolver, &PathComponent::from("missing.aff4")).is_err()
        );

        // Non-standard (legacy) primary volume.
        let legacy_zip: Vec<u8> = build_test_zip(&[
            ("version.txt", b"major=1\nminor=0\n"),
            ("container.description", b"aff4://vol\n"),
            (
                "information.turtle",
                b"@base <aff4://vol> .\n</img.raw> a aff4:Image .\n",
            ),
            ("img.raw", b"data"),
        ]);
        let resolver: FileResolverReference = FileResolverReference::new(Box::new(
            MemoryFileResolver::new(vec![("legacy.aff4", legacy_zip)]),
        ));
        assert!(Aff4StandardMedia::open(&resolver, &PathComponent::from("legacy.aff4")).is_err());

        // Standard volume without ImageStream node.
        let no_stream_zip: Vec<u8> = build_test_zip(&[
            ("version.txt", b"major=1\nminor=0\n"),
            ("container.description", b"aff4://vol\n"),
            (
                "information.turtle",
                b"@prefix : <aff4://vol> .\n<aff4://img> a aff4:Image .\n",
            ),
        ]);
        let resolver: FileResolverReference = FileResolverReference::new(Box::new(
            MemoryFileResolver::new(vec![("nostream.aff4", no_stream_zip)]),
        ));
        assert!(Aff4StandardMedia::open(&resolver, &PathComponent::from("nostream.aff4")).is_err());

        // ImageStream node without data entries in volume.
        let no_bevy_zip: Vec<u8> = build_test_zip(&[
            ("version.txt", b"major=1\nminor=0\n"),
            ("container.description", b"aff4://vol\n"),
            (
                "information.turtle",
                b"@prefix : <aff4://vol> .\n<aff4://s> a aff4:ImageStream ; aff4:chunkSize \"16\" .\n",
            ),
        ]);
        let resolver: FileResolverReference = FileResolverReference::new(Box::new(
            MemoryFileResolver::new(vec![("nobevy.aff4", no_bevy_zip)]),
        ));
        assert!(Aff4StandardMedia::open(&resolver, &PathComponent::from("nobevy.aff4")).is_err());

        // Map node missing its map/idx entries.
        let stream_prefix: String = DataVolume::percent_encode("aff4://s", false);
        let bevy_index: Vec<u8> = build_bevy_index(&[(0, 16)]);
        let bad_map_zip: Vec<u8> = build_test_zip(&[
            ("version.txt", b"major=1\nminor=0\n"),
            ("container.description", b"aff4://vol\n"),
            (
                "information.turtle",
                concat!(
                    "@prefix : <aff4://vol> .\n",
                    "<aff4://s> a aff4:ImageStream ; aff4:chunkSize \"16\" .\n",
                    "<aff4://m> a aff4:Map .\n",
                    "<aff4://img> a aff4:Image ; aff4:dataStream <aff4://m> .\n"
                )
                .as_bytes(),
            ),
            (&format!("{}/00000000", stream_prefix), &[0x11; 16]),
            (&format!("{}/00000000.index", stream_prefix), &bevy_index),
        ]);
        let resolver: FileResolverReference = FileResolverReference::new(Box::new(
            MemoryFileResolver::new(vec![("badmap.aff4", bad_map_zip)]),
        ));
        assert!(Aff4StandardMedia::open(&resolver, &PathComponent::from("badmap.aff4")).is_err());

        // Map node with invalid map data size (not a multiple of 28).
        let map_prefix: String = DataVolume::percent_encode("aff4://m", false);
        let bad_map_size_zip: Vec<u8> = build_test_zip(&[
            ("version.txt", b"major=1\nminor=0\n"),
            ("container.description", b"aff4://vol\n"),
            (
                "information.turtle",
                concat!(
                    "@prefix : <aff4://vol> .\n",
                    "<aff4://s> a aff4:ImageStream ; aff4:chunkSize \"16\" .\n",
                    "<aff4://m> a aff4:Map .\n",
                    "<aff4://img> a aff4:Image ; aff4:dataStream <aff4://m> .\n"
                )
                .as_bytes(),
            ),
            (&format!("{}/00000000", stream_prefix), &[0x11; 16]),
            (&format!("{}/00000000.index", stream_prefix), &bevy_index),
            (&format!("{}/map", map_prefix), &[0; 10]),
            (&format!("{}/idx", map_prefix), b"aff4://s\n"),
        ]);
        let resolver: FileResolverReference = FileResolverReference::new(Box::new(
            MemoryFileResolver::new(vec![("badmapsize.aff4", bad_map_size_zip)]),
        ));
        assert!(
            Aff4StandardMedia::open(&resolver, &PathComponent::from("badmapsize.aff4")).is_err()
        );

        // Map node with invalid UTF-8 idx entry.
        let bad_idx_zip: Vec<u8> = build_test_zip(&[
            ("version.txt", b"major=1\nminor=0\n"),
            ("container.description", b"aff4://vol\n"),
            (
                "information.turtle",
                concat!(
                    "@prefix : <aff4://vol> .\n",
                    "<aff4://s> a aff4:ImageStream ; aff4:chunkSize \"16\" .\n",
                    "<aff4://m> a aff4:Map .\n",
                    "<aff4://img> a aff4:Image ; aff4:dataStream <aff4://m> .\n"
                )
                .as_bytes(),
            ),
            (&format!("{}/00000000", stream_prefix), &[0x11; 16]),
            (&format!("{}/00000000.index", stream_prefix), &bevy_index),
            (&format!("{}/map", map_prefix), &[0; 28]),
            (&format!("{}/idx", map_prefix), &[0xff, 0xfe]),
        ]);
        let resolver: FileResolverReference = FileResolverReference::new(Box::new(
            MemoryFileResolver::new(vec![("badidx.aff4", bad_idx_zip)]),
        ));
        assert!(Aff4StandardMedia::open(&resolver, &PathComponent::from("badidx.aff4")).is_err());
    }

    #[test]
    fn test_reader_error_paths() -> Result<(), ErrorTrace> {
        let stream_prefix: String = DataVolume::percent_encode("aff4://s", false);
        let bad_compressed: &[u8] = &[0xff, 0xfe, 0xfd, 0xfc];

        // Test decompression errors for Snappy, Zlib, Deflate and unsupported Stored size.
        for compression_iri in [
            "http://code.google.com/p/snappy/",
            "https://www.ietf.org/rfc/rfc1950.txt",
            "https://www.ietf.org/rfc/rfc1951.txt",
            "http://aff4.org/Schema#NullCompressor",
        ] {
            let bevy_index: Vec<u8> = build_bevy_index(&[(0, bad_compressed.len() as u32)]);
            let turtle: String = format!(
                concat!(
                    "@prefix : <aff4://vol> .\n",
                    "<aff4://s> a aff4:ImageStream ;\n",
                    "    aff4:chunkSize \"16\" ;\n",
                    "    aff4:compressionMethod <{comp}> .\n"
                ),
                comp = compression_iri
            );
            let archive: Vec<u8> = build_test_zip(&[
                ("version.txt", b"major=1\nminor=0\n"),
                ("container.description", b"aff4://vol\n"),
                ("information.turtle", turtle.as_bytes()),
                (&format!("{}/00000000", stream_prefix), bad_compressed),
                (&format!("{}/00000000.index", stream_prefix), &bevy_index),
            ]);
            let resolver: FileResolverReference = FileResolverReference::new(Box::new(
                MemoryFileResolver::new(vec![("err.aff4", archive)]),
            ));
            let media: Aff4StandardMedia =
                Aff4StandardMedia::open(&resolver, &PathComponent::from("err.aff4"))?;
            assert!(!media.is_mapped());
            assert_eq!(media.get_media_size(), 16);

            let mut reader: Aff4StandardReader = Aff4StandardReader::new(media);
            let mut output: Vec<u8> = vec![0; 16];
            assert!(reader.read_data_from_blocks(&mut output, 0).is_err());
        }

        // Test out-of-bounds bevy offset for both uncompressed and compressed chunk paths.
        for stored_size in [16_u32, 4_u32] {
            let bevy_index: Vec<u8> = build_bevy_index(&[(100, stored_size)]);
            let turtle: &str = concat!(
                "@prefix : <aff4://vol> .\n",
                "<aff4://s> a aff4:ImageStream ;\n",
                "    aff4:chunkSize \"16\" ;\n",
                "    aff4:compressionMethod <http://code.google.com/p/snappy/> .\n"
            );
            let archive: Vec<u8> = build_test_zip(&[
                ("version.txt", b"major=1\nminor=0\n"),
                ("container.description", b"aff4://vol\n"),
                ("information.turtle", turtle.as_bytes()),
                (&format!("{}/00000000", stream_prefix), &[0; 16]),
                (&format!("{}/00000000.index", stream_prefix), &bevy_index),
            ]);
            let resolver: FileResolverReference = FileResolverReference::new(Box::new(
                MemoryFileResolver::new(vec![("oob.aff4", archive)]),
            ));
            let media: Aff4StandardMedia =
                Aff4StandardMedia::open(&resolver, &PathComponent::from("oob.aff4"))?;
            let mut reader: Aff4StandardReader = Aff4StandardReader::new(media);
            let mut output: Vec<u8> = vec![0; 16];
            assert!(reader.read_data_from_blocks(&mut output, 0).is_err());
        }

        // Test missing stream/segment/chunk errors directly on Aff4StandardReader.
        let empty_media: Aff4StandardMedia = Aff4StandardMedia {
            streams: Vec::new(),
            map: None,
            media_size: 16,
            image_name: String::new(),
            bytes_per_sector: 512,
        };
        let mut reader: Aff4StandardReader = Aff4StandardReader::new(empty_media);
        let mut output: Vec<u8> = vec![0; 16];
        assert!(reader.read_data_from_blocks(&mut output, 0).is_err());
        assert!(reader.get_bevy_data(0, 0).is_err());
        assert!(reader.get_chunk(0, 0).is_err());
        assert!(reader.read_stream_data(0, 0, &mut output).is_err());

        Ok(())
    }
}

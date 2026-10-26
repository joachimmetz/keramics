# Advanced Forensic Format 4 (AFF4)

The Advanced Forensic Format 4 (AFF4) is the successor of the
[Advanced Forensic Format (AFF)](aff.md).

> Note the term "AFF4" is also used by the AFFLIBv3 project for an early, experimental container
> implementation based on RDF data types, which predates the stand-alone AFF4 format.

The AFF4 format supports:

* storage virtualisation
* arbitrary metadata storage
* extensible compression and hashing schemes
* throughput scalable to high IO rates

> Note where there is ambiguity or difference between the specification and the AFF4 Canonical
> Reference Images v1.0 (available via <https://github.com/aff4/>), the reference images take
> precedence.

In AFF4, disk images are stored in a "Container", which generally is a single ZIP64 based file or a
folder. Arbitrary metadata is stored using an RDF data model within the container, in a file called
`information.turtle`.

Image content is stored using two abstractions, the **Map** (a virtual address space) and the
**Image Stream** (a seekable, contiguous sequence of fixed sized data blocks, optionally
compressed). Various **Virtual Streams** exist for representing common block sequences such as
sparse (all 0x00) streams.

## Overview

An AFF4 container consists of:

* one or more **segments** (file like objects) stored within a folder hierarchy, provided by the
  **storage layer** (a ZIP64 file or a directory)
* a container **version** file (`version.txt`)
* an RDF **information model** file (`information.turtle`)
* one or more **image streams**, for storing the image data
* optionally, a **map**, defining a virtual address space
* optionally, **block hash segments**, for integrity verification

### Characteristics

| Characteristics | Description |
| --- | --- |
| Byte order | TBD (dependent on ZIP64 and other formats used) |
| Date and time values | TBD (dependent on ZIP64 and other formats used) |
| Character strings | TBD (dependent on ZIP64 and other formats used) |

## Container version

The container version is stored in a text file located at the root of the container with the name
`version.txt`. The file contains name value pairs, separated by an equal (`=`) sign. Line endings
can be CRLF, CR or LF. The ordering of the name value pairs is arbitrary.

A `version.txt` file in an AFF4 container compliant with AFF4 1.0 looks like:

```text
major=1
minor=0
tool=libaff4 0.9
```

For AFF4 1.0, `major` is 1 and `minor` is 0. The `tool` line is vendor specific, to indicate the
producing tool.

The primary purpose of the `version.txt` file is for bootstrapping and enabling support for
backwards incompatible changes in the future. Implementations should only encode in the file
information that would be used to detect non-standard deviations from the AFF4 standard.

## Storage layer {#storage_layer}

AFF4 defines an abstract model of storage layers. Currently the standard has two implementations
of this abstraction: **ZIP** and **Directory**. The storage layer stores **segments** (file like
objects), in a folder hierarchy.

Both the ZIP and the Directory storage layers provide:

* the root of the volume (the "Context Root" for the volume). For Directory Volumes this is the
  root folder, for ZIP Volumes this is the root of the ZIP
* a unique AFF4 GUID based URI (the **Volume URI**), which implicitly maps to the defined Context
  Root

Objects in the information model may represent or refer to objects in the storage layer either as
direct segments, or via the **Map** and **ImageStream** abstractions. The mapping between these
objects and the segments is defined by the [URI to path mapping](#uri_to_path_mapping) rules.

### ZIP container

The ZIP container is a [ZIP archive](zip.md) with the following AFF4 specific requirements:

* All ZIP headers MUST be zip64, see [ZIP64 records](zip.md#zip64_eocd_record)
* The Volume URI MUST be stored within the ZIP container, in one of the following two locations:
  * **ZIP Comment**: the Volume URI is stored starting at offset 0 of the
    [ZIP file comment](zip.md#eocd_record)
  * **container.description**: the Volume URI is stored within a ZIP segment named
    `container.description`, in the root of the container
* A producer SHOULD store the Volume URI in both locations
* Where a ZIP Container stores the Volume URI in a `container.description` segment, the file MUST
  be the first file stored in the ZIP volume

## Information model

The information model is stored within the container in a file with the name `information.turtle`.
The file contains the RDF data model, using the namespace `http://aff4.org/Schema#`, with the
shorthand prefix `aff4`.

The naming convention for properties and classes is to follow RDF/OWL best practice of using
CamelCase, with leading capitalization for classes and leading lower case for properties.

### Base classes {#base_classes}

| Name | Purpose |
| --- | --- |
| `Map` | Virtual Address Space |
| `ImageStream` | Storage of image data chunks |
| `Image` | Representing an image |
| `ContiguousImage` | Representing a contiguous image, subclass of `Image` |
| `DiskImage` | Representing a disk image, subclass of `ContiguousImage` |

Implementations MUST use RDF multiple type semantics when storing types. For example, when
representing a disk image the `rdf:type` should reference `aff4:DiskImage` in addition to the
parent types `aff4:ContiguousImage` and `aff4:Image`.

### Base properties {#base_properties}

| Name | Context | Purpose |
| --- | --- | --- |
| `size` | `ImageStream`, `Map` | The length of the stream |
| `chunkSize` | `ImageStream` | The chunk size |
| `chunksInSegment` | `ImageStream` | Number of chunks per segment |
| `compressionMethod` | `ImageStream` | The compression algorithm, if any, applied to the chunks |
| `mapGapDefaultStream` | `Map` | The stream to be used to implicitly fill any discontinuities found in a discontiguous `Map` stream |
| `fileName` | Any segment URI | Explicit mapping of a URI to a segment path |
| `dataStream` | `Image` or subclass of `Image` | The `Map` or `ImageStream` which corresponds to the byte stream of the image |
| `dependentStream` | `Map` | The child `ImageStream`s that are used for block storage |
| `target` | `ImageStream`, `Map` | Backwards pointer to the parent of this object |
| `stored` | `Map`, `ImageStream` | The volume that the `Map` or `ImageStream` is stored in |
| `hash` | `Image` | The hash of the stream |

## Image stream {#image_stream}

An `ImageStream` stores the image data as a contiguous sequence of fixed sized data blocks,
**chunks**, optionally compressed. The chunks are stored within one or more segments ("bevvy"
segments), and an index entry per stored chunk.

The specification states that an `ImageStream` employs *n* index entries for *n* stored chunks.
The index entry has the following structure:

| Offset | Size | Description |
| --- | --- | --- |
| 0 | 8 | Bevvy offset, offset of the chunk within the bevvy segment (u64) |
| 8 | 4 | Chunk size, size of the stored chunk in bytes (u32) |

A bevvy index segment contains an array of index entries:

```c
struct index_entry index[bevy_size];
```

> TODO the value of `bevy_size` (the number of index entries per bevvy index segment) is not
> specified in the specification. This is currently unknown.

### Naming

Chunk storage segments (bevvy's) are named as per [reference 1](#references) of the
specification. Reference images use an 8 digit, hexadecimal, zero padded number, e.g. `00000000`.
Bevvy index segments are named with a `.index` extension on the base bevvy name, e.g.
`00000000.index`.

### Compression {#image_stream_compression}

Storage of uncompressed chunks is supported by the simple principle that if
`len(chunk) == aff4:chunkSize` then it is a stored chunk, and compression is not applied to
stored chunks.

The specification provides the following algorithm as guidance to writer implementations:

```python
compressedBytes = compress(chunkBytes)
compressedLen = len(compressedBytes)
if (compressedLen < (chunkSize - 16))
    write(compressedBytes)
    # store index
else
    write(chunkBytes)
    # store index
```

The specification provides the following algorithm as guidance to reader implementations:

```python
len = index[i+1] - index[i]
offset = index[i]

if (len == chunkSize)
    chunkBytes = read(base + offset, len)
else
    compressedBytes = read(base + offset, len)
    chunkBytes = decompress(compressedBytes)
```

### Compression methods

When compression is used within the `ImageStream`, the `aff4:compressionMethod` property MUST be
set to a resource identifying the compression algorithm. When no `aff4:compressionMethod` is set,
it is assumed that chunks are stored (uncompressed). Known used compression methods are:

* [DEFLATE](zlib.md#deflate_compressed_data)
* [LZ4](https://github.com/lz4/lz4)
* [Snappy](https://github.com/google/snappy)

### Information model

An `ImageStream` MUST have a corresponding object defined within the `information.turtle` file,
with the `rdf:type` of the object being `aff4:ImageStream`.

* The `ImageStream` object MUST have an `aff4:size` property set to the length in bytes of the
  stream
* The `ImageStream` object MUST have an `aff4:chunkSize` property set to the chunk size

## Map {#map}

A `Map` defines a virtual address space over one or more `ImageStream`s. A map entry has the
following structure:

| Offset | Size | Description |
| --- | --- | --- |
| 0 | 8 | Mapped offset, offset in the mapped address space (u64) |
| 8 | 8 | Length, size of the mapping in bytes (u64) |
| 16 | 8 | Target offset, address in the target address space (u64) |
| 24 | 4 | Target ID, the ID of the stream found in the target index (u32) |

The `Map` MAY be discontiguous. In that case a default value for holes **MAY** be specified by
the `aff4:mapGapDefaultStream` property of the `Map` object. If the map is discontiguous, and the
`aff4:mapGapDefaultStream` property is not set, then `aff4:Zero` is used to fill the holes.

### Target index {#target_index}

The target index is a text file containing URIs corresponding to target identifiers. The file
format is line based, separated by newline (0x0a) characters. The first line corresponds to target
identifier 0, the second line to target identifier 1 and so on.

### Naming

The map is stored under the container root, with the path `URI/map`, where `URI` is the map URI
encoded per the [storage layer](#storage_layer) description. The target identifiers are stored in
the file `URI/idx`.

### Information model

A `Map` stream MUST have a corresponding object defined within the `information.turtle` file, with
the `rdf:type` of the object being `aff4:Map`.

The `Map` stream object MUST have an `aff4:size` property set to the length in bytes of the
stream.

### Symbolic streams {#symbolic_streams}

The following resources are defined to represent known linear bitstreams, in support of both low
cost storage and describing discontinuities:

| Stream URI | Meaning |
| --- | --- |
| `aff4:Zero` | An infinite length byte stream wherein each byte is 0x00 |
| `aff4:SymbolicStreamXX` | An infinite length byte stream wherein each byte is 0xXX, e.g. `aff4:SymbolicStreamFF` is an infinite length byte stream wherein each byte is 0xFF. `aff4:SymbolicStream00` is a synonym for `aff4:Zero` |
| `aff4:UnknownData` | An infinite length stream which represents the possible existence of data. For example, an implementation which skips reading data may use this to represent address space regions which were purposefully not accessed |
| `aff4:UnreadableData` | An infinite length stream which represents the possible existence of data which was inaccessible. For example, an implementation which strikes bad sectors may use this stream to represent such address ranges |

Both `aff4:UnknownData` and `aff4:UnreadableData` are instances of **Repeated String Streams**.
The content of these streams is defined so that implementations which choose to read these
streams have reproducible results (for example, when producing a linear bitstream hash over an
image where parts of the address space are `aff4:UnknownData`).

Repeated String Streams are of infinite length, consisting of contiguous repeated chunks of
1 MiB. These chunks are filled with a repeated set of bytes (the "Repeated String"). In general
the repeated bytes will be truncated at the end of the chunk to fit within the chunk size. For
example, the 7 byte repeated string "UNKNOWN" will be truncated at the end of the 1 MiB chunk.

* The chunks in `aff4:UnknownData` are filled with the repeated ASCII string "UNKNOWN"
* The chunks in `aff4:UnreadableData` are filled with the repeated ASCII string
  "UNREADABLEDATA"

## URI to path mapping {#uri_to_path_mapping}

Any AFF4 URI is mapped into the volume (the storage layer) as follows:

1. if the URI begins with the Volume URI, then only the portion of the URI after the GUID is
   translated to paths within the volume (relative addressing)
2. if the URI does not begin with the Volume URI, the URI prefix (the scheme + GUID) is escaped
   and stored as a folder in the root of the volume

### Escaping

1. The URI prefix component containing "://" is URL encoded
2. Escaping MUST use upper case
3. Within the rest of the URI, "/" maps to the local filesystem/ZIP container folder separator
4. Data streams are only stored in paths that have no children
5. Folder/file names within the URI are URL encoded

### Explicit mapping

An implementation may override the mapping rules by using the `aff4:fileName` property to specify
a filesystem specific encoded path relative to the base of the volume:

```turtle
<aff4://8ec0479e-1712-4aef-9a3e-b34c65aaf4c2/PhysicalMemory/data/00000000/index>
      a aff4:File ;
      aff4:fileName "PhysicalMemory%2fdata%2f00000000%2findex"^^xsd:string .
```

### Storage mapping examples

The following examples assume the AFF4 Volume URI is
`aff4://9db79393-53fa-4147-b823-5c3e1d37544d`:

| URI | Storage mapping |
| --- | --- |
| `aff4://9db79393-53fa-4147-b823-5c3e1d37544d/Foobar.txt` | `/Foobar.txt` |
| `aff4://9db79393-53fa-4147-b823-5c3e1d37544d/Foobar/foo.txt` | `/Foobar/foo.txt` |
| `aff4://05e11366-036a-4997-8f10-cfbff91256f4/00000000` | `/aff4%3A%2F%2F05e11366-036a-4997-8f10-cfbff91256f4/00000000` |
| `aff4://05e11366-036a-4997-8f10-cfbff91256f4` | The ImageStream translation of the following sub elements: `/aff4%3A%2F%2F05e11366-036a-4997-8f10-cfbff91256f4/00000000`, `/aff4%3A%2F%2F05e11366-036a-4997-8f10-cfbff91256f4/00000000.index`, etc. |

## Hashing

The primary difference between a forensic image and a regular image is the inclusion of
integrity verification by way of the use of cryptographic hashes.

AFF4 provides two methods of hashing, which are associated with the `aff4:hash` property. The
method of hashing is identified both by the class that the `aff4:hash` property is associated
with, as well as the data type associated with the `aff4:hash` property.

### Linear bitstream hashing

The following RDF data type URIs are defined for use in linear bitstream hashing in the AFF4:

| AFF4 `hash` data type | Hash algorithm |
| --- | --- |
| `aff4:MD5` | MD5 over linear bitstream |
| `aff4:SHA1` | SHA1 over linear bitstream |
| `aff4:SHA256` | SHA-256 over linear bitstream |
| `aff4:SHA512` | SHA-512 over linear bitstream |
| `aff4:blake2b` | Blake2b (512 bit) over linear bitstream |

The encoding of these hashes is as hex characters.

Implementations will generally use the `aff4:hash` property with an `aff4:Image` class to
describe the hash of the image. For example:

```turtle
<aff4://9c80aac-c9f7-4cd7-bf15-67d7338af000>
    a          aff4:DiskImage, aff4:ContiguousImage, aff4:Image;
    aff4:dataStream <aff4://7c808ddc-c9f7-4cd7-bf15-67d733dc43bc> ;
    aff4:hash    "5fc17ee079cbe55c5b6dc150250eebc478ca7191"^^aff4:SHA1,
        "2c6f3709898df1913271f9ee0f531157"^^aff4:MD5 .
```

The interpretation of the above is: the `Map` or `ImageStream` instance that is pointed to by the
`aff4:dataStream` property, when linearly hashed with the MD5 algorithm, has the hex encoded hash
`2c6f3709898df1913271f9ee0f531157`.

### Block map hashing

Implementations **MAY** implement the hashing approach described in [reference 3](#references) of
the specification. This hybrid approach involves block hashing of data blocks and Merkle Tree-like
generation of a single hash protecting both the `ImageStream` and the `Map`.

In the following, the symbol "||" indicates concatenation of contiguous byte streams.

#### Image stream block hashes

Block hashes are stored within an `ImageStream` in **Block Hash Segments**, named based on the
hash algorithm used to hash. The following table summarizes the naming scheme:

| Algorithm | Block hash segment naming pattern | Digest size |
| --- | --- | --- |
| MD5 | `XXXXXX.md5` | 128 bits |
| SHA1 | `XXXXXX.sha1` | 160 bits |
| Blake2b | `XXXXXX.blake2b` | 512 bits |
| SHA256 | `XXXXXX.sha256` | 256 bits |
| SHA512 | `XXXXXX.sha512` | 512 bits |

For example, for a bevvy segment named `00000000`, with bevvy index named `00000000.index`, the
corresponding Block Hash Segment for storing SHA1 block hashes is called `00000000.sha1`.

Block Hash Segments store each hash as bytes and not in encoded form (e.g. hex). The order of the
hashes within the file corresponds to the ordering of chunks within the corresponding bevvy
segment.

A virtual URI is defined for representing the concatenation of all of the block hashes for a
particular image stream. For example, for the image stream with URI
`<aff4://c215ba20-5648-4209-a793-1f918c723610>`, the concatenation of the MD5 block hashes is
the URI `<aff4://c215ba20-5648-4209-a793-1f918c723610/blockhash.md5>`. This stream of bytes is
called **BlockHashes**, and is given the `rdf:type` `aff4:BlockHashes`:

```text
<aff4://c215ba20-5648-4209-a793-1f918c723610/blockhash.md5> =
    aff4://c215ba20-5648-4209-a793-1f918c723610/00000000.md5 ||
    aff4://c215ba20-5648-4209-a793-1f918c723610/00000001.md5 ||
    aff4://c215ba20-5648-4209-a793-1f918c723610/00000002.md5 ...
```

The hash of the `BlockHashes` is then expressed using the standard `aff4:hash` property. For
example:

```turtle
<aff4://c215ba20-5648-4209-a793-1f918c723610/blockhash.md5>
    a aff4:BlockHashes ;
    aff4:hash
        "9062f1c9f48438a6875a60b7e1323151e8ff583c8531ca7806d6c29b7d961ceddba"
        "8783e8e4c49ff37702304cdf1dc4c7a9b8f67c73af07fc14422c0be9ae20d"^^aff4:SHA512 .
```

Implementations **MAY** generate block hashes using multiple algorithms.

#### Map hashes

A number of properties are defined for the `Map` for assuring the integrity of the underlying
`Map` segments:

| Map property | Value | Meaning |
| --- | --- | --- |
| `aff4:mapIdxHash` | `H(idx)` | Hash of the index segment |
| `aff4:mapPointHash` | `H(point)` | Hash of the point segment |
| `aff4:mapPathHash` | `H(mapPath)` | Hash of the mapPath segment |
| `aff4:mapHash` | `H(H(map) || H(idx) || [H(mapPath)])` | Hash of the full map |

Implementations using block map hashing **WILL** employ `H` = SHA512 or SHA256.

#### The single block map hash

In order to have a single hash that protects both the chunks stored in the `ImageStream`, as well
as chunks stored synthetically in the `Map`, the **BlockMapHash** is defined:

```text
aff4:blockMapHash =
    H(
        H(BlockHashes1) || H(BlockHashes2) || H(BlockHashesn)
        || aff4:mapPointHash
        || aff4:mapIdxHash
        || [ aff4:mapPathHash ]
    )
```

All `BlockHashes` in the `ImageStream` are integrated into the block map hash. Their ordering is
based on the length of the hash, with the smallest first, such that:

```text
 MD5 < SHA1 < SHA256 < (SHA512, blake2b)
```

Where the hashes are of equal length, the ordering is:

```text
 SHA512 < blake2b
```

For example:

```text
aff4:blockMapHash = H( H(BlockHashes.md5) || H(BlockHashes.sha1)
                     || aff4:mapPointHash || aff4:mapIdxHash )
```

The block map hash MUST be stored in the `aff4:Image`, using either the
`aff4:blockMapHashSHA512` or `aff4:blockMapHashSHA256` data types to indicate the hashing method
used.

The block map hash MAY be stored in the `aff4:Map`, using either the `aff4:SHA512` or
`aff4:SHA256` data types to indicate the hashing method used.

## Container example

The following is a container file structure example for a ZIP container, as provided in the
specification:

| File | Description |
| --- | --- |
| `aff4://a2e14833-6020-44df-ba5e-3b224e102eb6` | ZIP comment (Volume URI) |
| `container.description` | Volume URI (first file stored within the ZIP) |
| `version.txt` | Container version |
| `aff4%3A%2F%2Fdac3f96d-8f2c-4116-adc3-e870fd009242/00000000` | Image stream chunk storage (bevvy) segment |
| `aff4%3A%2F%2Fdac3f96d-8f2c-4116-adc3-e870fd009242/00000000.index` | Image stream index segment |
| `aff4%3A%2F%2F7c808ddc-c9f7-4cd7-bf15-67d733dc43bc/idx` | Map target index |
| `aff4%3A%2F%2F7c808ddc-c9f7-4cd7-bf15-67d733dc43bc/map` | Map segment |
| `information.turtle` | RDF data model |

The contents of `container.description`:

```text
aff4://a2e14833-6020-44df-ba5e-3b224e102eb6
```

The contents of `version.txt`:

```text
major=1
minor=0
tool=libaff4 0.9
```

The contents of `information.turtle`:

```turtle
@prefix :    <aff4://a2e14833-6020-44df-ba5e-3b224e102eb6> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
@prefix aff4: <http://aff4.org/Schema#> .

<aff4://9c80aac-c9f7-4cd7-bf15-67d7338af000>
     a                 aff4:DiskImage, aff4:ContiguousImage, aff4:Image;
     aff4:dataStream   <aff4://7c808ddc-c9f7-4cd7-bf15-67d733dc43bc> ;
     aff4:size         "268435456"^^xsd:long ;
     aff4:hash         "5fc17ee079cbe55c5b6dc150250eebc478ca7191"^^aff4:SHA1 ,
                       "2c6f3709898df1913271f9ee0f531157"^^aff4:MD5 .

<aff4://7c808ddc-c9f7-4cd7-bf15-67d733dc43bc>
     a                 aff4:Map ;
     aff4:size         "268435456"^^xsd:long .

<aff4://dac3f96d-8f2c-4116-adc3-e870fd009242>
     a                 aff4:ImageStream ;
     aff4:chunkSize    "32768"^^xsd:int ;
     aff4:compressionMethod <http://tools.ietf.org/html/rfc1951> .
```

## References {#references}

<!-- rumdl-disable MD013 -->

* [AFF4StandardSpecification-v1.0.pdf](https://github.com/aff4/Standard/blob/master/AFF4StandardSpecification-v1.0.pdf)

<!-- rumdl-enable MD013 -->

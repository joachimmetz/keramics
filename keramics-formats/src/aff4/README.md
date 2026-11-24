# aff4

The aff4 module provides read-only support for the
[Advanced Forensic Format 4 (AFF4)](https://keramics.github.io/aff4.html).

Supported features:

| Category | Feature(s) |
| --- | --- |
| Format version | 1 (legacy and AFF4 Standard v1.0a) |
| Format variants | Zip (ZIP64) volume (.AFF4), split volume (.A01..A32) |
| Images | multiple per volume, Map, ImageStream (BEVY), symbolic streams |
| Compression | stored, deflate, zlib, Snappy |

Unsupported features:

| Category | Feature(s) |
| --- | --- |
| Format version | 2 |
| Format variants | Directory volume |
| Compression | LZ4 |
| | Block hashes |

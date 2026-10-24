# aff4

The aff4 module provides read-only support for the
[Advanced Forensic Format 4 (AFF4)](https://keramics.github.io/aff4.html).

Supported features:

| Category | Feature(s) |
| --- | --- |
| Format version | 1 |
| Format variants | Zip (ZIP64) volume (.AFF4) |
| Images | multiple per volume |
| Compression | stored, deflate |

Unsupported features:

| Category | Feature(s) |
| --- | --- |
| Format version | 2 |
| Format variants | Directory volume, split volume |
| Images | Map, ImageStream (BEVY) |
| Compression | LZ4, Snappy |
| | Block hashes, symbolic streams |

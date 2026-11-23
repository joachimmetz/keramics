# ffs

The ffs module provides read-only support for the
[Fast File System (FFS)](https://keramics.github.io/ffs.html) on-disk storage layout used by the
[Unix File System (UFS)](https://keramics.github.io/ufs.html).

Supported features:

| Category | Feature(s) |
| --- | --- |
| Format version | FFS1 (UFS1), FFS2 (UFS2), FFS2EA (UFS2EA) |
| Byte order | Little-endian, big-endian |
| Layout | Superblock probing, cylinder groups, blocks and fragments, sparse ranges |

Unsupported features:

| Category | Feature(s) |
| --- | --- |
| Integrity | Metadata CRC-32C check-hash verification |
| Journaling | Soft Updates Journaling (SU+J), WAPBL |
| Snapshots | Active snapshot reconstruction |

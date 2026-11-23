# ufs

The ufs module provides read-only support for the
[Unix File System (UFS)](https://keramics.github.io/ufs.html) format.

Supported features:

| Category | Feature(s) |
| --- | --- |
| Format version | 1 (UFS1), 2 (UFS2, UFS2EA) |
| Storage layout | [Fast File System (FFS)](https://keramics.github.io/ffs.html) (FFS1, FFS2, FFS2EA) |
| Byte order | Little-endian, big-endian |
| Directory format | 4.2BSD, 4.4BSD |
| Extended attributes | UFS2 native extended attributes |

Unsupported features:

| Category | Feature(s) |
| --- | --- |
| Storage layout | Log-structured File System (LFS) |
| Extended attributes | UFS1 .attribute backing files |
| | Journaling (Soft Updates Journaling, WAPBL) |
| | Metadata check-hash verification |
| | Snapshots |

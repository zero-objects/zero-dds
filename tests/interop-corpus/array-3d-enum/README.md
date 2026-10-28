# array-3d-enum

**Construct** — three-dimensional array of `enum`

A sixteen-bit enumeration in three dimensions. The bit bound decides the element width, the dimensions decide the count, and the delimiter question decides whether four bytes precede it — three independent decisions in one sixteen-byte member.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 21 bytes — `1000000002000000010002000000010002000000ab` |
| `--encoding xcdr2 --endian be` | 21 bytes — `0000001000020000000100020000000100020000ab` |
| `--encoding xcdr1 --endian le` | 17 bytes — `02000000010002000000010002000000ab` |
| `--encoding xcdr1 --endian be` | 17 bytes — `00020000000100020000000100020000ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

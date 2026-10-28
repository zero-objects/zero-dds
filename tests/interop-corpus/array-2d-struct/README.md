# array-2d-struct

**Construct** — two-dimensional array of a structure

Four appendable elements in two dimensions: one collection delimiter for the whole array under the specification, four element delimiters, and no per-row delimiter — a multi-dimensional array is one collection, not an array of arrays. An implementation that delimits per row writes one DHEADER too many.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 56 bytes — `2e00000006000000010000000200000006000000030000000400000006000000fffffffffeff000006000000ffffff7f00800000ffffffff` |
| `--encoding xcdr2 --endian be` | 56 bytes — `0000002e00000006000000010002000000000006000000030004000000000006fffffffffffe0000000000067fffffff80000000ffffffff` |
| `--encoding xcdr1 --endian le` | 36 bytes — `01000000020000000300000004000000fffffffffeff0000ffffff7f00800000ffffffff` |
| `--encoding xcdr1 --endian be` | 36 bytes — `00000001000200000000000300040000fffffffffffe00007fffffff80000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

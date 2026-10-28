# array-2d-string

**Construct** — two-dimensional array of `string`

Variable-length elements in two dimensions, with an empty one first so no two element boundaries are at the same spacing.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 48 bytes — `250000000100000000000000020000006100000006000000e188b4c3a4000000050000006162636400000000ffffffff` |
| `--encoding xcdr2 --endian be` | 48 bytes — `000000250000000100000000000000026100000000000006e188b4c3a4000000000000056162636400000000ffffffff` |
| `--encoding xcdr1 --endian le` | 44 bytes — `0100000000000000020000006100000006000000e188b4c3a4000000050000006162636400000000ffffffff` |
| `--encoding xcdr1 --endian be` | 44 bytes — `0000000100000000000000026100000000000006e188b4c3a4000000000000056162636400000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

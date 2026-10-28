# array-1d-union

**Construct** — one-dimensional array of `union`

Two array elements of different sizes. An array of unions has no fixed stride, so an implementation that computes element offsets by multiplication rather than by serialising in order reads the second element from inside the first.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `1c00000008000000010000007929edff0c0000000900000000000000000004c0ffffffff` |
| `--encoding xcdr2 --endian be` | 36 bytes — `0000001c0000000800000001ffed29790000000c00000009c004000000000000ffffffff` |
| `--encoding xcdr1 --endian le` | 28 bytes — `010000007929edff090000000000000000000000000004c0ffffffff` |
| `--encoding xcdr1 --endian be` | 28 bytes — `00000001ffed29790000000900000000c004000000000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

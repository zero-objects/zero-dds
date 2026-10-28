# map-value-string

**Construct** — `map` with a string value

String values, one empty. The empty value is five bytes — a length of one and a NUL — and not four, which is the same off-by-one `string-unbounded-empty` isolates, reached through a different container.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 44 bytes — `2200000002000000010000000100000000000000020000000a000000c3bc62657220e188b4000000ffffffff` |
| `--encoding xcdr2 --endian be` | 44 bytes — `0000002200000002000000010000000100000000000000020000000ac3bc62657220e188b4000000ffffffff` |
| `--encoding xcdr1 --endian le` | 40 bytes — `02000000010000000100000000000000020000000a000000c3bc62657220e188b4000000ffffffff` |
| `--encoding xcdr1 --endian be` | 40 bytes — `00000002000000010000000100000000000000020000000ac3bc62657220e188b4000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

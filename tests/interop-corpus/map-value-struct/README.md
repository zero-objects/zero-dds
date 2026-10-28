# map-value-struct

**Construct** — `map` with a structure value

An appendable value type, so each value carries its own DHEADER inside the map. A map is not a primitive collection, so the map itself takes one too — three delimiters in a two-entry map.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 44 bytes — `220000000200000001000000060000007929edfffeff000002000000060000000300000004000000ffffffff` |
| `--encoding xcdr2 --endian be` | 44 bytes — `00000022000000020000000100000006ffed2979fffe000000000002000000060000000300040000ffffffff` |
| `--encoding xcdr1 --endian le` | 32 bytes — `02000000010000007929edfffeff0000020000000300000004000000ffffffff` |
| `--encoding xcdr1 --endian be` | 32 bytes — `0000000200000001ffed2979fffe0000000000020000000300040000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# map-nested

**Construct** — `map` with a map value

A map of maps, the second inner one empty. Nested counts that a flattening implementation merges into one.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `1c0000000200000001000000010000000a0000007929edff0200000000000000ffffffff` |
| `--encoding xcdr2 --endian be` | 36 bytes — `0000001c0000000200000001000000010000000affed29790000000200000000ffffffff` |
| `--encoding xcdr1 --endian le` | 32 bytes — `0200000001000000010000000a0000007929edff0200000000000000ffffffff` |
| `--encoding xcdr1 --endian be` | 32 bytes — `0000000200000001000000010000000affed29790000000200000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

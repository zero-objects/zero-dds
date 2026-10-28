# map-value-sequence

**Construct** — `map` with a sequence value

A variable-length value, one of them empty. The map's own length cannot be derived from the entry count here, which is what separates an implementation that measures from one that computes.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `1c00000002000000010000000000000002000000020000007929edffffffff7fffffffff` |
| `--encoding xcdr2 --endian be` | 36 bytes — `0000001c0000000200000001000000000000000200000002ffed29797fffffffffffffff` |
| `--encoding xcdr1 --endian le` | 32 bytes — `02000000010000000000000002000000020000007929edffffffff7fffffffff` |
| `--encoding xcdr1 --endian be` | 32 bytes — `0000000200000001000000000000000200000002ffed29797fffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

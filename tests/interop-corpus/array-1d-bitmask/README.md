# array-1d-bitmask

**Construct** — one-dimensional array of `bitmask`

Three one-byte bitmasks, then a long: the padding before the long is one byte, so an implementation that gives the bitmask its declared bit bound rather than a machine word is the only one that lands the long at offset four.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `0300000001810000ffffffff` |
| `--encoding xcdr2 --endian be` | 12 bytes — `0000000301810000ffffffff` |
| `--encoding xcdr1 --endian le` | 8 bytes — `01810000ffffffff` |
| `--encoding xcdr1 --endian be` | 8 bytes — `01810000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

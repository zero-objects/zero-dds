# bitset-basic

**Construct** — `bitset` with two fields

Eight declared bits, so one byte on the wire. The fields are packed from the least significant bit upward in declaration order, giving `low=5` in bits 0-2 and `high=17` in bits 3-7: `10001101`, `0x8d`. An implementation that packs from the top, or that gives each field its own byte, produces a byte that decodes to different field values without any length difference.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 2 bytes — `8dab` |
| `--encoding xcdr2 --endian be` | 2 bytes — `8dab` |
| `--encoding xcdr1 --endian le` | 2 bytes — `8dab` |
| `--encoding xcdr1 --endian be` | 2 bytes — `8dab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# array-1d-enum

**Construct** — one-dimensional array of `enum`

`vendor_bytes.md` §1 measures this exact shape: Cyclone writes the collection delimiter and reaches twelve bytes, RTI and Fast DDS omit it and reach eight. There is no set of bytes all three accept, which is why `CollectionDelimiterPolicy` exists and why this case is in the corpus twice over — once here and once as a sequence.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `0c000000020000000000000001000000ffffffff` |
| `--encoding xcdr2 --endian be` | 20 bytes — `0000000c000000020000000000000001ffffffff` |
| `--encoding xcdr1 --endian le` | 16 bytes — `020000000000000001000000ffffffff` |
| `--encoding xcdr1 --endian be` | 16 bytes — `000000020000000000000001ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

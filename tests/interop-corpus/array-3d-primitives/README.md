# array-3d-primitives

**Construct** — three-dimensional array of each primitive kind

Three dimensions, eight elements, arranged so that no two of the four inner pairs are equal. Row-major over three indices is the rule; column-major, or a middle-dimension transposition, both survive a two-dimensional test and both fail here.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 376 bytes |
| `--encoding xcdr2 --endian be` | 376 bytes |
| `--encoding xcdr1 --endian le` | 376 bytes |
| `--encoding xcdr1 --endian be` | 376 bytes |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

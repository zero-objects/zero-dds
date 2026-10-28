# primitive-ushort

**Construct** — primitive `unsigned short`

The primitive alone, twice, with two different values, so a wrong width shows as a wrong total length and a wrong byte order shows in the first member. The second member also pins the stride: a reader that gets the width right and the advance wrong reads `short_unsigned_b` from the wrong offset.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 4 bytes — `efbe0300` |
| `--encoding xcdr2 --endian be` | 4 bytes — `beef0003` |
| `--encoding xcdr1 --endian le` | 4 bytes — `efbe0300` |
| `--encoding xcdr1 --endian be` | 4 bytes — `beef0003` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

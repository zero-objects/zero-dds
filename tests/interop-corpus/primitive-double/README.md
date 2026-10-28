# primitive-double

**Construct** — primitive `double`

The primitive alone, twice, with two different values, so a wrong width shows as a wrong total length and a wrong byte order shows in the first member. The second member also pins the stride: a reader that gets the width right and the advance wrong reads `double_float_b` from the wrong offset.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `182d4454fb21094000000000000004c0` |
| `--encoding xcdr2 --endian be` | 16 bytes — `400921fb54442d18c004000000000000` |
| `--encoding xcdr1 --endian le` | 16 bytes — `182d4454fb21094000000000000004c0` |
| `--encoding xcdr1 --endian be` | 16 bytes — `400921fb54442d18c004000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

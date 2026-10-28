# primitive-octet

**Construct** — primitive `octet`

The primitive alone, twice, with two different values, so a wrong width shows as a wrong total length and a wrong byte order shows in the first member. The second member also pins the stride: a reader that gets the width right and the advance wrong reads `raw_octet_b` from the wrong offset.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 2 bytes — `ab01` |
| `--encoding xcdr2 --endian be` | 2 bytes — `ab01` |
| `--encoding xcdr1 --endian le` | 2 bytes — `ab01` |
| `--encoding xcdr1 --endian be` | 2 bytes — `ab01` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# primitive-longlong

**Construct** — primitive `long long`

The primitive alone, twice, with two different values, so a wrong width shows as a wrong total length and a wrong byte order shows in the first member. The second member also pins the stride: a reader that gets the width right and the advance wrong reads `llong_signed_b` from the wrong offset.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `bd08370b1affffffffffffffffffff7f` |
| `--encoding xcdr2 --endian be` | 16 bytes — `ffffff1a0b3708bd7fffffffffffffff` |
| `--encoding xcdr1 --endian le` | 16 bytes — `bd08370b1affffffffffffffffffff7f` |
| `--encoding xcdr1 --endian be` | 16 bytes — `ffffff1a0b3708bd7fffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

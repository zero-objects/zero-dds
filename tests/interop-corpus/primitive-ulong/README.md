# primitive-ulong

**Construct** — primitive `unsigned long`

The primitive alone, twice, with two different values, so a wrong width shows as a wrong total length and a wrong byte order shows in the first member. The second member also pins the stride: a reader that gets the width right and the advance wrong reads `long_unsigned_b` from the wrong offset.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 8 bytes — `efbeadde05000000` |
| `--encoding xcdr2 --endian be` | 8 bytes — `deadbeef00000005` |
| `--encoding xcdr1 --endian le` | 8 bytes — `efbeadde05000000` |
| `--encoding xcdr1 --endian be` | 8 bytes — `deadbeef00000005` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

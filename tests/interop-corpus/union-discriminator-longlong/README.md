# union-discriminator-longlong

**Construct** — `union` over `long long`

An eight-byte discriminator: the alignment before the selected member differs between XCDR1 and XCDR2, so this case carries the max-alignment question as well as the discriminator one. The union is a member rather than the top-level type so that the padding between the discriminator and the selected member, and between the union and what follows it, are both on the wire.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `0c000000bd08370b1affffff7929edffffffffff` |
| `--encoding xcdr2 --endian be` | 20 bytes — `0000000cffffff1a0b3708bdffed2979ffffffff` |
| `--encoding xcdr1 --endian le` | 16 bytes — `bd08370b1affffff7929edffffffffff` |
| `--encoding xcdr1 --endian be` | 16 bytes — `ffffff1a0b3708bdffed2979ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

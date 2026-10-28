# union-discriminator-uint8

**Construct** — `union` over `uint8`

An unsigned one-byte discriminator with a label above 127. The union is a member rather than the top-level type so that the padding between the discriminator and the selected member, and between the union and what follows it, are both on the wire.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `08000000fe0000007929edffffffffff` |
| `--encoding xcdr2 --endian be` | 16 bytes — `00000008fe000000ffed2979ffffffff` |
| `--encoding xcdr1 --endian le` | 12 bytes — `fe0000007929edffffffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `fe000000ffed2979ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

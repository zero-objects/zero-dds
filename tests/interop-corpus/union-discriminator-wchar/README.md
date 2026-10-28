# union-discriminator-wchar

**Construct** — `union` over `wchar`

A wide-character discriminator: two bytes, and a label outside ASCII so an implementation that narrows the label to a byte selects the wrong branch. The union is a member rather than the top-level type so that the padding between the discriminator and the selected member, and between the union and what follows it, are both on the wire.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `08000000e40000007929edffffffffff` |
| `--encoding xcdr2 --endian be` | 16 bytes — `0000000800e40000ffed2979ffffffff` |
| `--encoding xcdr1 --endian le` | 12 bytes — `e40000007929edffffffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `00e40000ffed2979ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

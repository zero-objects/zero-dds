# union-discriminator-long

**Construct** — `union` over `long`

The common case, with a negative label so a sign error does not merely change the number but changes the branch. The union is a member rather than the top-level type so that the padding between the discriminator and the selected member, and between the union and what follows it, are both on the wire.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `080000007929edff7929edffffffffff` |
| `--encoding xcdr2 --endian be` | 16 bytes — `00000008ffed2979ffed2979ffffffff` |
| `--encoding xcdr1 --endian le` | 12 bytes — `7929edff7929edffffffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `ffed2979ffed2979ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

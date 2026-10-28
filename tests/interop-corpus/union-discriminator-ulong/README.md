# union-discriminator-ulong

**Construct** — `union` over `unsigned long`

An unsigned four-byte discriminator at `0xdeadbeef`, above the signed limit. The union is a member rather than the top-level type so that the padding between the discriminator and the selected member, and between the union and what follows it, are both on the wire.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `08000000efbeadde7929edffffffffff` |
| `--encoding xcdr2 --endian be` | 16 bytes — `00000008deadbeefffed2979ffffffff` |
| `--encoding xcdr1 --endian le` | 12 bytes — `efbeadde7929edffffffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `deadbeefffed2979ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

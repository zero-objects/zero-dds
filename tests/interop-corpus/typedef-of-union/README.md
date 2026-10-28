# typedef-of-union

**Construct** — `typedef` of a union

A union behind an alias. Same transparency question as the structure case, with the discriminator's alignment on top: the alias must not change where the discriminator lands.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `08000000010000007929edffffffffff` |
| `--encoding xcdr2 --endian be` | 16 bytes — `0000000800000001ffed2979ffffffff` |
| `--encoding xcdr1 --endian le` | 12 bytes — `010000007929edffffffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `00000001ffed2979ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

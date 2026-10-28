# struct-inheritance-mutable

**Construct** — mutable structure with a base

A mutable derived type. The base's members come first, in the base's declaration order, and the derived type does not restart the padding — the wire form is one flat aggregate, not a nested one. For the mutable case the member ids must not collide between base and derived. This case was written to expose a silent failure and did: the probe wrote only the derived members and skipped the base ones without saying so. Fixed; the case now guards the fix.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 37 bytes — `21000000000000207929edff01000010feff000002000030182d4454fb21094003000000ab` |
| `--encoding xcdr2 --endian be` | 37 bytes — `0000002120000000ffed297910000001fffe000030000002400921fb54442d1800000003ab` |
| `--encoding xcdr1 --endian le` | 40 bytes — `000004007929edff01000200feff000002000800182d4454fb21094003000100ab000000027f0000` |
| `--encoding xcdr1 --endian be` | 40 bytes — `00000004ffed297900010002fffe000000020008400921fb54442d1800030001ab0000007f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# struct-inheritance-final

**Construct** — final structure with a base

A final derived type. The base's members come first, in the base's declaration order, and the derived type does not restart the padding — the wire form is one flat aggregate, not a nested one. For the mutable case the member ids must not collide between base and derived. This case was written to expose a silent failure and did: the probe wrote only the derived members and skipped the base ones without saying so. Fixed; the case now guards the fix.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 17 bytes — `7929edfffeff0000182d4454fb210940ab` |
| `--encoding xcdr2 --endian be` | 17 bytes — `ffed2979fffe0000400921fb54442d18ab` |
| `--encoding xcdr1 --endian le` | 17 bytes — `7929edfffeff0000182d4454fb210940ab` |
| `--encoding xcdr1 --endian be` | 17 bytes — `ffed2979fffe0000400921fb54442d18ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

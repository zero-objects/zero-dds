# union-with-default

**Construct** — `union` with a `default` branch, default selected

The `vendor_bytes.md` `union_case` shape with the discriminator falling through to `default`. The discriminator on the wire is 9, the value the sample gives — not the first unused label, and not a sentinel. An implementation that normalises an unmatched discriminator to something canonical writes a different four bytes.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `0c00000009000000182d4454fb210940ffffffff` |
| `--encoding xcdr2 --endian be` | 20 bytes — `0000000c00000009400921fb54442d18ffffffff` |
| `--encoding xcdr1 --endian le` | 20 bytes — `0900000000000000182d4454fb210940ffffffff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `0000000900000000400921fb54442d18ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

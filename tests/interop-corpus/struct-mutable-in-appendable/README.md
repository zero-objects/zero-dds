# struct-mutable-in-appendable

**Construct** — `@mutable` inside `@appendable`

Two framings nested: the outer DHEADER covers the whole sample, and inside it the inner type writes an EMHEADER per member and a sentinel. The outer length has to count the inner headers, which an implementation that computes extents from declared member sizes gets wrong. §3 of the interop spec names this combination specifically.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `180000000e000000000000207929edff01000010feff0000ffffffff` |
| `--encoding xcdr2 --endian be` | 28 bytes — `000000180000000e20000000ffed297910000001fffe0000ffffffff` |
| `--encoding xcdr1 --endian le` | 24 bytes — `000004007929edff01000200feff0000027f0000ffffffff` |
| `--encoding xcdr1 --endian be` | 24 bytes — `00000004ffed297900010002fffe00007f020000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

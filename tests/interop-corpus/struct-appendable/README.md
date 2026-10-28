# struct-appendable

**Construct** — appendable structure

The `ext_appendable` shape from `vendor_bytes.md` §1: eighteen bytes under XCDR1 for every extensibility, and under XCDR2 fourteen for final and eighteen for appendable, the difference being the DHEADER. Mutable adds a header per member and a sentinel. Three members whose alignments are 4, 8 and 2 means the padding is different at every step.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 18 bytes — `0e0000007929edff182d4454fb210940feff` |
| `--encoding xcdr2 --endian be` | 18 bytes — `0000000effed2979400921fb54442d18fffe` |
| `--encoding xcdr1 --endian le` | 18 bytes — `7929edff00000000182d4454fb210940feff` |
| `--encoding xcdr1 --endian be` | 18 bytes — `ffed297900000000400921fb54442d18fffe` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

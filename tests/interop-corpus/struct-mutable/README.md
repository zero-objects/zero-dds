# struct-mutable

**Construct** — mutable structure

The `ext_mutable` shape from `vendor_bytes.md` §1: eighteen bytes under XCDR1 for every extensibility, and under XCDR2 fourteen for final and eighteen for appendable, the difference being the DHEADER. Mutable adds a header per member and a sentinel. Three members whose alignments are 4, 8 and 2 means the padding is different at every step.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 30 bytes — `1a000000000000207929edff01000030182d4454fb21094002000010feff` |
| `--encoding xcdr2 --endian be` | 30 bytes — `0000001a20000000ffed297930000001400921fb54442d1810000002fffe` |
| `--encoding xcdr1 --endian le` | 32 bytes — `000004007929edff01000800182d4454fb21094002000200feff0000027f0000` |
| `--encoding xcdr1 --endian be` | 32 bytes — `00000004ffed297900010008400921fb54442d1800020002fffe00007f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

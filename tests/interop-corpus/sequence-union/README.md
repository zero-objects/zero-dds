# sequence-union

**Construct** — `sequence<union>`

Elements of different sizes in one sequence: the `long` branch is eight bytes, the `double` branch twelve after alignment. An implementation that computes a sequence's length from the element type rather than from the elements gets the collection DHEADER wrong here and only here.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 52 bytes — `2c0000000300000008000000010000007929edff0c00000009000000182d4454fb2109400800000001000000ffffff7fffffffff` |
| `--encoding xcdr2 --endian be` | 52 bytes — `0000002c000000030000000800000001ffed29790000000c00000009400921fb54442d1800000008000000017fffffffffffffff` |
| `--encoding xcdr1 --endian le` | 36 bytes — `03000000010000007929edff09000000182d4454fb21094001000000ffffff7fffffffff` |
| `--encoding xcdr1 --endian be` | 36 bytes — `0000000300000001ffed297900000009400921fb54442d18000000017fffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

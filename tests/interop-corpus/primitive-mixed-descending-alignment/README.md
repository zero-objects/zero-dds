# primitive-mixed-descending-alignment

**Construct** — every primitive, widest first

The same fourteen primitives in the opposite order. Widest first means almost no padding is needed, so this and `primitive-mixed` have different lengths for the same set of members — an implementation that sorts members by alignment instead of keeping declaration order produces the same bytes for both, which is the bug this pair catches.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 47 bytes — `00000000000004c0000020400700000000000000ffffffffffffff7f05000000ffffff7f0300ff7f34127e7f030100` |
| `--encoding xcdr2 --endian be` | 47 bytes — `c0040000000000004020000000000000000000077fffffffffffffff000000057fffffff00037fff12347e7f030100` |
| `--encoding xcdr1 --endian le` | 51 bytes — `00000000000004c000002040000000000700000000000000ffffffffffffff7f05000000ffffff7f0300ff7f34127e7f030100` |
| `--encoding xcdr1 --endian be` | 51 bytes — `c004000000000000402000000000000000000000000000077fffffffffffffff000000057fffffff00037fff12347e7f030100` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

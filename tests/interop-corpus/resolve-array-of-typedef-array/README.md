# resolve-array-of-typedef-array

**Construct** — an array whose element is a typedef of an array

`typedef long Row[3]; Row grid[2];` — two dimensions written in two places. XTypes 1.3 §7.4.3.5 makes a multi-dimensional array one flat run on the wire, so the two have to fold into 2x3 and not nest. An implementation that emitted a sequence of arrays writes a length nobody asked for.

## The axis

One of the resolution and expansion edge cases: shapes where two
implementations can each do something defensible and produce different
octets. They are here rather than in a front-end test because the
disagreement is **visible on the wire** — a name bound to the wrong
declaration, an alias not followed, a dimension not folded, all of them
change what is written and none of them is reported as an error.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `18000000010000000200000003000000040000000500000006000000` |
| `--encoding xcdr2 --endian be` | 28 bytes — `00000018000000010000000200000003000000040000000500000006` |
| `--encoding xcdr1 --endian le` | 24 bytes — `010000000200000003000000040000000500000006000000` |
| `--encoding xcdr1 --endian be` | 24 bytes — `000000010000000200000003000000040000000500000006` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

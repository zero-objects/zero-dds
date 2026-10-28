# resolve-const-from-const

**Construct** — a sequence bound computed from another constant

`const long BASE = 4; const long CAP = BASE * 2;` and `sequence<long, CAP>`. The bound is an expression over a name, so evaluation has to follow the reference and then do the arithmetic. A bound that silently became zero or unbounded changes what the writer accepts.

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
| `--encoding xcdr2 --endian le` | 16 bytes — `0c000000020000000100000002000000` |
| `--encoding xcdr2 --endian be` | 16 bytes — `0000000c000000020000000100000002` |
| `--encoding xcdr1 --endian le` | 12 bytes — `020000000100000002000000` |
| `--encoding xcdr1 --endian be` | 12 bytes — `000000020000000100000002` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

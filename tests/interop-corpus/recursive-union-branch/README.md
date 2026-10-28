# recursive-union-branch

**Construct** — a cycle that closes through a union branch

The §7.2.2.4.3 collection rule reached through a union: the struct holds the union, one branch of the union holds a sequence of the struct. A planner that descends branches before it looks for the collection sees an unbounded type.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 44 bytes — `2800000007000000200000000100000018000000010000001000000008000000080000000000000003000000` |
| `--encoding xcdr2 --endian be` | 44 bytes — `0000002800000007000000200000000100000018000000010000001000000008000000080000000000000003` |
| `--encoding xcdr1 --endian le` | 24 bytes — `070000000100000001000000080000000000000003000000` |
| `--encoding xcdr1 --endian be` | 24 bytes — `000000070000000100000001000000080000000000000003` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

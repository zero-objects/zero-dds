# recursive-optional

**Construct** — a self-reference held by `@optional` alone

XTypes 1.3 §7.3.1.2.1.3 (`@optional`): an absent optional member ends a value, so `@mutable struct Node { long value; @optional Node next; }` has a finite value without a collection or `@external` on the cycle. The twin of `recursive-optional-external` without Table 21's annotation: on the wire the two are the same, and the bytes below are identical to that case's; what differs is the binding, which has to hold `next` behind an indirection of its own because nothing in the IDL asks for one.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 32 bytes — `1c0000000000002007000000010000400c000000080000000000002008000000` |
| `--encoding xcdr2 --endian be` | 32 bytes — `0000001c2000000000000007400000010000000c000000082000000000000008` |
| `--encoding xcdr1 --endian le` | 28 bytes — `000004000700000001000c000000040008000000027f0000027f0000` |
| `--encoding xcdr1 --endian be` | 28 bytes — `00000004000000070001000c00000004000000087f0200007f020000` |
| `--emit keyhash` | **no** — `m::Node` declares no `@key`, so it has one instance and no KeyHash |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

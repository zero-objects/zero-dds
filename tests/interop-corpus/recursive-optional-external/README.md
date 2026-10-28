# recursive-optional-external

**Construct** — a self-reference that is not a collection

XTypes 1.3 §7.3.1.2.1.3 (`@optional`) and Table 21 (`@external` on a structure member) together make a cycle finite without a collection in it: an absent optional member ends the chain, and `@external` is where the language mapping puts the indirection. This is the shape a linked list has. The map records that a bare `struct S { S inner; }` is refused because it has no finite size; an optional member does have one, and this case is the line between the two. The forward declaration is written because four of the six implementations answered "`Node` was not defined previously" to the form without it.

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

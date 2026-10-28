# recursive-mutual-optional

**Construct** — two types on one cycle, one member `@optional` and one not

XTypes 1.3 §7.3.1.2.1.3: `A.tail` holds a `B` optionally and `B.head` holds an `A` with no annotation; the absent `tail` is what ends a value. `Holder` is `@appendable` and holds a `B`, so a type outside the cycle reaches into it. Three findings came out of this case: a `@mutable` member's length code planned outside the `@optional` barrier and refused the type (`64159547c8d`); a type that holds a member of a recursive component got no TypeIdentifier (`198ad0f4dda`); and its first version named members `b` and `a` beside types `B` and `A`, which OMG IDL 4.2 §7.2.3.1 refuses.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 80 bytes — `4c00000005000000440000000000002001000000010000403400000030000000000000200200000001000040200000001c0000000000002003000000010000400c000000080000000000002004000000` |
| `--encoding xcdr2 --endian be` | 80 bytes — `0000004c00000005000000442000000000000001400000010000003400000030200000000000000240000001000000200000001c2000000000000003400000010000000c000000082000000000000004` |
| `--encoding xcdr1 --endian le` | 64 bytes — `05000000000004000100000001002c00000004000200000001001c00000004000300000001000c000000040004000000027f0000027f0000027f0000027f0000` |
| `--encoding xcdr1 --endian be` | 64 bytes — `0000000500000004000000010001002c00000004000000020001001c00000004000000030001000c00000004000000047f0200007f0200007f0200007f020000` |
| `--emit keyhash` | **no** — `m::Holder` declares no `@key`, so it has one instance and no KeyHash |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# enum-default-literal

**Construct** — `@default_literal` on an enumerator that is not the first

XTypes 1.3 §7.3.1.2.1.10 lets an enumeration name its own default — "the literal annotated `@default_literal`, or the first literal if none is annotated" — and Table 9 makes that value what an unset member of the enumeration takes. Putting it on the second enumerator is the only way to tell the rule from the convention. `docs/omg-xtypes-issues.md` already records the vendors disagreeing about whether the annotation reaches the default value at all.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 9 bytes — `0200000001000000ab` |
| `--encoding xcdr2 --endian be` | 9 bytes — `0000000200000001ab` |
| `--encoding xcdr1 --endian le` | 9 bytes — `0200000001000000ab` |
| `--encoding xcdr1 --endian be` | 9 bytes — `0000000200000001ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# annotation-external-union-branch

**Construct** — `@external` on a union branch

DDS-XTypes 1.3 Table 21 applies `@external` to union members as much as to structure members, and gives it no wire effect in either position: the indirection is the language mapping's. A branch is where the annotation is easiest to lose — the source writes it on the case, and a model that keeps case and member facets apart can drop it between the two without a diagnostic. The bytes must be the plain branch's bytes.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `0e00000001000000060000007929edfffeff000001000000` |
| `--encoding xcdr2 --endian be` | 24 bytes — `0000000e0000000100000006ffed2979fffe000000000001` |
| `--encoding xcdr1 --endian le` | 16 bytes — `010000007929edfffeff000001000000` |
| `--encoding xcdr1 --endian be` | 16 bytes — `00000001ffed2979fffe000000000001` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

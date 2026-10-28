# annotation-external-typedef

**Construct** — `@external` on a member whose type is a typedef

The alias is transparent on the wire (OMG IDL 4.2 §7.4.1.4.4) and the annotation stays the member's (DDS-XTypes 1.3 Table 21), so the bytes must be `annotation-external-struct`'s. The case exists because a binding takes its indirection through the alias — a mapping that resolved the alias first and one that boxed the alias name have to declare the same thing.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `060000007929edfffeff000001000000` |
| `--encoding xcdr2 --endian be` | 16 bytes — `00000006ffed2979fffe000000000001` |
| `--encoding xcdr1 --endian le` | 12 bytes — `7929edfffeff000001000000` |
| `--encoding xcdr1 --endian be` | 12 bytes — `ffed2979fffe000000000001` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# empty-struct-as-base

**Construct** — an empty structure as a base type

XTypes 1.3 §7.4.1.1.5.1 puts a base's members first, as if they had been declared in the derived type. A base with none contributes nothing to that order — but the derived type's own extensibility still decides its delimiter, which is what separates this from a type with no base at all.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 10 bytes — `060000007929edfffeff` |
| `--encoding xcdr2 --endian be` | 10 bytes — `00000006ffed2979fffe` |
| `--encoding xcdr1 --endian le` | 6 bytes — `7929edfffeff` |
| `--encoding xcdr1 --endian be` | 6 bytes — `ffed2979fffe` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

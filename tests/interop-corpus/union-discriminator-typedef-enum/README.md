# union-discriminator-typedef-enum

**Construct** — a union whose switch type is a typedef

IDL 4.2 §7.4.1.4.4.2.4 lets the switch type be a scoped name, and §7.4.1.4.4.5 makes a typedef an alias for the type it names. The case labels are still the enumerators, which a front end that resolves the alias only after it has read the labels cannot match.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `0c00000001000000182d4454fb210940ffffffff` |
| `--encoding xcdr2 --endian be` | 20 bytes — `0000000c00000001400921fb54442d18ffffffff` |
| `--encoding xcdr1 --endian le` | 20 bytes — `0100000000000000182d4454fb210940ffffffff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `0000000100000000400921fb54442d18ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

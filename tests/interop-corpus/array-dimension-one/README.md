# array-dimension-one

**Construct** — an array whose dimension is one

IDL 4.2 §7.4.1.4.4.4 makes a dimension of one a legal array and not the element type. It is the smallest array there is, and the one a language mapping is most likely to collapse into a scalar — which changes the type's TypeIdentifier without changing an octet.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 25 bytes — `7929edff0a00000006000000616c706861000000ffffff7fab` |
| `--encoding xcdr2 --endian be` | 25 bytes — `ffed29790000000a00000006616c7068610000007fffffffab` |
| `--encoding xcdr1 --endian le` | 21 bytes — `7929edff06000000616c706861000000ffffff7fab` |
| `--encoding xcdr1 --endian be` | 21 bytes — `ffed297900000006616c7068610000007fffffffab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

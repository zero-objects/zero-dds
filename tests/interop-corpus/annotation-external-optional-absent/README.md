# annotation-external-optional-absent

**Construct** — `@optional @external` on one member, left out

The absent form of `annotation-external-optional-present`: the presence marker is the plain `@optional` member's — DDS-XTypes 1.3 §7.4.3.5.2 — and the indirection carries the absence in the binding, not on the wire. A binding that stacks a second absence state over the indirection has one it can never write.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `7929edff00000000feffffff` |
| `--encoding xcdr2 --endian be` | 12 bytes — `ffed297900000000fffffffe` |
| `--encoding xcdr1 --endian le` | 12 bytes — `7929edff01000000feffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `ffed297900010000fffffffe` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

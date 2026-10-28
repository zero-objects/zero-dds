# bitmask-bitbound-seventeen

**Construct** — a `bitmask` bit bound that crosses an octet width

The same two clauses one width up: seventeen bits do not fit in two octets, so §7.4.3.4.2 makes the wire form four. A bound just past a boundary is where an implementation that rounds down rather than up writes a narrower field than the positions need.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 9 bytes — `0100000001000100ab` |
| `--encoding xcdr2 --endian be` | 9 bytes — `0000000100010001ab` |
| `--encoding xcdr1 --endian le` | 9 bytes — `0100000001000100ab` |
| `--encoding xcdr1 --endian be` | 9 bytes — `0000000100010001ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

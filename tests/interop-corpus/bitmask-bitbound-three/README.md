# bitmask-bitbound-three

**Construct** — a `bitmask` whose bit bound is not a multiple of eight

XTypes 1.3 §7.3.1.2.1.9 admits any `@bit_bound` from 1 to 64 for a bitmask, and §7.4.3.4.2 puts the smallest unsigned integer that holds the bound on the wire. Three bits are therefore one octet, and the corpus otherwise only asks bounds that are already a whole number of octets, where the two rules cannot be told apart.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 3 bytes — `0105ab` |
| `--encoding xcdr2 --endian be` | 3 bytes — `0105ab` |
| `--encoding xcdr1 --endian le` | 3 bytes — `0105ab` |
| `--encoding xcdr1 --endian be` | 3 bytes — `0105ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

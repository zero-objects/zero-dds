# enum-bitbound-twelve

**Construct** — an `enum` whose bit bound is not a multiple of eight

XTypes 1.3 §7.3.1.2.1.9 gives an enumeration a bound from 1 to 32 and §7.4.3.4.2 serialises it in the smallest signed integer that holds it. Twelve bits are two octets. Every enumeration in the corpus is bound at 8, 16 or 32, where the bound and the wire width are the same number.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 5 bytes — `02000000ab` |
| `--encoding xcdr2 --endian be` | 5 bytes — `00020000ab` |
| `--encoding xcdr1 --endian le` | 5 bytes — `02000000ab` |
| `--encoding xcdr1 --endian be` | 5 bytes — `00020000ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# float-negative-zero-denormal

**Construct** — IEEE 754 values that a decimal round trip loses

XTypes 1.3 §7.4.3.4.1 makes `float` and `double` IEEE 754 binary32 and binary64. Negative zero has the sign bit set and compares equal to zero; the smallest subnormal has an all-zero exponent. Both survive the wire and neither survives a path that goes through a decimal text form, which is how a value reaches several of the vendor probes.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `000000800000000000000080010000000100000000000000ffffffff` |
| `--encoding xcdr2 --endian be` | 28 bytes — `800000008000000000000000000000010000000000000001ffffffff` |
| `--encoding xcdr1 --endian le` | 36 bytes — `0000008000000000000000000000008001000000000000000100000000000000ffffffff` |
| `--encoding xcdr1 --endian be` | 36 bytes — `8000000000000000800000000000000000000001000000000000000000000001ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

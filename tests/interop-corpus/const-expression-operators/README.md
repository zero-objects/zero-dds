# const-expression-operators

**Construct** — a constant expression using shifts, bitwise operators and a remainder

IDL 4.2 §7.4.1.4.4.6 admits `<<`, `>>`, `|`, `&`, `^`, `%` and the arithmetic operators in a constant expression, and §7.2.6 admits hexadecimal and octal integer literals. The corpus computes one bound with a multiplication; every other operator and both non-decimal literal forms are unasked, and each is a place a front end can differ without differing on any type.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 40 bytes — `2400000001000000feffffff03000000fcffffff03000000070000000800000009000000ffffffff` |
| `--encoding xcdr2 --endian be` | 40 bytes — `0000002400000001fffffffe00000003fffffffc00000003000000070000000800000009ffffffff` |
| `--encoding xcdr1 --endian le` | 36 bytes — `01000000feffffff03000000fcffffff03000000070000000800000009000000ffffffff` |
| `--encoding xcdr1 --endian be` | 36 bytes — `00000001fffffffe00000003fffffffc00000003000000070000000800000009ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

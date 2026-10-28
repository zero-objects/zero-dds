# integer-extremes

**Construct** — every integer type at both ends of its range

IDL 4.2 §7.4.1.4.4.1 fixes the range of each integer type. The ends are where a narrowing conversion, a sign extension and a literal parser each show: `-9223372036854775808` has no positive counterpart to negate, and `18446744073709551615` does not fit the signed type a generator may hold it in.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 32 bytes — `80ff0080ffff000000000080ffffffff0000000000000080ffffffffffffffff` |
| `--encoding xcdr2 --endian be` | 32 bytes — `80ff8000ffff000080000000ffffffff8000000000000000ffffffffffffffff` |
| `--encoding xcdr1 --endian le` | 32 bytes — `80ff0080ffff000000000080ffffffff0000000000000080ffffffffffffffff` |
| `--encoding xcdr1 --endian be` | 32 bytes — `80ff8000ffff000080000000ffffffff8000000000000000ffffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

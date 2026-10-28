# empty-struct-in-sequence

**Construct** — an empty structure as a collection element

XTypes 1.3 §7.4.3.5.3 puts a DHEADER ahead of every appendable aggregate, members or none. Three empty elements are therefore three four-octet delimiters behind a length, not nothing behind a length — and an implementation that emits nothing per element produces a stream whose reader cannot count.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 40 bytes — `240000001000000003000000000000000000000000000000080000000000000000000000ffffffff` |
| `--encoding xcdr2 --endian be` | 40 bytes — `000000240000001000000003000000000000000000000000000000080000000000000000ffffffff` |
| `--encoding xcdr1 --endian le` | 8 bytes — `03000000ffffffff` |
| `--encoding xcdr1 --endian be` | 8 bytes — `00000003ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# string-bound-one

**Construct** — a `string` and a `wstring` bounded at one character

IDL 4.2 §7.4.1.4.4.3 counts the bound in characters. XTypes 1.3 §7.4.3.4.3 puts the terminating NUL inside the serialised length, so a full `string<1>` is a length of two. An implementation that sizes a buffer at the bound rather than the bound plus one fails here and nowhere else in the corpus.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `0200000061000000010000000000000002000000df000000ffffffff` |
| `--encoding xcdr2 --endian be` | 28 bytes — `000000026100000000000001000000000000000200df0000ffffffff` |
| `--encoding xcdr1 --endian le` | 28 bytes — `0200000061000000010000000000000002000000df000000ffffffff` |
| `--encoding xcdr1 --endian be` | 28 bytes — `000000026100000000000001000000000000000200df0000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# struct-mutable-in-final

**Construct** — a mutable member inside a final structure

The same clause with the parameter-list form: the `@mutable` member writes a DHEADER and an EMHEADER per member inside an outer that writes neither. A reader that takes the outer's extensibility for the whole sample mis-parses from the first member on.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `0e000000000000207929edff01000010feff0000ffffffff` |
| `--encoding xcdr2 --endian be` | 24 bytes — `0000000e20000000ffed297910000001fffe0000ffffffff` |
| `--encoding xcdr1 --endian le` | 24 bytes — `000004007929edff01000200feff0000027f0000ffffffff` |
| `--encoding xcdr1 --endian be` | 24 bytes — `00000004ffed297900010002fffe00007f020000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

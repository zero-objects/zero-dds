# sequence-bound-one

**Construct** — a `sequence` bounded at one element

IDL 4.2 §7.4.1.4.4.3 admits a bound of one. XTypes 1.3 §7.4.3.4.4 writes the length whatever the bound is, so the bound saves nothing on the wire — and an implementation that treats a one-element bound as an optional scalar writes something else entirely.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `010000007929edff00000000ffffffff` |
| `--encoding xcdr2 --endian be` | 16 bytes — `00000001ffed297900000000ffffffff` |
| `--encoding xcdr1 --endian le` | 16 bytes — `010000007929edff00000000ffffffff` |
| `--encoding xcdr1 --endian be` | 16 bytes — `00000001ffed297900000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# struct-appendable-in-final

**Construct** — an appendable member inside a final structure

XTypes 1.3 §7.4.3.5.3 decides the delimiter per aggregate and not per sample: the `@final` outer writes none and the `@appendable` member writes one. The corpus holds `@final` and `@appendable` inside `@mutable`, and `@mutable` inside `@appendable`, and nothing inside `@final` — where there is no outer delimiter to hide a missing inner one.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `060000007929edfffeff0000ffffffff` |
| `--encoding xcdr2 --endian be` | 16 bytes — `00000006ffed2979fffe0000ffffffff` |
| `--encoding xcdr1 --endian le` | 12 bytes — `7929edfffeff0000ffffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `ffed2979fffe0000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

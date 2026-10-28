# annotation-nested

**Construct** — `@nested` on a member type

XTypes 1.3 §7.3.1.2.1.7 marks a type as one that is only ever a member of another, which decides whether it is eligible to be a topic type (§7.6.2) and what flag its TypeObject carries. `docs/wire-feature-inventory.md` row B6 settles that it changes no octet of a nested use, and this case is that reading as a measurement. The annotation appears nowhere in the corpus.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `7929edfffeff0000ffffffff` |
| `--encoding xcdr2 --endian be` | 12 bytes — `ffed2979fffe0000ffffffff` |
| `--encoding xcdr1 --endian le` | 12 bytes — `7929edfffeff0000ffffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `ffed2979fffe0000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

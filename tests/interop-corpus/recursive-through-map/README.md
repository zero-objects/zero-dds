# recursive-through-map

**Construct** — a type that names itself through a `map`

XTypes 1.3 §7.2.2.4.3 admits recursion through a collection, and a `map` is a collection in the same sentence a `sequence` is: the entries sit behind a length, so the sample terminates even though the type does not. The corpus recurses through a sequence and through a second type, never through a map.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `20000000070000001800000001000000010000000c000000080000000400000000000000` |
| `--encoding xcdr2 --endian be` | 36 bytes — `00000020000000070000001800000001000000010000000c000000080000000400000000` |
| `--encoding xcdr1 --endian le` | 20 bytes — `0700000001000000010000000800000000000000` |
| `--encoding xcdr1 --endian be` | 20 bytes — `0000000700000001000000010000000800000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# recursive-typedef-sequence

**Construct** — the legal self-recursion, written through a typedef

`sequence<NodeRef>` with `typedef Node NodeRef` is `recursive-self-sequence`'s cycle spelled differently. OMG IDL 4.2 §7.4.1.4.4.4.4 admits it because the sequence's length is what makes a value finite — and a planner that compared raw type ids instead of following the alias chain would not see the cycle at all, descend it, and hit its depth limit instead of the sequence's barrier. The bytes must be `recursive-self-sequence`'s.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 32 bytes — `1c0000000700000014000000010000000c000000080000000400000000000000` |
| `--encoding xcdr2 --endian be` | 32 bytes — `0000001c0000000700000014000000010000000c000000080000000400000000` |
| `--encoding xcdr1 --endian le` | 16 bytes — `07000000010000000800000000000000` |
| `--encoding xcdr1 --endian be` | 16 bytes — `00000007000000010000000800000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# annotation-try-construct

**Construct** — all four spellings of `@try_construct` in one type

XTypes 1.3 §7.3.1.2.1.12 gives a member DISCARD, USE_DEFAULT or TRIM, and declares the annotation as `TryConstructFailAction value default USE_DEFAULT` — so a **bare** `@try_construct` is USE_DEFAULT while an **absent** one is DISCARD, and the two defaults are different. That is the fourth spelling and the one a corpus without this case never asks. None of the four changes what a writer puts on the wire, which is the assertion: a column whose octets move when the annotation does has put a reader's rule into a writer.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 56 bytes — `34000000000000207929edff010000500400000061626300020000500400000078797a0003000050040000007071720004000020ffffffff` |
| `--encoding xcdr2 --endian be` | 56 bytes — `0000003420000000ffed2979500000010000000461626300500000020000000478797a0050000003000000047071720020000004ffffffff` |
| `--encoding xcdr1 --endian le` | 56 bytes — `000004007929edff010008000400000061626300020008000400000078797a0003000800040000007071720004000400ffffffff027f0000` |
| `--encoding xcdr1 --endian be` | 56 bytes — `00000004ffed2979000100080000000461626300000200080000000478797a0000030008000000047071720000040004ffffffff7f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

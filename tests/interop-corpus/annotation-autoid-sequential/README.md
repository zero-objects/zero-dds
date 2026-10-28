# annotation-autoid-sequential

**Construct** — `@autoid(SEQUENTIAL)` interrupted by an explicit `@id`

The other value of the same clause, XTypes 1.3 §7.3.1.2.1.1, which also gives the implicit sequential rule: assignment continues from the last id used, so a member carrying `@id(7)` makes the member after it 8 and not 2. SEQUENTIAL is what an absent `@autoid` means, which makes this the shape every unannotated mutable type in the corpus silently relies on.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 30 bytes — `1a000000000000207929edff07000030182d4454fb21094008000010feff` |
| `--encoding xcdr2 --endian be` | 30 bytes — `0000001a20000000ffed297930000007400921fb54442d1810000008fffe` |
| `--encoding xcdr1 --endian le` | 32 bytes — `000004007929edff07000800182d4454fb21094008000200feff0000027f0000` |
| `--encoding xcdr1 --endian be` | 32 bytes — `00000004ffed297900070008400921fb54442d1800080002fffe00007f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

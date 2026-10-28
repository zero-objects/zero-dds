# annotation-autoid-hash

**Construct** — `@autoid(HASH)` over a whole structure

XTypes 1.3 §7.3.1.2.1.1 makes `@autoid(HASH)` derive every member's id from the hash of its name instead of from its position. Under `@mutable` those ids are what each EMHEADER carries and what a peer matches members by, so the annotation is the whole wire form. The corpus has `@hashid` on single members and no `@autoid` at all.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 30 bytes — `1a0000002c1743237929edff987bca30182d4454fb21094005b04817feff` |
| `--encoding xcdr2 --endian be` | 30 bytes — `0000001a2343172cffed297930ca7b98400921fb54442d181748b005fffe` |
| `--encoding xcdr1 --endian le` | 56 bytes — `017f08002c174303040000007929edff017f0800987bca0008000000182d4454fb210940017f080005b0480702000000feff0000027f0000` |
| `--encoding xcdr1 --endian be` | 56 bytes — `7f0100080343172c00000004ffed29797f01000800ca7b9800000008400921fb54442d187f0100080748b00500000002fffe00007f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

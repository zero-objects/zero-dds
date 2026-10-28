# identifier-escaped-keyword

**Construct** — identifiers escaped with a leading underscore

IDL 4.2 §7.2.3.2 lets a leading underscore escape a keyword so it can be used as an identifier, and says the underscore is **not part of the identifier**. So `_long` names `long`: the member id a `@hashid` derives comes from `long`, and a generator that keeps the underscore puts a different id on the wire.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 40 bytes — `240000000f5264237929edffe8cd7d30182d4454fb210940b36eb615feff0000b46eb625ffffffff` |
| `--encoding xcdr2 --endian be` | 40 bytes — `000000242364520fffed2979307dcde8400921fb54442d1815b66eb3fffe000025b66eb4ffffffff` |
| `--encoding xcdr1 --endian le` | 72 bytes — `017f08000f526403040000007929edff017f0800e8cd7d0008000000182d4454fb210940017f0800b36eb60502000000feff0000017f0800b46eb60504000000ffffffff027f0000` |
| `--encoding xcdr1 --endian be` | 72 bytes — `7f0100080364520f00000004ffed29797f010008007dcde800000008400921fb54442d187f01000805b66eb300000002fffe00007f01000805b66eb400000004ffffffff7f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against. The first member's id, `0x0364520F`, is
MD5("long") per §7.2.3.2, and is the id OpenDDS was measured to put in
its EMHEADER for this case; an earlier recording carried `0x04BF80B9` =
MD5("_long"), which hashed the escape spelling.

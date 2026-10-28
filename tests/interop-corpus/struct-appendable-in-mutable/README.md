# struct-appendable-in-mutable

**Construct** — `@appendable` inside `@mutable`

The other nesting order. The inner type's DHEADER sits inside the outer member's EMHEADER, so two lengths describe overlapping ranges and a member whose length code is chosen from its declared size rather than its written size disagrees with itself.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 32 bytes — `1c000000000000400a000000060000007929edfffeff000001000020ffffffff` |
| `--encoding xcdr2 --endian be` | 32 bytes — `0000001c400000000000000a00000006ffed2979fffe000020000001ffffffff` |
| `--encoding xcdr1 --endian le` | 24 bytes — `000006007929edfffeff000001000400ffffffff027f0000` |
| `--encoding xcdr1 --endian be` | 24 bytes — `00000006ffed2979fffe000000010004ffffffff7f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

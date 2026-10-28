# struct-final-in-mutable

**Construct** — `@final` inside `@mutable`

A final member inside a mutable aggregate: six bytes of content under an EMHEADER that has to carry a length of six, not eight. `vendor_bytes.md` §2 records RTI rounding a member's declared length up to its alignment and Fast DDS not doing so, so the length in that header is a measured divergence and not a detail.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `1800000000000040060000007929edfffeff000001000020ffffffff` |
| `--encoding xcdr2 --endian be` | 28 bytes — `000000184000000000000006ffed2979fffe000020000001ffffffff` |
| `--encoding xcdr1 --endian le` | 24 bytes — `000006007929edfffeff000001000400ffffffff027f0000` |
| `--encoding xcdr1 --endian be` | 24 bytes — `00000006ffed2979fffe000000010004ffffffff7f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

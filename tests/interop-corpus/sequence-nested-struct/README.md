# sequence-nested-struct

**Construct** — `sequence<sequence<struct>>`

Three nested delimiters where an implementation could plausibly write one, two or three: the outer sequence, each inner sequence, and each appendable element. A difference in the count of DHEADERs is the single most common cross-vendor XCDR2 failure and this is the shape that shows it.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 72 bytes — `3e000000030000000e0000000100000006000000010000000200000004000000000000001a0000000200000006000000fffffffffeff0000060000000300000004000000ffffffff` |
| `--encoding xcdr2 --endian be` | 72 bytes — `0000003e000000030000000e0000000100000006000000010002000000000004000000000000001a0000000200000006fffffffffffe0000000000060000000300040000ffffffff` |
| `--encoding xcdr1 --endian le` | 44 bytes — `030000000100000001000000020000000000000002000000fffffffffeff00000300000004000000ffffffff` |
| `--encoding xcdr1 --endian be` | 44 bytes — `000000030000000100000001000200000000000000000002fffffffffffe00000000000300040000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

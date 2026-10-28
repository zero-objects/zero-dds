# sequence-wstring

**Construct** — `sequence<wstring>`

The wide-string element kind, with the same delimiter question as `sequence-string` and the byte-versus-character length question on top of it. An implementation that gets the delimiter right and the element length wrong produces a DHEADER that disagrees with its own contents. Empty, one element, several and bounded in one sample, because a delimiter bug that shows on a populated sequence often does not show on an empty one.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 76 bytes — `04000000000000000c0000000100000004000000e4005a0016000000030000000000000004000000e4005a00020000003412000010000000020000000000000004000000e4005a00ffffffff` |
| `--encoding xcdr2 --endian be` | 76 bytes — `00000004000000000000000c000000010000000400e4005a0000001600000003000000000000000400e4005a00000002123400000000001000000002000000000000000400e4005affffffff` |
| `--encoding xcdr1 --endian le` | 60 bytes — `000000000100000004000000e4005a00030000000000000004000000e4005a000200000034120000020000000000000004000000e4005a00ffffffff` |
| `--encoding xcdr1 --endian be` | 60 bytes — `00000000000000010000000400e4005a00000003000000000000000400e4005a000000021234000000000002000000000000000400e4005affffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

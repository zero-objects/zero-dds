# string-unbounded-empty

**Construct** — unbounded `string`, empty

An empty unbounded string is four bytes of length one and one NUL byte, not four bytes of zero. The length in XCDR counts the terminator, which is the one place it differs from every other length-prefixed encoding, and an off-by-one here is invisible on a non-empty string of the wrong content but fatal on an empty one. The trailing `long` is -1 so that any padding the string leaves before it reads as zero bytes against a member that is all ones.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `0100000000000000ffffffff` |
| `--encoding xcdr2 --endian be` | 12 bytes — `0000000100000000ffffffff` |
| `--encoding xcdr1 --endian le` | 12 bytes — `0100000000000000ffffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `0000000100000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

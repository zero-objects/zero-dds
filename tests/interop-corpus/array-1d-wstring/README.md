# array-1d-wstring

**Construct** — one-dimensional array of `wstring`

Wide strings in an array: byte lengths, no terminators, and a collection DHEADER under the letter of the rule.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `120000000000000004000000e4005a000200000034120000ffffffff` |
| `--encoding xcdr2 --endian be` | 28 bytes — `00000012000000000000000400e4005a0000000212340000ffffffff` |
| `--encoding xcdr1 --endian le` | 24 bytes — `0000000004000000e4005a000200000034120000ffffffff` |
| `--encoding xcdr1 --endian be` | 24 bytes — `000000000000000400e4005a0000000212340000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

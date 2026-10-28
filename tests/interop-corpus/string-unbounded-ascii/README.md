# string-unbounded-ascii

**Construct** — unbounded `string`, ASCII

Three ASCII characters: length four, three bytes, a NUL, then padding to the trailing long. This is the row `vendor_bytes.md` records all three vendors agreeing on at twelve bytes, so a divergence here is this project's. The trailing `long` is -1 so that any padding the string leaves before it reads as zero bytes against a member that is all ones.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `0400000061626300ffffffff` |
| `--encoding xcdr2 --endian be` | 12 bytes — `0000000461626300ffffffff` |
| `--encoding xcdr1 --endian le` | 12 bytes — `0400000061626300ffffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `0000000461626300ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

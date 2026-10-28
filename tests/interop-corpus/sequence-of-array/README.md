# sequence-of-array

**Construct** — `sequence` of an array

An array element reached through a typedef, which is the only way IDL can name one as a sequence element. The count is the number of arrays, not the number of longs — an implementation that flattens writes six as the count instead of two.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `1c00000002000000010000000200000003000000ffffffffffffff7f7929edffffffffff` |
| `--encoding xcdr2 --endian be` | 36 bytes — `0000001c00000002000000010000000200000003ffffffff7fffffffffed2979ffffffff` |
| `--encoding xcdr1 --endian le` | 32 bytes — `02000000010000000200000003000000ffffffffffffff7f7929edffffffffff` |
| `--encoding xcdr1 --endian be` | 32 bytes — `00000002000000010000000200000003ffffffff7fffffffffed2979ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

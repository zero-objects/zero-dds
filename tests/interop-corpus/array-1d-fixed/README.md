# array-1d-fixed

**Construct** — one-dimensional array of `fixed`

`fixed<5,2>` is three bytes of BCD and aligns to one, so three of them plus an octet is ten bytes with no padding anywhere. Any implementation that aligns a fixed to anything produces a longer sample.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 14 bytes — `0900000012345d00001c99999cab` |
| `--encoding xcdr2 --endian be` | 14 bytes — `0000000912345d00001c99999cab` |
| `--encoding xcdr1 --endian le` | 10 bytes — `12345d00001c99999cab` |
| `--encoding xcdr1 --endian be` | 10 bytes — `12345d00001c99999cab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# bitset-multiword

**Construct** — `bitset` wider than one word

Sixty-four declared bits with fields that straddle byte and word boundaries: the second field starts at bit 20 and the third at bit 40. The first field is all ones and the second all zeroes, so a straddling error spills visible bits into a field the sample says is empty.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 9 bytes — `ffff0f00004e61bcab` |
| `--encoding xcdr2 --endian be` | 9 bytes — `bc614e00000fffffab` |
| `--encoding xcdr1 --endian le` | 9 bytes — `ffff0f00004e61bcab` |
| `--encoding xcdr1 --endian be` | 9 bytes — `bc614e00000fffffab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

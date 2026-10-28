# typedef-of-string

**Construct** — `typedef` of a bounded string

A bounded string behind two aliases, with a multi-byte character so the byte length and the character count differ under a bound that both satisfy.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `06000000c3bc626572000000ffffffff` |
| `--encoding xcdr2 --endian be` | 16 bytes — `00000006c3bc626572000000ffffffff` |
| `--encoding xcdr1 --endian le` | 16 bytes — `06000000c3bc626572000000ffffffff` |
| `--encoding xcdr1 --endian be` | 16 bytes — `00000006c3bc626572000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

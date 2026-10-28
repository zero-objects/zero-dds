# array-1d-sequence

**Construct** — one-dimensional array of `sequence`

A fixed number of variable-length elements. The array contributes no count and each sequence contributes its own, so the sample holds three counts and no fourth — an implementation that treats the array like a sequence writes a fourth.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 32 bytes — `1800000000000000010000007929edff020000000100000002000000ffffffff` |
| `--encoding xcdr2 --endian be` | 32 bytes — `000000180000000000000001ffed2979000000020000000100000002ffffffff` |
| `--encoding xcdr1 --endian le` | 28 bytes — `00000000010000007929edff020000000100000002000000ffffffff` |
| `--encoding xcdr1 --endian be` | 28 bytes — `0000000000000001ffed2979000000020000000100000002ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

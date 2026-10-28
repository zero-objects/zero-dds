# array-1d-map

**Construct** — one-dimensional array of `map`

Maps in an array, the second one empty. Same shape as the array of sequences with the key-and-value pairing on top.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `1000000001000000010000007929edff00000000ffffffff` |
| `--encoding xcdr2 --endian be` | 24 bytes — `000000100000000100000001ffed297900000000ffffffff` |
| `--encoding xcdr1 --endian le` | 20 bytes — `01000000010000007929edff00000000ffffffff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `0000000100000001ffed297900000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

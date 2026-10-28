# array-1d-bitset

**Construct** — one-dimensional array of `bitset`

Two eight-bit bitsets and an octet — three bytes total if the bitset is stored in the smallest integer that holds its bits, and more if it is not. The second element has its high field zero, so a bit-order error moves a set bit into a field the sample says is empty.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 7 bytes — `020000008d07ab` |
| `--encoding xcdr2 --endian be` | 7 bytes — `000000028d07ab` |
| `--encoding xcdr1 --endian le` | 3 bytes — `8d07ab` |
| `--encoding xcdr1 --endian be` | 3 bytes — `8d07ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

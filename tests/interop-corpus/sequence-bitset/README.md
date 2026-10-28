# sequence-bitset

**Construct** — `sequence<bitset>`

A bitset element. Its wire width is the smallest integer that holds its declared bits — one byte here for eight bits — so a sequence of two is two bytes plus the count. An implementation that gives every bitset a machine word writes six bytes too many.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `06000000020000008df80000ffffffff` |
| `--encoding xcdr2 --endian be` | 16 bytes — `00000006000000028df80000ffffffff` |
| `--encoding xcdr1 --endian le` | 12 bytes — `020000008df80000ffffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `000000028df80000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

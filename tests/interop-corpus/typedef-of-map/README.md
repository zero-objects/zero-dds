# typedef-of-map

**Construct** — `typedef` of a map

A map two aliases deep, which is the only way some vendors' generators accept a map as a member at all.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 40 bytes — `200000000200000006000000616c7068610000007929edff03000000c3bc0000ffffff7fffffffff` |
| `--encoding xcdr2 --endian be` | 40 bytes — `000000200000000200000006616c706861000000ffed297900000003c3bc00007fffffffffffffff` |
| `--encoding xcdr1 --endian le` | 36 bytes — `0200000006000000616c7068610000007929edff03000000c3bc0000ffffff7fffffffff` |
| `--encoding xcdr1 --endian be` | 36 bytes — `0000000200000006616c706861000000ffed297900000003c3bc00007fffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

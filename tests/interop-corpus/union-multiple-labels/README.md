# union-multiple-labels

**Construct** — `union` with several labels on one case

Three labels on the first case, two on the second, and a `default` besides. The sample picks the second label of the second case, so an implementation that matches only the first label of each case falls through to `default` and writes a `short` where a `double` belongs — six bytes short and the wrong member id in any framing that carries one.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `0c0000001400000000000000000004c0ffffffff` |
| `--encoding xcdr2 --endian be` | 20 bytes — `0000000c00000014c004000000000000ffffffff` |
| `--encoding xcdr1 --endian le` | 20 bytes — `140000000000000000000000000004c0ffffffff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `0000001400000000c004000000000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

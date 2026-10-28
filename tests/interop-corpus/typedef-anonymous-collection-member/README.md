# typedef-anonymous-collection-member

**Construct** — array declared on the member

Dimensions on the declarator rather than in a typedef — the other half of `typedef-of-array`, and the form the frontend does handle. Three anonymous collection types in one aggregate, so an implementation that interns anonymous types by shape rather than by position has to keep them apart.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 64 bytes — `010000000200000003000000040000000500000006000000070000000800000010000000010000000000000004000000e188b400010000007929edffffffffff` |
| `--encoding xcdr2 --endian be` | 64 bytes — `000000010000000200000003000000040000000500000006000000070000000800000010000000010000000000000004e188b40000000001ffed2979ffffffff` |
| `--encoding xcdr1 --endian le` | 60 bytes — `0100000002000000030000000400000005000000060000000700000008000000010000000000000004000000e188b400010000007929edffffffffff` |
| `--encoding xcdr1 --endian be` | 60 bytes — `0000000100000002000000030000000400000005000000060000000700000008000000010000000000000004e188b40000000001ffed2979ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# sequence-primitives-empty

**Construct** — `sequence` of each primitive kind, empty

The same fourteen sequences with no elements. An empty sequence is a count of zero and nothing else: no padding to the element's alignment, no terminator. The trailing long shows whether anything was aligned that should not have been — fourteen counts of zero put it at offset 56.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 60 bytes — `0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffff` |
| `--encoding xcdr2 --endian be` | 60 bytes — `0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffff` |
| `--encoding xcdr1 --endian le` | 60 bytes — `0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffff` |
| `--encoding xcdr1 --endian be` | 60 bytes — `0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# sequence-primitives-bounded

**Construct** — bounded `sequence` of each primitive kind

A bound of four, two elements written. The bound is not on the wire: a bounded sequence is byte-identical to an unbounded one holding the same elements. An implementation that writes the bound, or that pads out to it, diverges on every member here.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 160 bytes |
| `--encoding xcdr2 --endian be` | 160 bytes |
| `--encoding xcdr1 --endian le` | 168 bytes |
| `--encoding xcdr1 --endian be` | 168 bytes |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

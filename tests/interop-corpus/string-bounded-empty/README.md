# string-bounded-empty

**Construct** — bounded `string`, empty

The bound is a promise about the source, not a wire feature: a bounded string is the same bytes as an unbounded one. An implementation that writes a fixed-width field of the bound, or that omits the length because the bound is known, diverges here. The trailing `long` is -1 so that any padding the string leaves before it reads as zero bytes against a member that is all ones.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `0100000000000000ffffffff` |
| `--encoding xcdr2 --endian be` | 12 bytes — `0000000100000000ffffffff` |
| `--encoding xcdr1 --endian le` | 12 bytes — `0100000000000000ffffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `0000000100000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

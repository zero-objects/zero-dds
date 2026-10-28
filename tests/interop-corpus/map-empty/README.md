# map-empty

**Construct** — empty `map`

No entries: a count of zero and nothing else. If the map carries a collection DHEADER it is a DHEADER of four, covering the count alone — the case that separates an implementation that writes the delimiter unconditionally from one that writes it only when there is content.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `0400000000000000ffffffff` |
| `--encoding xcdr2 --endian be` | 12 bytes — `0000000400000000ffffffff` |
| `--encoding xcdr1 --endian le` | 8 bytes — `00000000ffffffff` |
| `--encoding xcdr1 --endian be` | 8 bytes — `00000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# sequence-primitives-nonempty

**Construct** — `sequence` of each primitive kind, non-empty

One unbounded sequence per primitive element kind, three elements each, values chosen so no two elements share a byte pattern. XTypes 1.3 §7.4.3.5 rule (11) exempts a sequence of primitives from the collection DHEADER, so every one of these is a bare four-byte count followed by the elements — an implementation that delimits unconditionally is four bytes long on each of the fourteen members.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 208 bytes |
| `--encoding xcdr2 --endian be` | 208 bytes |
| `--encoding xcdr1 --endian le` | 216 bytes |
| `--encoding xcdr1 --endian be` | 216 bytes |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

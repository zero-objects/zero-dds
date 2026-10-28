# array-2d-primitives

**Construct** — two-dimensional array of each primitive kind

The same kinds in two dimensions. XTypes 1.3 §7.4.3.5 flattens a multi-dimensional array into one run with the first index varying most slowly; the two rows here are each other reversed, so an implementation that walks the dimensions in the other order produces bytes of the right length and the wrong order — the failure mode a same-value sample cannot see.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 284 bytes |
| `--encoding xcdr2 --endian be` | 284 bytes |
| `--encoding xcdr1 --endian le` | 288 bytes |
| `--encoding xcdr1 --endian be` | 288 bytes |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

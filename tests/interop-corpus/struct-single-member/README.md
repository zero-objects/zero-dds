# struct-single-member

**Construct** — structure with one member

One byte. The smallest aggregate there is, which is where framing overhead is a larger fraction than the content and where an implementation that always emits a DHEADER is most obviously wrong.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 1 bytes — `ab` |
| `--encoding xcdr2 --endian be` | 1 bytes — `ab` |
| `--encoding xcdr1 --endian le` | 1 bytes — `ab` |
| `--encoding xcdr1 --endian be` | 1 bytes — `ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

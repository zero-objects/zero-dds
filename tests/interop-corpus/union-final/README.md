# union-final

**Construct** — final `union`

A union carries its own extensibility, and final means the same thing for a union as for a structure: a final union is bare, an appendable one carries a DHEADER over discriminator and member together, and a mutable one gives the discriminator member id 0 and the selected member its own header. The union is the top-level type here so the framing is not hidden inside an enclosing aggregate.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 8 bytes — `010000007929edff` |
| `--encoding xcdr2 --endian be` | 8 bytes — `00000001ffed2979` |
| `--encoding xcdr1 --endian le` | 8 bytes — `010000007929edff` |
| `--encoding xcdr1 --endian be` | 8 bytes — `00000001ffed2979` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

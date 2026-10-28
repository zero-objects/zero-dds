# array-1d-primitives

**Construct** — one-dimensional array of each primitive kind

One three-element array per primitive kind. An array has no count on the wire — the length is in the type — so an implementation that writes one is four bytes long on each of fourteen members. Rule (8) of XTypes 1.3 §7.4.3.5 exempts an array of primitives from the collection DHEADER as well, so each of these is bare elements and nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 144 bytes |
| `--encoding xcdr2 --endian be` | 144 bytes |
| `--encoding xcdr1 --endian le` | 152 bytes |
| `--encoding xcdr1 --endian be` | 152 bytes |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

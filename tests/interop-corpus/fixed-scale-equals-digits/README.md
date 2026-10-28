# fixed-scale-equals-digits

**Construct** — `fixed` with scale equal to precision

Every digit after the decimal point, which is legal and is where an implementation that stores an integer part separately has nothing to store.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 4 bytes — `01234dab` |
| `--encoding xcdr2 --endian be` | 4 bytes — `01234dab` |
| `--encoding xcdr1 --endian le` | 4 bytes — `01234dab` |
| `--encoding xcdr1 --endian be` | 4 bytes — `01234dab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

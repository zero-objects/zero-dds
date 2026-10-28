# enum-negative-value

**Construct** — `enum` with a negative `@value`

An enumerator with a negative value and one at the positive limit. OMG IDL 4.2 allows the whole signed range and the wire form is a 32-bit two's-complement integer, so `MINUS` is `ffffffff`. An implementation that stores enumerator values unsigned writes zero here, which is a different enumerator that exists in the same type — the sample decodes cleanly and means something else. Measured: the Rust probe did exactly that, writing `00000000` where the sample says `MINUS`. Fixed; the bytes below carry the two's complement and the case now guards the fix.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 8 bytes — `ffffffffffffff7f` |
| `--encoding xcdr2 --endian be` | 8 bytes — `ffffffff7fffffff` |
| `--encoding xcdr1 --endian le` | 8 bytes — `ffffffffffffff7f` |
| `--encoding xcdr1 --endian be` | 8 bytes — `ffffffff7fffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# typedef-of-array

**Construct** — `typedef` of an array

An array declared as a typedef rather than on the member. The frontend used to drop the declarator's dimensions and lower this to a plain alias of `long`, so the type said one integer where the source says six — a silent narrowing no diagnostic reported. Fixed; the case now guards the fix.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `010000000200000003000000ffffffffffffff7f7929edffffffffff` |
| `--encoding xcdr2 --endian be` | 28 bytes — `000000010000000200000003ffffffff7fffffffffed2979ffffffff` |
| `--encoding xcdr1 --endian le` | 28 bytes — `010000000200000003000000ffffffffffffff7f7929edffffffffff` |
| `--encoding xcdr1 --endian be` | 28 bytes — `000000010000000200000003ffffffff7fffffffffed2979ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

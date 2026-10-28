# enum-sparse-values

**Construct** — `enum` with non-contiguous `@value`

Gaps between the values, so an ordinal and a value are never the same number. Three members in a different order from the declaration, which catches an implementation that indexes the enumerator table by position when it should look the value up.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `ffff00000100000064000000` |
| `--encoding xcdr2 --endian be` | 12 bytes — `0000ffff0000000100000064` |
| `--encoding xcdr1 --endian le` | 12 bytes — `ffff00000100000064000000` |
| `--encoding xcdr1 --endian be` | 12 bytes — `0000ffff0000000100000064` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

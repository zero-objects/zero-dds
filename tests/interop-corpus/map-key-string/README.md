# map-key-string

**Construct** — `map` keyed by `string`

A string key, including the empty one, which is a legal key and a common off-by-one. Keys and values alternate, each key length-prefixed, so a wrong length on one key shifts every pair after it.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 56 bytes — `300000000300000001000000000000007929edff06000000616c706861000000ffffff7f06000000c3bce188b400000001000000ffffffff` |
| `--encoding xcdr2 --endian be` | 56 bytes — `00000030000000030000000100000000ffed297900000006616c7068610000007fffffff00000006c3bce188b400000000000001ffffffff` |
| `--encoding xcdr1 --endian le` | 52 bytes — `0300000001000000000000007929edff06000000616c706861000000ffffff7f06000000c3bce188b400000001000000ffffffff` |
| `--encoding xcdr1 --endian be` | 52 bytes — `000000030000000100000000ffed297900000006616c7068610000007fffffff00000006c3bce188b400000000000001ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

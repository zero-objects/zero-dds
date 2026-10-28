# map-key-enum

**Construct** — `map` keyed by an `enum`

An enumeration key at a non-default bit bound, so the key is two bytes and each value is preceded by two of padding. The key width comes from the bit bound and not from the enumeration being a constructed type.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `1400000002000000020000007929edff00000000ffffff7fffffffff` |
| `--encoding xcdr2 --endian be` | 28 bytes — `000000140000000200020000ffed2979000000007fffffffffffffff` |
| `--encoding xcdr1 --endian le` | 24 bytes — `02000000020000007929edff00000000ffffff7fffffffff` |
| `--encoding xcdr1 --endian be` | 24 bytes — `0000000200020000ffed2979000000007fffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

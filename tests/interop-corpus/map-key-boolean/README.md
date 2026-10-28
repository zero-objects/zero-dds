# map-key-boolean

**Construct** — `map` keyed by `boolean`

A boolean key, which admits exactly two entries. One byte then three of padding before each value — the padding an implementation that packs key and value adjacently omits.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `02000000010000007929edff00000000ffffff7fffffffff` |
| `--encoding xcdr2 --endian be` | 24 bytes — `0000000201000000ffed2979000000007fffffffffffffff` |
| `--encoding xcdr1 --endian le` | 24 bytes — `02000000010000007929edff00000000ffffff7fffffffff` |
| `--encoding xcdr1 --endian be` | 24 bytes — `0000000201000000ffed2979000000007fffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

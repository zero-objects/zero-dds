# map-key-char

**Construct** — `map` keyed by `char`

A one-byte key and a one-byte value: six bytes of content after the count, with no padding anywhere. An implementation that aligns each entry writes more.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `0300000061ab5a017eff0000ffffffff` |
| `--encoding xcdr2 --endian be` | 16 bytes — `0000000361ab5a017eff0000ffffffff` |
| `--encoding xcdr1 --endian le` | 16 bytes — `0300000061ab5a017eff0000ffffffff` |
| `--encoding xcdr1 --endian be` | 16 bytes — `0000000361ab5a017eff0000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

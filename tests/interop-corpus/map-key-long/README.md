# map-key-long

**Construct** — `map` keyed by `long`

An integer key with a negative value in it, so a key read unsigned sorts and compares differently. Keys are written in the order the sample gives; an implementation that reorders them produces bytes no other implementation reproduces.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 32 bytes — `030000007929edff01000000ffffff7ffeffffff0100000003000000ffffffff` |
| `--encoding xcdr2 --endian be` | 32 bytes — `00000003ffed2979000000017ffffffffffffffe0000000100000003ffffffff` |
| `--encoding xcdr1 --endian le` | 32 bytes — `030000007929edff01000000ffffff7ffeffffff0100000003000000ffffffff` |
| `--encoding xcdr1 --endian be` | 32 bytes — `00000003ffed2979000000017ffffffffffffffe0000000100000003ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

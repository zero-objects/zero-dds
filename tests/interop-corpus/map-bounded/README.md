# map-bounded

**Construct** — bounded `map`

A bound of four with two entries. As with sequences, the bound is not on the wire: the bytes must equal the unbounded form.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `02000000010000007929edff02000000ffffff7fffffffff` |
| `--encoding xcdr2 --endian be` | 24 bytes — `0000000200000001ffed2979000000027fffffffffffffff` |
| `--encoding xcdr1 --endian le` | 24 bytes — `02000000010000007929edff02000000ffffff7fffffffff` |
| `--encoding xcdr1 --endian be` | 24 bytes — `0000000200000001ffed2979000000027fffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

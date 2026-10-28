# map-key-ulonglong

**Construct** — `map` keyed by `unsigned long long`

An eight-byte key above 2^63 and a two-byte value: the alignment between entries is eight under XCDR1 and four under XCDR2, so this is a max-alignment case as well as a map case.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 32 bytes — `02000000bec2addeefbeaddec7cf00000700000000000000ff7f0000ffffffff` |
| `--encoding xcdr2 --endian be` | 32 bytes — `00000002deadbeefdeadc2becfc7000000000000000000077fff0000ffffffff` |
| `--encoding xcdr1 --endian le` | 40 bytes — `0200000000000000bec2addeefbeaddec7cf0000000000000700000000000000ff7f0000ffffffff` |
| `--encoding xcdr1 --endian be` | 40 bytes — `0000000200000000deadbeefdeadc2becfc700000000000000000000000000077fff0000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

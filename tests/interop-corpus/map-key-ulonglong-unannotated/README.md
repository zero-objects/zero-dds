# map-key-ulonglong-unannotated

**Construct** — `map` keyed by `unsigned long long`, unannotated

The `map-key-ulonglong` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `map-key-ulonglong` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `2000000002000000bec2addeefbeaddec7cf00000700000000000000ff7f0000ffffffff` |
| `--encoding xcdr2 --endian be` | 36 bytes — `0000002000000002deadbeefdeadc2becfc7000000000000000000077fff0000ffffffff` |
| `--encoding xcdr1 --endian le` | 40 bytes — `0200000000000000bec2addeefbeaddec7cf0000000000000700000000000000ff7f0000ffffffff` |
| `--encoding xcdr1 --endian be` | 40 bytes — `0000000200000000deadbeefdeadc2becfc700000000000000000000000000077fff0000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

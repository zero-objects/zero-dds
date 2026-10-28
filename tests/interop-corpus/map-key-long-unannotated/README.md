# map-key-long-unannotated

**Construct** — `map` keyed by `long`, unannotated

The `map-key-long` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `map-key-long` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `20000000030000007929edff01000000ffffff7ffeffffff0100000003000000ffffffff` |
| `--encoding xcdr2 --endian be` | 36 bytes — `0000002000000003ffed2979000000017ffffffffffffffe0000000100000003ffffffff` |
| `--encoding xcdr1 --endian le` | 32 bytes — `030000007929edff01000000ffffff7ffeffffff0100000003000000ffffffff` |
| `--encoding xcdr1 --endian be` | 32 bytes — `00000003ffed2979000000017ffffffffffffffe0000000100000003ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

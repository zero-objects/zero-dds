# map-value-string-unannotated

**Construct** — `map` with a string value, unannotated

The `map-value-string` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `map-value-string` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 48 bytes — `2c0000002200000002000000010000000100000000000000020000000a000000c3bc62657220e188b4000000ffffffff` |
| `--encoding xcdr2 --endian be` | 48 bytes — `0000002c0000002200000002000000010000000100000000000000020000000ac3bc62657220e188b4000000ffffffff` |
| `--encoding xcdr1 --endian le` | 40 bytes — `02000000010000000100000000000000020000000a000000c3bc62657220e188b4000000ffffffff` |
| `--encoding xcdr1 --endian be` | 40 bytes — `00000002000000010000000100000000000000020000000ac3bc62657220e188b4000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

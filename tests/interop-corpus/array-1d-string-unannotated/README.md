# array-1d-string-unannotated

**Construct** — one-dimensional array of `string`, unannotated

The `array-1d-string` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `array-1d-string` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 40 bytes — `240000001a0000000100000000000000040000006162630006000000c3bce188b4000000ffffffff` |
| `--encoding xcdr2 --endian be` | 40 bytes — `000000240000001a0000000100000000000000046162630000000006c3bce188b4000000ffffffff` |
| `--encoding xcdr1 --endian le` | 32 bytes — `0100000000000000040000006162630006000000c3bce188b4000000ffffffff` |
| `--encoding xcdr1 --endian be` | 32 bytes — `0000000100000000000000046162630000000006c3bce188b4000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

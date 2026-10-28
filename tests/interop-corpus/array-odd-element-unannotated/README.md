# array-odd-element-unannotated

**Construct** — array of an odd-sized structure, unannotated

The `array-odd-element` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `array-odd-element` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 31 bytes — `1b000000160000000600000001000000020000000600000003000000040077` |
| `--encoding xcdr2 --endian be` | 31 bytes — `0000001b000000160000000600000001000200000000000600000003000477` |
| `--encoding xcdr1 --endian le` | 15 bytes — `010000000200000003000000040077` |
| `--encoding xcdr1 --endian be` | 15 bytes — `000000010002000000000003000477` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

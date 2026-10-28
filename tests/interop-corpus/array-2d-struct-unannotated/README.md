# array-2d-struct-unannotated

**Construct** — two-dimensional array of a structure, unannotated

The `array-2d-struct` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `array-2d-struct` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 60 bytes — `380000002e00000006000000010000000200000006000000030000000400000006000000fffffffffeff000006000000ffffff7f00800000ffffffff` |
| `--encoding xcdr2 --endian be` | 60 bytes — `000000380000002e00000006000000010002000000000006000000030004000000000006fffffffffffe0000000000067fffffff80000000ffffffff` |
| `--encoding xcdr1 --endian le` | 36 bytes — `01000000020000000300000004000000fffffffffeff0000ffffff7f00800000ffffffff` |
| `--encoding xcdr1 --endian be` | 36 bytes — `00000001000200000000000300040000fffffffffffe00007fffffff80000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

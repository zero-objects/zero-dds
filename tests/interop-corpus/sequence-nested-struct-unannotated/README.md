# sequence-nested-struct-unannotated

**Construct** — `sequence<sequence<struct>>`, unannotated

The `sequence-nested-struct` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `sequence-nested-struct` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 76 bytes — `480000003e000000030000000e0000000100000006000000010000000200000004000000000000001a0000000200000006000000fffffffffeff0000060000000300000004000000ffffffff` |
| `--encoding xcdr2 --endian be` | 76 bytes — `000000480000003e000000030000000e0000000100000006000000010002000000000004000000000000001a0000000200000006fffffffffffe0000000000060000000300040000ffffffff` |
| `--encoding xcdr1 --endian le` | 44 bytes — `030000000100000001000000020000000000000002000000fffffffffeff00000300000004000000ffffffff` |
| `--encoding xcdr1 --endian be` | 44 bytes — `000000030000000100000001000200000000000000000002fffffffffffe00000000000300040000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

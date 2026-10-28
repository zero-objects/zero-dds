# array-3d-struct-unannotated

**Construct** — three-dimensional array of a structure, unannotated

The `array-3d-struct` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `array-3d-struct` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 72 bytes — `440000003f00000003000000ffff0100030000000200020003000000fdff0300030000000400040003000000fbff0500030000000600060003000000f9ff070003000000080008ab` |
| `--encoding xcdr2 --endian be` | 72 bytes — `000000440000003f00000003ffff0100000000030002020000000003fffd0300000000030004040000000003fffb0500000000030006060000000003fff9070000000003000808ab` |
| `--encoding xcdr1 --endian le` | 32 bytes — `ffff010002000200fdff030004000400fbff050006000600f9ff0700080008ab` |
| `--encoding xcdr1 --endian be` | 32 bytes — `ffff010000020200fffd030000040400fffb050000060600fff90700000808ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

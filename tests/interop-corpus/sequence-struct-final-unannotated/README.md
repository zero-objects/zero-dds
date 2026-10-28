# sequence-struct-final-unannotated

**Construct** — `sequence` of a final structure, unannotated

The `sequence-struct-final` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `sequence-struct-final` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 80 bytes — `4c00000004000000000000000e00000001000000060000007929edfffeff0000260000000300000006000000010000000200000006000000ffffffffff7f000006000000ffffff7f00800000ffffffff` |
| `--encoding xcdr2 --endian be` | 80 bytes — `0000004c00000004000000000000000e0000000100000006ffed2979fffe0000000000260000000300000006000000010002000000000006ffffffff7fff0000000000067fffffff80000000ffffffff` |
| `--encoding xcdr1 --endian le` | 48 bytes — `00000000010000007929edfffeff0000030000000100000002000000ffffffffff7f0000ffffff7f00800000ffffffff` |
| `--encoding xcdr1 --endian be` | 48 bytes — `0000000000000001ffed2979fffe0000000000030000000100020000ffffffff7fff00007fffffff80000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

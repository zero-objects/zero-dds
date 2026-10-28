# sequence-nested-unannotated

**Construct** — `sequence<sequence<long>>`, unannotated

The `sequence-nested` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `sequence-nested` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 48 bytes — `2c000000240000000300000000000000010000007929edff0400000001000000020000000300000004000000ffffffff` |
| `--encoding xcdr2 --endian be` | 48 bytes — `0000002c00000024000000030000000000000001ffed29790000000400000001000000020000000300000004ffffffff` |
| `--encoding xcdr1 --endian le` | 40 bytes — `0300000000000000010000007929edff0400000001000000020000000300000004000000ffffffff` |
| `--encoding xcdr1 --endian be` | 40 bytes — `000000030000000000000001ffed29790000000400000001000000020000000300000004ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

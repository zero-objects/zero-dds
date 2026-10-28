# annotation-must-understand-unannotated

**Construct** — `@must_understand`, unannotated

The `annotation-must-understand` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `annotation-must-understand` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 26 bytes — `160000007929edffffffff7f0100000006000000c3bc62657200` |
| `--encoding xcdr2 --endian be` | 26 bytes — `00000016ffed29797fffffff0100000000000006c3bc62657200` |
| `--encoding xcdr1 --endian le` | 24 bytes — `7929edffffffff7f02400a0006000000c3bc626572000000` |
| `--encoding xcdr1 --endian be` | 24 bytes — `ffed29797fffffff4002000a00000006c3bc626572000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

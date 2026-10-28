# sequence-string-unannotated

**Construct** — `sequence<string>`, unannotated

The `sequence-string` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `sequence-string` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 100 bytes |
| `--encoding xcdr2 --endian be` | 100 bytes |
| `--encoding xcdr1 --endian le` | 80 bytes — `0000000001000000040000006162630003000000010000000000000004000000616263000e000000c3bc62657220e2809420e188b40000000200000001000000000000000400000061626300ffffffff` |
| `--encoding xcdr1 --endian be` | 80 bytes — `0000000000000001000000046162630000000003000000010000000000000004616263000000000ec3bc62657220e2809420e188b40000000000000200000001000000000000000461626300ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

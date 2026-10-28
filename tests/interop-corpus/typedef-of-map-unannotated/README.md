# typedef-of-map-unannotated

**Construct** — `typedef` of a map, unannotated

The `typedef-of-map` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `typedef-of-map` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 44 bytes — `28000000200000000200000006000000616c7068610000007929edff03000000c3bc0000ffffff7fffffffff` |
| `--encoding xcdr2 --endian be` | 44 bytes — `00000028000000200000000200000006616c706861000000ffed297900000003c3bc00007fffffffffffffff` |
| `--encoding xcdr1 --endian le` | 36 bytes — `0200000006000000616c7068610000007929edff03000000c3bc0000ffffff7fffffffff` |
| `--encoding xcdr1 --endian be` | 36 bytes — `0000000200000006616c706861000000ffed297900000003c3bc00007fffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

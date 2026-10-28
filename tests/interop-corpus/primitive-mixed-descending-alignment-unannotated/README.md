# primitive-mixed-descending-alignment-unannotated

**Construct** — every primitive, widest first, unannotated

The `primitive-mixed-descending-alignment` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `primitive-mixed-descending-alignment` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 51 bytes — `2f00000000000000000004c0000020400700000000000000ffffffffffffff7f05000000ffffff7f0300ff7f34127e7f030100` |
| `--encoding xcdr2 --endian be` | 51 bytes — `0000002fc0040000000000004020000000000000000000077fffffffffffffff000000057fffffff00037fff12347e7f030100` |
| `--encoding xcdr1 --endian le` | 51 bytes — `00000000000004c000002040000000000700000000000000ffffffffffffff7f05000000ffffff7f0300ff7f34127e7f030100` |
| `--encoding xcdr1 --endian be` | 51 bytes — `c004000000000000402000000000000000000000000000077fffffffffffffff000000057fffffff00037fff12347e7f030100` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

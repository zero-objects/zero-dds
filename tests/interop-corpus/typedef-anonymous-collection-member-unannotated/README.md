# typedef-anonymous-collection-member-unannotated

**Construct** — array declared on the member, unannotated

The `typedef-anonymous-collection-member` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `typedef-anonymous-collection-member` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 68 bytes — `40000000010000000200000003000000040000000500000006000000070000000800000010000000010000000000000004000000e188b400010000007929edffffffffff` |
| `--encoding xcdr2 --endian be` | 68 bytes — `00000040000000010000000200000003000000040000000500000006000000070000000800000010000000010000000000000004e188b40000000001ffed2979ffffffff` |
| `--encoding xcdr1 --endian le` | 60 bytes — `0100000002000000030000000400000005000000060000000700000008000000010000000000000004000000e188b400010000007929edffffffffff` |
| `--encoding xcdr1 --endian be` | 60 bytes — `0000000100000002000000030000000400000005000000060000000700000008000000010000000000000004e188b40000000001ffed2979ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# annotation-optional-mixed-unannotated

**Construct** — several `@optional` members, some absent, unannotated

The `annotation-optional-mixed` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `annotation-optional-mixed` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 27 bytes — `17000000000000007929edff0100000006000000c3bc6265720000` |
| `--encoding xcdr2 --endian be` | 27 bytes — `0000001700000000ffed29790100000000000006c3bc6265720000` |
| `--encoding xcdr1 --endian le` | 28 bytes — `000000007929edff02000a0006000000c3bc62657200000003000000` |
| `--encoding xcdr1 --endian be` | 28 bytes — `00000000ffed29790002000a00000006c3bc62657200000000030000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# struct-inheritance-final-unannotated

**Construct** — final structure with a base, unannotated

The `struct-inheritance-final` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `struct-inheritance-final` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 21 bytes — `110000007929edfffeff0000182d4454fb210940ab` |
| `--encoding xcdr2 --endian be` | 21 bytes — `00000011ffed2979fffe0000400921fb54442d18ab` |
| `--encoding xcdr1 --endian le` | 17 bytes — `7929edfffeff0000182d4454fb210940ab` |
| `--encoding xcdr1 --endian be` | 17 bytes — `ffed2979fffe0000400921fb54442d18ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

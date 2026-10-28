# string-unbounded-utf8-unannotated

**Construct** — unbounded `string`, multi-byte UTF-8, unannotated

The `string-unbounded-utf8` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `string-unbounded-utf8` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 40 bytes — `240000001c0000006e61c3af766520e2809420c3bcc3b1c3aec3a7c3b8e28882e188b400ffffffff` |
| `--encoding xcdr2 --endian be` | 40 bytes — `000000240000001c6e61c3af766520e2809420c3bcc3b1c3aec3a7c3b8e28882e188b400ffffffff` |
| `--encoding xcdr1 --endian le` | 36 bytes — `1c0000006e61c3af766520e2809420c3bcc3b1c3aec3a7c3b8e28882e188b400ffffffff` |
| `--encoding xcdr1 --endian be` | 36 bytes — `0000001c6e61c3af766520e2809420c3bcc3b1c3aec3a7c3b8e28882e188b400ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

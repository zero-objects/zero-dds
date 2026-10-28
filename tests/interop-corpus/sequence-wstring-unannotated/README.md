# sequence-wstring-unannotated

**Construct** — `sequence<wstring>`, unannotated

The `sequence-wstring` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `sequence-wstring` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 80 bytes — `4c00000004000000000000000c0000000100000004000000e4005a0016000000030000000000000004000000e4005a00020000003412000010000000020000000000000004000000e4005a00ffffffff` |
| `--encoding xcdr2 --endian be` | 80 bytes — `0000004c00000004000000000000000c000000010000000400e4005a0000001600000003000000000000000400e4005a00000002123400000000001000000002000000000000000400e4005affffffff` |
| `--encoding xcdr1 --endian le` | 60 bytes — `000000000100000004000000e4005a00030000000000000004000000e4005a000200000034120000020000000000000004000000e4005a00ffffffff` |
| `--encoding xcdr1 --endian be` | 60 bytes — `00000000000000010000000400e4005a00000003000000000000000400e4005a000000021234000000000002000000000000000400e4005affffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

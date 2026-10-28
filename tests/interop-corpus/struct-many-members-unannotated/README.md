# struct-many-members-unannotated

**Construct** — structure with many members of one kind, unannotated

The `struct-many-members` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `struct-many-members` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 68 bytes — `40000000fffefefe02020202fdfcfcfc04040404fbfafafa06060606f9f8f8f808080808f7f6f6f60a0a0a0af5f4f4f40c0c0c0cf3f2f2f20e0e0e0ef1f0f0f010101010` |
| `--encoding xcdr2 --endian be` | 68 bytes — `00000040fefefeff02020202fcfcfcfd04040404fafafafb06060606f8f8f8f908080808f6f6f6f70a0a0a0af4f4f4f50c0c0c0cf2f2f2f30e0e0e0ef0f0f0f110101010` |
| `--encoding xcdr1 --endian le` | 64 bytes — `fffefefe02020202fdfcfcfc04040404fbfafafa06060606f9f8f8f808080808f7f6f6f60a0a0a0af5f4f4f40c0c0c0cf3f2f2f20e0e0e0ef1f0f0f010101010` |
| `--encoding xcdr1 --endian be` | 64 bytes — `fefefeff02020202fcfcfcfd04040404fafafafb06060606f8f8f8f908080808f6f6f6f70a0a0a0af4f4f4f50c0c0c0cf2f2f2f30e0e0e0ef0f0f0f110101010` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

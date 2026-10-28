# annotation-key-multiple-unannotated

**Construct** — `@key` on several members, unannotated

The `annotation-key-multiple` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `annotation-key-multiple` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `140000007929edffffffff7f06000000c3bc62657200feff` |
| `--encoding xcdr2 --endian be` | 24 bytes — `00000014ffed29797fffffff00000006c3bc62657200fffe` |
| `--encoding xcdr1 --endian le` | 20 bytes — `7929edffffffff7f06000000c3bc62657200feff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `ffed29797fffffff00000006c3bc62657200fffe` |
| `--emit keyhash` | 16 bytes — `22ac5facfdc0ea087a1d37ca96ef6cb6` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

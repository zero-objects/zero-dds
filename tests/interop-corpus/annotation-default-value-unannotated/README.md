# annotation-default-value-unannotated

**Construct** — `@default`, unannotated

The `annotation-default-value` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `annotation-default-value` it isolates the default, because the two files differ in nothing else.

It carries the other case's answer to the keyword-named-annotation report as well — `@default` is a builtin annotation whose name is a reserved IDL keyword, and this file puts that name in front of the vendor's own default rather than in front of an explicit `@final`, so a front end that takes the keyword only in one of the two is visible. The tracker number is on `annotation-default-value`, deliberately and only there: this case is in the default-extensibility family, and a number written here would be carried to every other case in it.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `0c0000007929edffff7f000001000000` |
| `--encoding xcdr2 --endian be` | 16 bytes — `0000000cffed29797fff000000000001` |
| `--encoding xcdr1 --endian le` | 12 bytes — `7929edffff7f000001000000` |
| `--encoding xcdr1 --endian be` | 12 bytes — `ffed29797fff000000000001` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

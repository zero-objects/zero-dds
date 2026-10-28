# map-key-enum-unannotated

**Construct** — `map` keyed by an `enum`, unannotated

The `map-key-enum` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `map-key-enum` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 32 bytes — `1c0000001400000002000000020000007929edff00000000ffffff7fffffffff` |
| `--encoding xcdr2 --endian be` | 32 bytes — `0000001c000000140000000200020000ffed2979000000007fffffffffffffff` |
| `--encoding xcdr1 --endian le` | 24 bytes — `02000000020000007929edff00000000ffffff7fffffffff` |
| `--encoding xcdr1 --endian be` | 24 bytes — `0000000200020000ffed2979000000007fffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

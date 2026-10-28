# sequence-union-unannotated

**Construct** — `sequence<union>`, unannotated

The `sequence-union` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `sequence-union` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 56 bytes — `340000002c0000000300000008000000010000007929edff0c00000009000000182d4454fb2109400800000001000000ffffff7fffffffff` |
| `--encoding xcdr2 --endian be` | 56 bytes — `000000340000002c000000030000000800000001ffed29790000000c00000009400921fb54442d1800000008000000017fffffffffffffff` |
| `--encoding xcdr1 --endian le` | 36 bytes — `03000000010000007929edff09000000182d4454fb21094001000000ffffff7fffffffff` |
| `--encoding xcdr1 --endian be` | 36 bytes — `0000000300000001ffed297900000009400921fb54442d18000000017fffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

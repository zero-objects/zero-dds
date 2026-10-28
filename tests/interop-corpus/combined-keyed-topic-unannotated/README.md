# combined-keyed-topic-unannotated

**Construct** — a keyed topic type, unannotated

The `combined-keyed-topic` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `combined-keyed-topic` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 73 bytes — `4500000008000000722dc3bc2d3031007929edff10000000000000000000f83f00000000000004c00200000003000000000000000000c03f00000000000013c0000000205fa0024200` |
| `--encoding xcdr2 --endian be` | 73 bytes — `0000004500000008722dc3bc2d303100ffed2979000000103ff8000000000000c00400000000000002000000000000033fc0000000000000c0130000000000004202a05f2000000000` |
| `--encoding xcdr1 --endian le` | 68 bytes — `08000000722dc3bc2d3031007929edff000000000000f83f00000000000004c00200000003000000000000000000c03f00000000000013c0000000205fa0024205000000` |
| `--encoding xcdr1 --endian be` | 68 bytes — `00000008722dc3bc2d303100ffed29793ff8000000000000c00400000000000002000000000000033fc0000000000000c0130000000000004202a05f2000000000050000` |
| `--emit keyhash` | 16 bytes — `23f288d176d8caf8aba3caa178f7f69f` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# module-issue41-shape

**Construct** — the unannotated #41 and #42 shape

The IDL from the two reported issues exactly as reported, unannotated throughout. `vendor_bytes.md` §3 measures it: Cyclone writes 48 bytes under both encodings, reading the types as final; RTI writes 48 under XCDR1 and 60 under XCDR2, reading them as appendable and adding three DHEADERs. XTypes 1.3 §7.3.1.2.1.8 says appendable. This is the single case that decides the project's default and the reason the corpus twins every annotated case.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 60 bytes — `3800000018000000000000000000f83f00000000000004c00000000000000a4018000000000000000000e0bf000000000000c03f00000000000013c0` |
| `--encoding xcdr2 --endian be` | 60 bytes — `00000038000000183ff8000000000000c004000000000000400a00000000000000000018bfe00000000000003fc0000000000000c013000000000000` |
| `--encoding xcdr1 --endian le` | 48 bytes — `000000000000f83f00000000000004c00000000000000a40000000000000e0bf000000000000c03f00000000000013c0` |
| `--encoding xcdr1 --endian be` | 48 bytes — `3ff8000000000000c004000000000000400a000000000000bfe00000000000003fc0000000000000c013000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

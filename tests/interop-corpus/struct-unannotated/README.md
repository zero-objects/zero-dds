# struct-unannotated

**Construct** — structure with no extensibility annotation

The same three members with nothing said about extensibility. XTypes 1.3 §7.3.1.2.1.8 says an unspecified extensibility is appendable; Cyclone's `idlc` reads it as final and warns that it may change; RTI reads it as appendable. Under XCDR2 that is a four-byte DHEADER present or absent, so the two readings do not interoperate at all. This is the reason every case in the corpus has an unannotated twin.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 18 bytes — `0e0000007929edff182d4454fb210940feff` |
| `--encoding xcdr2 --endian be` | 18 bytes — `0000000effed2979400921fb54442d18fffe` |
| `--encoding xcdr1 --endian le` | 18 bytes — `7929edff00000000182d4454fb210940feff` |
| `--encoding xcdr1 --endian be` | 18 bytes — `ffed297900000000400921fb54442d18fffe` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

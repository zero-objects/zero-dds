# module-root-anchored

**Construct** — root-anchored scoped names

The shape from GitHub issues #41 and #42, annotated `@final`. `::common::util::Location` is the fully scoped name of OMG IDL 4.2 §7.5.2 and is valid; `vendor_bytes.md` §3 records `fastddsgen` rejecting it outright with "'::' was not defined previously", so this case cannot be measured against Fast DDS at all. That is a finding about the vendor and it belongs in the corpus rather than being worked around.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 48 bytes — `000000000000f83f00000000000004c00000000000000a40000000000000e0bf000000000000c03f00000000000013c0` |
| `--encoding xcdr2 --endian be` | 48 bytes — `3ff8000000000000c004000000000000400a000000000000bfe00000000000003fc0000000000000c013000000000000` |
| `--encoding xcdr1 --endian le` | 48 bytes — `000000000000f83f00000000000004c00000000000000a40000000000000e0bf000000000000c03f00000000000013c0` |
| `--encoding xcdr1 --endian be` | 48 bytes — `3ff8000000000000c004000000000000400a000000000000bfe00000000000003fc0000000000000c013000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

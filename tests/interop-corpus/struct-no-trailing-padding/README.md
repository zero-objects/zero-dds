# struct-no-trailing-padding

**Construct** — structure that does not pad to its own alignment

The `Odd` case from `vendor_bytes.md` §1. Six bytes, not eight: the type aligns to four but does not pad out to it. An implementation that rounds a structure up to its own alignment adds two bytes here and every peer rejects the sample.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 6 bytes — `7929edfffeff` |
| `--encoding xcdr2 --endian be` | 6 bytes — `ffed2979fffe` |
| `--encoding xcdr1 --endian le` | 6 bytes — `7929edfffeff` |
| `--encoding xcdr1 --endian be` | 6 bytes — `ffed2979fffe` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

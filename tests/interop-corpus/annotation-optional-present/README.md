# annotation-optional-present

**Construct** — `@optional` with a value

An optional member that is present. In a mutable type its EMHEADER is enough to say so; in a final or appendable one the marker is a BOOLEAN `<is_present>` under XCDR2 (§7.4.3.5.2 rule 20) and a parameter header under XCDR1 (§7.4.1.2). Which marker it is is exactly what an implementation can get wrong without noticing, because its own reader makes the same assumption — this project wrote an EMHEADER in both versions until `vendor_bytes.md` §1 measured what the three write.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `7929edff01000000ffffff7ffeffffff` |
| `--encoding xcdr2 --endian be` | 16 bytes — `ffed2979010000007ffffffffffffffe` |
| `--encoding xcdr1 --endian le` | 16 bytes — `7929edff01000400ffffff7ffeffffff` |
| `--encoding xcdr1 --endian be` | 16 bytes — `ffed2979000100047ffffffffffffffe` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

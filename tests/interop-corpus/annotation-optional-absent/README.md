# annotation-optional-absent

**Construct** — `@optional` left out

The same type with the key left out of the sample — not `null`, which the probe rejects. An `@optional` member of a type that is not `@mutable` is an `OPT_FMEMBER`, and the absence is written: a BOOLEAN `<is_present>` of zero under XCDR2 (§7.4.3.5.2 rule 20) and a parameter header declaring a length of zero under XCDR1 (§7.4.1.2). `vendor_bytes.md` §1 records all three implementations writing both. The pair with `annotation-optional-present` is what shows that the two forms differ only in the marker's value and in what follows it.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `7929edff00000000feffffff` |
| `--encoding xcdr2 --endian be` | 12 bytes — `ffed297900000000fffffffe` |
| `--encoding xcdr1 --endian le` | 12 bytes — `7929edff01000000feffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `ffed297900010000fffffffe` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

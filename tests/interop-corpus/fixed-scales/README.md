# fixed-scales

**Construct** — `fixed` at several precisions and scales

One digit, five digits and the maximum thirty-one, so the byte counts are 1, 3 and 16. The widths follow from the digit count alone — an implementation that rounds a fixed up to a machine word gets all three wrong and by different amounts.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 21 bytes — `7c12345d0123456789012345678901234567890dab` |
| `--encoding xcdr2 --endian be` | 21 bytes — `7c12345d0123456789012345678901234567890dab` |
| `--encoding xcdr1 --endian le` | 21 bytes — `7c12345d0123456789012345678901234567890dab` |
| `--encoding xcdr1 --endian be` | 21 bytes — `7c12345d0123456789012345678901234567890dab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

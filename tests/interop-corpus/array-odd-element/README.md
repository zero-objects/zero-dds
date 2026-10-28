# array-odd-element

**Construct** — array of an odd-sized structure

The `ArrOdd` case from `vendor_bytes.md` §1, reproduced exactly: all three vendors produce `01000000 0200 0000 03000000 0400 77` under XCDR1, fifteen bytes with the trailing octet at offset fourteen. Padding after the last element instead of only between elements is the error this catches, and it is a difference of two bytes that no vendor accepts.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 19 bytes — `0e000000010000000200000003000000040077` |
| `--encoding xcdr2 --endian be` | 19 bytes — `0000000e000000010002000000000003000477` |
| `--encoding xcdr1 --endian le` | 15 bytes — `010000000200000003000000040077` |
| `--encoding xcdr1 --endian be` | 15 bytes — `000000010002000000000003000477` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

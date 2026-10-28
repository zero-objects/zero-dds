# array-1d-struct-mutable

**Construct** — one-dimensional array of a mutable structure

The `ArrOdd` shape from `vendor_bytes.md` §1 with a mutable element. Six-byte elements in a four-byte-aligned array: the first element is padded to the element's alignment and the last is not, so the trailing octet lands at offset fourteen. All three vendors agree on that under XCDR1; under XCDR2 they split over the collection delimiter. Getting the trailing padding wrong instead adds two bytes that every vendor rejects.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 43 bytes — `260000000e000000000000200100000001000010020000000e000000000000200300000001000010040077` |
| `--encoding xcdr2 --endian be` | 43 bytes — `000000260000000e200000000000000110000001000200000000000e200000000000000310000001000477` |
| `--encoding xcdr1 --endian le` | 41 bytes — `00000400010000000100020002000000027f000000000400030000000100020004000000027f000077` |
| `--encoding xcdr1 --endian be` | 41 bytes — `000000040000000100010002000200007f020000000000040000000300010002000400007f02000077` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

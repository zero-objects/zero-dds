# array-1d-struct-final

**Construct** — one-dimensional array of a final structure

The `ArrOdd` shape from `vendor_bytes.md` §1 with a final element. Six-byte elements in a four-byte-aligned array: the first element is padded to the element's alignment and the last is not, so the trailing octet lands at offset fourteen. All three vendors agree on that under XCDR1; under XCDR2 they split over the collection delimiter. Getting the trailing padding wrong instead adds two bytes that every vendor rejects.

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

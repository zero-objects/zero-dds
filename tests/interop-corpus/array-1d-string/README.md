# array-1d-string

**Construct** — one-dimensional array of `string`

A three-element array of strings, one empty, so the element boundaries are not evenly spaced. Not a primitive, so rule (9) calls for the collection DHEADER that `vendor_bytes.md` §1 records RTI omitting.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `1a0000000100000000000000040000006162630006000000c3bce188b4000000ffffffff` |
| `--encoding xcdr2 --endian be` | 36 bytes — `0000001a0000000100000000000000046162630000000006c3bce188b4000000ffffffff` |
| `--encoding xcdr1 --endian le` | 32 bytes — `0100000000000000040000006162630006000000c3bce188b4000000ffffffff` |
| `--encoding xcdr1 --endian be` | 32 bytes — `0000000100000000000000046162630000000006c3bce188b4000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

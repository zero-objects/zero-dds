# sequence-struct-unannotated

**Construct** — `sequence` of a unannotated structure

A sequence whose element is an aggregate, unannotated. Two delimiters are in question at once: the collection's own DHEADER, which `vendor_bytes.md` §1 shows RTI omitting, and the per-element one the element's extensibility calls for — which is present for appendable and mutable elements and absent for final ones. `Cell` is deliberately four bytes wide and six bytes long so the inter-element padding is visible, and the trailing `long` shows whether the last element was padded, which `vendor_bytes.md` says it must not be.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 76 bytes — `04000000000000000e00000001000000060000007929edfffeff0000260000000300000006000000010000000200000006000000ffffffffff7f000006000000ffffff7f00800000ffffffff` |
| `--encoding xcdr2 --endian be` | 76 bytes — `00000004000000000000000e0000000100000006ffed2979fffe0000000000260000000300000006000000010002000000000006ffffffff7fff0000000000067fffffff80000000ffffffff` |
| `--encoding xcdr1 --endian le` | 48 bytes — `00000000010000007929edfffeff0000030000000100000002000000ffffffffff7f0000ffffff7f00800000ffffffff` |
| `--encoding xcdr1 --endian be` | 48 bytes — `0000000000000001ffed2979fffe0000000000030000000100020000ffffffff7fff00007fffffff80000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

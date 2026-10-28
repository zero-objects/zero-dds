# sequence-struct-mutable

**Construct** — `sequence` of a mutable structure

A sequence whose element is an aggregate, mutable. Two delimiters are in question at once: the collection's own DHEADER, which `vendor_bytes.md` §1 shows RTI omitting, and the per-element one the element's extensibility calls for — which is present for appendable and mutable elements and absent for final ones. `Cell` is deliberately four bytes wide and six bytes long so the inter-element padding is visible, and the trailing `long` shows whether the last element was padded, which `vendor_bytes.md` says it must not be.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 108 bytes |
| `--encoding xcdr2 --endian be` | 108 bytes |
| `--encoding xcdr1 --endian le` | 96 bytes — `0000000001000000000004007929edff01000200feff0000027f00000300000000000400010000000100020002000000027f000000000400ffffffff01000200ff7f0000027f000000000400ffffff7f0100020000800000027f0000ffffffff` |
| `--encoding xcdr1 --endian be` | 96 bytes — `000000000000000100000004ffed297900010002fffe00007f02000000000003000000040000000100010002000200007f02000000000004ffffffff000100027fff00007f020000000000047fffffff00010002800000007f020000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

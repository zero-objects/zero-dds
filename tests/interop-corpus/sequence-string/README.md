# sequence-string

**Construct** — `sequence<string>`

A string is not a primitive under XTypes 1.3 §7.2.2.2, so rule (12) gives this sequence a collection DHEADER under XCDR2. `vendor_bytes.md` §1 records RTI omitting that delimiter in every case it was measured on, which is why the policy exists. The elements are empty, ASCII and multi-byte so the per-element lengths all differ. Empty, one element, several and bounded in one sample, because a delimiter bug that shows on a populated sequence often does not show on an empty one.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 96 bytes — `04000000000000000c0000000100000004000000616263002600000003000000010000000000000004000000616263000e000000c3bc62657220e2809420e188b4000000140000000200000001000000000000000400000061626300ffffffff` |
| `--encoding xcdr2 --endian be` | 96 bytes — `00000004000000000000000c0000000100000004616263000000002600000003000000010000000000000004616263000000000ec3bc62657220e2809420e188b4000000000000140000000200000001000000000000000461626300ffffffff` |
| `--encoding xcdr1 --endian le` | 80 bytes — `0000000001000000040000006162630003000000010000000000000004000000616263000e000000c3bc62657220e2809420e188b40000000200000001000000000000000400000061626300ffffffff` |
| `--encoding xcdr1 --endian be` | 80 bytes — `0000000000000001000000046162630000000003000000010000000000000004616263000000000ec3bc62657220e2809420e188b40000000000000200000001000000000000000461626300ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

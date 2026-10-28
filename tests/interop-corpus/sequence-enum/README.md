# sequence-enum

**Construct** — `sequence<enum>`

An enumeration is a constructed type under XTypes 1.3 §7.2.2.4.1, so the letter of rule (12) gives this sequence a DHEADER. `vendor_bytes.md` §1 measures Cyclone writing it, RTI and Fast DDS omitting it — Fast DDS following the specification's own rationale that a fixed-width element needs no delimiter. This is the row the `enum-is-primitive` delimiter policy exists for. Empty, one element, several and bounded in one sample, because a delimiter bug that shows on a populated sequence often does not show on an empty one.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 64 bytes — `04000000000000000800000001000000000000001400000004000000020000000000000001000000020000000c000000020000000200000000000000ffffffff` |
| `--encoding xcdr2 --endian be` | 64 bytes — `00000004000000000000000800000001000000000000001400000004000000020000000000000001000000020000000c000000020000000200000000ffffffff` |
| `--encoding xcdr1 --endian le` | 48 bytes — `0000000001000000000000000400000002000000000000000100000002000000020000000200000000000000ffffffff` |
| `--encoding xcdr1 --endian be` | 48 bytes — `0000000000000001000000000000000400000002000000000000000100000002000000020000000200000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

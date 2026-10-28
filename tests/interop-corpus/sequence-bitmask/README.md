# sequence-bitmask

**Construct** — `sequence<bitmask>`

A bitmask is a fixed-width constructed type, so it lands on the same side of the collection-delimiter question as an enumeration: the letter of XTypes 1.3 §7.4.3.5 rule (12) says delimit, the rationale says it is unnecessary. The third element is the empty bitmask, which is two bytes of zero and not an absent element.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `04000000000000000a000000030000000100808000000000ffffffff` |
| `--encoding xcdr2 --endian be` | 28 bytes — `00000004000000000000000a000000030001808000000000ffffffff` |
| `--encoding xcdr1 --endian le` | 20 bytes — `00000000030000000100808000000000ffffffff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `00000000000000030001808000000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

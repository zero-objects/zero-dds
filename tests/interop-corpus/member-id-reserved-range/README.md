# member-id-reserved-range

**Construct** — a member whose id falls inside the range the short parameter header reserves

XTypes 1.3 §7.4.1.2.2 puts the short header's usable ids "from 0 to 16,128 (0x3F00) inclusive" and reserves 0x3F01–0x3FFF. The extended header carries twenty-eight bits and has no reserved range, so `@id(16132)` — 0x3F04 — is a legal member id that can only travel in the extended form.

That is what makes the reserved range measurable at all. A stream carrying the short id `0x3F04` and a type declaring a member numbered 16132 are two different things, and a reader that masks the reserved range down to a member number makes them the same: the reserved parameter's payload becomes that member's value, with the right length and no error. `tools/inventory-probes/parameter-edges/` builds that stream; three of the four DDS implementations read it as the member.

There is no `-unannotated` twin, for the same reason as `member-id-private-parameter`: the type has to be `@mutable` for a parameter list to exist.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `100000000100002007000000043f002009000000` |
| `--encoding xcdr2 --endian be` | 20 bytes — `00000010200000010000000720003f0400000009` |
| `--encoding xcdr1 --endian le` | 28 bytes — `0100040007000000017f0800043f00000400000009000000027f0000` |
| `--encoding xcdr1 --endian be` | 28 bytes — `00010004000000077f01000800003f0400000004000000097f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

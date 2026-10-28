# string-bounded-overrun

**Construct** — a `string<4>` with a member behind it, the shape a bound overrun is visible in

A bound that is respected costs no octet, which is why `string-bounded-utf8` cannot answer what a bound *does*. §7.2.2.7 makes an overrun DISCARD; what each implementation actually does with one is measured by `tools/inventory-probes/bound-overrun/`, which sets the member to eight characters after the type support has been generated, because the harness refuses an over-long value before any vendor sees it.

The four answers are in four different places. Cyclone's `idlc` maps `string<4>` to `char s[5]`, so the value is not representable at all; RTI's holder is a bare `DDS_Char *` and its serialiser refuses — *"String length (at least 5) is larger than maximum 4"*; OpenDDS's holder is a `TAO::String_Manager` that takes any length and its serialiser refuses the sample; and Fast DDS's is a `fixed_string<4>` that truncates on assignment, so the payload it writes for eight characters is byte-identical to the one it writes for four.

`tail` is what makes the truncation matter: it is the member every octet of an unenforced overrun would move.

There is no `-unannotated` twin: the question is the bound and not the extensibility.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `0500000061626364000000002a000000` |
| `--encoding xcdr2 --endian be` | 16 bytes — `0000000561626364000000000000002a` |
| `--encoding xcdr1 --endian le` | 16 bytes — `0500000061626364000000002a000000` |
| `--encoding xcdr1 --endian be` | 16 bytes — `0000000561626364000000000000002a` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

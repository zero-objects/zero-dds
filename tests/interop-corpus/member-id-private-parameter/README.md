# member-id-private-parameter

**Construct** — a `@mutable` type with two explicitly numbered members, so a parameter list can be hand-built around them

The type is unremarkable and the case exists for what can be written *beside* it. XTypes 1.3 §7.4.1.2.2 gives bit 15 of a short parameter id to `FLAG_IMPL_EXTENSION` and says a user-defined member "will never set" it; §7.4.1.2.1 gives `FLAG_3` and `FLAG_4` of the extended `eMemberHeader` to `FLAG_UNSPECIFIED1` and `FLAG_UNSPECIFIED2`, which "should be set to zero"; and §7.4.1.2.2 gives `0x3F03` to `PID_IGNORE`, which "All consumers of this Data Representation shall ignore".

None of the three is a byte any writer produces, so none of them could be measured by handing a vendor a sample and reading what came out. They are measured by handing a vendor's own deserialiser a stream nobody would write — `tools/inventory-probes/parameter-edges/` builds one per edge, in each vendor's own framing — and the two explicit ids are what makes that possible: a private parameter's low fourteen bits have to collide with a real member's number for the collision to be observable.

There is no `-unannotated` twin. The type is `@mutable` because a parameter list is what carries the headers being asked about, so there is no default for the twin to isolate.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `1000000001000020070000000200002009000000` |
| `--encoding xcdr2 --endian be` | 20 bytes — `0000001020000001000000072000000200000009` |
| `--encoding xcdr1 --endian le` | 20 bytes — `01000400070000000200040009000000027f0000` |
| `--encoding xcdr1 --endian be` | 20 bytes — `000100040000000700020004000000097f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

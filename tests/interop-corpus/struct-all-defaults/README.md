# struct-all-defaults

**Construct** — every member at the value Table 9 gives its type

XTypes 1.3 §7.4.1.2 requires an encoder to write every non-optional member, and a member being at its default value is not an exception: only `@optional` gives a member an absence to be written. The case is the one shape under which the requirement is falsifiable — a writer that elided a defaulted member would produce a *shorter* payload, not a different one, so any sample with a non-default value in it hides the question.

The member list is one per shape that could plausibly be treated as absent: a zero `long`, an empty `string` (which is still four octets of length and a NUL), a `false` `boolean`, a zero `double` and a zero `short`. Twenty-six octets under XCDR1 and twenty-two under XCDR2 is what all of them present costs.

There is no `-unannotated` twin: the case is about member presence and not about extensibility, and `struct-final` already carries the annotated/unannotated pair for this shape.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 22 bytes — `00000000010000000000000000000000000000000000` |
| `--encoding xcdr2 --endian be` | 22 bytes — `00000000000000010000000000000000000000000000` |
| `--encoding xcdr1 --endian le` | 26 bytes — `0000000001000000000000000000000000000000000000000000` |
| `--encoding xcdr1 --endian be` | 26 bytes — `0000000000000001000000000000000000000000000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

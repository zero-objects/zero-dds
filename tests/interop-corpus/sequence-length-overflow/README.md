# sequence-length-overflow

**Construct** — an unbounded sequence with a member behind it, so a hand-built length word can be given a value no stream can satisfy

XTypes 1.3 §7.4.3.2 constrains `O.ssize` below 4 GiB, which is what makes the length field of a collection thirty-two bits. Nothing writes an over-long one, so the constraint is only observable on the read side: the case exists so a stream claiming `0xFFFFFFFF` elements, and one claiming 2^30 — four gibibytes of body exactly — can be put in front of each implementation's own deserialiser by `tools/inventory-probes/parameter-edges/`.

The `tail` member is what makes the refusal matter rather than merely happen. A reader that took the length on trust and ran off the end of the buffer would be reading `tail`'s octets, and everything after them, as sequence elements.

There is no `-unannotated` twin: the type is `@final` and the sample carries two elements, so the default extensibility has nothing to change that `sequence-long` does not already record.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `0200000007000000090000002a000000` |
| `--encoding xcdr2 --endian be` | 16 bytes — `0000000200000007000000090000002a` |
| `--encoding xcdr1 --endian le` | 16 bytes — `0200000007000000090000002a000000` |
| `--encoding xcdr1 --endian be` | 16 bytes — `0000000200000007000000090000002a` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

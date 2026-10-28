# longdouble-framed

**Construct** — a `long double` with an octet on each side, so its offset and its width are both readable off the payload

Table 31 gives Float128 an encoded size of 16 and an alignment of 8, which under encoding version 2 becomes `MIN(16, MAXALIGN(VERSION2))` — four. `primitive-longdouble` cannot show either: six `long double` members in a row put the second one at a multiple of sixteen whatever the alignment rule is, and there is nothing behind them to say where the last one stopped.

`head` at offset 0 forces the padding to be visible, and `tail` says where the member ended. Twenty-five octets under XCDR1 is `1 + 7 + 16 + 1`; twenty-one under XCDR2 is `1 + 3 + 16 + 1`.

The value and the layout are two questions and only one of them has a foreign answer. `tools/inventory-probes/longdouble-xcdr2/` measures the layout: Fast DDS and OpenDDS both put `d` at payload offset 8 under XCDR1 and at offset 4 under XCDR2, sixteen octets wide in both — the same lengths this project writes. The bits inside those sixteen octets are not the same and cannot be: both copy the platform's x87 80-bit extended and pad, where this project writes CORBA 3.3 part 2 §9.3.1.3's binary128. Cyclone's `idlc` refuses the type — *"Long doubles are currently unsupported"* — and RTI's `rtiddsgen` accepts it as sixteen opaque octets, `DDS_LongDouble`.

There is no `-unannotated` twin: the question is the primitive's layout, and `primitive-longdouble` already carries the pair for the default extensibility.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 21 bytes — `010000000000000000000000000000000080ff3f02` |
| `--encoding xcdr2 --endian be` | 21 bytes — `010000003fff800000000000000000000000000002` |
| `--encoding xcdr1 --endian le` | 25 bytes — `01000000000000000000000000000000000000000080ff3f02` |
| `--encoding xcdr1 --endian be` | 25 bytes — `01000000000000003fff800000000000000000000000000002` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# union-enum-crossmodule

**Construct** — a union switching on an enumeration from another module

The discriminator's type, its case labels and the union itself sit in two modules. The label `::k::A` has to resolve to an enumerator and then to its value; an implementation that resolved it to a constant, or not at all, writes a different discriminator.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `080000000000000003000000` |
| `--encoding xcdr2 --endian be` | 12 bytes — `000000080000000000000003` |
| `--encoding xcdr1 --endian le` | 8 bytes — `0000000003000000` |
| `--encoding xcdr1 --endian be` | 8 bytes — `0000000000000003` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# enum-value-gaps

**Construct** — an enumeration with `@value` gaps and a negative

-5, 0 and 1000 with nothing between. The wire carries the value and not the ordinal, so an implementation that wrote positions sends 2 where this sends 1000.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 8 bytes — `04000000e8030000` |
| `--encoding xcdr2 --endian be` | 8 bytes — `00000004000003e8` |
| `--encoding xcdr1 --endian le` | 4 bytes — `e8030000` |
| `--encoding xcdr1 --endian be` | 4 bytes — `000003e8` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

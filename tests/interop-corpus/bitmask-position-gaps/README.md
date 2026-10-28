# bitmask-position-gaps

**Construct** — a bitmask with a `@position` gap

Bits 0 and 9 in a 16-bit holder. The holder's width and the bit's position are two separate facts; writing the second bit as bit 1 instead of bit 9 is a value the peer reads as a different flag.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 6 bytes — `020000000002` |
| `--encoding xcdr2 --endian be` | 6 bytes — `000000020200` |
| `--encoding xcdr1 --endian le` | 2 bytes — `0002` |
| `--encoding xcdr1 --endian be` | 2 bytes — `0200` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

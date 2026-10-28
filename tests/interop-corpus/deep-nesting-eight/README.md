# deep-nesting-eight

**Construct** — eight distinct struct types in one file, two of them used

Not depth but breadth: an implementation that plans every declared type rather than the reachable ones does eight times the work, and one that plans the wrong two writes the wrong sample.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `080000000100000008000000` |
| `--encoding xcdr2 --endian be` | 12 bytes — `000000080000000100000008` |
| `--encoding xcdr1 --endian le` | 8 bytes — `0100000008000000` |
| `--encoding xcdr1 --endian be` | 8 bytes — `0000000100000008` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

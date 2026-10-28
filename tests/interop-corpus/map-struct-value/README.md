# map-struct-value

**Construct** — a map whose value is a struct

`map<long, K>`. The entry is a pair and the value half is an aggregate, so the entry's own alignment is the struct's.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `100000000c000000010000000100000002000000` |
| `--encoding xcdr2 --endian be` | 20 bytes — `000000100000000c000000010000000100000002` |
| `--encoding xcdr1 --endian le` | 12 bytes — `010000000100000002000000` |
| `--encoding xcdr1 --endian be` | 12 bytes — `000000010000000100000002` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

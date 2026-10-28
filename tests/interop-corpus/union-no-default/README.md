# union-no-default

**Construct** — a union with no `default:` case

Every branch is explicit. A discriminator value no case selects is §7.4.3.5's open question, and the answer decides whether a writer may send one at all.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `080000000100000007000000` |
| `--encoding xcdr2 --endian be` | 12 bytes — `000000080000000100000007` |
| `--encoding xcdr1 --endian le` | 8 bytes — `0100000007000000` |
| `--encoding xcdr1 --endian be` | 8 bytes — `0000000100000007` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

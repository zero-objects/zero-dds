# string-bound-from-const

**Construct** — a string whose bound is a named constant

`string<CAP>` where `CAP` is declared elsewhere. The bound has to be evaluated through the name; one that silently became unbounded changes what the writer accepts without changing what it writes.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `080000000400000061626300` |
| `--encoding xcdr2 --endian be` | 12 bytes — `000000080000000461626300` |
| `--encoding xcdr1 --endian le` | 8 bytes — `0400000061626300` |
| `--encoding xcdr1 --endian be` | 8 bytes — `0000000461626300` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

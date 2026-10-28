# empty-struct

**Construct** — a struct with no members

Legal IDL. XCDR2 still writes a delimiter for an @appendable type, so the sample is not empty — an implementation that emitted nothing produces a stream the reader cannot advance past.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 4 bytes — `00000000` |
| `--encoding xcdr2 --endian be` | 4 bytes — `00000000` |
| `--encoding xcdr1 --endian le` | 0 bytes — `` |
| `--encoding xcdr1 --endian be` | 0 bytes — `` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

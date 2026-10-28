# char-and-wchar

**Construct** — a `char` beside a `wchar`

One octet and two, with the alignment that follows. A `wchar` framed as a `char` shifts everything after it.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 8 bytes — `0400000061007a00` |
| `--encoding xcdr2 --endian be` | 8 bytes — `000000046100007a` |
| `--encoding xcdr1 --endian le` | 4 bytes — `61007a00` |
| `--encoding xcdr1 --endian be` | 4 bytes — `6100007a` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

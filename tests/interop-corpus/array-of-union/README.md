# array-of-union

**Construct** — an array whose element is a union

As above without a length on the wire: the count is in the type. Two elements selecting different branches make the element size vary, which is what an array is not supposed to do.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `200000001c0000000800000001000000010000000c000000000000000000000000000040` |
| `--encoding xcdr2 --endian be` | 36 bytes — `000000200000001c0000000801000000000000010000000c000000004000000000000000` |
| `--encoding xcdr1 --endian le` | 24 bytes — `010000000100000000000000000000000000000000000040` |
| `--encoding xcdr1 --endian be` | 24 bytes — `010000000000000100000000000000004000000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

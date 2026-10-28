# sequence-of-union

**Construct** — a sequence whose element is a union

Each element carries its own discriminator. Where the delimiter goes — round the sequence, round each element, or both — is what XTypes 1.3 §7.4.3.5 leaves to be read carefully.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `140000001000000001000000080000000100000001000000` |
| `--encoding xcdr2 --endian be` | 24 bytes — `000000140000001000000001000000080100000000000001` |
| `--encoding xcdr1 --endian le` | 12 bytes — `010000000100000001000000` |
| `--encoding xcdr1 --endian be` | 12 bytes — `000000010100000000000001` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

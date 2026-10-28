# wstring-member

**Construct** — a `wstring` member

The length is in code units under XTypes and in octets under some vendors — the divergence documented in docs/peer-defects.md for Cyclone. Two readings of one field.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 14 bytes — `0a00000006000000610062006300` |
| `--encoding xcdr2 --endian be` | 14 bytes — `0000000a00000006006100620063` |
| `--encoding xcdr1 --endian le` | 10 bytes — `06000000610062006300` |
| `--encoding xcdr1 --endian be` | 10 bytes — `00000006006100620063` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# optional-nested-struct

**Construct** — an absent `@optional` member of aggregate type

§7.4.1.2.3 makes the **absence** the value. The sample here leaves it out; an implementation that wrote a zeroed struct sends a member the writer did not set.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `080000000100002005000000` |
| `--encoding xcdr2 --endian be` | 12 bytes — `000000082000000100000005` |
| `--encoding xcdr1 --endian le` | 12 bytes — `0100040005000000027f0000` |
| `--encoding xcdr1 --endian be` | 12 bytes — `00010004000000057f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

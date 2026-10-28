# key-on-nested-struct

**Construct** — `@key` on a member whose type is itself keyed

The KeyHash is over the key members, and the nested type has its own. Which of them reaches the hash — the whole struct, or only its keyed member — decides the instance.

## The axis

An edge case in what the specification allows and implementations can
each read differently. It is on the wire axis because the disagreement
shows up as octets — and the point of the corpus is that ZeroDDS is the
column every other one is read against, including where another
implementation refuses the file outright. A refusal is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `0c000000010000000200000003000000` |
| `--encoding xcdr2 --endian be` | 16 bytes — `0000000c000000010000000200000003` |
| `--encoding xcdr1 --endian le` | 12 bytes — `010000000200000003000000` |
| `--encoding xcdr1 --endian be` | 12 bytes — `000000010000000200000003` |
| `--emit keyhash` | 16 bytes — `00000001000000000000000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

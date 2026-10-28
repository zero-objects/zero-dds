# sequence-map

**Construct** — `sequence<map>`

A map as a sequence element, including an empty map in the middle so an implementation that skips empty maps shifts everything after it.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 48 bytes — `280000000300000001000000010000007929edff000000000200000002000000030000000400000005000000ffffffff` |
| `--encoding xcdr2 --endian be` | 48 bytes — `00000028000000030000000100000001ffed2979000000000000000200000002000000030000000400000005ffffffff` |
| `--encoding xcdr1 --endian le` | 44 bytes — `0300000001000000010000007929edff000000000200000002000000030000000400000005000000ffffffff` |
| `--encoding xcdr1 --endian be` | 44 bytes — `000000030000000100000001ffed2979000000000000000200000002000000030000000400000005ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

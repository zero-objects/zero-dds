# struct-nested-two-deep

**Construct** — structure nested two levels

Two levels of nesting with the alignment increasing as it goes in: a six-byte leaf, then a double that forces padding the leaf did not need. A nested aggregate is inlined, not length-prefixed, when it is final — so the total is a single run and the trailing octet lands where the arithmetic says, not where a nested framing would put it.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 17 bytes — `7929edfffeff0000182d4454fb210940ab` |
| `--encoding xcdr2 --endian be` | 17 bytes — `ffed2979fffe0000400921fb54442d18ab` |
| `--encoding xcdr1 --endian le` | 17 bytes — `7929edfffeff0000182d4454fb210940ab` |
| `--encoding xcdr1 --endian be` | 17 bytes — `ffed2979fffe0000400921fb54442d18ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

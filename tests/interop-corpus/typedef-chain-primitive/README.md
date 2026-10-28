# typedef-chain-primitive

**Construct** — `typedef` chain over a primitive

Three aliases deep. A typedef is transparent on the wire, so `far` and `near` are the same four bytes at the same alignment. The case exists because an implementation that stops resolving after one alias, or that treats a named type as a constructed one, changes the delimiter and alignment decisions that depend on the resolved kind.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 8 bytes — `7929edffffffff7f` |
| `--encoding xcdr2 --endian be` | 8 bytes — `ffed29797fffffff` |
| `--encoding xcdr1 --endian le` | 8 bytes — `7929edffffffff7f` |
| `--encoding xcdr1 --endian be` | 8 bytes — `ffed29797fffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

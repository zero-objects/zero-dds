# typedef-of-sequence

**Construct** — `typedef` of a sequence

A collection reached through two aliases. §3 of the interop spec asks for a typedef of a collection specifically, because the collection-delimiter rule is decided on the element kind and an implementation that reads the alias as the element gets it backwards.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `030000007929edffffffff7f01000000ffffffff` |
| `--encoding xcdr2 --endian be` | 20 bytes — `00000003ffed29797fffffff00000001ffffffff` |
| `--encoding xcdr1 --endian le` | 20 bytes — `030000007929edffffffff7f01000000ffffffff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `00000003ffed29797fffffff00000001ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

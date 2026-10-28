# union-member-sequence

**Construct** — `union` with a sequence branch

A two-byte discriminator followed by a four-byte-aligned sequence count: two bytes of padding between them that an implementation packing the branch straight after the discriminator omits.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `10000000ffff0000020000007929edffffffff7fffffffff` |
| `--encoding xcdr2 --endian be` | 24 bytes — `00000010ffff000000000002ffed29797fffffffffffffff` |
| `--encoding xcdr1 --endian le` | 20 bytes — `ffff0000020000007929edffffffff7fffffffff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `ffff000000000002ffed29797fffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

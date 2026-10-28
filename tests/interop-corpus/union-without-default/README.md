# union-without-default

**Construct** — `union` with no `default`

No `default` branch, so a discriminator outside the labels selects nothing and the union is the discriminator alone. The sample selects a declared branch; the case exists to pair with `union-default-unselected`, where the same type meets an unmatched discriminator.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `0c0000000200000000000000000004c0ffffffff` |
| `--encoding xcdr2 --endian be` | 20 bytes — `0000000c00000002c004000000000000ffffffff` |
| `--encoding xcdr1 --endian le` | 20 bytes — `020000000000000000000000000004c0ffffffff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `0000000200000000c004000000000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

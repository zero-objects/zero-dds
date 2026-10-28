# bitmask-default-bitbound

**Construct** — `bitmask` with no `@bit_bound`

No bound given. XTypes 1.3 §7.3.1.2.1.6 makes the default 32, so this is four bytes with bit 0 and bit 31 set — `01000080` little-endian. An implementation that infers the width from the highest declared position happens to agree here and disagrees on `bitmask-sparse-positions`.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 5 bytes — `01000080ab` |
| `--encoding xcdr2 --endian be` | 5 bytes — `80000001ab` |
| `--encoding xcdr1 --endian le` | 5 bytes — `01000080ab` |
| `--encoding xcdr1 --endian be` | 5 bytes — `80000001ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

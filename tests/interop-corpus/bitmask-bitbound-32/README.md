# bitmask-bitbound-32

**Construct** — `bitmask` at bit bound 32

A bitmask 32 bits wide with a bit at position 31, the highest the bound allows. The wire form is the unsigned integer of that width, so the top bit is set in `set_high` — an implementation that stores the mask in a signed type, or that writes a machine word regardless of the bound, gets a different length or a sign-extended value. The empty member is 4 bytes of zero and not an omission, and the trailing octet pins where the third member ended.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 13 bytes — `030000000000008000000000ab` |
| `--encoding xcdr2 --endian be` | 13 bytes — `000000038000000000000000ab` |
| `--encoding xcdr1 --endian le` | 13 bytes — `030000000000008000000000ab` |
| `--encoding xcdr1 --endian be` | 13 bytes — `000000038000000000000000ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

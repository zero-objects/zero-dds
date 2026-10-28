# bitmask-bitbound-8

**Construct** — `bitmask` at bit bound 8

A bitmask 8 bits wide with a bit at position 7, the highest the bound allows. The wire form is the unsigned integer of that width, so the top bit is set in `set_high` — an implementation that stores the mask in a signed type, or that writes a machine word regardless of the bound, gets a different length or a sign-extended value. The empty member is 1 byte of zero and not an omission, and the trailing octet pins where the third member ended.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 4 bytes — `038000ab` |
| `--encoding xcdr2 --endian be` | 4 bytes — `038000ab` |
| `--encoding xcdr1 --endian le` | 4 bytes — `038000ab` |
| `--encoding xcdr1 --endian be` | 4 bytes — `038000ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# fixed-basic

**Construct** — `fixed<9,3>`

Nine digits, three after the point: five bytes of packed BCD, two digits per byte, the sign in the low nibble of the last. One positive value using all nine digits and one negative value using one, so both the padding nibble and the sign nibble differ between the two members. A `fixed` aligns to one, so the trailing octet lands at offset ten.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 11 bytes — `123456789c000000001dab` |
| `--encoding xcdr2 --endian be` | 11 bytes — `123456789c000000001dab` |
| `--encoding xcdr1 --endian le` | 11 bytes — `123456789c000000001dab` |
| `--encoding xcdr1 --endian be` | 11 bytes — `123456789c000000001dab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

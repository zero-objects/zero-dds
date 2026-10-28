# bitset-typed-fields

**Construct** — `bitset` with declared field types

Sixteen declared bits with a per-field destination type. The destination type changes the generated API, not the wire: the storage is still the smallest integer that holds sixteen bits. An implementation that widens the storage to the largest field type writes more bytes than the bit count calls for.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 3 bytes — `1dfaab` |
| `--encoding xcdr2 --endian be` | 3 bytes — `fa1dab` |
| `--encoding xcdr1 --endian le` | 3 bytes — `1dfaab` |
| `--encoding xcdr1 --endian be` | 3 bytes — `fa1dab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

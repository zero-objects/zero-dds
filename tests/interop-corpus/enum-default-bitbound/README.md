# enum-default-bitbound

**Construct** — `enum` with no `@bit_bound`

No `@bit_bound` at all. XTypes 1.3 §7.3.1.2.1.5 makes the default 32, so this is four bytes and the octet lands at offset four. The case exists because the default is a rule and not an accident: an implementation that picks the narrowest width that holds the largest enumerator would write one byte here and interoperate with nobody.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 5 bytes — `02000000ab` |
| `--encoding xcdr2 --endian be` | 5 bytes — `00000002ab` |
| `--encoding xcdr1 --endian le` | 5 bytes — `02000000ab` |
| `--encoding xcdr1 --endian be` | 5 bytes — `00000002ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# enum-bitbound-32-plain

**Construct** — `enum` at bit bound 32, implicit values

Three enumerators with no `@value`, so they are 0, 1, 2 by declaration order. The bit bound decides the width: 4 bytes per member, so the trailing octet lands at offset 8. An implementation that ignores `@bit_bound` and writes four bytes shifts everything after it — the failure is silent when the bound happens to be 32 and fatal otherwise.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 9 bytes — `0200000000000000ab` |
| `--encoding xcdr2 --endian be` | 9 bytes — `0000000200000000ab` |
| `--encoding xcdr1 --endian le` | 9 bytes — `0200000000000000ab` |
| `--encoding xcdr1 --endian be` | 9 bytes — `0000000200000000ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

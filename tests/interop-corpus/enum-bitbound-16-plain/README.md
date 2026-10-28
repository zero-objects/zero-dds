# enum-bitbound-16-plain

**Construct** — `enum` at bit bound 16, implicit values

Three enumerators with no `@value`, so they are 0, 1, 2 by declaration order. The bit bound decides the width: 2 bytes per member, so the trailing octet lands at offset 4. An implementation that ignores `@bit_bound` and writes four bytes shifts everything after it — the failure is silent when the bound happens to be 32 and fatal otherwise.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 5 bytes — `02000000ab` |
| `--encoding xcdr2 --endian be` | 5 bytes — `00020000ab` |
| `--encoding xcdr1 --endian le` | 5 bytes — `02000000ab` |
| `--encoding xcdr1 --endian be` | 5 bytes — `00020000ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

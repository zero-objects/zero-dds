# enum-bitbound-8-plain

**Construct** — `enum` at bit bound 8, implicit values

Three enumerators with no `@value`, so they are 0, 1, 2 by declaration order. The bit bound decides the width: 1 byte per member, so the trailing octet lands at offset 2. An implementation that ignores `@bit_bound` and writes four bytes shifts everything after it — the failure is silent when the bound happens to be 32 and fatal otherwise.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 3 bytes — `0200ab` |
| `--encoding xcdr2 --endian be` | 3 bytes — `0200ab` |
| `--encoding xcdr1 --endian le` | 3 bytes — `0200ab` |
| `--encoding xcdr1 --endian be` | 3 bytes — `0200ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

# enum-bitbound-8-value

**Construct** — `enum` at bit bound 8, explicit `@value`

The same shape with `@value` on every enumerator, and a largest value that needs most of the bound: 200 does not fit in the next bound down. An implementation that renumbers by declaration order writes 2 where the sample says 200, which is a wrong value at the right offset — the class of error a length check never finds. The second member is 9, so ordinal and value differ there too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 3 bytes — `c809ab` |
| `--encoding xcdr2 --endian be` | 3 bytes — `c809ab` |
| `--encoding xcdr1 --endian le` | 3 bytes — `c809ab` |
| `--encoding xcdr1 --endian be` | 3 bytes — `c809ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

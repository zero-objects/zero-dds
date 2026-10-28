# enum-bitbound-32-value

**Construct** — `enum` at bit bound 32, explicit `@value`

The same shape with `@value` on every enumerator, and a largest value that needs most of the bound: 70000 does not fit in the next bound down. An implementation that renumbers by declaration order writes 2 where the sample says 70000, which is a wrong value at the right offset — the class of error a length check never finds. The second member is 9, so ordinal and value differ there too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 9 bytes — `7011010009000000ab` |
| `--encoding xcdr2 --endian be` | 9 bytes — `0001117000000009ab` |
| `--encoding xcdr1 --endian le` | 9 bytes — `7011010009000000ab` |
| `--encoding xcdr1 --endian be` | 9 bytes — `0001117000000009ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

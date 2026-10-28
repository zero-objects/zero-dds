# module-reopened-unqualified

**Construct** — a module reopened, unqualified reference

The same shape as `module-reopened` with the member type named without a scope, which OMG IDL 4.2 §7.5.2 allows because the reopened module is the same scope. The frontend did not resolve it: the name was looked up from the root and reported as not found, while the identical reference inside a single opening of the module resolved. Fixed; byte for byte this now equals `module-reopened`, which is what the case checks.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `7929edff182d4454fb210940` |
| `--encoding xcdr2 --endian be` | 12 bytes — `ffed2979400921fb54442d18` |
| `--encoding xcdr1 --endian le` | 16 bytes — `7929edff00000000182d4454fb210940` |
| `--encoding xcdr1 --endian be` | 16 bytes — `ffed297900000000400921fb54442d18` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

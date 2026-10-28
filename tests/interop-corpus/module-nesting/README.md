# module-nesting

**Construct** — modules nested two deep

A type two modules deep. Nesting has no wire form; what it changes is the type's name, which is what a TypeIdentifier is computed from and what a topic's registered type name is. The case is here so that the same bytes under a different scoped name are on record.

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

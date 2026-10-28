# module-reopened

**Construct** — a module reopened

The same module opened twice in one file, with the second half referring to the first through its root-anchored name. Reopening converges on one scope; an implementation that creates two produces a type whose member cannot be resolved, or resolves it to a duplicate with a different TypeIdentifier, which is what issue #42 reports as a topic that never matches.

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

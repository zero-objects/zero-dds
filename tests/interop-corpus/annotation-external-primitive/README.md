# annotation-external-primitive

**Construct** — `@external` on a primitive member

`@external` says the member is held by reference in the generated language mapping. XTypes 1.3 §7.3.1.2.1.10 gives it no wire effect at all, so this must be byte for byte `struct Sample { long a; long b; }`. An implementation that writes a presence flag or a pointer-shaped placeholder diverges on a member whose IDL says nothing about the wire.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 8 bytes — `7929edffffffff7f` |
| `--encoding xcdr2 --endian be` | 8 bytes — `ffed29797fffffff` |
| `--encoding xcdr1 --endian le` | 8 bytes — `7929edffffffff7f` |
| `--encoding xcdr1 --endian be` | 8 bytes — `ffed29797fffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

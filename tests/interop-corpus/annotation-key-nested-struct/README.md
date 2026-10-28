# annotation-key-nested-struct

**Construct** — `@key` inside a nested structure

A key member whose type is itself keyed. XTypes 1.3 §7.2.2.4.5 says the key of the outer type is the key members of the inner one, recursively — so the KeyHash covers `nested.k` and not `nested.v`. An implementation that treats `@key` on an aggregate as "the whole aggregate" includes `v` and computes a different hash for the same instance, which shows up as duplicate instances rather than as an error.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `7929edffffffff7ffeffffff` |
| `--encoding xcdr2 --endian be` | 12 bytes — `ffed29797ffffffffffffffe` |
| `--encoding xcdr1 --endian le` | 12 bytes — `7929edffffffff7ffeffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `ffed29797ffffffffffffffe` |
| `--emit keyhash` | 16 bytes — `ffed2979000000000000000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

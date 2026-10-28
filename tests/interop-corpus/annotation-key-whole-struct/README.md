# annotation-key-whole-struct

**Construct** — `@key` on an unkeyed nested structure

The other half of the recursive rule: the inner type has no `@key` of its own, so all of its members are key. The pair with `annotation-key-nested-struct` is what distinguishes the two readings — the same outer declaration, different inner types, and a KeyHash that must differ.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `7929edfffeff000001000000` |
| `--encoding xcdr2 --endian be` | 12 bytes — `ffed2979fffe000000000001` |
| `--encoding xcdr1 --endian le` | 12 bytes — `7929edfffeff000001000000` |
| `--encoding xcdr1 --endian be` | 12 bytes — `ffed2979fffe000000000001` |
| `--emit keyhash` | 16 bytes — `ffed2979fffe00000000000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

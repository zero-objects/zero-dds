# annotation-external-struct

**Construct** — `@external` on an aggregate member

The same no-op on an aggregate, which is where an implementation is most tempted to make it mean something: a recursive type needs `@external` to terminate, and an implementation that marks the indirection on the wire cannot read anyone else's bytes.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `060000007929edfffeff000001000000` |
| `--encoding xcdr2 --endian be` | 16 bytes — `00000006ffed2979fffe000000000001` |
| `--encoding xcdr1 --endian le` | 12 bytes — `7929edfffeff000001000000` |
| `--encoding xcdr1 --endian be` | 12 bytes — `ffed2979fffe000000000001` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

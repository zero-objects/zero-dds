# annotation-id-explicit

**Construct** — `@id`

Explicit member ids out of order with the declaration. The wire order is the declaration order and the ids are what they say — an implementation that sorts members by id, or that renumbers them sequentially, produces headers that a peer resolves to the wrong members while every length stays correct.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 26 bytes — `160000000a0000207929edff02000020ffffff7f07000010feff` |
| `--encoding xcdr2 --endian be` | 26 bytes — `000000162000000affed2979200000027fffffff10000007fffe` |
| `--encoding xcdr1 --endian le` | 28 bytes — `0a0004007929edff02000400ffffff7f07000200feff0000027f0000` |
| `--encoding xcdr1 --endian be` | 28 bytes — `000a0004ffed2979000200047fffffff00070002fffe00007f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

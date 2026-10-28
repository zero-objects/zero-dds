# sequence-bounded-full

**Construct** — bounded `sequence` filled to its bound

The bound reached exactly. Separate from the under-filled case because an implementation that treats a full bounded sequence as a fixed array — dropping the count — produces four fewer bytes and still decodes against itself.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 17 bytes — `030000007929edffffffff7fffffffffab` |
| `--encoding xcdr2 --endian be` | 17 bytes — `00000003ffed29797fffffffffffffffab` |
| `--encoding xcdr1 --endian le` | 17 bytes — `030000007929edffffffff7fffffffffab` |
| `--encoding xcdr1 --endian be` | 17 bytes — `00000003ffed29797fffffffffffffffab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

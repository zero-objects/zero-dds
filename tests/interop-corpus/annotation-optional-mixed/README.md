# annotation-optional-mixed

**Construct** — several `@optional` members, some absent

Three optionals of which two are present and one is not, in a mutable type where each member carries its id. The absent one leaves no gap, so the member ids on the wire are not consecutive — a reader that assumes they are reads `last` from `text`'s header.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 26 bytes — `16000000010000207929edff0200005006000000c3bc62657200` |
| `--encoding xcdr2 --endian be` | 26 bytes — `0000001620000001ffed29795000000200000006c3bc62657200` |
| `--encoding xcdr1 --endian le` | 28 bytes — `010004007929edff02000a0006000000c3bc626572000000027f0000` |
| `--encoding xcdr1 --endian be` | 28 bytes — `00010004ffed29790002000a00000006c3bc6265720000007f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

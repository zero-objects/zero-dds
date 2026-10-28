# annotation-must-understand

**Construct** — `@must_understand`

The must-understand bit lives in the EMHEADER, so it is only visible in a mutable type. Set on one required and one optional member and clear on the third, so the flag is not a constant across the sample. A reader that ignores an unknown member it was told to understand accepts data it should have rejected, which is a QoS-level failure that looks like success at the byte level.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 34 bytes — `1e000000000000a07929edff01000020ffffff7f020000d006000000c3bc62657200` |
| `--encoding xcdr2 --endian be` | 34 bytes — `0000001ea0000000ffed2979200000017fffffffd000000200000006c3bc62657200` |
| `--encoding xcdr1 --endian le` | 36 bytes — `004004007929edff01000400ffffff7f02400a0006000000c3bc626572000000027f0000` |
| `--encoding xcdr1 --endian be` | 36 bytes — `40000004ffed2979000100047fffffff4002000a00000006c3bc6265720000007f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

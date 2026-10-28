# struct-many-members

**Construct** — structure with many members of one kind

Sixteen members, each with a different four-byte pattern, no padding anywhere. A pure offset test: any member written at the wrong index is visible as a distinct value, which a sample of small integers would not show. Under `@mutable` framing the same shape would exercise sixteen member ids, which `annotation-id-sparse` covers.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 64 bytes — `fffefefe02020202fdfcfcfc04040404fbfafafa06060606f9f8f8f808080808f7f6f6f60a0a0a0af5f4f4f40c0c0c0cf3f2f2f20e0e0e0ef1f0f0f010101010` |
| `--encoding xcdr2 --endian be` | 64 bytes — `fefefeff02020202fcfcfcfd04040404fafafafb06060606f8f8f8f908080808f6f6f6f70a0a0a0af4f4f4f50c0c0c0cf2f2f2f30e0e0e0ef0f0f0f110101010` |
| `--encoding xcdr1 --endian le` | 64 bytes — `fffefefe02020202fdfcfcfc04040404fbfafafa06060606f9f8f8f808080808f7f6f6f60a0a0a0af5f4f4f40c0c0c0cf3f2f2f20e0e0e0ef1f0f0f010101010` |
| `--encoding xcdr1 --endian be` | 64 bytes — `fefefeff02020202fcfcfcfd04040404fafafafb06060606f8f8f8f908080808f6f6f6f70a0a0a0af4f4f4f50c0c0c0cf2f2f2f30e0e0e0ef0f0f0f110101010` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

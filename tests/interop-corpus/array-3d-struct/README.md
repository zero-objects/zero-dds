# array-3d-struct

**Construct** — three-dimensional array of a structure

Eight three-byte elements aligned to two: seven inter-element pads of one byte, none after the last, and the trailing octet at offset twenty-three. Every distinct value, so a flattening-order error is visible byte for byte rather than as a length difference.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `1f000000ffff010002000200fdff030004000400fbff050006000600f9ff0700080008ab` |
| `--encoding xcdr2 --endian be` | 36 bytes — `0000001fffff010000020200fffd030000040400fffb050000060600fff90700000808ab` |
| `--encoding xcdr1 --endian le` | 32 bytes — `ffff010002000200fdff030004000400fbff050006000600f9ff0700080008ab` |
| `--encoding xcdr1 --endian be` | 32 bytes — `ffff010000020200fffd030000040400fffb050000060600fff90700000808ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

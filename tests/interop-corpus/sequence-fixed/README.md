# sequence-fixed

**Construct** — `sequence<fixed>`

`fixed<9,3>` is five bytes of packed BCD on the wire and aligns to one, so a sequence of three is fifteen bytes with no inter-element padding. An implementation that stores the value as a decimal string or as a scaled integer produces a different length for every element.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `1300000003000000012345678d000000001c999999999c00ffffffff` |
| `--encoding xcdr2 --endian be` | 28 bytes — `0000001300000003012345678d000000001c999999999c00ffffffff` |
| `--encoding xcdr1 --endian le` | 24 bytes — `03000000012345678d000000001c999999999c00ffffffff` |
| `--encoding xcdr1 --endian be` | 24 bytes — `00000003012345678d000000001c999999999c00ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

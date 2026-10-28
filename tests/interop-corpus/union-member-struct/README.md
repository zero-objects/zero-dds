# union-member-struct

**Construct** — `union` with an aggregate branch

The selected member is an appendable structure, so its own DHEADER sits inside the union. An implementation that computes the union's extent from the branch's declared size rather than from what it wrote is wrong by exactly those four bytes.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `0e00000001000000060000007929edfffeff0000ffffffff` |
| `--encoding xcdr2 --endian be` | 24 bytes — `0000000e0000000100000006ffed2979fffe0000ffffffff` |
| `--encoding xcdr1 --endian le` | 16 bytes — `010000007929edfffeff0000ffffffff` |
| `--encoding xcdr1 --endian be` | 16 bytes — `00000001ffed2979fffe0000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

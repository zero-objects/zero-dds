# union-default-unselected

**Construct** — `union` with no `default`, discriminator matching nothing

A discriminator no label matches and no `default` to catch it. XTypes 1.3 §7.4.3.5.4 says the union is then the discriminator and nothing else — four bytes, not four bytes plus a zeroed branch. An implementation that writes a branch anyway pushes everything after it, which is a length difference every peer notices; one that writes the largest branch zeroed is worse, because it decodes.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `0400000063000000ffffffff` |
| `--encoding xcdr2 --endian be` | 12 bytes — `0000000400000063ffffffff` |
| `--encoding xcdr1 --endian le` | 8 bytes — `63000000ffffffff` |
| `--encoding xcdr1 --endian be` | 8 bytes — `00000063ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

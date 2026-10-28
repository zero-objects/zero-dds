# union-discriminator-enum

**Construct** — `union` over an `enum`

An enumeration discriminator, which is the fourth legal kind and the one every other one is a special case of: the label is a name, its value comes from the enumeration, and its width comes from the enumeration's bit bound. Two labels share the second branch. This case did not lower when it was written — an enumerator used as a case label resolved as a type name and was not found — which is exactly the kind of gap the corpus is meant to make visible. Fixed; the case now guards the fix.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `0c00000001000000182d4454fb210940ffffffff` |
| `--encoding xcdr2 --endian be` | 20 bytes — `0000000c00000001400921fb54442d18ffffffff` |
| `--encoding xcdr1 --endian le` | 20 bytes — `0100000000000000182d4454fb210940ffffffff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `0000000100000000400921fb54442d18ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

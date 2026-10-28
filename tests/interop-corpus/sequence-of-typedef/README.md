# sequence-of-typedef

**Construct** — `sequence` of a typedef chain

The element is two typedefs away from `long`. A typedef is transparent on the wire, so this must be byte-identical to `sequence<long>` — and, more to the point, the collection-delimiter rule must be decided on the resolved element kind, not on the alias. An implementation that sees a named type and therefore a non-primitive writes a DHEADER that nothing else expects.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `030000007929edffffffff7f01000000ffffffff` |
| `--encoding xcdr2 --endian be` | 20 bytes — `00000003ffed29797fffffff00000001ffffffff` |
| `--encoding xcdr1 --endian le` | 20 bytes — `030000007929edffffffff7f01000000ffffffff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `00000003ffed29797fffffff00000001ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

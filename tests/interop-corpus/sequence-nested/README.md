# sequence-nested

**Construct** — `sequence<sequence<long>>`

One level of nesting, with an empty inner sequence first so the outer count and the inner count cannot be confused. The inner element is a primitive and needs no delimiter; the outer element is a sequence, which is not a primitive, so it does. Also a parser case: `>>` at the end has to split into two closing brackets, which `zerodds-idl-frontend-1.0` §2.3 names as an ambiguity the parser must decide.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 44 bytes — `240000000300000000000000010000007929edff0400000001000000020000000300000004000000ffffffff` |
| `--encoding xcdr2 --endian be` | 44 bytes — `00000024000000030000000000000001ffed29790000000400000001000000020000000300000004ffffffff` |
| `--encoding xcdr1 --endian le` | 40 bytes — `0300000000000000010000007929edff0400000001000000020000000300000004000000ffffffff` |
| `--encoding xcdr1 --endian be` | 40 bytes — `000000030000000000000001ffed29790000000400000001000000020000000300000004ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

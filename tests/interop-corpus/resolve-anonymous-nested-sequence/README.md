# resolve-anonymous-nested-sequence

**Construct** — a sequence whose element is itself an anonymous sequence

`sequence<sequence<long> >` — the inner type has no name, so nothing can refer to it and the lowering has to give it an identity of its own. Two anonymous types that are structurally equal must not become one, and one type must not become two.

## The axis

One of the resolution and expansion edge cases: shapes where two
implementations can each do something defensible and produce different
octets. They are here rather than in a front-end test because the
disagreement is **visible on the wire** — a name bound to the wrong
declaration, an alias not followed, a dimension not folded, all of them
change what is written and none of them is reported as an error.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `18000000140000000200000001000000010000000100000002000000` |
| `--encoding xcdr2 --endian be` | 28 bytes — `00000018000000140000000200000001000000010000000100000002` |
| `--encoding xcdr1 --endian le` | 20 bytes — `0200000001000000010000000100000002000000` |
| `--encoding xcdr1 --endian be` | 20 bytes — `0000000200000001000000010000000100000002` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

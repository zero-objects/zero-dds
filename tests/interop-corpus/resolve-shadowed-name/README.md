# resolve-shadowed-name

**Construct** — the inner declaration of a name that also exists further out

`Point` is declared in `outer` and again in `outer::inner`, and the member writes the simple name from inside `inner`. §7.5 binds the innermost. An implementation that reached the outer one produces a sample with a different member — same length here by construction, different meaning, and nothing on the wire says so.

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
| `--encoding xcdr2 --endian le` | 8 bytes — `0400000003000000` |
| `--encoding xcdr2 --endian be` | 8 bytes — `0000000400000003` |
| `--encoding xcdr1 --endian le` | 4 bytes — `03000000` |
| `--encoding xcdr1 --endian be` | 4 bytes — `00000003` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

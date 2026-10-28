# resolve-module-named-as-type

**Construct** — a struct whose name is also its enclosing module's

`module common { struct common { … }; }` — legal IDL, and the member writes `common`, which names both. §7.5 resolves it to the struct, because a module is not a type. An implementation that bound the module produces no member at all.

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
| `--encoding xcdr2 --endian le` | 8 bytes — `0400000001000000` |
| `--encoding xcdr2 --endian be` | 8 bytes — `0000000400000001` |
| `--encoding xcdr1 --endian le` | 4 bytes — `01000000` |
| `--encoding xcdr1 --endian be` | 4 bytes — `00000001` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

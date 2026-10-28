# annotation-external-sequence

**Construct** — `@external` on collection members

XTypes 1.3 Table 21 applies `@external` to structure members, and the indirection it asks for is the language mapping's and not the wire form's — `docs/corba-column.md` records the CORBA column dropping it for exactly that reason. A collection member is where the two are easiest to confuse, because the collection already carries a pointer in most mappings. The corpus applies `@external` to a primitive and to a structure only.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 32 bytes — `0300000001000000feffffffffffff7f06000000616c706861000000ffffffff` |
| `--encoding xcdr2 --endian be` | 32 bytes — `0000000300000001fffffffe7fffffff00000006616c706861000000ffffffff` |
| `--encoding xcdr1 --endian le` | 32 bytes — `0300000001000000feffffffffffff7f06000000616c706861000000ffffffff` |
| `--encoding xcdr1 --endian be` | 32 bytes — `0000000300000001fffffffe7fffffff00000006616c706861000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

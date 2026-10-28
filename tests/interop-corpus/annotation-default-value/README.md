# annotation-default-value

**Construct** — `@default`

This is also the case GitHub issue #6 needs: `default` is a reserved IDL
keyword and `@default` is a builtin annotation whose name is that keyword, and
the report was that an annotation named by a keyword made the parser reject
the whole file. The case fails to lower if that returns, which is the only
form of that report a corpus can hold. The other half of the same production —
IDL 4.2 §8.2 writes the name as a `<scoped_name>`, so `@::default(42)` and
`@m::mark(7)` are the same rule — is held by
[`annotation-scoped-name`](../annotation-scoped-name/);
`docs/annotation-scoped-name-followup.md` has the measurement that found it.

`@default` states the value a reader substitutes for a member it did not receive. It has no wire form: the sample gives every member a value, and all three must be on the wire, none of them the default. An implementation that omits a member equal to its default, or that writes the default instead of the sample's value, is wrong in a way that only a sample deliberately unlike its defaults exposes.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `7929edffff7f000001000000` |
| `--encoding xcdr2 --endian be` | 12 bytes — `ffed29797fff000000000001` |
| `--encoding xcdr1 --endian le` | 12 bytes — `7929edffff7f000001000000` |
| `--encoding xcdr1 --endian be` | 12 bytes — `ffed29797fff000000000001` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

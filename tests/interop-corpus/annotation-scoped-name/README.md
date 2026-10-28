# annotation-scoped-name

**Construct** — an annotation applied by a scoped name

IDL 4.2 §8.2 writes the application of an annotation as

```text
<annotation_appl> ::= "@" <scoped_name> [ "(" <annotation_appl_params> ")" ]
```

and rule (4) makes a `<scoped_name>` an identifier, a root-anchored `::`
identifier, or a chain of them. This is the half of that production a corpus
did not reach. The other half — an annotation whose name collides with a
keyword, GitHub issue #6's `@default` — is held by
`annotation-default-value`, and both halves are the same rule.

`@::key` and `@ann::mark(7)` were both refused. The first diagnosed on the
`::` with *expected an annotation name after `@`*; the second took `ann` for
the whole name and resumed parsing at the `::` inside what it then believed
was a member declaration, which produced *expected a declarator name* and
eleven cascading diagnostics, none of which named an annotation.
`docs/annotation-scoped-name-followup.md` is the measurement.

The two spellings say different things and both have to reach the model.
`@::key` is the builtin `@key` named from the global scope, so `a` is the
key member; `@ann::mark(7)` names a user `@annotation` declared in another
module, which is kept whole in the model and has no wire form of its own.
The sample gives every member a value and all three are on the wire.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 12 bytes — `7929edffff7f000001000000` |
| `--encoding xcdr2 --endian be` | 12 bytes — `ffed29797fff000000000001` |
| `--encoding xcdr1 --endian le` | 12 bytes — `7929edffff7f000001000000` |
| `--encoding xcdr1 --endian be` | 12 bytes — `ffed29797fff000000000001` |
| `--emit keyhash` | 16 bytes — `ffed2979000000000000000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

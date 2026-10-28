# `zerodds-idl-compose`

The shared IDL composition pipeline. One call — `compose` — takes a path
and produces the model plus its registered type objects, so a `build.rs`
and the command-line tool see the same thing from the same file.

## What this crate no longer does

It used to do five things, and it existed because only the CLI did all
five: the `build.rs` scripts called the parser directly and got none of
them. Four have since moved into the front end:

| step | where it is now |
|---|---|
| `#include` / `#define` expansion | `zerodds-idl-parse::preprocess` |
| parse | `zerodds-idl-parse::parse_preprocessed` |
| vendor key pragmas → `@key` | `zerodds-idl-lower`'s `apply_keylists` |
| default extensibility | `Ast::effective_extensibility` |
| type objects | `zerodds-types::from_ast` |

The fourth is worth naming. It used to **write** an explicit
`@final`/`@appendable` onto every unannotated aggregate so each backend
would agree. The model answers the question instead, in one place, and
nothing is written into the tree — which is what stops two backends from
disagreeing without either being wrong.

`ComposeOptions::default_extensibility` is still there, because XTypes
1.3 §7.3.1.2.1.8 provides for it in as many words. It fills silence and
never overrules the source.

## Include resolution

A quoted include resolves beside the including file first, then along
the search list, and only last as written. An include that works because
of where the build ran is one that breaks on another machine.

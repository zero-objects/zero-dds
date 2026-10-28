# `zerodds-idl-parse`

OMG IDL 4.2 in, a lossless tree and a diagnostic list out.

The lexer, the preprocessor — `#include`, conditionals, macro expansion,
`#pragma once` — and a recovering parser that produces a tree for a file
it could not read, so a later stage can still point at what was there.

A diagnostic names its clause. `IDL0007` is not a number this project
chose to mean something; it carries the section of the specification
that refuses the construct, and the tests assert the clause, not the
number.

Safety classification is **STANDARD**, not SAFE, and that is the
architecture's own line: `docs/architecture/04_safety_by_architecture.md`
§2 puts the `zerodds-idl*` tools outside the safe subset. They run at
build time and never in a participant.

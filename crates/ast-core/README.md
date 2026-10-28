# `zerodds-ast-core`

The arena the common AST is built in.

Generational arenas with typed handles, stable node identity across
edits, three-valued references — resolved, pending, broken — and
undo-logged transactions that emit a changeset. An observer sees what
changed without diffing the tree.

This crate knows **nothing about IDL, XTypes or the wire**. It has no
node kinds of its own; `zerodds-ast` supplies those. Keeping the two
apart is what lets a second frontend reuse the storage without
inheriting one language's node space.

See `docs/specs/zerodds-ast-1.0.md` §3 for the identity and transaction
rules this crate implements.

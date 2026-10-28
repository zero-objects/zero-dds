# `zerodds-lang-render`

Turns a language CST into text.

The renderer **decides nothing**. It does not consult the common AST, it
does not branch on anything but the node in hand, and it never inserts a
construct that is not in the tree. Every decision about what generated
code contains sits in a rule, where it can be compared with the same rule
for another language.

Each language renders in one documented style — Rust as `rustfmt` would,
C++ and C in the brace style the previous emitters used, Java in the
Oracle convention. Style is not configurable; a downstream formatter is
the place for that.

This is the one crate where a `String` appears, and it appears only at
the leaves.

See `docs/specs/zerodds-lang-cst-1.0.md` §5.

# `zerodds-lang-rules`

Maps the common AST onto the language CSTs.

The rules are **data**, not code. A rule is a `RuleV2` value: it names an
L-side pattern over the model, an R-side pattern over a language CST, and
the correspondences between them. It can be compared, serialised, and
checked for completeness. Two languages' treatment of `@optional` is a
diff between two values, not a reading of two emitters three thousand
lines apart.

Rules are minimal and positive: small rules that compose, monotone and
add-only, with no negative application conditions. One rule maps a struct
to a type declaration; a second maps a member to a field; a third adds a
derive. A single rule doing all three could not be reused for the members
of a union.

The engine is seesaw's v2 layer — `seesaw_core::v2`. The crate carries
both rule layers side by side until the 1.1.0 cut, so binding the crate
is not by itself enough to land on the right one.

See `docs/specs/zerodds-lang-cst-1.0.md` §2 to §4.

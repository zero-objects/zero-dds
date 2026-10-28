# `zerodds-ast-layout`

Where a member sits, and what frames it.

Wire layout planning over the common AST: alignment, extent, header
framing and offsets, computed per `(encoding, endianness, purpose)`.
Every difference between XCDR1 and XCDR2, between a plain and a
delimited stream, between a payload and a KeyHash, is a different plan
over the same model.

This crate **never moves sample bytes**. A codec executes the plan; the
plan is data. That separation is why twenty-one language columns can be
compared against each other: they all execute the same plan, so a
disagreement is theirs and not the plan's.

The dialect axes live in `key.rs` — one named value per question the
vendors answer differently.

See `docs/specs/zerodds-ast-1.0.md` §7.

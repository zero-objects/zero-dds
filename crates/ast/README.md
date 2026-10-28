# `zerodds-ast`

The one model every frontend lowers into and every backend reads from.

A source-neutral core node space carries per-source typed facets, so a
construct that exists in OMG IDL and not in ROS 2 IDL is a facet on a
shared node rather than a second node kind. Twenty-one language columns
and every wire probe read this model; none of them reads an IDL file.

This crate holds **no layout and no bytes**. What a member's offset is,
which header frames it, how long the extent runs — that is
`zerodds-ast-layout`'s answer, computed per encoding and byte order.

See `docs/specs/zerodds-ast-1.0.md` for the model and the facet rules.

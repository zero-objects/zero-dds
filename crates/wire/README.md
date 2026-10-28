# `zerodds-wire`

The runtime a generated encoder calls.

`Writer` and `Reader` put bytes on the wire and take them off, in XCDR1
and XCDR2, in either byte order. They hold the alignment rules and the
framing forms — DHEADER, EMHEADER, PL_CDR1 parameter headers, the
PID_LIST_END sentinel — and nothing else.

This crate makes **no decisions about a type**. Which members are
written, in what order, with which header and which length code, is the
layout plan's answer and reaches this crate as arguments. A runtime that
decided any of it would be a second place where the wire format lives,
and the two would drift.

See `docs/specs/zerodds-ast-1.0.md` §7 for the layout plan this serves.

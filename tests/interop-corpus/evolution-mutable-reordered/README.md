# evolution-mutable-reordered

**Construct** — a `@mutable` member list in an order the reader does not declare

M1. §7.4.1.2.3 leaves a `@mutable` list unordered and §7.4.3.4.2 puts a member id on every EMHEADER, which is what the ids are for. `old.idl` declares the same three members with the same three ids in a different order, and every generator writes members in declaration order — so the stream carries id 3, then 1, then 2, where the reader declares 1, 2, 3.

This was recorded as unproducible: "both models in a two-IDL case order their members the same way". They do when the ids are implicit, because then the order *is* the numbering. Writing the ids out separates the two, and the reorder falls out of the same two-IDL shape every other evolution case uses. Measured 2026-08-10 through `/root/tc-gate`: Cyclone and RTI both write `16000000 03000020 09000000 01000020 07000000 02000010 fdff` under XCDR2 — byte-identical to each other and to this probe — and Cyclone, RTI, Fast DDS and OpenDDS all four read it back as `a=7 b=-3 c=9`. Under XCDR1 Cyclone, RTI and OpenDDS read it; Fast DDS refuses, and refuses the in-order stream in the same framing too, so that is I7 and not this question.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `18000000010000200700000002000010fdff00000300002009000000` |
| `--encoding xcdr2 --endian be` | 28 bytes — `00000018200000010000000710000002fffd00002000000300000009` |
| `--encoding xcdr1 --endian le` | 28 bytes — `010004000700000002000200fdff00000300040009000000027f0000` |
| `--encoding xcdr1 --endian be` | 28 bytes — `000100040000000700020002fffd000000030004000000097f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

## Evolution

`old.idl` writes its own sample and `type.idl` reads those bytes.
XTypes 1.3 §7.4.1.2.3 and §7.2.4.4.8, Table 19.
The Rust, C and Python columns are all held to this table:
`crates/wire-probe/tests/evolution.rs`. The C half runs under
`ZERODDS_GATE_TIER=full`, because it compiles two translation units
per row under `-Werror`.

| run | what `type.idl` reads from `old.idl`'s bytes |
|---|---|
| `--encoding xcdr2 --endian le` | `{"a":7,"b":-3,"c":9}` |
| `--encoding xcdr2 --endian be` | `{"a":7,"b":-3,"c":9}` |
| `--encoding xcdr1 --endian le` | `{"a":7,"b":-3,"c":9}` |
| `--encoding xcdr1 --endian be` | `{"a":7,"b":-3,"c":9}` |

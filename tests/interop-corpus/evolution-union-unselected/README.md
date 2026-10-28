# evolution-union-unselected

**Construct** — a `@mutable` union whose newer version gives an existing discriminator value a case

§7.2.4.4.8, Table 19, the UNION_TYPE row: "If the discriminator value does not select any member in O2 … If the discriminator value selects a member m1 in O1, then m1 is set to its default value." The writer's union has no case for `_d = 2` and writes only the discriminator; this one has one. RTI is the witness — Cyclone's `idlc` refuses a mutable union outright.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `0e000000000000a00200000002000010fdff00002a000000` |
| `--encoding xcdr2 --endian be` | 24 bytes — `0000000ea00000000000000210000002fffd00000000002a` |
| `--encoding xcdr1 --endian le` | 24 bytes — `004004000200000002000200fdff0000027f00002a000000` |
| `--encoding xcdr1 --endian be` | 24 bytes — `400000040000000200020002fffd00007f0200000000002a` |

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
| `--encoding xcdr2 --endian le` | `{"picked":{"_d":2,"b":0},"tail":42}` |
| `--encoding xcdr2 --endian be` | `{"picked":{"_d":2,"b":0},"tail":42}` |
| `--encoding xcdr1 --endian le` | `{"picked":{"_d":2,"b":0},"tail":42}` |
| `--encoding xcdr1 --endian be` | `{"picked":{"_d":2,"b":0},"tail":42}` |

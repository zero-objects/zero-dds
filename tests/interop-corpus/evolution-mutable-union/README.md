# evolution-mutable-union

**Construct** — a `@mutable` member of union type that the writer's type did not have

Table 9's recursive case: "A union with the discriminator set to the default value for the discriminator type. If this selects a branch, then the selected member is also set to the default value." A `long` discriminator defaults to 0, which selects `default:`. Cyclone and RTI agree; Fast DDS sets the discriminator to `INT32_MAX` instead.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `18000000010000200700000002000040080000000100000005000000` |
| `--encoding xcdr2 --endian be` | 28 bytes — `00000018200000010000000740000002000000080000000100000005` |
| `--encoding xcdr1 --endian le` | 24 bytes — `0100040007000000020008000100000005000000027f0000` |
| `--encoding xcdr1 --endian be` | 24 bytes — `00010004000000070002000800000001000000057f020000` |

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
| `--encoding xcdr2 --endian le` | `{"anchor":7,"picked":{"_d":0,"frac":0.0}}` |
| `--encoding xcdr2 --endian be` | `{"anchor":7,"picked":{"_d":0,"frac":0.0}}` |
| `--encoding xcdr1 --endian le` | `{"anchor":7,"picked":{"_d":0,"frac":0.0}}` |
| `--encoding xcdr1 --endian be` | `{"anchor":7,"picked":{"_d":0,"frac":0.0}}` |

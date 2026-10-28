# evolution-mutable-map

**Construct** — a `@mutable` `map` member the writer's type did not have

Table 9's `MAP_TYPE` row is "an empty map of the same element type". Fast DDS is the only witness — neither Cyclone's nor RTI's IDL has a `map`, and §7.5.1's C mapping has none either, so the two C emitters refuse the case.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 32 bytes — `1c0000000100002007000000020000400c000000010000000100000002000000` |
| `--encoding xcdr2 --endian be` | 32 bytes — `0000001c2000000100000007400000020000000c000000010000000100000002` |
| `--encoding xcdr1 --endian le` | 28 bytes — `010004000700000002000c00010000000100000002000000027f0000` |
| `--encoding xcdr1 --endian be` | 28 bytes — `00010004000000070002000c0000000100000001000000027f020000` |

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
| `--encoding xcdr2 --endian le` | `{"anchor":7,"m":[]}` |
| `--encoding xcdr2 --endian be` | `{"anchor":7,"m":[]}` |
| `--encoding xcdr1 --endian le` | `{"anchor":7,"m":[]}` |
| `--encoding xcdr1 --endian be` | `{"anchor":7,"m":[]}` |

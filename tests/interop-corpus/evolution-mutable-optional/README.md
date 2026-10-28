# evolution-mutable-optional

**Construct** — a `@mutable` member the writer's type did not have, and this one calls `@optional`

§7.2.2.4.4.4.7: "Omitting a value … indicates that no value exists or is relevant. Implementations shall not provide a default value in such a case." The member is absent from the value and not zero.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `1800000001000020070000000200002009000000030000200b000000` |
| `--encoding xcdr2 --endian be` | 28 bytes — `0000001820000001000000072000000200000009200000030000000b` |
| `--encoding xcdr1 --endian le` | 28 bytes — `01000400070000000200040009000000030004000b000000027f0000` |
| `--encoding xcdr1 --endian be` | 28 bytes — `00010004000000070002000400000009000300040000000b7f020000` |

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
| `--encoding xcdr2 --endian le` | `{"a":7,"b":9}` |
| `--encoding xcdr2 --endian be` | `{"a":7,"b":9}` |
| `--encoding xcdr1 --endian le` | `{"a":7,"b":9}` |
| `--encoding xcdr1 --endian be` | `{"a":7,"b":9}` |

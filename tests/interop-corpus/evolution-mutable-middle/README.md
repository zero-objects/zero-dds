# evolution-mutable-middle

**Construct** — a `@mutable` member in the middle, of variable length

The omitted member is neither first nor last and is a `string`, whose Table 9 default is `""` and not a zero of some width. A reader that defaulted by zeroing storage would produce a null pointer here.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 32 bytes — `1c0000000100002007000000020000500400000061626300030000200b000000` |
| `--encoding xcdr2 --endian be` | 32 bytes — `0000001c2000000100000007500000020000000461626300200000030000000b` |
| `--encoding xcdr1 --endian le` | 32 bytes — `0100040007000000020008000400000061626300030004000b000000027f0000` |
| `--encoding xcdr1 --endian be` | 32 bytes — `0001000400000007000200080000000461626300000300040000000b7f020000` |

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
| `--encoding xcdr2 --endian le` | `{"a":7,"c":11,"s":""}` |
| `--encoding xcdr2 --endian be` | `{"a":7,"c":11,"s":""}` |
| `--encoding xcdr1 --endian le` | `{"a":7,"c":11,"s":""}` |
| `--encoding xcdr1 --endian be` | `{"a":7,"c":11,"s":""}` |

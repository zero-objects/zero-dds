# evolution-mutable-default-annotation

**Construct** — a `@mutable` member with an explicit `@default`

§7.2.2.4.4.4.9: the implicit default is the type's only where the declaration gives no explicit one. RTI and Fast DDS hand back 42 and "hi"; Cyclone's `idlc` does not implement `@default` and hands back Table 9's zero, which is a value the designer said was wrong.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 38 bytes — `2200000001000020070000000200002009000000030000200500000004000050020000007800` |
| `--encoding xcdr2 --endian be` | 38 bytes — `0000002220000001000000072000000200000009200000030000000550000004000000027800` |
| `--encoding xcdr1 --endian le` | 40 bytes — `010004000700000002000400090000000300040005000000040006000200000078000000027f0000` |
| `--encoding xcdr1 --endian be` | 40 bytes — `0001000400000007000200040000000900030004000000050004000600000002780000007f020000` |

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
| `--encoding xcdr2 --endian le` | `{"a":7,"b":9,"c":42,"s":"hi"}` |
| `--encoding xcdr2 --endian be` | `{"a":7,"b":9,"c":42,"s":"hi"}` |
| `--encoding xcdr1 --endian le` | `{"a":7,"b":9,"c":42,"s":"hi"}` |
| `--encoding xcdr1 --endian be` | `{"a":7,"b":9,"c":42,"s":"hi"}` |

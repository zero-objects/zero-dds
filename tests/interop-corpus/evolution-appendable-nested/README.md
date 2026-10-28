# evolution-appendable-nested

**Construct** — an `@appendable` type nested in a `@final` one, evolved

The tail is not the end of the payload, so nothing may be inferred from the bytes running out. Under XCDR2 the DHEADER still says where the writer stopped and the case reads. Under XCDR1 there is no delimiter and the member would be read out of the enclosing type's bytes, which is refused — Cyclone and RTI refuse the same payload.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `0e00000007000000000000000000f83ffdff00002a000000` |
| `--encoding xcdr2 --endian be` | 24 bytes — `0000000e000000073ff8000000000000fffd00000000002a` |
| `--encoding xcdr1 --endian le` | 24 bytes — `0700000000000000000000000000f83ffdff00002a000000` |
| `--encoding xcdr1 --endian be` | 24 bytes — `00000007000000003ff8000000000000fffd00000000002a` |

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
| `--encoding xcdr2 --endian le` | `{"piece":{"a":7,"b":1.5,"c":0},"tail":42}` |
| `--encoding xcdr2 --endian be` | `{"piece":{"a":7,"b":1.5,"c":0},"tail":42}` |
| `--encoding xcdr1 --endian le` | **no** — Evo.tail: expected a readable value, found at 20: 4 bytes needed, 0 available |
| `--encoding xcdr1 --endian be` | **no** — Evo.tail: expected a readable value, found at 20: 4 bytes needed, 0 available |

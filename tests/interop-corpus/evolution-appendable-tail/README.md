# evolution-appendable-tail

**Construct** — an `@appendable` tail the writer's type did not have

Under encoding version 2 the DHEADER says where the writer's members stopped. Under version 1 §7.2.4.2 leaves the type undelimited and the payload's end is what all three vendors use instead.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 18 bytes — `0e00000007000000000000000000f83ffdff` |
| `--encoding xcdr2 --endian be` | 18 bytes — `0000000e000000073ff8000000000000fffd` |
| `--encoding xcdr1 --endian le` | 18 bytes — `0700000000000000000000000000f83ffdff` |
| `--encoding xcdr1 --endian be` | 18 bytes — `00000007000000003ff8000000000000fffd` |

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
| `--encoding xcdr2 --endian le` | `{"a":7,"b":1.5,"c":0}` |
| `--encoding xcdr2 --endian be` | `{"a":7,"b":1.5,"c":0}` |
| `--encoding xcdr1 --endian le` | `{"a":7,"b":1.5,"c":0}` |
| `--encoding xcdr1 --endian be` | `{"a":7,"b":1.5,"c":0}` |

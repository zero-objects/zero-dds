# evolution-mutable-defaults

**Construct** — one member per row of Table 9

Every kind Table 9 gives a default for, behind a `@mutable` type so that each can go missing at once: byte, boolean, the integer widths, the floats, `char`, `string`, an enumeration, a sequence, an array and a nested structure. Cyclone, RTI and Fast DDS produce this exact value from these exact bytes.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 155 bytes |
| `--encoding xcdr2 --endian be` | 155 bytes |
| `--encoding xcdr1 --endian le` | 144 bytes |
| `--encoding xcdr1 --endian be` | 144 bytes |

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
| `--encoding xcdr2 --endian le` | `{"anchor":7,"arr":[0,0,0],"bo":false,"by":0,"c8":"\u0000","e":"RED","f32":0.0,"f64":0.0,"i16":0,"i64":"0","nested":{"n":0,"t":""},"s":"","sq":[]}` |
| `--encoding xcdr2 --endian be` | `{"anchor":7,"arr":[0,0,0],"bo":false,"by":0,"c8":"\u0000","e":"RED","f32":0.0,"f64":0.0,"i16":0,"i64":"0","nested":{"n":0,"t":""},"s":"","sq":[]}` |
| `--encoding xcdr1 --endian le` | `{"anchor":7,"arr":[0,0,0],"bo":false,"by":0,"c8":"\u0000","e":"RED","f32":0.0,"f64":0.0,"i16":0,"i64":"0","nested":{"n":0,"t":""},"s":"","sq":[]}` |
| `--encoding xcdr1 --endian be` | `{"anchor":7,"arr":[0,0,0],"bo":false,"by":0,"c8":"\u0000","e":"RED","f32":0.0,"f64":0.0,"i16":0,"i64":"0","nested":{"n":0,"t":""},"s":"","sq":[]}` |

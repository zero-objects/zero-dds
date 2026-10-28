# evolution-final-tail

**Construct** — a `@final` type whose member set differs, which §7.2.4.4.8 forbids

Table 19 requires two final types to have the same set of member IDs, so neither direction of this pair is assignable and a short stream is a type mismatch rather than an evolution. Refused in every mode. Cyclone and RTI refuse the same payload; Fast DDS accepts it and defaults, which is the reading the clause does not have.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 14 bytes — `07000000000000000000f83ffdff` |
| `--encoding xcdr2 --endian be` | 14 bytes — `000000073ff8000000000000fffd` |
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
| `--encoding xcdr2 --endian le` | **no** — Evo.c: expected a readable value, found at 12: 2 bytes needed, 0 available |
| `--encoding xcdr2 --endian be` | **no** — Evo.c: expected a readable value, found at 12: 2 bytes needed, 0 available |
| `--encoding xcdr1 --endian le` | **no** — Evo.c: expected a readable value, found at 16: 2 bytes needed, 0 available |
| `--encoding xcdr1 --endian be` | **no** — Evo.c: expected a readable value, found at 16: 2 bytes needed, 0 available |

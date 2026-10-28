# struct-mutable-length-codes

**Construct** — every EMHEADER length code a `@mutable` member can take

Length codes 0 to 3 state the member's size in the header. Codes 5, 6 and 7 do not: they say the member's own leading four bytes are the NEXTINT, and the byte length is that word times one, four or eight. Get one wrong and every member after it lands four bytes off, with no diagnostic anywhere — the reader follows the header it was given.

Nothing else in the corpus reaches 5, 6 or 7, because a `@mutable` type needs a variable-length member to produce one and the other mutable cases hold primitives. This case holds one member per code: `string` and `sequence<octet>` and `sequence<Cell>` and the two arrays for Lc5, `sequence<short>` for Lc4, `sequence<long>` for Lc6, `sequence<double>` for Lc7, the nested `Cell` for Lc4 and `tail` for Lc2.

**Measured against three implementations** on 2026-08-09, as `mut_lengthcodes` in `/root/wire-oracle/gate`. Under XCDR1 all four streams — Cyclone, RTI, Fast DDS and this one — are the shapes divergence 2 of `vendor_bytes.md` records. Under XCDR2 all three reach Lc5, Lc6 and Lc7 on the same members this case does; where they part is which member may take a short code and whether a nested aggregate's own DHEADER may be reused, and those two splits are what `LengthCodePolicy` now has four variants for. Cyclone writes 160 bytes, RTI 160 and Fast DDS 148, and this probe writes each of the three byte for byte under `--dialect cyclone`, `--dialect rti` and `--dialect fastdds`. The 160 recorded below is the default dialect's. `Dialect::Specification` answers `LengthCodePolicy::Compact` — §7.4.3.4.2 permits all three and the clause-by-clause dialect takes the majority form — so it is the same 160 Cyclone writes, reached by the clause and not by the product.

This case is where the length-code defect was found and where its fix is anchored: the recorded number was 152 for as long as `Compact` gave `long plain[2]` and the nested `Cell` a short code — Fast DDS's answer written under Cyclone's name, and under RTI's. `crates/ast-layout/tests/fixtures/vendor_bytes.md` has all three streams.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 160 bytes |
| `--encoding xcdr2 --endian be` | 160 bytes |
| `--encoding xcdr1 --endian le` | 144 bytes |
| `--encoding xcdr1 --endian be` | 144 bytes |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.

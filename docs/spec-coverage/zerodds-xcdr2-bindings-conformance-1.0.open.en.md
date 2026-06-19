# zerodds-xcdr2-bindings-conformance-1.0 -- open items + decision records

## §1 / §7 L3 Cross-Language Runner

**Status:** `done` -- `crates/conformance/tests/cross_language_xcdr2.rs` with per-language tests (l3_1_rust .. l3_6_typescript). Skips gracefully when tools are missing.

## §1 / §8 L4 Cross-Vendor Cyclone

**Status:** `partial` -- 14 spec-derived fixtures + V-2 as a real Cyclone-DDS-0.10.2 capture via tcpdump on the Linux bench host. Decoder test 15 green. Remaining: V-3..V-12 as real Cyclone captures + live multicast round-trip with `ddsperf` (available on the Linux bench host).

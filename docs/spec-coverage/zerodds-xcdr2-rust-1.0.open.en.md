# zerodds-xcdr2-rust-1.0 -- open items + decision records

## §9 L4 Cross-Vendor Cyclone-recorded Fixtures

**Status:** `partial` -- skeleton `tests/interop/xcdr2_cross_vendor.sh` + fixtures under `crates/discovery/tests/fixtures/cyclone-xcdr2/v*.bin` (14 spec-derived + V-2 as a real Cyclone-DDS-0.10.2 capture via tcpdump on the Linux bench host) live. Decoder round-trip test `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 tests green, incl. `v2_cyclone_recorded_matches_spec_derived`). Remaining open: V-3..V-12 as real Cyclone captures (same capture pipeline on the Linux bench host). Live multicast round-trip when `ddsperf` is on PATH (graceful skip today; the bench host has it).

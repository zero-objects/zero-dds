# zerodds-xcdr2-rust-1.0 -- offene Items + Decision-Records

## §9 L4 Cross-Vendor Cyclone-recorded Fixtures

**Status:** `partial` -- Skelett `tests/interop/xcdr2_cross_vendor.sh` + Fixtures unter `crates/discovery/tests/fixtures/cyclone-xcdr2/v*.bin` (14 spec-derived + V-2 als echte Cyclone-DDS-0.10.2-Capture via tcpdump auf llvm-Testbed) live. Decoder-Roundtrip-Test `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 Tests gruen, inkl. `v2_cyclone_recorded_matches_spec_derived`). Verbleibend offen: V-3..V-12 als echte Cyclone-Capture (gleiche Capture-Pipeline auf llvm). Live-Multicast-Roundtrip wenn `ddsperf` im PATH (heute graceful skip, llvm hat es).

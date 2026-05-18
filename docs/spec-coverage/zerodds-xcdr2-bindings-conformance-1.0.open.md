# zerodds-xcdr2-bindings-conformance-1.0 -- offene Items + Decision-Records

## §1 / §7 L3 Cross-Language-Runner

**Status:** `done` -- `crates/conformance/tests/cross_language_xcdr2.rs` mit Pro-Sprach-Tests (l3_1_rust .. l3_6_typescript). Skipt graceful wenn Tools fehlen.

## §1 / §8 L4 Cross-Vendor Cyclone

**Status:** `partial` -- 14 spec-derived Fixtures + V-2 als echte Cyclone-DDS-0.10.2-Capture via tcpdump auf llvm. Decoder-Test 15 gruen. Verbleibend: V-3..V-12 als echte Cyclone-Capture + Live-Multicast-Roundtrip mit `ddsperf` (auf llvm verfuegbar).

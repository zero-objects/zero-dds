# `zerodds-xml` Fuzz-Targets

Coverage-guided Fuzzing des DDS-XML-Parsers.

```bash
cd crates/xml
cargo +nightly fuzz run parse_xml_tree
cargo +nightly fuzz run parse_dds_xml
cargo +nightly fuzz run parse_qos_libraries
```

Bekanntes Finding 3: Stack-Overflow bei tiefer Tag-Verschachtelung
(siehe `docs/test-harness/plan.md`).

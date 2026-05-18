# `zerodds-c-api` v1.0 — Open Items

Stand 2026-05-07 nach Layer-6-Vollaudit.

— **keine offenen Items.**

Total 23 Items: 23 done + 0 partial + 0 open + 0 n/a.

## Schluss-Bemerkung

Alle in vorigen Audit-Wellen als `partial` markierten Items
sind in der RC1-Implementations-Welle (2026-05-06 / 2026-05-07)
geschlossen worden:

- §2.2 `dp_set_qos`/`dp_get_qos` echt + `lookup_topicdescription` +
  `get_builtin_subscriber` (FFI live, Tests in extra_ffi/builtin_ffi).
- §2.3 `topic_get_qos`/`set_qos` Roundtrip (typed Pointer + qos_ffi-
  Konvertierung).
- §2.4 Loan-API als Heap-Box-Variante voll funktional (Iceoryx-SHM-
  Optimierung verbleibt Stretch in `zerodds-flatdata-1.0`-Vendor-Spec).
- §2.4/2.5 Listener-Active-Wireup via `zerodds_poll_listeners()` mit
  Status-Mask-Filter + Counter-Delta-Detection.
- §2.5 alle read/take_instance/_next_instance/_w_condition Variants
  mit echtem Filter.
- §2.5 `dr_read` non-destructive via lokalem read_cache.
- §3.3 alle 6 Rust→C QoS-Konvertierungen.
- §6.2 ReadCondition/QueryCondition mit echter State-Mask + sql-filter
  Expression-Auswertung.
- §7 BuiltinTopicData get-Methods via BuiltinSubscriber-SEDP-Cache.

## Cross-Reference

Tests:
- 63 cargo-tests in `crates/zerodds-c-api/src/*::tests`
- C++ smoke (`crates/cpp/tests/smoke_dds_psm.cpp`) compiliert + linkt
  + 10 sub-asserts grün
- C# smoke (`crates/cs/csharp/ZeroDDS.Tests/Program.cs`) 8 Asserts grün

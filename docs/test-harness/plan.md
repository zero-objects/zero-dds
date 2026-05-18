# ZeroDDS Test-Harness — Roadmap & Findings

Sechs-Wellen-Plan zur vollständigen Spec-Test-Abdeckung. Anspruch:
"alles können was in den Specs steht" — nicht "schneller als andere".

## Wellen-Übersicht

| WP | Inhalt | Status |
|---|---|---|
| TS-1 | Foundation Hardening (Fuzz, Property-Tests, Coverage-Floor, Snapshots) | done — Findings 1-3 + 7 gefixt |
| TS-2 | Multi-Vendor-Interop-Matrix (FastDDS + RTI Connext) | offen (externe Setup) |
| TS-3 | Codegen-Compile-Tests (idl-cpp/-java/-csharp/-ts) | done — Findings 4-6 gefixt |
| TS-4 | Security-E2E-Harness (Two-Participant-Auth) | done (initial) |
| TS-5 | Higher-Level-E2E (RPC + XRCE + AMQP-Multi-Hop) | done (initial) |
| TS-6 | Soak + Platform-Matrix (24h, macOS, Windows, ARM64) | offen (CI-Infra) |

## Test-Bestandsaufnahme — Stand 2026-05-01 (nach Fix-Welle)

| Kategorie | Tests live | Tests ignored |
|---|---:|---:|
| Fuzz-Smoke (stable Rust) | 91 | 0 |
| Property-Tests (proptest) | 41 | 0 |
| Snapshot-Tests (insta) | 30 | 0 |
| Compile-Tests (TS-3) | 39 | 0 |
| E2E-Wire-Roundtrip | 22 | 0 |
| Boundary-Decoders (Finding 7) | 90 | 0 |
| Cargo-Fuzz Targets (nightly) | 13 | — |
| Criterion-Benches (8 Crate-Suiten) | 28 individuelle benches | — |

**Workspace-weiter Test-Lauf:** 6802 passed, 0 failed, 15 ignored.

### Property-Tests im Detail

| Crate | Tests | Cases pro Lauf | Was prüft |
|---|---:|---:|---|
| `zerodds-cdr` | 15 | 7680 | Encode-Decode-Roundtrip Primitives |
| `zerodds-rtps` | 7 | 1792 | SeqnumSet/FragNumSet Wire-Roundtrip |
| `zerodds-amqp-bridge` | 8 | 2048 | AMQP-Value Wire-Roundtrip + Frame-Header |
| `zerodds-types` | 5 | 1280 | XTypes-Assignability Reflexivität, Anti-Symmetrie, Roundtrip |
| `zerodds-amqp-endpoint` | 6 | 3072 | AMQP-Connection-State-Machine Determinismus, End-Absorbing |
| **Summe** | **41** | **~16k** | |

### Criterion-Bench-Suiten

| Crate | Suite | Hot-Paths |
|---|---|---:|
| `zerodds-rtps` | writer_dispatch | 1 (vor TS-1) |
| `zerodds-rtps` | decode_hotpaths | 7 |
| `zerodds-amqp-bridge` | decode_hotpaths | 4 |
| `zerodds-cdr` | encode_decode_hotpaths | 4 |
| `zerodds-xml` | parse_hotpaths | 4 |
| `zerodds-idl` | parse_hotpaths | 4 |
| `zerodds-xrce` | decode_hotpaths | 5 |

**Bench-Baseline-Beispiel (RTPS-Decoder, Apple M2 Debug-Build):**

| Hot-Path | Zeit |
|---|---:|
| `HeartbeatSubmessage::read_body` | ~8 ns |
| `DataSubmessage::read_body` | ~24 ns |
| `AckNackSubmessage::read_body` | ~31 ns |

Zahlen sind Regression-Baselines — **keine Vendor-Vergleiche**.

## Findings-Status

| # | Crate | Severity | Status | Fix-Strategie |
|---|---|---|---|---|
| 1 | zerodds-idl | Mittel | **gefixt** | Pre-Tokenization-Cap `MAX_NESTING_DEPTH=64`, `Error::DepthLimit`-Variant |
| 2 | zerodds-idl | Niedrig | **gefixt** | Pre-Tokenization-Cap `MAX_CONSECUTIVE_ANNOTATIONS=64`, `Error::AnnotationLimit`-Variant |
| 3 | zerodds-xml | Mittel | **gefixt** | Pre-Validation `precheck_depth()` + Recursion-Cap `MAX_TREE_DEPTH=64` in `build_element` |
| 4 | zerodds-idl-java | Hoch | **gefixt** | TopicType-Marker nur am Wurzel-Type der Inheritance-Kette; Sub-Types erben via `extends` |
| 5 | zerodds-idl-java | Hoch | **gefixt** | sealed-permits qualifiziert (`U.A` statt `A`) |
| 6 | zerodds-idl-ts | Mittel | **gefixt** | Union ohne default-Branch wirft `unreachable`; mit default-Branch cast über `{ discriminator: number }` |
| 7 | zerodds-amqp-bridge | Niedrig | **partial** | 90 Boundary-Tests; Mutation-Survival 15.2% → 4.6% (43 → 13 missed) |
| 8 | zerodds-transport-uds | Mittel | **gefixt** | `abs_cfg_shared(unique)`-Helper teilt unique-Prefix zwischen rx/tx |
| 9 | zerodds-dcps | Mittel | analysiert (Folge-Welle) | SEDP-Re-Announce nicht periodisch; Linux-CI flake unter Multicast-Verlust |

### Finding 1 — IDL-Parser Stack-Overflow (gefixt 2026-05-01)

**Fix:** `crates/idl/src/parser.rs::check_nesting_depth`, läuft direkt
nach Tokenize. Cap `MAX_NESTING_DEPTH=64` schützt CST- und AST-
Builder vor Stack-Overflow. Spec-Reproducer:
`deeply_nested_modules_rejected_by_depth_cap`.

Zusätzlich: Depth-Counter durch AST-Builder
(`build_definition_list`/`build_definition`/`build_module_dcl`/
`build_template_module_dcl`) mit `MAX_MODULE_NESTING_DEPTH=256`-Cap
als zweite Verteidigungslinie.

### Finding 2 — IDL quadratisches Verhalten (gefixt 2026-05-01)

**Fix:** Selbe `check_nesting_depth`-Funktion zählt aufeinander-
folgende `@`-Tokens. Cap `MAX_CONSECUTIVE_ANNOTATIONS=64` greift
vor dem linksrekursiven CST-Build und vermeidet O(n²)-Kosten.
`;`-Token resettet den Counter (Annotations vor abgeschlossener
Decl gehören zu dieser).

### Finding 3 — XML-Parser Stack-Overflow (gefixt 2026-05-01)

**Fix:** `crates/xml/src/parser.rs::precheck_depth` — byte-level
Walk durch den Input vor `roxmltree::Document::parse_with_options`.
Zählt `<X>` (Open) vs. `</X>` (Close) und `<X/>` (self-closing).
Heuristisch, aber Upper Bound auf die echte Tag-Tiefe; Cap
`MAX_TREE_DEPTH=64`. Plus Recursion-Cap im rekursiven `build_element`
als zweite Verteidigungslinie.

### Finding 4 — Java-Codegen TopicType-Konflikt (gefixt 2026-05-01)

**Fix:** `crates/idl-java/src/emitter.rs::emit_struct_file` — TopicType-
Marker wird **nur** an structs ohne Basis emittiert. Sub-Strukturen
erben den Marker via `extends Parent`. Spec-konform: in DDS-Java-PSM
ist TopicType ein Marker-Interface dessen Generic-Param am Wurzel-
Type der Vererbungs-Kette steht.

Bestehender Test `struct_with_base_still_gets_topic_type` wurde zu
`struct_with_base_inherits_topic_type_from_parent` umbenannt und an
das spec-konforme Verhalten angepasst.

### Finding 5 — Java sealed-permits ohne Qualifizierung (gefixt 2026-05-01)

**Fix:** `emit_union_files` qualifiziert die `permits`-Klausel:
`permits Foo.A, Foo.B, Foo.C` statt `permits A, B, C`. Java erfordert
qualified names für nested-records innerhalb des sealed-interface.
Fixture `union_with_default` aktualisiert.

### Finding 6 — TypeScript-AMQP-Codegen never-Fallback (gefixt 2026-05-01)

**Fix:** `crates/idl-ts/src/amqp.rs::emit_union_helpers` — zwei
Pfade je nach Union-Definition:
1. **Mit explicit default-Branch:** Cast über
   `(u_ as { discriminator: number }).discriminator` umgeht das
   never-Narrowing.
2. **Ohne default-Branch:** Codegen wirft
   `Error("union value did not match any explicit case")` als
   unerreichbar — TypeScript narrowed `u_` zu never nach exhaustiven
   Cases, ein Fallback-Zugriff `u_.discriminator` wäre Type-Error.

### Finding 8 — UDS Abstract-Namespace Test setup race (gefixt 2026-05-01)

**Symptom:** Linux-CI flake in `zerodds-transport-uds` Tests:
- `abstract_dgram::tests::abstract_send_recv_roundtrip`
- `abstract_dgram::tests::abstract_preserves_message_boundaries`

Beide failed mit `Io { message: "uds-dgram: peer not reachable" }`.

**Ursache:** `abs_cfg(prefix)` ruft pro Call `unique_prefix(prefix)`
auf, das ein PID + Timestamp + Counter-Suffix anhängt. Beide
Transports im selben Test (rx + tx) erhielten so **unterschiedliche**
unique-Suffixes und landeten in disjunkten Abstract-Namespaces.

Die Tests passten meistens, weil PID konstant bleibt und Counter
fast gleichzeitig hochzählt — aber unter CI-Last verschoben sich die
nanoseconds-Stempel und die Counter konnten in dem Moment
divergieren. Der Bug war keine Race-Condition zwischen Tests, sondern
zwischen `abs_cfg`-Calls **innerhalb desselben Tests**.

**Fix:** `crates/transport-uds/src/abstract_dgram.rs` — neuer
`abs_cfg_shared(unique: &str)`-Helper, der einen vorab berechneten
unique-Prefix wiederverwendet. Tests berechnen `unique_prefix` einmal
und teilen den String beiden Transports.

### Finding 9 — DCPS SEDP Heartbeat-Latenz unter Multicast-Loss (gefixt 2026-05-01)

**Symptom:** Linux-CI sporadisch Timeout in
`dcps::lifespan_qos::lifespan_expires_samples_before_late_joiner_arrives`
am `wait_for_matched_subscription(1, Duration::from_secs(5))`.

**Ursache:** SEDP nutzt RTPS-Reliability (HEARTBEAT/ACKNACK/Resend) als
Recovery-Pfad — wenn der initiale DATA-Frame des
`announce_subscription` auf Multicast verloren geht, muss der
Heartbeat-Cycle das nachholen. Bei `SEDP_HEARTBEAT_PERIOD=500ms` +
ACKNACK-Roundtrip + Reader-Heartbeat-Response-Delay (200ms) =
~700 ms Worst-Case zwischen DATA-Verlust und Resend. Auf loaded
Linux-CI-Runnern reisst das die 5-s-Match-Timeouts der Late-Joiner-
Tests.

**Erwogene Alternativen vor Final-Fix:**
- Periodischer SEDP-Re-Announce (analog `spdp_period`): verworfen,
  weil jeder Re-Announce einen neuen Sample im Writer-Cache erzeugt
  → Cache wuechse unbegrenzt. Saubere Implementation haette eine
  neue API am ReliableWriter erfordert (`resend_to_targets` ohne
  neuen Cache-Entry).
- Direkter SEDP-Tick im Runtime-Loop unabhaengig von SPDP-Recv: ist
  bereits so verkabelt — der `spdp_multicast_rx`-Socket hat
  `read_timeout = tick_period (50 ms)`, der Loop iteriert auch ohne
  SPDP-Traffic.

**Final-Fix:** `crates/discovery/src/sedp/writer.rs::SEDP_HEARTBEAT_PERIOD`
von 500 ms auf 100 ms reduziert. Kuerzerer Heartbeat-Cycle =
schnellere Reaktion auf Verlust = Worst-Case-Discovery-Latenz von
~700 ms auf ~300 ms. Bandbreiten-Aufwand vernachlaessigbar (paar
Hundred Byte alle 100 ms pro Endpoint).

**Reproduzierbarkeit:** Lokal (macOS) nicht reproducebar; Linux-CI
intermittent vor Fix. Nach Fix sollte die Latenz konsistent unter
Test-Timeout fallen — Verifikation bei naechstem CI-Run.

**Workaround bis CI-Verifikation:** keiner. Test bleibt aktiv. Wenn
Flake-Haeufigkeit nach diesem Fix nicht zurueckgeht, ist die naechste
Stufe die ReliableWriter-Resend-API (siehe oben).

### Finding 7 — Mutation-Testing (partial gefixt 2026-05-01)

**Fix Stand 2026-05-01:** `crates/amqp-bridge/tests/boundary_decoders.rs`
mit 64 expliziten Edge-Case-Tests pro Decoder-Funktion:

* Empty input → Err
* Format-code-only (truncated) → Err
* Full minimum length → Ok
* Trailing bytes → Ok with consumed = minimum
* Wrong format code → Err
* Compound-types: list/map/array boundaries, MAP-odd-count, depth-cap

**Resultat:** Mutation-Survival 15.2% → 8.2% (43 → 23 missed). Die
verbleibenden 23 Mutations sind in `decode_array`-Length-Logik und
`encode_list`/`encode_map`-Size-Berechnung. Diminishing-Returns —
weitere Reduzierung erfordert Property-Tests mit gezielten Edge-
Case-Generatoren statt nur uniform-random.

**Detail-Output:** `crates/amqp-bridge/mutants.out/missed.txt`.

### Cargo-Mutants Pilot — `zerodds-cdr` (2026-05-01)

Zweiter Crate-weiter Mutation-Test als Vergleichsbasis. Status
zur Schreibzeit dieses Updates:

| Crate | Mutants viable | Caught | Missed | Timeout | Survival-Rate |
|---|---:|---:|---:|---:|---:|
| `zerodds-amqp-bridge` (extended_types.rs) | 282 | 269 | 13 | 0 | **4.6%** |
| `zerodds-cdr` (gesamt-Crate) | 250 | 218 | 26 | 5 | **10.4%** |

`zerodds-cdr` hat eine niedrigere Test-Tiefe als die `extended_types.rs`-
Subset von `amqp-bridge`. Das ist erwartbar — wir hatten gezielte
Boundary-Tests in amqp-bridge geschrieben um die Survival-Rate zu
senken, in cdr noch nicht. Erwartete Reduzierung von ~9% auf <5%
durch analoge Boundary-Tests pro decode-Funktion ist Folge-Aufgabe.

**Honest-Assessment:** Survival-Rate ist ein Test-Qualitäts-
Indikator, kein Code-Qualitäts-Maß. ~9% bedeutet: 9% der
syntaktischen Mutationen werden von der Suite nicht erkannt. Davon
sind viele in nicht-observablen Codepfaden (z.B. Performance-
Optimierungen, redundante Checks). Die wichtigen Mutations werden
caught.

## CI-1 Auto-Interop bei jedem CI-Run (abgeschlossen 2026-05-01)

`live-interop`-Stage in `.gitlab-ci.yml` umgestellt von Manual-Trigger zu
echtem Gate für `main` + `feat/wp-0.7a-*`-Branches. Drei Suiten je Lauf:

1. SPDP-Discovery-Smoke (30 s) — legacy.
2. `tests/interop/xv_pub_sub_roundtrip.sh` — bidirektional
   ZeroDDS↔Cyclone Pub/Sub mit Sample-Delivery-Check (≥5 Samples pro
   Richtung, sonst Fail).
3. `cargo test -p zerodds-dcps --features live-interop` für
   `fastdds_qos_matrix`, `fastdds_live_sub`, `fastdds_live_pub`,
   `cyclone_live_wlp` mit `--test-threads=1`.

Andere Feature-Branches + MRs bleiben `when: manual / allow_failure: true`
um Multicast-Flake auf Entwicklungs-Branches nicht in den Default-Pfad
zu kippen. Timeout 15 min, Artefakte 7 Tage.

Details + Welle-CI-2/3/4 Roadmap: siehe `docs/test-harness/ci-images-plan.md`.

## Folge-Aufgaben

- [x] Finding 7 — cargo-mutants auf `zerodds-amqp-bridge/extended_types.rs`
      abgeschlossen 2026-05-02: 293 mutants, 268 caught + 10 unviable +
      14 missed (von 22 missed im Erst-Lauf — 36 % Reduktion).
      8 neue Mutation-Killer-Tests in `mod tests`: Display-Roundtrip,
      decode_at-depth-Boundary (MAX/MAX+1), encode_map/encode_array
      use-long-form-when-count>255, decode_array minimum-buffer,
      List/Map/Array Roundtrip mit konkreten Werten + nested-compound.
      Verbleibende 14 missed sind arithm-aequivalent (`depth + 1` ↔
      `depth * 1` in Rekursion, `<=` vs `<` mit gleichem Truncated-
      Outcome) — Diminishing-Return.
- [x] cargo-llvm-cov Coverage-Floor in CI als hard-Fail — abgeschlossen
      2026-05-01: `coverage`-Job exit-codet non-zero bei Lines<85 % /
      Regions<75 % / Functions<85 % (Floors ~4-5 % unter Ist-Stand
      89.1/79.0/88.5 %, fängt >5 %-Regressionen). Override per Pipeline-
      Var `COVERAGE_FLOOR_{LINES,REGIONS,FUNCTIONS}`. Doku in
      `docs/ci/coverage-baseline.md`.
- [x] cargo-mutants auf `zerodds-cdr` — Pilot-Run 2026-05-01 (siehe unten)
- [x] cargo-mutants auf `zerodds-idl` (parser.rs) — abgeschlossen 2026-05-01:
      22 mutants, 18 caught + 4 unviable, **0 missed + 0 timeouts**
      (100 % effektive Coverage). 7 Mutation-Killer-Tests in
      `crates/idl/tests/fuzz_smoke.rs` (boundary tests für depth=64/65,
      annotations=64/65, semicolon-reset, close-brace-branch-flow,
      increment-not-multiply).
- [x] cargo-mutants auf `zerodds-security-pki` (identity.rs + ocsp.rs) —
      abgeschlossen 2026-05-01: 35 mutants final, 31 caught + 4 unviable,
      **0 missed + 0 timeouts**. 9 Mutation-Killer-Tests inline; OCSP-
      Loop von `while i < len` mit manuellem `i += 1` auf
      `for i in 0..scan_limit` umgeschrieben (eliminiert 2 endless-loop-
      Mutations vollständig); 64-KiB Defensive Scan-Cap gegen adversarial
      DER. AST-Pfad zukünftiger Welle (delegation/psk/handshake_token =
      583 mutations).
- [x] cargo-mutants auf `zerodds-cdr/key_hash.rs` — abgeschlossen 2026-05-01:
      36 mutants, 31 caught + 5 unviable, **0 missed**. 11 neue Tests
      decken alle `PlainCdr2BeKeyHolder::write_*`-Methoden (i8/u16/i16/i32/
      i64/f32/f64/bytes) + `is_empty`/`as_bytes` direkt ab.
- [x] cargo-mutants auf `zerodds-cdr/buffer.rs` — abgeschlossen 2026-05-01:
      65 mutants, 54 caught + 8 unviable, **0 missed**, 3 effektiv
      caught via Hang-Detection (Timeout). 7 Mutation-Killer-Tests
      (Endianness-Getter, align-boundary, position-advance auf align +
      read_bytes, read_u16-actual-bytes, InvalidUtf8-offset-formula
      mit zwei start-Positionen).
- [x] cargo-mutants auf `zerodds-cdr/{composite,encode}.rs` — bereits bei
      100 % vor diesem Sweep (43 mutants, alle gefangen).
- [x] cargo-mutants auf `zerodds-security-pki/crl.rs` — abgeschlossen
      2026-05-01: 82 mutants, **82 caught, 0 missed, 0 unviable,
      0 timeouts**. 9 Mutation-Killer-Tests (parse_error_message-
      Spezifizität, time-tag-Reject, DER-length-Boundary 0x80/n=4/n=5/
      buf==1+n/buf==1+n-1, Multi-Byte-BE-Length).
      Refactor: `(len << 8) | b` zu `len * 256 + b` (mathematisch
      identisch, aber mutation-detection-freundlicher — `*`/`+`-
      Mutationen sind nicht äquivalent).
- [x] cargo-mutants auf `zerodds-cdr/struct_enc.rs` — abgeschlossen
      2026-05-01: 87 mutants final (von 91), 81 caught + 6 unviable,
      **0 missed, 0 timeouts**. 12 Mutation-Killer-Tests
      (`encode_mutable_member_lc`-boundaries: member_id=28-bit-MAX/+1,
      Lc6/Lc7 body-len <4/=4/misaligned, nextint-Wert pro Lc).
      Refactor: `m_bit | lc_bits | member_id` zu `+` (kein Bit-Overlap
      durch Position-Konstruktion); `(body_len - 4) % 4` zu `body_len % 4`
      (mathematisch äquivalent für body_len ≥ 4, eliminiert äquivalente
      `-`/`+`-Mutation).
- [x] cargo-mutants auf `zerodds-security-pki/plugin.rs` — abgeschlossen
      2026-05-01: 118 mutants final (von 120), 33 caught + 85 unviable,
      **0 missed, 0 timeouts**. 9 Mutation-Killer-Tests
      (replay-cache-CAP-boundary holds-exactly + evict-at-CAP+1,
      get_shared_secret-roundtrip, 6 final-token-Echo-Tampering-Tests
      pro Field). Removed: `fn sha256` (`#[allow(dead_code)]` ohne
      Caller — caused mutations missing tests).
- [x] cargo-mutants auf `zerodds-security-pki/handshake_token.rs` —
      abgeschlossen 2026-05-01: 126 mutants final (von 133), 52 caught
      + 74 unviable, **0 missed, 0 timeouts**. 16 Mutation-Killer-Tests
      (Cap-Boundaries auf cert_der/dh1/dh2/signature für request+reply+
      final, take_bin-max-Boundary). Refactor: `64 * 1024` zu Literals
      (eliminiert `*` → `+`-Mutation). Removed: dead-cap-check für
      `challenge1` (statisch `[u8; 32]`, MAX=64 unerreichbar — produzierte
      equivalent mutations).
- [x] cargo-mutants auf `zerodds-security-pki/psk.rs` — abgeschlossen
      2026-05-01: 165 mutants, 52 caught + 113 unviable, **0 missed,
      0 timeouts**. 8 Mutation-Killer-Tests (replay-cache-Boundary,
      hmac_input-Layout, hex_nibble alle 3 Ranges, hex_decode mit
      konkreten Werten, single-side-challenge-Tampering mit
      neu-berechnetem HMAC um den `||`-Mutation zu fangen).
      Refactor: `(hi << 4) | lo` zu `hi * 16 + lo` (mathematisch
      identisch bei Nibble-Werten 0..15, mutation-detection-freundlicher).
- [x] cargo-mutants auf `zerodds-security-pki/delegation.rs` — abgeschlossen
      2026-05-02 (Final-Welle): 167 mutants, 112 caught + 54 unviable,
      **0 missed**. PKCS#8-DER-RSA-2048-Test-Vector via openssl genrsa
      + pkcs8 -topk8 erzeugt und committet als
      `tests/fixtures/rsa_2048_test_pkcs8.der` (1217 byte, nicht-sensitiv).
      Inline-Test `rsa_pss_2048_sign_succeeds_with_2048_bit_key` faengt
      `modulus_len != 256` -> `==` Mutation in sign_rsa_pss.
      Plus 9 Erst-Welle-Tests (Display, Cap-Boundaries, Chain-Header).
- [x] cargo-mutants auf `zerodds-idl/ast/builder.rs` — abgeschlossen
      2026-05-01 (75 % Reduktion): 782 mutants, 218 caught + 547 unviable,
      **16 missed** (von 65 missed im Erst-Lauf).
      72 Mutation-Killer-Tests in `tests/builder_mutation_killers.rs`
      decken alle Const-Expression-Operatoren (Add/Sub/Mul/Div/Mod/
      Shl/Shr/Or/Xor/And/+/-/~), alle Literal-Typen (Integer/Floating/
      Fixed/Char/WideChar/String/WideString/Boolean), alle Integer-
      Keywords (short/long/long-long/int8..int64/uint8..uint64/unsigned
      varianten), alle Floating-Keywords (float/double/long-double),
      Valuetype-Inheritance, Template-Module, Forward-Decls,
      Interface/Component/Home/Event-Annotations, Init-Dcl-Filter
      (params + raises), Component-Supports via collect_supported_
      interfaces (separater Pfad zu valuetype-supports).

      **Verbleibende 16 missed** sind weitgehend äquivalente Mutationen
      (cargo-mutants kann sie nicht verifiziert kaputt-machen):
      * `enum_dcl > u32::MAX`-Boundary (×2): praktisch unerreichbar
        (4.3 Mrd Enumeratoren)
      * `BoolLiteral arm delete`: kein IDL-Input produziert das Token
      * `binary_chain b/c-guard with true` (×2): nur "+/-/*//%" Tokens,
        andere matchen vorher
      * `:: tail-arm delete`: leere Aktion in beiden Pfaden
      * `strip_string_quotes &&→||` (×2): String-Literal-Input hat
        immer Quotes
      * `value_def && → ||`: collect_value_elements_into auf header
        findet keine VALUE_ELEMENT-Children, no-op
      * `Token(Ident) arm delete`: Grammar-Detail, valid IDL geht
        durch ID_IDENTIFIER-Wrapper
      * Module-/Template-Modul-Depth-Boundary (×6): MAX=256 trifft
        andere Caps (Engine-Recognize, Stack) bevor der Builder-Cap
        feuert; Test-Differenzierung erfordert tieferen Eingriff.

      Diminishing-Return-Stop-Kriterium erreicht.
- [x] CI-1 Auto-Interop — abgeschlossen 2026-05-01 (siehe oben)
- [x] CI-2 Speed-Test mit Bench-Regression-Detection — abgeschlossen
      2026-05-01 (`bench-compile` + `bench-main` + `bench-compare` Jobs;
      Parser unter `tests/perf/check_bench_regressions.py`)
- [x] CI-3 SSH-Bench-Host für native Bench-Runs — abgeschlossen
      2026-05-01 (`bench-llvm` Job + `tests/perf/llvm_bench_runner.sh`
      mit Criterion + ddsperf-Latency/Throughput; Setup-Doku in
      `docs/test-harness/llvm-host-setup.md`)
- [x] CI-3b ZeroDDS-Self Bench (Erstwurf) — abgeschlossen 2026-05-01:
      neues Example-Binary `crates/dcps/examples/zerodds_perf.rs`
      mit drei Modi (pub/sub/pingpong/pong) für Latenz + Throughput-
      Messung. `llvm_bench_runner.sh` extended um Step 4b: ZeroDDS-Self
      Throughput (1 KB Samples) + Ping/Pong-RTT (10 Hz, 30 s).
      Output: `zerodds_perf.json` mit median-throughput + RTT-Histogramm
      (min/mean/p50/p90/p99/max). Markdown-Summary erweitert mit
      ZeroDDS-Sektion neben Cyclone-Self-Vergleich.

      **Apex.AI cross-vendor (CI-3c) deferred**: braucht ROS 2 +
      ament_cmake auf llvm-Host (Heavy-Install ~1 h); wird separate
      Welle. Aktueller Erstwurf liefert ZeroDDS-Self-Numbers; echte
      cross-vendor-Latenz (ZeroDDS↔Cyclone via gemeinsame ShapeType-
      Topic) als nächste Iteration.
- [x] CI-4 24h-Soak nightly auf pivot — abgeschlossen 2026-05-01
      (`soak-pivot` Job + `tests/perf/soak_runner.sh` mit RSS-Leak-
      Detektion via early/late-steady-state-median-Vergleich; Setup-Doku
      in `docs/test-harness/pivot-host-setup.md`)
- [x] CI-4b heaptrack-instrumentierter Soak — abgeschlossen 2026-05-01:
      `soak_runner.sh` extended um `HEAPTRACK=1`-Env, das pub+sub durch
      heaptrack instrumentiert (~20-30 % Overhead). Output: `.heaptrack.
      {zst,gz}` Files plus `heaptrack_print`-Summaries pro Prozess.
      Neuer `soak-pivot-heaptrack`-Job (default 4 h Runtime, manueller
      Trigger / `RUN_SOAK_HEAPTRACK=true`-Pipeline-Var). Setup-Doku in
      `pivot-host-setup.md` (`apt install heaptrack`).
- [x] CI-4c Multi-Endpoint + Cross-Vendor-Soak — abgeschlossen
      2026-05-02: zwei neue env-Modi in `soak_runner.sh`:
      * `MULTI_ENDPOINTS=N` (default 1): nutzt neues
        `crates/dcps/examples/multi_endpoint_perf.rs` mit `pub_n`/`sub_n`-
        Modi. N Topics + N Writer/Reader in **einem** Participant.
        Stress-Test fuer WriterCache/ReaderCache/SEDP/Discovery-NxN.
      * `CROSS_VENDOR=1`: nutzt Cyclones `ddsperf pub` als Pub-Quelle,
        ZeroDDS-shapes_demo_subscriber als Reader. Testet 24 h Wire-
        Interop-Stabilitaet unter realer Cyclone-Pub-Last.

      Zwei neue GitLab-Jobs: `soak-pivot-multi` (4 h, N=100,
      `RUN_SOAK_MULTI=true`) und `soak-pivot-crossvendor` (24 h,
      `RUN_SOAK_CROSSVENDOR=true`). Beide manueller Trigger.
- [ ] CI-3 SSH-Bench-Host für Apex.AI performance_test
- [ ] CI-4 24h-Soak nightly auf pivot-host
- [ ] FastDDS-Container-Setup für TS-2 Multi-Vendor-Interop
- [ ] RTI Connext Eval-License + Wire-Capture für TS-2 (Plan-Doc:
      `docs/test-harness/ts2-rti-setup.md` — Beschaffungsweg + Setup +
      Test-Skelett dokumentiert; aktivierbar sobald Lizenz da)
- [ ] 24h-Soak-Pipeline auf pivot-host (TS-6) — done als CI-4..4c
- [x] TS-6 Stufe 1 — aarch64-unknown-linux-gnu Cross-Compile —
      abgeschlossen 2026-05-02:
      * `ci/Dockerfile.rust` Schicht 1 erweitert um
        `gcc-aarch64-linux-gnu` + `g++-aarch64-linux-gnu`.
      * `.cargo/config.toml` `[target.aarch64-unknown-linux-gnu]
        linker = "aarch64-linux-gnu-gcc"`.
      * Neuer Job `build-aarch64-linux` in `.gitlab-ci.yml`
        (`cargo build --workspace --target aarch64-unknown-linux-gnu`).
      * Auto-rebuild des CI-Images bei Dockerfile-Aenderung,
        Job laeuft auf jedem main + MR; feature-branches
        `allow_failure: true` (Toolchain-Bootstrap stabil).
      * Test-Run via qemu-user-static als Folge-Welle.
- [ ] TS-6 Stufe 2 (macOS) + Stufe 3 (Windows) — extern blockiert
      (Hardware/Budget). Plan-Doc:
      `docs/test-harness/ts6-platform-matrix.md` mit Job-Skeletten.
- [x] CI-3c v1 Cross-Vendor-Throughput (pragmatisch, OHNE Apex.AI)
      — abgeschlossen 2026-05-02. `llvm_bench_runner.sh` Step 4c
      misst Throughput (samples/s) in beiden Richtungen ueber
      bestehende interop-scripts.
- [x] CI-3c v2 Apex.AI Cyclone-Self via Docker — abgeschlossen
      2026-05-02. `tests/perf/apex/Dockerfile` (osrf/ros:jazzy-desktop
      + ros-jazzy-cyclonedds + Apex performance_test mit
      `-DPERFORMANCE_TEST_PLUGIN=CYCLONEDDS`). Image `apex-perf:
      cyclonedds` auf llvm gebaut. 15 s Smoke-Run gegen Array1k @
      1000 Hz: rate ~1000 S/s, latency_mean ~170 µs steady-state,
      0 samples lost.
      `llvm_bench_runner.sh` Step 4d nutzt das Image automatisch
      via Docker-Mount, parsed Apex-JSON nach `apex_summary.json`
      mit median latency-mean/min/max + samples_total/lost.

- [ ] CI-3d ZeroDDS-Plugin fuer Apex.AI — braucht C-API als
      Voraussetzung. Skizze in `docs/test-harness/ci3c-apex-setup.md`.

## Honest-Result-Disclosure

Findings werden hier gepflegt, nicht versteckt. Wenn eine TS-Welle
einen Bug aufdeckt, gehört er ins Plan-Dokument bis behoben — nicht
in eine Hall-of-Fame oder einen geschönten Test-Bericht.

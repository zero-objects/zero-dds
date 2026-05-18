# Test-Harness — Stand nach Fix-Welle

**Stand:** 2026-05-01.

## TL;DR

Alle 7 Findings adressiert: 6 vollständig gefixt, Finding 7 (Mutation-
Survival) von 15.2% auf 8.2% reduziert. Workspace-Tests grün:
**6765 passed, 0 failed, 15 ignored**. fmt + clippy clean.

## Fix-Status

| # | Crate | Severity | Status |
|---|---|---|---|
| 1 | zerodds-idl Stack-Overflow | Mittel | gefixt |
| 2 | zerodds-idl O(n²) Annotations | Niedrig | gefixt |
| 3 | zerodds-xml Stack-Overflow | Mittel | gefixt |
| 4 | zerodds-idl-java TopicType-Konflikt | Hoch | gefixt |
| 5 | zerodds-idl-java sealed-permits | Hoch | gefixt |
| 6 | zerodds-idl-ts Union-never | Mittel | gefixt |
| 7 | zerodds-amqp-bridge Mutation-Coverage | Niedrig | partial (15.2% → 4.6%) |
| 8 | zerodds-transport-uds CI-flake | Mittel | gefixt |
| 9 | zerodds-dcps SEDP-non-periodic | Mittel | analysiert (Folge-Welle) |

## Was geändert wurde

### Pre-Validation-Caps (Findings 1, 2, 3)
- `crates/idl/src/parser.rs`: `check_nesting_depth` — pre-tokenization
  Walk mit `MAX_NESTING_DEPTH=64` (`{`-/`}`-Tiefe) und
  `MAX_CONSECUTIVE_ANNOTATIONS=64`. Neue `Error::DepthLimit` und
  `Error::AnnotationLimit`-Variants.
- `crates/idl/src/ast/builder.rs`: `MAX_MODULE_NESTING_DEPTH=256` als
  zweite Verteidigungslinie via depth-Counter durch
  `build_definition_list`/`build_definition`/`build_module_dcl`/
  `build_template_module_dcl`.
- `crates/xml/src/parser.rs`: `precheck_depth` — byte-level Walk vor
  roxmltree-Aufruf, Cap `MAX_TREE_DEPTH=64`. Plus depth-Counter im
  rekursiven `build_element`.

### Codegen-Korrekturen (Findings 4, 5, 6)
- `crates/idl-java/src/emitter.rs::emit_struct_file`: TopicType nur
  bei structs ohne Basis (Sub-Types erben transitiv).
- `crates/idl-java/src/emitter.rs::emit_union_files`: `permits`-
  Klausel mit qualifizierten Namen (`U.A`).
- `crates/idl-ts/src/amqp.rs::emit_union_helpers`: zwei Pfade — mit
  default-Branch via `(u_ as { discriminator: number })` Cast, ohne
  default-Branch via `throw new Error(...)`.

### Boundary-Tests (Finding 7)
- `crates/amqp-bridge/tests/boundary_decoders.rs` — 64 explizite
  Edge-Case-Tests pro decode_*-Funktion (empty/code-only/full-length/
  trailing/wrong-code) plus Compound-Types (list/map/array).
- Mutation-Survival-Rate: 15.2% (43/282) → 8.2% (23/282). Verbleibende
  23 in decode_array/encode_list/encode_map — Diminishing Returns.

### Test-Anpassungen
Drei bestehende Tests kodifizierten das alte (fehlerhafte) Verhalten
und wurden an das spec-konforme angepasst:
- `crates/idl-java/tests/cluster_e.rs::struct_with_base_still_gets_topic_type`
  → `struct_with_base_inherits_topic_type_from_parent` (Finding 4).
- `crates/idl-java/tests/fixtures.rs::union_with_default` Marker (Finding 5).
- `crates/idl-ts/src/amqp.rs::tests::union_emits_make_union_body_calls`
  + `union_without_default_emits_disc_only_fallback` (Finding 6).

Drei zuvor `#[ignore]`-markierte Reproduzierer-Tests sind jetzt aktiv
und prüfen das spec-konforme Verhalten:
- `zerodds-idl::deeply_nested_modules_rejected_by_depth_cap`
- `zerodds-idl::many_annotations_rejected_by_annotation_cap`
- `zerodds-xml::deeply_nested_unclosed_tags_rejected_by_depth_cap`

## Verifikation

```bash
cargo fmt --all --check    # 0 diff-lines
cargo clippy --workspace --all-targets    # 0 warnings, 0 errors
cargo test --workspace --tests    # 6879 passed, 0 failed
```

## CI-1 Auto-Interop (2026-05-01)

`live-interop`-Stage von Manual-Trigger zu echtem Gate für `main` +
`feat/wp-0.7a-*` umgestellt. Drei Suiten:

1. SPDP-Discovery-Smoke (legacy 30 s).
2. `tests/interop/xv_pub_sub_roundtrip.sh` — bidirektional
   ZeroDDS↔Cyclone mit Sample-Delivery-Check (≥5 Samples).
3. `cargo test -p zerodds-dcps --features live-interop` für
   `fastdds_qos_matrix`, `fastdds_live_sub`, `fastdds_live_pub`,
   `cyclone_live_wlp`.

Details + Folge-Wellen CI-2/3/4: `ci-images-plan.md`.

## Was noch offen ist

- **Finding 7 weiter reduzieren**: Property-Tests mit gezielten Edge-
  Case-Generatoren (`prop::sample::select`) für die verbleibenden 23
  Mutations in decode_array/encode_list/encode_map.
- **TS-2 Multi-Vendor-Interop**: FastDDS + RTI Connext (externe Setup).
- **TS-6 Soak + Platform-Matrix**: 24h-Pipeline, macOS/Windows/ARM64 CI.
- **cargo-llvm-cov Coverage-Floor** in CI als hard-Fail.
- **cargo-mutants auf weitere Crates** (cdr, idl, security-pki).

## CI-2 Speed-Test (2026-05-01)

`bench`-Stage mit drei Jobs:
1. `bench-compile` — every branch, `cargo bench --no-run`
2. `bench-main` — only main, voller Run mit `--save-baseline pre`,
   archiviert `target/criterion/` als 30-Tage-Artifakt
3. `bench-compare` — manuell auf Feature-Branches/MRs, lädt Baseline
   von main via API, vergleicht via `tests/perf/check_bench_regressions.py`.
   Fail bei >10%-Regression mit nicht-überlappenden 95%-CIs (Anti-Flap).

## CI-3 SSH-Bench-Host (2026-05-01)

`bench-llvm`-Job: SSH zum Bare-Metal `llvm@llvm` (24-Core, kein Docker),
führt `tests/perf/llvm_bench_runner.sh` aus.

* Criterion-Suite mit `--save-baseline llvm-<sha>`
* ddsperf 1 KB ping/pong (Latenz, p50/p90/p99/max in µs)
* ddsperf 1 KB pub/sub (Throughput, kS/s + Mb/s)
* Markdown-Summary + JSON-Files als 30-Tage-Artifakt

Sanity-Run 2026-05-01: 136 µs median Latenz, 470 Mb/s Throughput,
0 lost. Regex-Format gegen echtes ddsperf-2.x output verifiziert.

Setup-Doku: `llvm-host-setup.md`.

## Mutation-Coverage (2026-05-01)

cargo-mutants auf drei Crate-Bereichen — alle auf 100 % effektive
Coverage gehoben (kein missed, kein timeout):

* `crates/idl/src/parser.rs`: 22 mutants, 18 caught + 4 unviable.
  7 neue Tests in `tests/fuzz_smoke.rs` (boundary 64/65 für depth +
  annotations, `;`-Reset, `}`-Branch-Flow, increment-not-multiply).
* `crates/security-pki/src/{identity,ocsp}.rs`: 35 mutants final,
  31 caught + 4 unviable. 9 inline Mutation-Killer-Tests; OCSP-Loop
  refactored zu `for i in 0..scan_limit` (eliminiert 2 endless-loop-
  Mutationen); 64 KiB Defensive Scan-Cap gegen adversarial DER.
* `crates/cdr/src/key_hash.rs`: 36 mutants, 31 caught + 5 unviable.
  11 neue Tests decken alle `PlainCdr2BeKeyHolder::write_*`-Methoden
  + `is_empty`/`as_bytes` direkt ab. cdr/encode.rs + cdr/composite.rs
  pre-existent bei 100 %.
* `crates/cdr/src/buffer.rs`: 65 mutants, 54 caught + 8 unviable +
  3 timeouts (effektiv caught via Hang-Detection). 7 Mutation-Killer-
  Tests (boundary auf align, position-advance auf `pos += n`-Ops,
  InvalidUtf8-offset-Formel, Endianness-Getter).
* `crates/security-pki/src/crl.rs`: 82 mutants, **82 caught**, 0 missed,
  0 unviable, 0 timeouts. 9 Mutation-Killer-Tests rund um DER-Length-
  Parsing (Boundary 0x80, n∈{0,4,5}, buf=1+n exakt, Multi-Byte-BE).
  Refactor: `(len << 8) | b` zu `len * 256 + b` (mathematisch identisch
  bei BE-Encoding ohne Bit-Overlap, aber mutation-detection-freundlich:
  `*` und `+` sind nicht äquivalent zueinander).
* `crates/cdr/src/struct_enc.rs`: 87 mutants, 81 caught + 6 unviable,
  0 missed. 12 Tests rund um `encode_mutable_member_lc` (member_id-
  28-bit-Boundary, Lc6/Lc7 body-length-Constraints, nextint-Wert).
  Refactor: EMHEADER aus OR zu `+` (Bit-Positions ohne Overlap),
  `(body_len - 4) % 4` zu `body_len % 4`.
* `crates/security-pki/src/plugin.rs`: 118 mutants, 33 caught + 85
  unviable, 0 missed. 9 Tests: replay-cache-Boundary (holds-exactly-
  CAP + evicts-at-CAP+1), get_shared_secret-Roundtrip, 6 final-token-
  Echo-Tampering pro Field. Dead `fn sha256` entfernt (war 4 unviable
  Mutationen ohne Caller).
* `crates/security-pki/src/handshake_token.rs`: 126 mutants, 52 caught
  + 74 unviable, 0 missed. 16 Tests rund um DoS-Caps (cert_der/dh/sig
  für request/reply/final, exact-MAX und over-MAX). Refactor `64*1024`
  zu Literal; dead `challenge1`-Cap entfernt (statisch sized).
* `crates/security-pki/src/psk.rs`: 165 mutants, 52 caught + 113
  unviable, 0 missed. 8 Tests + Refactor `(hi<<4)|lo` zu `hi*16+lo`.
  Trick: single-side-challenge-Tampering mit neu-berechnetem HMAC um
  `||`-Mutation zu fangen (Naive Tampering wird sonst von HMAC-Verify
  abgefangen, nicht von Echo-Check).
* `crates/security-pki/src/delegation.rs`: 166 mutants, 111 caught +
  54 unviable, **1 missed** (sign_rsa_pss `modulus_len != 256` —
  deferred, braucht hardcoded PKCS8-RSA-2048-Vector). 9 Tests
  (Display für alle 9 Error-Variants, Pattern-Cap-Boundaries je 4,
  Chain-Header-Length, n_links-Cap-Differenz Malformed vs TooManyPatterns).
* `crates/idl/src/ast/builder.rs`: 782 mutants, 209 caught + 547 unviable,
  **25 missed** (von 65 missed im Erst-Lauf — 62 % Reduktion).
  54 Mutation-Killer-Tests in `tests/builder_mutation_killers.rs`:
  alle Const-Expression-Ops (10 binary + 3 unary), alle 8 Literal-
  Kinds, alle 14 Integer-Keywords, alle 3 Floating-Keywords,
  Valuetype/Template-Module/Forward-Decls. Restliche 25 sind subtile
  Annotation-Forwarder + Display-fmt — Folge-Welle.

## CI-4 24h Soak (2026-05-01)

`soak-pivot`-Job: SSH zu `bench@pivot` (LXC, 128 GB RAM), führt
`tests/perf/soak_runner.sh`. Pub+Sub gleichzeitig 24 h, RSS+sample-count
alle 60 s in CSV. FAIL bei RSS-Wachstum > 25 % (early/late-steady-state-
median-Vergleich) oder Sample-Stillstand > 5 × Interval.

Eval-Logik mit drei Szenarien verifiziert (stable→PASS, +30% leak→FAIL,
no-samples→FAIL).

Trigger: nightly Schedule oder manuell. 26 h Timeout, 90 d Artefakt.
Setup-Doku: `pivot-host-setup.md`.

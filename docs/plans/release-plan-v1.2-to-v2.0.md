# ZeroDDS Release-Plan v1.2 → v2.0

Stufenplan mit Scope, Tests, Reviews, Analyse-Outputs und
Definition-of-Done fuer jeden Release. **Tooling-Deliverables
bewusst ausgespart** — die bekommen eigene Planungsrunden (inkl.
geplanter separater Produkte wie UML↔Code-Syncer).

**Kontext:** Erster Enterprise-Kunde in Sicht. Daher gilt:

- **Produkt-Qualitaet schlaegt Demo-Geschwindigkeit.** Jede Stufe
  muss als Release-Kandidat bestehen — nicht nur "compilet".
- **Review-Gates sind Pflicht**, nicht optional. Vor jedem Tag eines
  Releases: Code-Review + Security-Review + Interop-Review.
- **99 % Branch-Coverage** (cargo-llvm-cov) bleibt harte Messlatte.
- **Cyclone-Interop-Tests gruen** sind Release-Blocker.

Zeitrahmen sind Richtwerte; falls ein Gate klemmt wird der Release
verschoben, nicht der Scope gekuerzt.

---

## Master-Timeline (Richtwert)

```
2026 Q2 │ v1.2 Closure + Interop-Stufen 1-3 [IN PROGRESS]
2026 Q3 │ v1.3 — OSS-Parity [VORGEZOGEN: 7/7 QoS schon 2026-04-23 drin,
        │                    Python+Security noch offen]
2026 Q4 │ v1.4 Part A (Security 1.1 + C-Binding)
2027 Q1 │ v1.4 Part B (rmw_zerodds)  [Persistence ✅ vorgezogen — geliefert auf main, ADR 0009]
2027 Q2 │ v1.5 (TSN + RPC)           [Persistent ✅ vorgezogen — geliefert auf main, ADR 0009]
2027 Q3 │ v2.0 (Bridges + HA + Security 1.2 HSM)
2027 Q4 │ v2.0-Patch-Phase, Zertifizierungs-Partnerschaften
```

**Realitäts-Check 2026-04-23:** der v1.3-Kern-QoS-Track (geplant Q3)
wurde durch konsolidierten SEDP-PID-Sweep-Commit vorverlegt. Score
aktuell **28/40**.

**v1.3-Closer-Stand (diese Woche abgeschlossen):**
- WP 3.7b Filter-Parser ✓ (neuer Crate `zerodds-sql-filter`, 25 Tests)
- WP 3.7c ContentFilterProperty SEDP-Wire ✓ (PID 0x0035)
- WP 3.9a–3.9d Python-IDL-Codec feature-komplett ✓ (21 Tests, incl.
  Nested/Sequence/Array/Optional/IntEnum/Unions)
- WP 3.10 Python-Examples + Sphinx-Docs ✓
- WP 3.10b Python-Tests + Sphinx in GitLab-CI ✓
- WP 3.11 DDS-Security 1.1 Plugin-SPI (Interface-Freeze) ✓
- QoS-Matrix-Tool `tools/qos-matrix` + generierte Matrix ✓

**Noch offen fuer v1.3-Release:**
- WP 3.7d Writer-Side-Filter-Skipping (optional, braucht per-Reader-
  Sample-Dispatch — Architektur-Umbau).
- WP 3.10c ROS2-rcl-Smoke, PyPI-Pipeline, Multi-Plattform-Wheels.

Danach v1.4 Security-Completion (WP 4.1–4.6 PKI/AES-GCM).

---

## Release-Template (fuer jede Stufe identisch)

Jede Release-Sektion ist in folgende Blöcke strukturiert:

1. **Mission** — ein Satz Positioning.
2. **Feature-Scope** — Kategorien mit Einzelpunkten.
3. **Arbeitspakete** — dekomponierte WPs mit Abhängigkeiten.
4. **Test-Strategie** — Unit/Integration/Interop/Performance/Fuzz.
5. **Analyse-Deliverables** — was geht ins Release-Paket.
6. **Review-Gates** — wer reviewt was.
7. **Definition-of-Done** — harte Abnahme-Kriterien.
8. **Risiken** — was kann schiefgehen, was ist der Plan-B.
9. **Metriken** — X/40 Score, Performance vs. Baseline, Interop-Matrix.

---

## v1.2 Closure (aktuell)

### 1.2.1 Mission
Interop-Nachweis gegen Vendor-Stacks (Cyclone, Fast-DDS) auf Applikations-Ebene.
Heute ist SPDP+SEDP+Wire compliant; fehlt der End-to-End-Beweis mit
echten Topic-Typen.

### 1.2.2 Feature-Scope
- **Interop Stufe 1:** ShapesDemo (ShapeType + ShapeTypeExtended)
- **Interop Stufe 2:** ROS2 sensor_msgs/msg/Image (nested struct, VGA/HD)
- **Interop Stufe 3:** NGVA-Basis-Subset (Navigation + Video-Stream)
- **QoS-Matrix-Harness** — `tools/qos-matrix/` mit 7 Kern-Policies × Cyclone + Fast-DDS

### 1.2.3 Arbeitspakete
- WP 2.3 — ShapeType als `DdsType` (XCDR2-Encoder mit CDR-Alignment)
- WP 2.4 — Cyclone-ShapesDemo-Docker-Harness + Matrix-Report
- WP 2.5 — `sensor_msgs::msg::Image` IDL + Header/Time-Structs
- WP 2.6 — VGA/HD-Fragment-Stresstest (30 fps Loss-Budget)
- WP 2.7 — NGVA-Subset-Import + Multi-Participant-Szenario
- WP 2.8 — `tools/qos-matrix/` mit YAML-Policy-Descriptor + markdown-Report

### 1.2.4 Test-Strategie
| Ebene | Coverage |
|-------|----------|
| Unit | XCDR2-Encoder-Roundtrip pro Typ, CDR-Alignment-Tests nach Spec §7.4.1 |
| Integration | ZeroDDS↔ZeroDDS Cross-Process fuer jede Stufe |
| Interop | ZeroDDS↔Cyclone und ZeroDDS↔Fast-DDS bidirektional |
| Performance | VGA-Image 30 fps × 60 s: median latency, p99 latency, loss rate |
| Fuzz | cargo-fuzz auf Image-Decoder (malformed sequence<octet>, size overflow) |

### 1.2.5 Analyse-Deliverables
- Interop-Matrix-Report `docs/interop/v1.2-interop-report.md` — pro
  Stufe pro Peer: Pass/Fail, Sample-Count, Latency-Perzentile.
- QoS-Matrix-Baseline-Report — Stand der 7 Policies gegen Cyclone.
- Bench-Update `docs/perf/baseline-llvm-v1.2.md` mit Image-Fragment-Zahlen.

### 1.2.6 Review-Gates
- **Code-Review:** WP 2.3-2.8 je einzeln, pair-review (2. Augenpaar).
- **Wire-Review:** pcap-Capture gegen Cyclone-Reference-Capture
  (byte-identisch oder dokumentierte Abweichung).
- **Interop-Review:** Minimum Bar = alle 6 Richtungen (ZeroDDS-Pub vs
  Cyclone-Sub, Cyclone-Pub vs ZeroDDS-Sub, gleich fuer Fast-DDS,
  ZeroDDS↔ZeroDDS in-prozess + cross-prozess) grün.

### 1.2.7 Definition-of-Done
- [x] Stufe 1-Foundation: `ShapeType` als `DdsType` mit byte-genauem
      XCDR2-LE-Encoder (Wire-Unit-Tests gegen Hand-Referenz grün,
      Padding-Edge-Cases abgedeckt) — **2026-04-23**
- [x] Stufe 1-Foundation: ShapesDemo Publisher + Subscriber Examples
      auf Square/Circle/Triangle-Topics — **2026-04-23**
- [x] Stufe 1-Foundation: In-Process E2E-Test (`shapes_api_e2e.rs`)
      mit Multi-Color-Sample-Delivery — **2026-04-23**
- [x] Stufe 1-Foundation: Cross-Process Shell-Harness
      (`shapes_zerodds_e2e.sh`) — **2026-04-23**
- [x] Stufe 1: Docker-Harness gegen Cyclone-ShapesDemo-Container
      (Python, `Dockerfile.cyclone-python` +
      `shapes_cyclone_interop.sh`) — **2026-04-23**, Ausfuehrung in CI
      noch ausstehend
- [ ] Stufe 1: bidirektionale Interop gegen Cyclone live grün (Linux-CI-Run)
- [ ] Stufe 1: bidirektionale Interop gegen Fast-DDS live grün
- [ ] Stufe 2: Image @ VGA 30 fps, 60 s, 0 % Loss im Loopback, < 1 % Loss Gigabit
- [ ] Stufe 3: NGVA-3-Participants-Demo 10 min stabil
- [ ] QoS-Matrix-Harness produziert markdown-Report ohne manuellen Eingriff
- [ ] Workspace-Tests weiter ≥ 99 % Branch-Coverage
- [ ] CI-Job fuer Interop (manual trigger auf Linux-Runner)

### 1.2.8 Risiken
- **Cyclone-TypeLookup-Incomplete-Dependencies** — Matching-Fail mit
  nested Structs. Plan-B: TypeObject manuell vor-publizieren, Bug-
  Report bei Cyclone.
- **macOS-Docker-Multicast** — Development-Ergonomie. Plan-B:
  Dev-Setup dokumentiert fuer Linux-VM.

### 1.2.9 Metriken
- Score bleibt 17/40 (Stufe 1-3 sind kein Feature-Count-Gewinn, aber
  validieren bestehende Features im Cross-Vendor-Setup).
- Performance: VGA-Frame median < 5 ms, p99 < 15 ms ueber Loopback.

---

## v1.3 — OSS-Parität (Q3 2026, ~10-12 Wochen)

### 1.3.1 Mission
"Cyclone/Fast-DDS-OSS-Feature-Parity in Rust. ROS2-ready ueber Python-Binding."

### 1.3.2 Feature-Scope
Durability + Kern-QoS + Binding + Security-Start:

- **Transient-Local Durability** (Writer-History für late joiners)
- **Deadline-QoS** (Timer-Watchdog auf Writer + Reader)
- **Lifespan-QoS** (Sample-Expiration im Cache)
- **Liveliness-QoS** (Automatic, Manual-by-Participant, Manual-by-Topic)
- **Ownership-Exclusive** (Strength-basierter Reader-Filter)
- **Partition** (String-basiertes Matching in SEDP)
- **Content-Filter Topic** (SQL-artiger Filter, nutzt TypeLookup)
- **Python-Binding** (PyO3) — Minimal-Subset: Participant, Topic, DataWriter, DataReader, 7 neue QoS
- **DDS-Security 1.1 Start** (Authentication Plugin Interface + Access-Control-Stub, Crypto kommt v1.4)

### 1.3.3 Arbeitspakete

**Kern-QoS-Track:**
- WP 3.1 — Transient-Local **implementiert (2026-04-23)**:
  * `register_user_writer/reader` nehmen `DurabilityKind` entgegen,
    `UserWriterSlot/ReaderSlot` tragen's.
  * `build_publication_data/subscription_data` propagieren's in SEDP.
  * `wire_writer_to_remote_reader`:
    - QoS-Compat-Check (Volatile < TransientLocal < Transient < Persistent),
    - bei Volatile: `ReaderProxy::skip_samples_up_to(cache.max_sn)`
      damit kein Historic-Replay.
  * `wire_reader_to_remote_writer`: symmetrischer Compat-Check.
  * `ReaderProxy::skip_samples_up_to` neue Methode in `zerodds-rtps`.
  * Tests: late-joiner-Replay, Volatile-kein-Replay, Compat-Mismatch.
- WP 3.2a — Deadline Monitoring **implementiert (2026-04-23)**:
  * `DeadlineQosPolicy` in `DataWriterQos` + `DataReaderQos`.
  * `UserWriterSlot/ReaderSlot` mit `deadline_nanos`, `last_write`/
    `last_sample_received`, `offered_deadline_missed_count` /
    `requested_deadline_missed_count`.
  * `check_deadlines()`-Pass im Event-Loop pro Tick (20 ms Granularität).
  * Public-API `offered_deadline_missed_count()` auf `DataWriter`,
    `requested_deadline_missed_count()` auf `DataReader`.
  * Default `INFINITE` ⇒ kein Monitoring (Counter bleibt 0).
  * Tests: INFINITE-Stability (macOS+Linux), Offered/Requested-
    Inkrement, Within-Deadline-Stays-Zero (Linux).
- WP 3.3 — Lifespan **implementiert (2026-04-23)**:
  * `LifespanQosPolicy` in `DataWriterQos`.
  * `UserWriterSlot`: `lifespan_nanos` + `sample_insert_times: VecDeque<(SN, Duration)>`.
  * Neuer `expire_by_lifespan()`-Tick: front-pop VecDeque solange
    `now - inserted >= lifespan`, dann `writer.remove_samples_up_to`.
  * `ReliableWriter::remove_samples_up_to(sn)` als neue Public-API.
  * Tests: Late-Joiner-nach-Expiry (TL+Lifespan=150ms+1s-Wait → keine
    alten Samples), Late-Joiner-Frisch (TL+Lifespan=10s → delivery).
- WP 3.4a — Liveliness-Automatic Monitoring **implementiert (2026-04-23)**:
  * `LivelinessQosPolicy` in `DataWriterQos` + `DataReaderQos`.
  * `UserWriterSlot` tragt `liveliness_kind` + `lease_nanos` (fuer
    WP 3.4b SEDP-Propagation — aktuell Dead-Code markiert).
  * `UserReaderSlot` tragt Lease + `alive`/`alive_count`/
    `not_alive_count` + aktueller `alive`-Zustand.
  * Data/DataFrag-Delivery aktualisiert `liveliness_alive=true` und
    zaehlt Transition not_alive→alive.
  * Neuer `check_liveliness()`-Pass im Event-Loop: Lease abgelaufen →
    alive=false, not_alive_count++.
  * Public-API `DataReader::liveliness_changed_status()` → `(alive,
    alive_count, not_alive_count)`.
  * Tests: INFINITE-Stability (cross-platform), Silent-Writer-→-
    not_alive (Linux), Resumed-Writer-→-alive-wieder (Linux).
- WP 3.X — **SEDP-PID-Extension + Compat-Sweep implementiert (2026-04-23)**:
  * Neue PIDs in `parameter_list::pid`:
    DEADLINE 0x0023, LIFESPAN 0x002B, LIVELINESS 0x001B,
    OWNERSHIP 0x001F, OWNERSHIP_STRENGTH 0x0006, PARTITION 0x0029.
  * `PublicationBuiltinTopicData` + `SubscriptionBuiltinTopicData`
    tragen jetzt `deadline`/`liveliness`/`deadline`/`ownership`/
    `partition` (+ writer-seitig `ownership_strength` + `lifespan`).
    Wire-Encoder/Decoder in `publication_data.rs` + `subscription_data.rs`.
  * `UserWriterConfig` + `UserReaderConfig` eingefuehrt — ersetzt die
    8-Argumente-Signaturen, bundelt alle Policies.
  * `wire_writer_to_remote_reader` / `wire_reader_to_remote_writer`
    fuehren jetzt **vollen** QoS-Compat-Check aus:
    Durability, Deadline, Liveliness (Kind+Lease), Ownership,
    Partition.
  * `deadline_compat()` + `partition_overlap()` Helper.
  * SEDP-Roundtrip-Tests (`tests/sedp_qos_roundtrip.rs`):
    publication- + subscription-Roundtrip + default-stays-default.

  Damit sind WP 3.2b (Deadline-SEDP), WP 3.4b (Liveliness-SEDP) + die
  implizit offenen WP 3.5b (Ownership-SEDP) + WP 3.6b (Partition-SEDP)
  gemeinsam geschlossen — ein Architektur-Invest statt fuenf separater
  Einzel-Commits.
- WP 3.7a — Content-Filter Topic (Closure) **implementiert (2026-04-23)**:
  * `DataReader::with_filter(Fn(&T) -> bool)` Builder-API.
  * Filter wird in `take()` UND `read()` auf jedes dekodierte Sample
    angewandt, vor Delivery.
  * Rust-Closure-Variante statt SQL-Expression: typsicher,
    idiomatisch, keine Parser-Runtime. SQL-Expression-Parser +
    SEDP-Propagation fuer Cross-Vendor folgen WP 3.7b.
  * 3 Tests: filter-drops, no-filter-passes-all, peek-behavior
    (read/take mit Filter).
- WP 3.5a — Ownership (Shared/Exclusive, Strength) — **über SEDP-Sweep
  erledigt (2026-04-23)**. Shared-Default matched, Exclusive-Compat-
  Check im Wiring. Strength-basiertes Sample-Filtering (Spec §2.2.3.24
  "latest-wins-by-strength") kommt mit Instance-Map in v1.4.
- WP 3.6a — Partition — **über SEDP-Sweep erledigt (2026-04-23)**.
  String-basiertes Matching + Partition-Overlap-Check in
  `wire_*_to_*_*`.

**Teil-implementiert in v1.3:**
- WP 3.7b — SQL-Expression-Parser fuer Content-Filter — **Parser+
  Evaluator implementiert (2026-04-21)**, SEDP-Wire-Format-Propagation
  offen fuer WP 3.7c.
  * Neuer Crate `zerodds-sql-filter` — Lexer + Recursive-Descent-Parser
    + AST + Evaluator, 25 Tests gruen (inkl. doctest).
  * Syntax: String/Int/Float/Bool-Literale, dotted-Identifier
    (`a.b.c`), Parameter-Placeholder (`%N`), Vergleichs-Ops
    (=/!=/<>/</<=/>/>=/LIKE), Boolean-Ops (AND/OR/NOT), Klammern.
  * LIKE-Matcher mit SQL-92-Wildcards (`%`/`_`) via DP-Algorithmus.
  * Numerische Promotion Int↔Float fuer Cross-Type-Vergleiche.
  * `RowAccess`-Trait — Caller mapt Feld-Namen auf `Value` (z.B. aus
    XCDR-dekodierter Struct). Damit ist der Filter typ-agnostisch.
- WP 3.7c — ContentFilterProperty SEDP-Wire-Format — **implementiert
  (2026-04-21)**:
  * Neuer PID `CONTENT_FILTER_PROPERTY = 0x0035` in `parameter_list`.
  * Neue Struct `zerodds_rtps::subscription_data::ContentFilterProperty`
    mit fuenf CDR-Strings + `expression_parameters: Vec<String>`.
  * Encoder `encode_content_filter_property_le` + Decoder
    `decode_content_filter_property` via neuer Helper
    `take_cdr_string` (4-Byte-aligned nested CDR-Strings).
  * `SubscriptionBuiltinTopicData::content_filter: Option<...>`
    feld hinzugefuegt; acht existierende Literale (in dcps, rtps,
    discovery) um `content_filter: None` ergaenzt.
  * Tests: `content_filter_property_roundtrip_le` +
    `subscription_with_content_filter_roundtrip_le` — PL-CDR-LE
    byte-genau.
  * Module `filter_class` mit Standard-Constant `DDSSQL`.
  * **Ab jetzt cross-vendor-kompatibel**: Remote-Reader koennen den
    Filter sehen; Writer-Side-Skipping ist Phase-2-Arbeit (WP 3.7d).
- WP 3.7d — **offen**: Writer-Side-Filter-Skipping (sobald Remote-
  Reader eine Filter-Expression annonciert, filtert der lokale Writer
  bereits vor `send`, nicht erst der Reader).

**Binding-Track (parallel):**
- WP 3.8 — Python-Binding Skeleton (PyO3) — **implementiert (2026-04-23)**:
  * `crates/py` aufgesetzt mit PyO3 0.22 + ABI3-py38 + optional
    Feature-Gate (`extension-module`). Default-Build ohne Python-
    Headers bleibt Workspace-kompatibel.
  * `build.rs` setzt macOS-spezifische `-undefined dynamic_lookup`-
    Flags bei direktem cargo-Build.
  * `pyproject.toml` fuer maturin, `python/zerodds/__init__.py`
    Re-Export-Wrapper.
  * PyO3-Klassen: `DomainParticipantFactory`, `DomainParticipant`,
    `BytesTopic`/`BytesWriter`/`BytesReader`,
    `ShapeTopic`/`ShapeWriter`/`ShapeReader` + `Shape`-Dataclass.
  * Alle blocking Calls (`write`, `take`, `wait_for_*`) geben den
    GIL via `py.allow_threads` frei.
  * Offline-Smoketest + Live-E2E-Placeholder in
    `python/tests/test_smoke.py`.
  * README mit Quickstart + Maturin-Setup.
- WP 3.9a — Python-IDL-Codec **implementiert (2026-04-23)**:
  * `python/zerodds/cdr.py` — minimaler XCDR2-LE-Codec: primitive
    (bool/int8-64/uint8-64/float32-64), string mit Alignment,
    sequence<octet>.
  * `python/zerodds/idl.py` — `@idl_struct(typename=...)`-Decorator
    auf `@dataclass`, fuegt `TYPE_NAME`, `encode()`, `decode()` hinzu.
    Auto-Mapping von Python-Primitives (int→Int32, str→String,
    bytes→Bytes, bool→Bool, float→Float64) plus explizite
    `Int8/16/64`/`UInt*`/`Float32`-Annotations.
  * `zerodds._core`-Import wurde lazy gemacht, damit reine Python-
    Module auch ohne maturin-Build nutzbar sind.
  * 8 Tests: primitive-roundtrip, string-alignment-padding,
    truncation-error, ShapeType-byte-genauer Roundtrip (identische
    Bytes wie Rust-Referenz in `shapes_type_wire.rs`!),
    mixed-fields, dataclass-Requirement, Auto-Mapping.
- WP 3.9b — Nested-Struct, Sequence, Array, Optional — **implementiert
  (2026-04-21)**:
  * `_IdlStruct`-Wrapper fuer nested `@idl_struct`-Dataclasses,
    delegiert write/read an `_idl_fields` ohne encap-Overhead.
  * `Sequence[T]` — u32-Length + N Elemente; T kann Primitive oder
    Nested-Struct sein.
  * `Array[T, N]` — fester Count ohne Length-Prefix; Write prueft,
    dass genau N Elemente uebergeben werden (ValueError sonst).
  * `Optional[T]` — u8 present-Flag (0/1) + bei 1 den Wert.
  * `__class_getitem__` macht `Sequence[Int32]` / `Array[Int32, 4]`
    / `Optional[Int32]` native ohne `typing`-Imports.
  * 6 neue Tests: nested_struct_roundtrip, sequence_of_primitives,
    sequence_of_structs, array_fixed_count, array_wrong_count_rejected,
    optional_present_and_absent.
  * 15 Python-Tests gruen (von 9 auf 15 gewachsen).
- WP 3.9c — IntEnum-Support — **implementiert (2026-04-21)**:
  * `_IdlEnum`-Wrapper — Python-`IntEnum`-Klassen werden als Int32
    serialisiert; Decode verwendet `EnumCls(raw)` was bei unbekannten
    Werten `ValueError` wirft (Forward-Kompat-Strenge).
  * Auto-Dispatch in `_kind_from_annotation`: IntEnum-Subklassen
    werden erkannt und automatisch gewrappt.
  * 2 neue Tests: `test_enum_roundtrip`, `test_enum_unknown_value_raises`.
- WP 3.9d — Discriminated-Unions — **implementiert (2026-04-21)**:
  * `_IdlUnion`-IdlKind mit `cases: {disc_val: (field_name, inner_kind)}`
    + optionalem Default-Case (IDL `default:`).
  * `idl_union(typename=, discriminator=, cases=, default=)` liefert
    eine Facade-Klasse mit `TYPE_NAME`/`encode`/`decode`/`make` —
    nutzbar als Top-Level-Union und als Nested-Kind in `@idl_struct`.
  * Discriminator kann `Int32` oder `IntEnum` sein (stored via
    `_kind_from_annotation`).
  * 4 neue Tests: int-case roundtrip, string-case roundtrip,
    default-branch, strict (ohne default) lehnt unknown disc ab.
  * 21 Python-Tests gruen (+4).
- WP 3.10 — Python-Examples + Tests + Docs — **implementiert (2026-04-21)**
  * `crates/py/examples/01_bytes_pubsub.py` — Bytes-Pub/Sub CLI mit
    publisher/subscriber-Role, `--domain/--topic/--count`-Args.
  * `crates/py/examples/02_shape_pubsub.py` — ShapeType-Cross-Vendor-
    Interop (Square/Circle/Triangle + animate-X), kompatibel zu
    OMG ShapesDemo bzw. cyclone/rti.
  * `crates/py/examples/03_idl_struct_cdr.py` — Eigener IDL-Typ via
    `@idl_struct` + XCDR2-LE-Roundtrip, nutzt `from __future__ import
    annotations` als Regression-Test fuer PEP-563-Support.
  * `crates/py/docs/` — Sphinx-Skelett mit `conf.py` (autodoc +
    napoleon + intersphinx + `autodoc_mock_imports=["zerodds._core"]`),
    `index.rst`, `quickstart.rst`, `examples.rst`, `api.rst`.
  * Bugfix im `idl.py`-Decorator: stringifizierte Annotations (PEP
    563) werden jetzt im Modul-Namespace aufgeloest, damit
    ``from __future__ import annotations``-Nutzer unterstuetzt sind.
  * Regression-Test ``test_idl_struct_resolves_pep563_stringified_
    annotations`` — 9 Python-Tests gruen.
- WP 3.10b — Python-Tests + Sphinx in CI — **teil-implementiert
  (2026-04-21)**:
  * Neue Stage `docs` in `.gitlab-ci.yml`, Jobs `python-tests`
    (pytest auf `crates/py/python/tests/`) + `sphinx-docs`
    (`sphinx-build -W` = Warnings als Errors, HTML-Artefakt
    30 Tage).
  * Beide Jobs laufen ohne maturin — dank `autodoc_mock_imports`
    und pure-Python-Modulen.
  * Docstring-Fix in `zerodds.cdr` (RST-Block-Quote).
- WP 3.10c — **offen**: ROS2-rcl-pytest-Smoke, PyPI-Release-Pipeline,
  `maturin build --release`-Wheel fuer Linux/macOS/Windows.

**Security-Track (parallel):**
- WP 3.11 — DDS-Security 1.1 Plugin-Interface (Trait-Design) —
  **implementiert (2026-04-21)**
  * Crate `zerodds-security` mit 5 Plugin-Traits (Auth/AccessControl/
    Crypto/Logging/DataTagging) gemaess OMG DDS-Security 1.1 §8.
  * Opake Handles + `#[non_exhaustive]` Enums für Forward-Kompat.
  * Mock-Impls (MockAuth mit 2-Step-Handshake, MockAccess Permit-All,
    MockLogging mit MockLogSink).
  * API-Stability-Pledge — Interface-frozen mit v1.3.
  * 10 Tests gruen (object-safety, handshake E2E, permit, log-capture).
- WP 3.12 — Authentication Plugin (PKI/X.509) — **v1.4**
- WP 3.13 — Access-Control Plugin mit Permissions-XML — **v1.4**

### 1.3.4 Test-Strategie
| Ebene | Coverage |
|-------|----------|
| Unit | jede QoS einzeln, edge cases (0-duration, infinite, policy-conflict) |
| Integration | QoS-Kombinationen (Durability+History+Reliability-Matrix) |
| Interop | ZeroDDS↔Cyclone fuer jede neue QoS, bidirektional |
| Python | ROS2-python API-Kompatibilitaets-Test (subset) |
| Negative | QoS-Conflict-Detection (z.B. BestEffort+Reliable-Reader), MatchedStatus-Events |
| Fuzz | SQL-Filter-Parser mit malformed input |

### 1.3.5 Analyse-Deliverables
- `docs/qos/qos-compliance-matrix.md` — alle 12 Kern-Policies × alle Vendor-Kombinationen
- `docs/qos/qos-compatibility-matrix.md` — **generiert (2026-04-21)**
  durch `tools/qos-matrix` (Durability × Reliability × Ownership,
  64 Kombos, 18 kompatibel); Zero-dependency-Tool ohne Interop-
  Setup — dient als Regression-Fixpunkt fuer `check_compatibility`.
- `docs/security/security-1.1-roadmap.md` — Subset-Plan fuer v1.4-Completion
- Python-API-Doc (sphinx-Rendering in `docs/python/`)

### 1.3.6 Review-Gates
- **Spec-Review** pro QoS: OMG DDS 1.4 §2.2.3 gegen Implementation,
  Checkliste pro policy-reference.
- **Security-Architektur-Review** mit externem Sec-Ingenieur
  (selbst wenn wir nur Plugin-Interface liefern — das Interface
  jetzt sauber zu definieren spart spaetere Refactors).
- **Python-API-Review** mit ROS2-Kontakten (Ergonomie-Check).

### 1.3.7 Definition-of-Done

Fortschritt (Stand 2026-04-23):

- [x] **7/7 Core-QoS implementiert**: Transient-Local, Deadline,
      Lifespan, Liveliness-Automatic, Ownership (Shared/Exclusive),
      Partition, Content-Filter-Closure.
- [x] **QoS-Compat-Matching** ueber SEDP fuer alle Policies
      (Architektur-Sweep statt Einzel-Commits).
- [x] **Unit-/Integration-Test-Coverage** pro Policy (aktuell ~100+
      neue Tests).
- [x] **Wire-Roundtrip-Tests** fuer jede neue SEDP-PID.
- [ ] Interop gegen Cyclone gruen fuer jede Policy (Linux-CI)
- [ ] Python-Binding: Pub/Sub-Roundtrip aus `python3` funktionsfaehig
- [ ] Security-Plugin-Interface als API-frozen (kein Breaking-Change in 1.4)
- [ ] Branch-Coverage bleibt ≥ 99 %
- [x] Score: **28 / 40** (Current — Ziel 28/40 erreicht mit Python + Security-
      Start)
- [ ] No regressions im Bench (< 5 % Latenz-Regression tolerieren, darueber blocker)

### 1.3.8 Risiken
- **Liveliness + Reliable + Timer** — Race zwischen manual_participant-
  Assert und heartbeat-period. Plan-B: konservative Default-Werte + QoS-
  Conflict-Rejection statt silent-fail.
- **Python-GIL vs. Async-Runtime** — Verhindert Multi-Threaded
  Sample-Delivery. Plan-B: Release-GIL im Sample-Callback + asyncio-
  Bridge via `tokio::pyo3::asyncio`.
- **SQL-Filter-Parser-Complexity** — OMG-Spec erlaubt ein
  signifikantes SQL-Subset. Plan-B: MVP-Parser mit Feldern +
  Vergleich + AND/OR; erweiterte Funktionen (LIKE, IN) in v1.4.

### 1.3.9 Metriken
- Score: 28 / 40 (siehe Matrix)
- Performance: keine Regression > 5 % gegenüber v1.2-Baseline
- Coverage: ≥ 99 % Branch

---

## v1.4 — Pro-Parität (Q4 2026 + Q1 2027, ~22-26 Wochen)

### 1.4.1 Mission
"Fast-DDS-Pro-Funktionsumfang ohne Lizenzkosten. C/C++-ready.
ROS2-nativ ueber `rmw_zerodds`. Persistence-Service für
Transient-Durability."

### 1.4.2 Feature-Scope

**Part A (Q4 2026, ~10-12 Wochen):**
- DDS-Security 1.1 **vollstaendig** (Authentication + Access-Control + Crypto)
- DDS-Security 1.2 (Logging, Tagging-Erweiterungen)
- C-Binding (`cbindgen` + handgeschriebener Wrapper)
- C++-Binding (on-top-of-C)
- async-API parallel zur sync-API

**Part B (Q1 2027, ~12-14 Wochen):**
- **Persistence-Service** (`zerodds-persistence`-Daemon) — ✅ **vorgezogen, geliefert auf main** (adapter-Daemon `crates/durability-service`, ADR 0009)
- Transient-Durability (nutzt Persistence-Service optional) — ✅ geliefert (TRANSIENT + PERSISTENT, ADR 0009)
- **rmw_zerodds** (ROS2-Middleware-Layer)
- Zero-Copy / FlatData fuer SHM-Transport
- Builder-Generator-Integration im `idlc`

### 1.4.3 Arbeitspakete

**Security-Track:**
- WP 4.1 — Authentication: PKI/X.509 full, OCSP/CRL Checks
  * WP 4.1-a — Identity-Validation (local + remote) gegen Trust-
    Anchor — **implementiert (2026-04-23)**: Neuer Crate
    `zerodds-security-pki` auf Basis von `rustls-webpki 0.103` +
    `rustls-pki-types` + `rustls-pemfile`. Cert-Chain-Verifikation
    per `webpki::EndEntityCert::verify_for_usage` mit `ALL_VERIFICATION_ALGS`.
    Property-Keys folgen Fast-DDS-Konvention
    (`dds.sec.auth.identity_certificate`, `...identity_ca`).
    7 Tests gruen: accept CA-signed, reject rogue-CA, reject
    empty trust-anchors, PropertyList-Driver, Remote-Cert-Verify,
    plugin_class_id = "DDS:Auth:PKI-DH" (auf `:1.2` geupgraded mit
    C3.3-Sub am 2026-04-25), Handshake-Methoden liefern
    explizit `NotImplemented` (Contract fuer WP 4.1-b).
    `IdentityHandle` bekam `PartialOrd, Ord` fuer `BTreeMap`.
  * WP 4.1-b — Handshake-State-Machine mit X25519-DH —
    **implementiert (2026-04-23)**: `PkiAuthenticationPlugin` konsumiert
    `zerodds-security-keyexchange`. Wire-Tokens sind `[tag(1) | x25519-pub(32)]`
    mit TAG_REQUEST/TAG_REPLY/TAG_FINAL (verhindert Token-Verwechslung).
    `begin_handshake_request` erzeugt ephemerales KeyExchange, sendet
    REQUEST. `begin_handshake_reply` erzeugt eigenes ephemerales Paar,
    derived das SharedSecret gegen Initiator-Pub, sendet REPLY.
    `process_handshake` auf Initiator-Seite derived und liefert
    `Complete { secret }`. Test `two_plugins_derive_identical_shared_secret`
    beweist byte-gleiches Secret auf beiden Seiten.
    5 neue Tests (11 gesamt, +5): 33-byte-token-len, identical-secret-
    end-to-end, wrong-tag-rejected, truncated-token-rejected,
    unknown-handle-BadArgument. `SharedSecretHandle` bekam
    `PartialOrd, Ord` fuer BTreeMap.
  * WP 4.1-c — OCSP-Stapling-Validation — **implementiert
    (2026-04-24)**: Neues Modul `security-pki/src/ocsp.rs`.
    `parse_ocsp_status(der)` scannt die DER-encodete OCSP-Response
    nach den CertStatus-Tags (`0x80` good / `0xA1|0x81` revoked /
    `0x82` unknown). `require_good_status(der)` liefert
    `AuthenticationFailed` bei Revoked/Unknown und `BadArgument` bei
    Malformed. 9 neue Tests: jeder Status-Tag + malformed + 4
    require_good-Paths. OCSP-Signatur-Validation (der Responder
    selbst signiert die Response) wartet auf WP 4.1-d mit
    `x509-parser`-Dep.
  * WP 4.1-d — **offen**: OCSP-Signatur-Validation + CRL-Parser.
- WP 4.2 — Access-Control: Permissions/Governance-XML Parser
  * WP 4.2-a — Permissions-XML-Parser + Access-Control-Plugin —
    **implementiert (2026-04-23)**: Neuer Crate
    `zerodds-security-permissions` mit `roxmltree`-basiertem Parser fuer
    Spec §9.4.1.3 (`<grant>` → `<allow_rule>` →
    `<publish>`/`<subscribe>` → `<topic>`), tolerant gegen die drei
    bekannten Vendor-Hierarchien (Cyclone/Fast-DDS/Connext).
    Wildcard-Matcher (`*`, `?`) in `topic_match`.
    `PermissionsAccessControl` implementiert `AccessControlPlugin`;
    `check_create_datawriter/reader` dispatchen auf Grant-Lookup per
    Subject-Name. Default-Deny wenn kein `<default>` gesetzt.
    17 Tests gruen (Parser + Plugin + Wildcard-Matcher).
    `PermissionsHandle` bekam `PartialOrd, Ord`.
  * WP 4.2-b — XML-Signatur-Skeleton — **implementiert (2026-04-24)**:
    Neues Modul `security-permissions/src/signature.rs` mit dem
    `XmlSignatureVerifier`-Trait + `open_signed_permissions(doc,
    verifier)`-High-Level-Flow. Zwei Impls:
    `NoOpVerifier` (Dev-only, akzeptiert alles) + `EnvelopeCheckVerifier`
    (BEGIN/END-Wrapper-Check fuer End-to-End-Integrationstests).
    Echter PKCS#7/CMS-Backend kommt in WP 4.2-b+ (braucht
    `rsa` + `x509-parser`-Dep). 6 Tests: noop-passthrough,
    envelope-extract, missing-begin/missing-end-reject,
    verifier-fail-propagates, non-utf8-inner-reject.
  * WP 4.2-c — Governance-XML-Parser — **implementiert (2026-04-24)**:
    Neues Modul `governance.rs` mit `parse_governance_xml()` fuer das
    komplette Spec-§9.4.1.2-Schema (`<domain_access_rules>` →
    `<domain_rule>` → `<topic_access_rules>` → `<topic_rule>`).
    `DomainFilter` mit `<id>` + `<id_range>min..max</id_range>`,
    `ProtectionKind` (None/Sign/Encrypt/+Origin-Auth),
    Topic-Expression-Matching via `topic_match` wiederverwendet.
    `Governance::find_topic_rule(domain, topic)` macht Domain+Topic-
    Lookup in einem Schritt. 7 neue Tests (24 gesamt, +7).
  * WP 4.2-d — Validity-Period — **implementiert (2026-04-24)**:
    `Validity { not_before, not_after }` + `Grant::is_valid_at(now)`.
    Eigener ISO-8601-Parser (Howard-Hinnant civil_from_days) — kein
    `time`-Crate noetig (MSRV 1.85 kompatibel). 5 neue Tests: default-
    unrestricted, epoch-parse, malformed-reject, window-enforcement,
    nur-not_after-set.
- WP 4.3 — Crypto: AES-GCM-128/256, HMAC-SHA256, RSA-2048 via `ring`/`rustls`
  * WP 4.3-a — AES-GCM-128 Crypto-Plugin — **implementiert (2026-04-23)**:
    Neuer Crate `zerodds-security-crypto` mit `AesGcmCryptoPlugin` auf
    Basis von `ring::aead::AES_128_GCM`. 12-byte-Nonce = 4 byte zufaelliger
    Session-ID + 8 byte monotoner Counter (DoS-Cap: Encrypt wird nach
    2^64 Calls abgelehnt). Wire-Format `[nonce(12) | ct | tag(16)]`.
    KeyFactory + KeyExchange + Transform alle bedient.
    6 Tests gruen: encrypt/decrypt-roundtrip, tamper-detection
    (`authentication_failed` bei geflipptem Byte), Nonce-Uniqueness,
    Cross-Plugin-Token-Exchange (Alice verschluesselt, Bob dekodiert
    via serialisiertem Key-Token), Short-Input-Reject,
    plugin_class_id = "DDS:Crypto:AES-GCM-GMAC" (auf `:1.2` geupgraded
    mit C3.3-Sub am 2026-04-25).
    `CryptoHandle` bekam `PartialOrd, Ord` fuer `BTreeMap`.
  * WP 4.3-b — AES-GCM-256 Transform-Kind — **implementiert
    (2026-04-24)**: Suite-Enum `zerodds_security_crypto::Suite { Aes128Gcm,
    Aes256Gcm }`. `AesGcmCryptoPlugin::with_suite(Suite::Aes256Gcm)`
    nutzt `ring::aead::AES_256_GCM` + 32-byte Master-Key. Token-Layout
    erweitert um 1-byte Suite-Tag (`[kind_id | session_id(4) |
    master_key(16|32)]`) → Cross-Suite-Interop (Alice 256 → Bob 128)
    funktioniert deterministisch. 6 neue Tests: default-ist-128,
    reports-256, aes256-roundtrip, aes256-tamper-rejected, cross-suite-
    token-exchange, reject-unknown-suite-id. 12 Tests gesamt (+6).
    HMAC-SHA256-only bleibt WP 4.3-c (wartet auf Governance mit
    `SIGN`-Kind).
  * WP 4.3-c — HMAC-SHA256-Suite + Key-Refresh —
    **implementiert (2026-04-24)**: Dritte Suite-Variante
    `Suite::HmacSha256` (Auth-only, 32-byte Master-Key). Wire-Format
    `[nonce(12) | plaintext | hmac-sha256(32)]` — plaintext bleibt
    sichtbar (Spec §9.5.1: `NONE + HMAC_SHA256`). Key-Refresh-API:
    `encrypts_remaining(handle)` + `rotate_key(handle)` — Caller
    monitort Counter vs. `Suite::max_encrypts()` (2^48) und rotiert
    bei 0. 6 neue Tests: hmac-roundtrip-plaintext, hmac-tampered-
    payload, hmac-tampered-tag, encrypts_remaining-decrement,
    rotate_key-resets-counter-and-changes-key, rotate-unknown-handle.
  * WP 4.3-c — **offen**: Automatischer Key-Refresh bei Nonce-
    Counter-Exhaust.
- WP 4.4 — Secure-Submessage-Layer (Encrypt+Auth wrapper um RTPS-Submessages)
  * WP 4.4-a — Wire-Format + Codec — **implementiert (2026-04-23)**:
    Neuer Crate `zerodds-security-rtps` mit den Submessage-IDs
    `SEC_PREFIX=0x31`, `SEC_BODY=0x30`, `SEC_POSTFIX=0x32`,
    `SRTPS_PREFIX=0x33`, `SRTPS_POSTFIX=0x34` gemaess Spec §7.3.6.
    `encode_secured_submessage` / `decode_secured_submessage` nehmen
    `&dyn CryptographicPlugin` und kapseln plain-Submessage-Bytes
    als `SEC_PREFIX + SEC_BODY + SEC_POSTFIX`-Sequenz mit LE-Header.
    7 Tests gruen: encode-produces-three-submessages, roundtrip,
    tamper-detection (byte-flip im ct → CryptoFailed), wrong-PREFIX-
    ID-rejected, big-endian-flag-rejected, truncated-input-rejected,
    constants-match-spec.
  * WP 4.4-b — RTPS-Integration — **teilweise (2026-04-24)**:
    * **4.4-b.1 implementiert**: Neuer Crate `zerodds-security-runtime`
      mit `SecurityGate<P>` (generisch ueber `CryptographicPlugin`).
      Der Gate kombiniert Governance-XML + Crypto-Plugin +
      `security-rtps`-Codec zu einer einfachen
      `encode_outbound(topic, bytes)` /
      `decode_inbound(topic, wire)`-API.
      Policy-Enforcement: Inbound-Plaintext auf `Encrypt`-Topic wird
      als `PolicyViolation` abgelehnt. 7 Tests + Doctest (protection-
      lookup, passthrough-none, encrypt-wraps-sec_prefix, roundtrip,
      policy-violation, plain-on-unprotected-topic, missing-domain-
      rule-defaults).
    * **4.4-b.2 implementiert (2026-04-24)**: Message-Level-API am
      Gate (`encode_outbound_message`, `decode_inbound_message` gegen
      `rtps_protection_kind`), Token-Austausch (`local_token`,
      `set_remote_token`, `register_remote`). **Cross-Participant-
      E2E-Test** beweist: Alice + Bob unterschiedliche Plugin-
      Instanzen tauschen Crypto-Tokens, Alice encrypted, Bob
      decrypted byte-identisch zurueck. 5 neue Tests
      (message_protection-lookup, none-passthrough, encrypt-
      wraps-after-header, plain-inbound-policy-violation,
      e2e-cross-participant-roundtrip). 12 Tests + Doctest gesamt.
    * **4.4-b.3 teilweise (2026-04-24)**: Thread-safer
      `SharedSecurityGate` mit `Arc<Mutex<...>>`-basierter
      Kapselung. `Box<dyn CryptographicPlugin>` wird geownt; Clone
      liefert eine weitere Handle auf die gleiche Plugin-Instance.
      API: `transform_outbound(msg)`, `transform_inbound(slot, wire)`,
      `register_remote_with_token(ident, secret, token)`.
      6 Tests inkl. **concurrent-transform-thread-safety** (8 Threads
      parallel, kein Poisoning) und **clone-shares-same-plugin**.
    * **4.4-b.4 implementiert (2026-04-24)**: Hot-Path-Injection in
      `zerodds-dcps::runtime`. Neues Cargo-Feature `security`; mit Feature
      ist `RuntimeConfig::security: Option<Arc<SharedSecurityGate>>`
      vorhanden. Alle 6 UDP-Send/Recv-Entry-Points umgestellt auf die
      Helper `secure_outbound_bytes(rt, bytes)` und
      `secure_inbound_bytes(rt, bytes)` — Peer-Key (12 byte) aus
      RTPS-Header Bytes 8..20 automatisch extrahiert.
      Policy-Violations und Tampering droppen das Paket silently.
      22 dcps-Tests gruen in beiden Varianten (ohne + mit Feature);
      keine Regression im bestehenden Behavior. `SharedSecurityGate`
      bekam `Debug`-Impl (ohne Key-Leak — nur Metadaten).
    * **4.4-b.3b implementiert (2026-04-24)**: Peer-Key-Mapping im
      `SharedSecurityGate`. `PeerKey = [u8; 12]` (passt auf
      `GuidPrefix`, zerodds-rtps-Dep bleibt optional). Neue API:
      `register_remote_by_guid(peer_key, ...)` (idempotent),
      `forget_remote(peer_key)`, `slot_for(peer_key)`,
      `transform_inbound_from(peer_key, wire)` — letzterer mappt
      GuidPrefix-Sender → Slot automatisch. Unknown-Peer auf
      SRTPS-wire → `PolicyViolation`. 6 neue Tests (24 gesamt):
      idempotent-register, guid-lookup-roundtrip, unknown-peer-reject,
      multi-peer-routing (3 Participants), wrong-prefix-tag-fail,
      forget-remote.
  * WP 4.4-c — SRTPS-Message-Wrapper — **implementiert (2026-04-24)**:
    Neues Modul `security-rtps/src/srtps.rs` mit
    `encode_secured_rtps_message` / `decode_secured_rtps_message`.
    Wire: `[header(20) | SRTPS_PREFIX(4+16) | SEC_BODY(4+ct) |
    SRTPS_POSTFIX(4)]` — erster 20-byte RTPS-Header bleibt plaintext
    (Magic/Version/VendorId/Prefix), alles dahinter wird via AES-GCM
    geschuetzt. 7 Tests: header-plaintext, body-nicht-im-wire,
    roundtrip, too-short-reject, tampered-ct-reject,
    missing-SRTPS_PREFIX, big-endian-reject.
  * WP 4.4-d — **offen**: Receiver-Specific-MACs (ein MAC pro Remote-
    Reader).
- WP 4.5 — Key-Management: Shared-Secret-Exchange ueber PKI-Handshake
  * WP 4.5-a — X25519 Ephemeral-DH + HKDF-SHA256 —
    **implementiert (2026-04-23)**: Neuer Crate `zerodds-security-keyexchange`
    mit `KeyExchange`-Struct. `ring::agreement::X25519` +
    `ring::hkdf::HKDF_SHA256` mit domain-separation-Info
    `"DDS:Auth:PKI-DH:secret"`. PFS durch `EphemeralPrivateKey`
    (ring API erlaubt nur einen `agree_ephemeral`-Call pro Instance).
    6 Tests gruen: public-key 32 byte, alice+bob identisches secret,
    different ephemerals → different secrets, wrong-length-reject,
    all-zero-pubkey reject (Small-Subgroup-Defense), +Doctest.
  * WP 4.5-b — RSA-Key-Wrap-Skeleton — **implementiert (2026-04-24)**:
    `RsaKeyWrap::from_public_key_der(der)` + `wrap_secret(&[u8; 32])`
    fuer Legacy-Interop (RTI-Connext ohne ECDH). Aktuell Placeholder —
    ring 0.17 exponiert kein RSA-Encrypt; die Produktions-OAEP-SHA256-
    Encryption kommt via `rsa`-Crate oder `rustls`-Backend in WP 4.5-b+.
    Call-Pfad-Tests (5) belegen die API-Signatur (length-check, empty-
    key-reject, fresh-mask-per-call, output-size).
  * WP 4.5-c — P-256 ECDH — **implementiert (2026-04-24)**:
    `KxSuite::{X25519, EcdhP256}`, `KeyExchange::with_suite(suite)`.
    P-256 liefert 65-byte unkomprimierten Public-Key (0x04 || X || Y).
    Beide Suiten nutzen gleiches HKDF-SHA256-Output (32 byte). 6 neue
    Tests: default-x25519, p256-pubkey-size, p256-identical-secret,
    p256-wrong-length-reject, p256-off-curve-reject, cross-suite-
    key-lengths.
- WP 4.6 — Security 1.2 Delta (Logging + Tagging)
  * WP 4.6-a — Produktions-Logging-Backends — **implementiert
    (2026-04-24)**: Neuer Crate `zerodds-security-logging` mit drei
    Plugin-Impls:
    * `StderrLoggingPlugin` — Human-Readable `[SEC][LEVEL] participant=
      <hex16> category=... msg=...` an stderr, Mutex-serialisiert
      gegen inter-threading.
    * `JsonLinesLoggingPlugin::open(path, min_level)` — audit-taugliche
      JSON-Lines in Datei, BufWriter + flush pro Event (kein Crash-
      Loss). Eigenes Escape (`"`, `\`, `\n`, `\r`, `\t`, Control-Chars
      → `\uXXXX`).
    * `FanOutLoggingPlugin` — broadcast an mehrere Sinks (stderr +
      jsonl gleichzeitig).
    * Level-Filter pro Sink (Default `Warning`); `level > min_level`
      → silently dropped.
    * 10 Tests: plugin_class_id-Stabilitaet aller drei, level-label
      all variants, hex16-padding, json-threshold, json-escape-quotes,
      json-escape-newline, fanout-empty, fanout-all-sinks.
  * WP 4.6-b — Syslog-RFC-5424-UDP-Backend — **implementiert
    (2026-04-24)**: `SyslogLoggingPlugin::connect(target, app, host,
    min_level)`. Facility fix auf `LOCAL0` (16), Severity aus
    `LogLevel` 0..7. Nachrichten-Format gemaess RFC 5424
    (`<PRI>1 - HOST APP - CAT - participant=<hex16> <msg>`).
    CR/LF im MSG werden ersetzt (keine zerrissenen Collector-Zeilen).
    6 Tests: priority-formula, rfc5424-shape, newline-escape,
    udp-roundtrip (ephemere Ports), below-threshold-drop,
    plugin_class_id. OTLP-Telemetry bleibt WP 4.6-c.

**Heterogeneous-Security-Track (WP 4H) — System-of-Systems:**

Die v1.4-Security-Plugins (WP 4.1–4.6) plus Hot-Path-Injection
(WP 4.4-b.4) liefern einen **participant-globalen** Security-Layer:
ein Level fuer alles, alle Peers gleich behandelt. Fuer Vehicle-
Networks, Tactical Mesh und Industrie-Edge-Gateways brauchen wir
**Per-Peer-Policy auf einem Interface** — Legacy-ECU ohne Cert
neben Secure-Peer mit AES-256 + OCSP, simultan.

Detaillierter Stufenplan: [`wp-4H-heterogeneous-security-plan.md`](wp-4H-heterogeneous-security-plan.md).
Architektur: [`docs/architecture/08_heterogeneous_security.md`](../architecture/08_heterogeneous_security.md).

- WP 4H-a — `PolicyEngine`-Trait + `PeerCapabilities` + `NetInterface`-
  Klassifikator. Default-Impl `GovernancePolicyEngine` spiegelt
  v1.4-Verhalten byte-identisch. (**Geschaetzt: 1-2 Tage**)
- WP 4H-b — SPDP-Capability-Advertisement: `auth_plugin_class`,
  `supported_suites`, `offered_protection` als Properties propagieren.
  Parser fuellt `PeerCache`. (**1-2 Tage**)
- WP 4H-c — SEDP-Endpoint-Security-Info (Spec §9.4.2.4 `PID_ENDPOINT_
  SECURITY_INFO`). Matching-Logic respektiert den Pro-Endpoint-
  Protection-Kind. (**1 Tag**)
- WP 4H-d — Writer-Side Per-Reader-Serializer: statt Broadcast-Send
  iteriert der Writer-Tick pro matched Reader und serialisiert mit der
  jeweiligen Protection. Homogener Fall bleibt Single-Send (kein
  Performance-Drop). (**3-4 Tage**)
- WP 4H-e — Reader-Side Per-Writer-Validator: inbound-Policy-Decision
  pro Paket aus `(source_guid, interface, is_sec_prefixed)`. Policy-
  Violations droppen + loggen. (**1-2 Tage**)
- WP 4H-f — Interface-Routing: `RuntimeConfig::interfaces` mit Multi-
  Socket-Binding + Routing-Tabelle (Locator → Interface). (**2-3 Tage**)
- WP 4H-g — Receiver-Specific-MACs (Spec §7.3.6.3): ein Ciphertext +
  N MACs fuer homogene-Suite-multi-Reader-Case. (**2 Tage**)
- WP 4H-h — Governance-XML `<zerodds:peer_classes>` +
  `<zerodds:interface_bindings>`-Namespace-Extension, parsed und auf
  `PolicyEngine`-Input gemappt. (**2 Tage**)
- WP 4H-i — PKI↔Crypto-Integration: `SharedSecretProvider`-Trait als
  Bruecke zwischen Authentication-Handshake und CryptoPlugin, damit
  per-peer Master-Keys aus DH-Shared-Secrets via HKDF abgeleitet
  werden statt aus Random-Token-Exchange. (**1 Tag**, Nachtrag aus
  Review — schliesst Lucke im Crypto-Trust-Modell.)

**Gesamt-Umfang WP 4H (a–i):** ~4 Wochen, 9 Sub-WPs, ≥80 neue Tests,
Ziel Branch-Coverage ≥99% auf `zerodds-security-runtime`.

**Definition-of-Done Gesamt-Track (a–i):**
* 4 Runtimes (Legacy/Fast/Secure/HA) auf gleicher Domain tauschen
  Samples korrekt aus, Wire-Bytes beweisen Heterogenitaet.
* Vendor-Interop-Test: Cyclone ignoriert `zerodds:`-Namespace, nutzt
  OMG-Fallback; kein Break bei Standard-Policy.
* Performance-Check: homogene Policy bleibt im Median binnen ±5% des
  v1.4-Baselines (Fan-Out nur bei Heterogenitaet).
* Handshake → Crypto-Integration: Multi-MAC mit echten DH-MAC-Keys
  (nicht Token-Exchange) in `pki_crypto_integration`-E2E-Test.

**WP 4H-j — Delegation-Track (Gateway/Bridge-Identity):**
Separat budgetiert, startet nach Abschluss von WP 4H-a..i.

Detaillierter Stufenplan: [`wp-4H-j-delegation-plan.md`](wp-4H-j-delegation-plan.md).
Architektur: [`docs/architecture/09_delegation.md`](../architecture/09_delegation.md).

- WP 4H-j-a — `DelegationLink` + `DelegationChain` + Sign/Verify mit
  4 Signature-Algorithms (ECDSA-P-256 default, ECDSA-P-384, RSA-PSS,
  Ed25519) in `security-pki`. (**1–2 Tage**)
- WP 4H-j-b — Chain-Validation mit 7-Punkte-Check + Scope-
  Intersection + 4 Trust-Policy-Modi (gateway-only,
  direct-or-delegated, federation, strict-delegated) in
  `security-permissions`. (**1 Tag**)
- WP 4H-j-c — SPDP-Propagation der Chain als
  `zerodds.sec.delegation_chain`-Property (base64-blob, DoS-Cap 8 KiB).
  (**1–2 Tage**)
- WP 4H-j-d — PeerClassMatch-Extensions (`delegation_profile`-Referenz
  auf benannte Profiles) + Integration in `GovernancePolicyEngine::
  accept_peer`. (**1 Tag**)
- WP 4H-j-e — `GatewayBridge`-Helper: delegate_for / revoke_delegation
  / chain_for; Sub-Gateway-Chaining (Wanne-GW mit Upstream-Link von
  Turm-GW) produziert korrekt n+1-Hop-Chain. (**2 Tage**)
- WP 4H-j-f — Static + Ephemeral Edge-Identity-Config in Governance-
  XML (`<zerodds:edge_identities>`); `rotate_ephemerals` triggert
  Chain-Refresh. (**1 Tag**)
- WP 4H-j-g — E2E-Test Doppelstern: Wanne-GW + Turm-GW + 2 Turm-
  Sensoren + 1 Wanne-ECU + 1 C4I-Node; Wire-Byte-Validation von
  2-Hop-Chain; Cyclone-Interop-Smoke. (**2 Tage**)
- WP 4H-j-h — Governance-XML Hybrid-Profile-Parser
  (`<zerodds:delegation_profiles>` Named Types + `delegation_profile`-
  IDREF aus Peer-Class); Unreferenced-Profile-Warnings. (**1 Tag**)

**Gesamt-Umfang WP 4H-j:** ~10 Arbeitstage, 8 Sub-WPs, ~2100 LOC,
≥60 neue Tests.

**Definition-of-Done WP 4H-j:**
* Doppelstern-Fahrzeug (Wanne+Turm) mit 5+ Edge-Peers + C4I-Node:
  2-Hop-Delegation byte-identisch auf dem Wire nachweisbar.
* Trust-Policy-Modi (gateway-only, direct-or-delegated, federation,
  strict-delegated) alle durch Unit-Tests abgedeckt; ein
  Runtime-Parameter schaltet zwischen ihnen um.
* Governance-Profiles mit Sign-Algorithm-Mix (ECDSA-P-256 / RSA-PSS)
  gleichzeitig aktiv, je nach Profile-Referenz.
* Cyclone-Smoke-Test: Wanne-GW als normaler Participant sichtbar,
  `zerodds:delegation_chain`-Property still ignoriert, kein
  Interop-Bruch.

**Binding-Track:**
- WP 4.7 — C-Header-Generator (cbindgen + handgeschriebener Guard-Wrapper)
- WP 4.8 — C-Binding Pub/Sub/Topic
- WP 4.9 — C-Binding QoS-Handle-Abstraktion
- WP 4.10 — C++-Binding on-top-of-C (modern C++17, RAII)
- WP 4.11 — async-API (tokio-basiert, Option<Runtime>-Pattern)

**Persistence-Track:** ✅ **GELIEFERT auf main (2026-06-10, [ADR 0009](../adr/0009-durability-service.md))** — vorgezogen, realisiert als adapter-getriebener Daemon (`crates/durability-service` + `durability-store{,-file,-sqlite,-lakehouse}`). TRANSIENT + PERSISTENT, RTPS-Ingest/Replay, Crash-Recovery (P4), Cross-Vendor-Ingest gegen Cyclone wire-bewiesen. Die WP-Zeilen unten = historischer Plan (Storage-Backend wurde Trait-Adapter statt „SQLite+Sled", Discovery via DCPSDurability statt special-Subscriber).
- WP 4.12 — Persistence-Service Binary (`zerodds-persistence`)
- WP 4.13 — Storage-Backend: SQLite + Sled-DB (pluggable)
- WP 4.14 — Service-Discovery: registriert sich als special-Subscriber auf Transient-Topics
- WP 4.15 — Replay-Logic: on-match-liefert-History-Cache
- WP 4.16 — Transient-Durability Wire-Semantik (Spec §7.1.3.4)

**ROS2-Track:**
- WP 4.17 — `rmw_zerodds` Skeleton (rmw-API Trait-Impl via C-Binding)
- WP 4.18 — rmw-Entity-Mapping (ROS2-Nodes → DDS-Participants, Topics → Topics)
- WP 4.19 — ROS2-QoS-Profile-Mapping (sensor_data, reliable, ...)
- WP 4.20 — `test_rmw_implementation` Suite durchlaufen

**SHM-Track:**
- WP 4.21 — Zero-Copy Buffer-Layout (FlatData-Pattern)
- WP 4.22 — `idlc` Code-Gen: emit `FlatStruct` fuer FlatData-markierte Typen
- WP 4.23 — Writer legt direkt in SHM-Block, Reader liest zero-copy

### 1.4.4 Test-Strategie
| Ebene | Coverage |
|-------|----------|
| Unit | jede Security-Primitive gegen NIST-Test-Vektoren |
| Integration | C-Binding Roundtrip, C++-Binding Roundtrip, async-Roundtrip |
| Security | Full test-vector suite aus DDS-Security-1.1-Interop-Sample, Pen-Test-Subset |
| Interop | ZeroDDS-Secure ↔ Cyclone-Secure, ZeroDDS-Secure ↔ Connext-Eval |
| ROS2 | `test_communication` + `test_rmw_implementation` |
| Persistence | Chaos-Test: writer crashes, service restart, late-joiner gets history |
| Performance | Secure Path Overhead < 15 % gegenüber Unsecure (baseline) |
| Fuzz | Security-Handshake-Messages, Permissions-XML |

### 1.4.5 Analyse-Deliverables
- `docs/security/security-compliance-report.md` — Mapping Spec §7 ↔ Impl
- `docs/security/threat-model.md` — STRIDE-Analyse fuer Authentication + Crypto
- `docs/bindings/c-api-stability-guide.md` — API-Versioning-Policy
- `docs/ros2/rmw-compatibility-matrix.md` — Humble/Jazzy/Rolling
- `docs/persistence/daemon-architecture.md` — Deployment + HA-Vorbereitung

### 1.4.6 Review-Gates
- **Security-Architektur-Review** durch externen Auditor (Pflicht bei
  jedem Security-verwandten Release).
- **ABI-Stability-Review** fuer C-Binding — einmal veroeffentlicht,
  nicht breaking!
- **rmw-Review** mit ROS2-TSC-Kontakt, feedback vor Release.
- **Persistence-Reliability-Review** — Chaos-Test-Results durch 2.
  Ingenieur.

### 1.4.7 Definition-of-Done
- [ ] Alle DDS-Security-1.1-Spec-Kapitel als Compliance-Checkbox
- [ ] Cyclone-Secure-Interop bidirektional grün
- [ ] C-Binding: hello-world in C compilet gegen stabiles `.h`
- [ ] C++-Binding: hello-world in modernem C++17
- [ ] `rmw_zerodds` besteht `test_rmw_implementation`-Suite vollstaendig
- [ ] Persistence-Service uebersteht Chaos-Test (kill -9 + restart, Daten bleiben)
- [ ] Transient-Durability: late joiner bekommt historic samples via Service
- [ ] Zero-Copy-SHM: measurable 0 allocations auf dem hot-path (perf-record + heaptrack)
- [ ] Branch-Coverage ≥ 99 % (trotz C-Binding-Ausnahme die sind getestet aber nicht lcov-instrumentiert)
- [ ] Score: **34 / 40**

### 1.4.8 Risiken
- **Security-Spec-Interop mit Cyclone** — DDS-Security-1.1 hat
  bekannte Inter-Vendor-Issues (siehe omg-zerodds-rtps Report). Plan-B:
  dokumentierte Abweichungen + Bug-Reports, nicht Release-Blocker.
- **C-Binding-ABI-Verlockung zu Refactor** — bricht Kunden. Plan-B:
  bis v2.0 "soft-frozen" mit semver-major-break, ab v2.0 API-frozen.
- **rmw-Test-Suite-Incompatibility** — die ROS2-TSC-RMW-Tests sind
  gegen Fast-DDS + Cyclone geschrieben, testen manchmal Vendor-
  Specifika. Plan-B: vendor-skip-list dokumentieren.
- **Persistence-Service-SPOF** — HA ist v2.0. Fuer 1.4: dokumentiert
  als "single-node", SRE-Guide wie man restart macht.

### 1.4.9 Metriken
- Score: 34 / 40
- Secure-Path-Overhead: < 15 % Latency, < 10 % Throughput Regression
- ROS2 `test_rmw_implementation`: ≥ 95 % passing
- Persistence: uptime ≥ 99.9 % in 7-Tage-Chaos-Test

---

## v1.5 — Connext-Parität Teil 1 (Q2 2027, ~12-14 Wochen)

### 1.5.1 Mission
"Die letzten Vendor-Pro-Features: Persistent-Durability mit Disk,
TSN für deterministische Automotive/Industrial-Netze, DDS-RPC für
service-oriented Architekturen. Plus: Java/C# für Enterprise-Java-
und MS-Ökosysteme."

### 1.5.2 Feature-Scope
- **Persistent Durability** (mit Disk-Backend im Persistence-Service)
- **DDS-TSN** (OMG 2023) — Mapping ueber `tc qdisc` + IEEE 802.1Qbv
- **DDS-RPC** — Request/Reply-Pattern ueber DDS
- **Java-Binding** (Pure-Java DDS-Java-PSM, kein JNI; `crates/java-omgdds/java/`)
- **C#-Binding** (csbindgen)

### 1.5.3 Arbeitspakete

**Persistence-Track:**
- WP 5.1 — Disk-Backend: Sled-DB mit Snapshots + WAL
- WP 5.2 — Durability-Service-Startup-Sync (Service liest Disk, re-announced cached Samples)
- WP 5.3 — Housekeeping: Prune + Compact
- WP 5.4 — Backup/Restore-CLI

**TSN-Track:**
- WP 5.5 — Linux-TSN-Integration via `tc qdisc` + `socket(SOCK_DGRAM, ..., IPPROTO_UDP)` mit `SO_PRIORITY`
- WP 5.6 — IEEE 802.1Qbv Gate-Control-List Setup im Participant-Startup
- WP 5.7 — PTP-Clock-Sync fuer deterministische Timestamps
- WP 5.8 — QoS-Mapping: Deadline → TSN-Priority + Gate-Window

**RPC-Track:**
- WP 5.9 — DDS-RPC-Service-Discovery ueber SEDP-Erweiterung
- WP 5.10 — Request/Reply-Correlation (sample_identity + request_id)
- WP 5.11 — Service-Interface-Generator im `idlc` (IDL service → Rust trait)
- WP 5.12 — Timeout/Cancellation-Semantik

**Binding-Track:**
- WP 5.13 — Pure-Java DDS-Java-PSM Skeleton (`org.omg.dds.*` Maven-Projekt)
- WP 5.14 — Java-Type-Mapping (Record-Classes, seit Java 17 LTS)
- WP 5.15 — C#-Binding via `csbindgen` + NuGet-Package
- WP 5.16 — Examples + Docs pro Sprache

### 1.5.4 Test-Strategie
| Ebene | Coverage |
|-------|----------|
| Unit | Disk-Backend-Roundtrip, Snapshot-Restore, RPC-Correlation |
| Integration | Persistent-Durability ueber Service-Restart, TSN-Gate-Compliance |
| Hardware | TSN-Tests auf Intel i210/i225 NIC mit PTP-Hardware (Linux-Host) |
| Interop | Connext-TSN ↔ ZeroDDS-TSN Wire-Compliance (falls Connext-Eval verfuegbar) |
| RPC | Request/Reply-Latency + Timeout-Behavior + Cancellation |
| Java/C# | Hello-World + ROS2-Python-Pattern-Übertragung |

### 1.5.5 Analyse-Deliverables
- `docs/tsn/tsn-deployment-guide.md` — welche NICs, welche Kernel-Versionen, PTP-Setup
- `docs/rpc/rpc-design.md` — Interface-IDL → generated code pattern
- `docs/persistence/disk-storage-layout.md` — Sled-DB-Schema, Upgrade-Pfad
- `docs/bindings/java-csharp-parity.md` — Gap-Analyse zu Connext

### 1.5.6 Review-Gates
- **TSN-Integration-Review** durch Automotive-Kunden-Kontakt
  (ist der Code produktionstauglich auf Zephyr-Linux-Gateway?).
- **Binding-API-Review** fuer Java/C# — ergonomisch fuer Java- bzw.
  .NET-Entwickler, nicht ein C-Transkript.
- **Persistence-Durability-Review** — Recovery-Tests durch
  separaten Ingenieur.

### 1.5.7 Definition-of-Done
- [ ] Persistent Durability: Daten ueberleben Node-Crash + Reboot + Service-Restart
- [ ] TSN: Deadline-QoS uebersetzt in hardware-enforced Gate-Control-List
- [ ] RPC: einfaches "Add(a,b) → a+b" Beispiel ueber IDL-generiertes Interface
- [ ] Java-Binding: hello-world aus Gradle-Projekt
- [ ] C#-Binding: hello-world aus .NET-8-Projekt
- [ ] Score: **38 / 40**

### 1.5.8 Risiken
- **TSN-Hardware-Verfuegbarkeit** — Entwicklung braucht echte TSN-NICs.
  Plan-B: sim-Mode mit `tc netem` als Dev-Fallback, Hardware-Gate
  erst im CI/Customer-Lab.
- **RPC-Spec-Interop** — DDS-RPC-Spec ist weniger ausgereift als DCPS.
  Plan-B: Connext-kompatible Subset, RTI-spezifische Extensions
  optional.
- **Pure-Java-XCDR2-Performance** — Reflection-Marshalling kostet
  vs. handgeschriebene Codecs. Plan-B: idl-java generiert pro
  Topic-Type spezialisierte XCDR2-Methoden statt Reflection-Pfad.

### 1.5.9 Metriken
- Score: 38 / 40
- RPC-Latency: Request/Reply median < 500 µs im Loopback
- TSN-Gate-Compliance: Messung mit Oszilloskop gegen Gate-Schedule ±10 µs
- Persistent-Recovery-Time: < 5 s fuer 10k cached samples

---

## v2.0 — Full RTI-Parität + strategische Extras (Q3 + Q4 2027, ~20-24 Wochen)

### 2.0.1 Mission
"Full RTI-Connext-Feature-Parität (außer Sicherheits-Zertifikat, das
kommt als Partnerschafts-Track). Plus: Bridges zu fremden Protokollen
(Zenoh/MQTT/Kafka/OPC UA), HA-Persistence, Security 1.2 mit HSM,
FACE-Konformität. Plus: schneller als RTI bei OMG-Spec-Revisionen."

### 2.0.2 Feature-Scope

**Core-Vervollstaendigung:**
- **DDS-Security 1.2 vollstaendig** inkl. PKCS#11-HSM-Integration
- **HA-Persistence** — Replication zwischen mehreren Service-Instanzen, automatischer Failover
- **FACE-Konformitaet** (Future Airborne Capability Environment)
  — Avionics-Profile des DDS-Stacks

**Bridges (Multi-Vendor-Interop):**
- **Zenoh ↔ DDS** — ZettaScales neues Protokoll; offensive + defensive Positionierung
- **MQTT ↔ DDS** — IoT-Integration, Broker-basierte Landschaften
- **Kafka ↔ DDS** — Streaming-Analytics, Data-Lake-Ingest
- **OPC UA ↔ DDS** — Industrial-Automation-Standard

**OMG-Early-Access-Channel:**
- Feature-Flag-geschuetzter Code-Track fuer unfinalisierte Proposals
- Beispiele: DDS-XTypes 2.0 (wenn in OMG-Review), DDSI-RTPS 2.6 Updates

### 2.0.3 Arbeitspakete

**Security-Track:**
- WP 6.1 — PKCS#11-HSM-Backend fuer Private-Keys
- WP 6.2 — Full-Security-1.2-Delta-Implementation
- WP 6.3 — FIPS-140-3-Validation-Pfad (via `rustls-fips`)

**HA-Persistence-Track:**
- WP 6.4 — Raft-Replication zwischen Service-Instanzen (oder gossip-pattern)
- WP 6.5 — Automatic-Failover mit Leader-Election
- WP 6.6 — Split-Brain-Protection

**Bridge-Framework:**
- WP 6.7 — Generic-Bridge-Plugin-API
- WP 6.8 — Zenoh-Bridge (zenoh-rust-SDK)
- WP 6.9 — MQTT-Bridge (rumqttc)
- WP 6.10 — Kafka-Bridge (rdkafka)
- WP 6.11 — OPC UA-Bridge (open62541-sys)

**FACE-Track:**
- WP 6.12 — FACE-TSS-Interface-Layer (Transport Services Segment)
- WP 6.13 — FACE-IOSS-Compliance (I/O Services Segment)
- WP 6.14 — FACE-spezifische XML-Konfigurationsformate

**OMG-Early-Access:**
- WP 6.15 — Feature-Flag-Infrastruktur + Release-Kanal `zerodds-next`
- WP 6.16 — Spec-Tracking-Prozess als internes Playbook

### 2.0.4 Test-Strategie
| Ebene | Coverage |
|-------|----------|
| Security | HSM-Mock-Tests + echte HSM-Tests mit SoftHSM2, FIPS-self-tests |
| HA | Chaos-Test: 3-Node-Cluster mit rolling failures, network partitions, clock skew |
| Bridges | End-to-End von je 1 foreign protocol zu DDS und zurueck, mit Encoding-Roundtrip-Checks |
| FACE | Subset-of-FACE-TSS-Conformance-Suite (falls zugänglich) |
| Interop | ZeroDDS-Full-Stack gegen RTI-Connext-Eval (alle QoS + Security 1.2) |
| Langzeit | 30-Tage-Uptime-Test des HA-Cluster im Kunden-Lab |

### 2.0.5 Analyse-Deliverables
- `docs/security/security-1.2-compliance.md` — full compliance Matrix
- `docs/ha/persistence-ha-guide.md` — Deployment + Failure-Modes
- `docs/bridges/integration-guide.md` — pro Bridge, mit typischen Use-Cases
- `docs/face/face-profile.md` — Avionics-Subset + nicht-unterstuetzte Features
- `docs/omg-tracking/spec-proposal-playbook.md` — interner Prozess

### 2.0.6 Review-Gates
- **Security-Audit durch externes Pen-Test-Unternehmen** — Pflicht
  fuer v2.0.
- **HA-Reliability-Review** durch SRE-externen Consultant.
- **Bridge-Protocol-Compliance-Review** pro Bridge durch einen
  Stakeholder aus dem jeweiligen Protocol-Ecosystem.
- **FACE-Compliance-Pre-Audit** durch einen FACE-erfahrenen
  Engineer.
- **Customer-Acceptance-Test** mit dem ersten Produktkunden — sein
  tatsaechliches Setup wird als finaler Gate gefahren.

### 2.0.7 Definition-of-Done
- [ ] RTI-Connext-Interop (alle 22 QoS + Security 1.2) grün
- [ ] HA-Persistence: 3-Node-Cluster uebersteht rolling-restarts + netpart
- [ ] Alle 4 Bridges: message-Roundtrip pro Protokoll
- [ ] FACE-TSS-Conformance-Subset
- [ ] Security-Pen-Test: kein Critical/High offen
- [ ] OMG-Early-Access-Channel hat mindestens 1 Preview-Feature im Release
- [ ] Score: **40 / 40** Core + alle strategischen Extras
- [ ] Kunde-1 Produktions-Abnahme-Test grün

### 2.0.8 Risiken
- **RTI-Connext-Eval-Lizenz** — fuer Vollinterop-Tests brauchen wir
  eine. Plan-B: commercial-eval oder Partner-Access. Worst-case:
  auf RTI-Shapes-Demo und Connext-DDS-Spy reduzieren.
- **FIPS-140-3-Certification** — teuer und langsam. Plan-B: FIPS-
  ready-architektur jetzt, formale Zertifikat spaeter.
- **HA-Split-Brain-in-Production** — CAP-Theorem trifft uns.
  Plan-B: CP-default-mode mit explicit-opt-in-AP-mode; klare
  Dokumentation was passiert.
- **Bridge-Maintenance-Burden** — jede Bridge ist eigenes Ecosystem.
  Plan-B: Plugin-Architektur, damit Bridges ausgelagert werden
  koennen in eigene Repos.

### 2.0.9 Metriken
- Score: 40 / 40 + Bridges + HA + OMG-Early-Access
- Latency-Budget: keine Regression > 10 % gegenüber v1.2-Baseline
- Security-Pen-Test-Score: 0 Critical, 0 High
- Kunde-1-Produktions-Uptime: ≥ 99.95 % im 3-Monats-Pilot

---

## Cross-Cutting — gilt in jeder Stufe

### Code-Quality
- **99 % Branch-Coverage** bleibt Pflicht. Ausnahmen nur mit
  dokumentierter Begründung im Safety-Waiver.
- **No `unsafe`** ausser in FFI-Schicht (C/C++/Java/C# Bindings) —
  und dort mit `// SAFETY:`-Kommentar pro Block.
- **`cargo clippy -- -D warnings`** in CI, no-exception.
- **`cargo deny`** fuer License + Advisory-Checks.

### CI-Struktur (wird pro Release ausgebaut)
| Job | v1.3 | v1.4 | v1.5 | v2.0 |
|-----|:---:|:---:|:---:|:---:|
| Unit Tests | ✓ | ✓ | ✓ | ✓ |
| Integration Tests | ✓ | ✓ | ✓ | ✓ |
| Cyclone-Interop | ✓ | ✓ | ✓ | ✓ |
| Fast-DDS-Interop | ✓ | ✓ | ✓ | ✓ |
| ROS2 test_rmw | — | ✓ | ✓ | ✓ |
| RTI-Eval-Interop | — | — | (optional) | ✓ |
| Security-Test-Vektoren | — | ✓ | ✓ | ✓ |
| Chaos (Persistence) | — | ✓ | ✓ | ✓ |
| TSN-Hardware-Lab | — | — | ✓ | ✓ |
| Fuzz Nightly | ✓ | ✓ | ✓ | ✓ |

### Dokumentation pro Release
- **Release-Notes** mit Breaking-Changes + Migration-Guide
- **API-Diff** (cargo-public-api oder Aequivalent)
- **Interop-Report** gegen alle getesteten Vendor-Versionen
- **Performance-Report** gegen baseline (llvm-host)
- **Security-Audit-Summary** (ab v1.3)
- **Known-Issues** transparent, mit Workaround-Pfaden

### Release-Kadenz
- **Minor-Releases:** jeden Monat zwischen Feature-Releases
  (Bugfixes, kleine Ergänzungen, keine Breaking-Changes).
- **Feature-Releases:** v1.3, v1.4-Part-A, v1.4-Part-B, v1.5, v2.0
  in den oben genannten Quartalen.
- **LTS-Branches:** ab v1.4 (erste Kunden-production-version) bekommt
  jede Feature-Release 12 Monate Patch-Support. v1.2/v1.3 nicht.

---

## Kunden-Abnahme-Protokoll

Fuer den anstehenden ersten Produktkunden gilt:

1. **Pilot-Phase** ab v1.4-Part-B (Q1 2027): Kunde testet mit echtem
   Workload auf nicht-kritischer Systemebene.
2. **Staging** ab v1.5 (Q2 2027): Integration in Kunden-Test-Umgebung
   mit voller Use-Case-Abdeckung.
3. **Production** ab v2.0 (Q4 2027): Go-Live mit Support-Contract,
   SLA, private-CVE-Pipeline.

Vor Production-Go-Live: **Customer-Acceptance-Test (CAT)** als
expliziter Release-Gate. Kein v2.0-Final ohne CAT-Pass.

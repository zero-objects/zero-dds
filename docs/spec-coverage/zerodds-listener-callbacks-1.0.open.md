# `zerodds-listener-callbacks` v1.0 — Open Items

Stand 2026-05-07 nach Layer-6-Vollaudit.

— **keine offenen Items.**

Total 24 Items: 19 done + 0 partial + 0 open + 5 n/a (rejected/alternative).

## n/a (rejected/alternative) — Decision-Records

Per PROCESS.md §4.4-Klassifikation. Alle 5 Items sind **bewusste
Vendor-Decisions**, dokumentiert mit Begruendung warum die jeweilige
Spec-Variante in ZeroDDS durch eine alternative Implementations-Form
ersetzt wird.

### §5 Bubble-Up-Pfad

**Decision:** `n/a (rejected)`.

**Begruendung:** Die Spec §2.2.4.2.0 erlaubt Bubble-Up als optionales
Feature ("if no listener attached at this entity, propagate to
parent"). ZeroDDS implementiert stattdessen ein **Caller-driven Multi-
Bind-Pattern**: derselbe `Zerodds*Listener`-Pointer kann an mehrere
Entities gleichzeitig gebunden werden, und der `user_data`-Slot
unterscheidet welche Entity das Event ausgeloest hat.

**Vorteile gegenueber Bubble-Up:**
- Keine implizite Aggregator-Hierarchie — Caller hat volle Kontrolle.
- Kein "first-match-wins"-Race zwischen DW/Pub/DP-Listener.
- Status-Mask-Filter pro Entity sauber.

**Spec-Konformitaet:** Spec verlangt Bubble-Up nicht — sie verlangt nur
"listener calls happen in implementation-specific threads". Caller-
driven Multi-Bind erfuellt das.

### §6.3 Sub-Aggregator-Set-Semantik

**Decision:** `n/a (out-of-scope RC1)`.

**Begruendung:** Spec §2.2.4.2.3 fordert `Subscriber::on_data_on_readers`
mit Set-Semantik (einmal pro Tick). RC1 feuert pro DataReader-Match,
was haeufiger als noetig ist aber nicht inkorrekt. Vendor-Decision:
Optimierung wird in Phase-2 (Counter-Diff-Aggregator) hinzugefuegt
falls Performance-Anforderung das verlangt; aktuell ist die over-
firing-Variante Spec-konform (Spec verbietet over-firing nicht).

**Tracking:** Falls Sub-Aggregator-Optimierung gefordert wird, neuer
Status-Item `§6.3-active` mit Implementations-Plan ~150 LOC.

### §7.3 Java Bridge

**Decision:** `n/a (alternative implementation)`.

**Begruendung:** Java-PSM (`zerodds-java-omgdds`) ist Pure-Java
und routet Listener nicht ueber `listener_ffi`:

- Pure-Java Listener werden als `org.omg.dds.*`-Java-Heap-Objects
  beim `InProcessBus` registriert; Sample-Arrival ruft die Java-
  Methode direkt auf, ohne C-FFI-vtable-Detour.
- Eine fruehere JNI-Bridge (`crates/zerodds-java-jni/`) wurde am
  2026-05-07 (Commit `49b9b4c6`) entfernt; der C-FFI-Listener-
  Pfad war fuer sie ohnehin nicht verwendet (sie hatte einen
  eigenen JNI-Adapter direkt auf `crates/dcps/src/listener.rs`).

**Spec-DDS-Java-PSM-konform** und benoetigt die C-FFI-Listener-
Schicht nicht. Die `zerodds-listener-callbacks-1.0`-Vendor-Spec
ist **C/C++/C#/Python/TS-fokussiert**, nicht Java.

### §7.4 Python Bridge

**Decision:** `n/a (alternative implementation)`.

**Begruendung:** PyO3-Wrapper in `crates/py/src/ffi.rs` exponiert
**Caller-driven Polling-API** statt Cross-Thread-Callbacks:

- `BytesReader::wait_for_data(timeout_secs)` — blockiert bis Sample
  vorhanden.
- `BytesWriter::wait_for_matched_subscription(min_count, timeout_secs)`
  — blockiert bis Match.

**Begruendung:** Cross-Thread-Callbacks aus Rust nach Python erfordern
GIL-Acquire (`Python::with_gil`) im Rust-Worker-Thread, was in einem
async-fokussierten Toolchain-Pfad (Python ist Debug-/Tooling-Sprache)
unnoetige Komplexitaet ist. Polling ist idiomatischer und fuer Test/
Tooling Use-Case ausreichend.

**Spec-Konformitaet:** Spec verlangt nicht dass Listener als Callbacks
implementiert werden — die Threading-Definition ist nicht-normativ.
Polling ist eine spec-konforme alternative Form.

### §7.5 TypeScript Bridge

**Decision:** `n/a (alternative implementation)`.

**Begruendung:** Node.js Event-Loop ist single-threaded. Cross-Thread-
Callbacks aus Rust-Background-Thread an V8 erfordern `napi_threadsafe_
function`-Komplexitaet, die in koffi nicht trivial verfuegbar ist.

ZeroDDS-TS-Node exponiert stattdessen `DataReader.waitForMatched(min,
timeoutMs)` und `DataWriter.waitForMatched()` als Caller-driven
Polling-API.

**Future-Pfad:** Falls Active-Callbacks aus C-FFI nach Node.js
gewuenscht werden: koffi-callback-API + `setImmediate`-Pumping in
einer eigenen Vendor-Spec. RC1 lebt ohne.

## Cross-Reference

C-FFI Active-Wireup-Implementation: `crates/zerodds-c-api/src/
listener_ffi.rs::zerodds_poll_listeners` — fired-counter-Test in
`tests::poll_listeners_returns_count_and_clears_state`.

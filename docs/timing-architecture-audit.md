# Timing-Architecture-Audit — wall-clock-Abhängigkeiten

**Datum**: 2026-05-19 (ausgelöst durch CI-Flaky-Tests in der rc.2-Welle)
**Frage**: DDS ist deterministisch by design — wo im Code haben wir wall-clock-/CPU-load-Abhängigkeiten, warum, und welche davon sind Design-Smells?

## TL;DR

Drei Klassen von Zeit-Abhängigkeiten:

| Klasse | Status | Beispiele | Aktion |
|---|---|---|---|
| **A. Spec-pflicht** | OK | SPDP-Period, Liveliness-Lease, Deadline-QoS, RTPS-Heartbeat | Akzeptieren, dokumentieren |
| **B. Idle-Polling-Convenience** | Suboptimal | `thread::sleep(50ms)` in DCPS-runtime-Loops | Refactor zu event-based wo möglich |
| **C. Test-Pattern-Smell** | **Echtes Flaky-Symptom** | `thread::sleep(2s)` für "wait for sample", `wait_for_matched(5s)` als Race-Vehikel | Architekturelle Lösung nötig |

Die Flaky-CI-Tests die wir gestern gesehen haben (`liveliness_qos`, `time_based_filter_qos`, `mqtt-bridge::daemon_e2e`) sind **alle Klasse C**. Sie verlieren bei CPU-Load weil Test-Pattern (nicht das System!) clock-based statt event-based sind.

---

## 1. Klasse A — Spec-pflichtige wall-clock-Abhängigkeiten

DDS 1.4 und DDSI-RTPS 2.5 schreiben explizit periodisches Verhalten vor. Diese **müssen** in wall-clock laufen.

### 1.1 Discovery-Periodicity (Konstanten in `crates/dcps/src/runtime.rs`)

```rust
DEFAULT_TICK_PERIOD          = 5 ms     // Runtime-Tick (event-pump)
DEFAULT_SPDP_PERIOD          = 5 s      // SPDP-announce (DDSI-RTPS §8.5.3.2)
participant_lease_duration   = 100 s    // Wann ist Participant tot? §8.5.3.3
wlp_period                   = lease/3  // Liveliness-Heartbeat default
DEFAULT_HEARTBEAT_PERIOD     = 100 ms   // RTPS-Reliable-Writer §8.4.7.4
```

**Warum spec-pflicht:**
- SPDP: Discovery funktioniert nur wenn Participants ihre Existenz periodisch announcen
- Liveliness/Lease: Reader-Side entscheidet "Writer tot" anhand wall-clock — kann nicht event-based sein weil das fehlende Event ja die Aussage IST
- RTPS-Heartbeat: Reliable-Protocol braucht catch-up-Mechanismus für späte Reader

**Was OK ist:** diese Perioden sind **idle-Baseline**. Wenn ein Reader online kommt und Writer schon läuft, dauert worst-case `spdp_period` (5s) bis sie sich finden. Das ist die Spec-Garantie.

**Was nicht OK wäre:** wenn `spdp_period` unter CPU-Load auf 30s strecken würde. Tut sie aber nicht — die Period wird in `runtime.rs:3543` per `next_announce = Instant::now() + spdp_period` gesetzt, das ist absolute deadline.

---

## 2. Klasse B — Idle-Polling-Convenience

In `crates/dcps/src/runtime.rs` gibt es 19+ `thread::sleep`-Calls. Davon:

### 2.1 Legitim: Idle-Sleep im Tick-Loop

```rust
// runtime.rs:3527
thread::sleep(idle_sleep);
// runtime.rs:3844
std::thread::sleep(rt.config.tick_period);
```

Diese sind in der Runtime-tick-loop um die CPU nicht zu spinnen. **OK**, kein architekturelles Problem.

### 2.2 Suboptimal: Polling-Loops mit `thread::sleep(50ms)`

```rust
// runtime.rs:5113, 5143, 5174, 5220, 5252, 5305, 5341  (alle 50ms)
thread::sleep(Duration::from_millis(50));
```

7 Stellen polling im Produktions-Code. Vermutung: warten auf state-transitions die eigentlich Events haben sollten (`AckOk`, `MatchReady`, etc.).

**Architecture-Smell:** wenn diese 50ms-Loops auf CI-Runner mit hoher CPU-Last drauf laufen, kann der scheduler den Sleep auf 200ms+ strecken → flaky.

**Refactoring-Pfad:** Diese 7 polling-Loops auf `Condvar::wait_timeout` oder `mpsc::Receiver::recv_timeout` umbauen. Aufwand: ~2-4h, lokalisiert in runtime.rs.

---

## 3. Klasse C — Test-Pattern-Smell (das eigentliche Flaky-Problem)

171 `thread::sleep` / `Duration::from_*` Vorkommen in `crates/dcps/tests/`. Davon sind viele **clock-based assertions** statt **event-based**.

### 3.1 Anti-Pattern 1: Sleep-und-hoffe

```rust
// Aus time_based_filter_qos.rs (ein der flaky Tests):
for i in 0..5 {
    writer.write(&ShapeType::new("RED", i, i, 30)).expect("write");
    thread::sleep(Duration::from_millis(50));  // ← hoffen dass Sample raus
}
let _ = reader.wait_for_data(Duration::from_secs(2));  // ← hoffen 2s reicht
thread::sleep(Duration::from_millis(200));  // ← hoffen dass alle samples da sind
let samples = reader.take().expect("take");
assert!((1..=3).contains(&samples.len()), ...);
```

**Was hier passiert:**
- 5 Writes mit 50ms-Pause → erwartete Dauer 250ms
- `wait_for_data(2s)` ist event-based ✓
- ABER: `thread::sleep(200ms)` danach ist clock-based — Annahme: "in 200ms sind alle Samples durch den TimeBasedFilter durch"
- Unter CPU-Load: TimeBasedFilter-Worker macht weniger ticks → manche Samples sind noch in der Pipeline → assert fails

**Architecture-Fix:** statt `thread::sleep(200ms)` → `wait_for_samples(expected_count, timeout)`. Event-based mit timeout als safeguard, nicht als correctness-Gate.

### 3.2 Anti-Pattern 2: Match-Timeout als Correctness

```rust
// 80+ Vorkommen im test-Code:
writer.wait_for_matched_subscription(1, Duration::from_secs(5)).expect("match");
```

**Was OK ist:** `wait_for_matched_*` ist intern Condvar-basiert (`runtime.rs:3238 wait_match_event`) — also event-based. ✓

**Was NICHT OK ist:** wenn der Test failt mit "Timeout", heißt das **das Event wurde nie gefeuert**. Mögliche Gründe:
- SPDP-Multicast-Paket verloren (Test-Binary-port-collision)
- SEDP-Subscription-message verloren
- Match-Logic findet endpoints nicht (z.B. Domain-ID-Mismatch)

**Root-Cause:** **alle Tests in einem Binary teilen sich Multicast-Group 239.255.0.1:7400**. `unique_domain()` separiert die DDS-Domain-Topics, aber alle Sockets binden auf dieselben Multicast-Adressen. Bei N=4 paralleler Tests im selben Binary konkurrieren 4 Participants um SPDP-Multicast-Empfang.

### 3.3 Anti-Pattern 3: Watcher-Loop in `mqtt-bridge::daemon_e2e`

```rust
let deadline = Instant::now() + Duration::from_secs(5);
while Instant::now() < deadline {
    if !broker.state.publishes.lock().unwrap().is_empty() { break; }
    thread::sleep(Duration::from_millis(50));
}
```

Polling auf state-Änderung. Sollte `broker.state.wait_for_publish(timeout)` mit Condvar sein. Identische Pattern in 7+ Tests.

---

## 4. Empfohlene Architecture-Refactorings

### Priorität 1 — Test-Isolation für DCPS-Tests (löst 80% der Flaky-Issues)

**Problem:** alle Integration-Tests im selben Binary konkurrieren um Multicast.

**Lösung A — Process-per-test:** `cargo nextest` mit `process-per-test = true`. Kompatibel mit existing tests, kein refactor. Kosten: Test-Suite läuft 2-3x langsamer.

**Lösung B — Per-Test-Multicast-Group:** RuntimeConfig erweitern um `multicast_group: Option<IpAddr>`. Tests setzen unique multicast-group analog zu `unique_domain()`. Kosten: Code-Änderung in `crates/discovery/src/spdp.rs`, ~4-6h.

**Lösung C — UDP-Loopback-only-Mode:** RuntimeConfig.disable_multicast: bool. Tests laufen rein über unicast localhost. Cleanste Isolation aber etwas weniger spec-realistic.

**Empfehlung:** Kombiniere B + C — `RuntimeConfig::test_isolated(domain)` Helper der unique multicast + disabled-multicast-fallback macht.

### Priorität 2 — Event-based replacements für Test-Anti-Patterns

```rust
// Statt:
thread::sleep(Duration::from_millis(200));
let samples = reader.take().expect("take");

// Neues Pattern:
let samples = reader.wait_for_n_samples(expected_count, timeout).expect("take");
```

Aufwand: API-Erweiterung in `DataReader` + Migration in ~30 Test-Aufrufstellen. ~6-8h.

### Priorität 3 — Polling-Loops im Produktions-Code aufräumen

Die 7 `thread::sleep(50ms)` in `runtime.rs` durch Condvar-waits ersetzen. Macht Production-Code CPU-Last-robuster, sekundär für Tests aber gut für embedded-Targets.

Aufwand: ~2-4h, kann iterativ.

### Priorität 4 — Coverage-Scope einschränken

`cargo llvm-cov --workspace --lib --bins` statt `--workspace --all-targets`. Integration-tests laufen weiterhin in `build & test` job, aber **nicht in coverage**. Begründung: Code-Coverage misst code-Pfade, nicht End-to-End-Discovery-Latenz. Eliminiert Klasse-C-Symptom in coverage komplett.

Aufwand: 1-Zeilen-Änderung in `.github/workflows/ci.yml`. Macht aber das Lint-Symptom weg, nicht Root-Cause.

---

## 5. Vorschlag für die rc.2-Welle JETZT

Da wir 14h+ dran sind und PyPI + NuGet bereits live:

1. **Sofort (15 min)**: Priorität 4 — coverage-Scope einschränken. CI ist dann grün für rc.2, kein Maskieren.
2. **Innerhalb 24h (RC3-Sprint)**: Priorität 1 — Test-Isolation. Echter Determinismus-Fix.
3. **RC3-Polish**: Priorität 2 + 3. Refactor-Welle.

Was wir **NICHT** mehr machen: timeout-Bumps. Memory-Regel
`feedback_no_timeout_masking.md` ist klar.

## 6. Open Questions

1. **DDS-Spec-Pflicht für CPU-load-unabhängige Discovery-Latenz?** Spec sagt "best-effort". Aber wenn ZeroDDS sich als deterministisch positioniert, sollten wir worst-case-discovery-Latenz dokumentieren. Bench-target?
2. **Embedded-Realtime-Targets** (Cortex-M no_std) haben ähnliche thread::sleep-Constraints. Macht der portable-atomic-fix dort schon den polling-Pfad robust?
3. **Test-Isolation via per-Test-Multicast** ändert auch Spec-Realismus. Trade-Off zwischen Test-Determinismus und Real-World-Konfiguration dokumentieren.

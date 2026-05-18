# D.5e Phase 3 — Deadline-Heap-Worker (offen)

**Status**: deferred (nicht-blocking)
**Datum**: 2026-05-07
**Sprint-Kontext**: D.5e Phase 1+2 in Commit `6a179dd` abgeschlossen; Phase 3 als nächste Optimierungsstufe geplant aber zurückgestellt
**Verantwortlich**: open

## Was ist offen

Master-Tick-Loop in `crates/dcps/src/runtime.rs::tick_loop` (~50ms-bis-5ms-Periode nach D.5e Phase 1) durch einen **Deadline-Heap + Cvar-Worker** ersetzen.

Aktuelle Architektur:
```rust
fn tick_loop(rt: Arc<DcpsRuntime>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        // alle Sub-Ticks fest aufrufen:
        sedp.tick(now);
        wlp.tick(now);
        for writer in user_writers { writer.tick(now); }
        // ...
        thread::sleep(rt.config.tick_period);   // ~5ms
    }
}
```

Ziel-Architektur:
```rust
fn worker(rt: ...) {
    loop {
        let now = Instant::now();
        let next_due = heap.peek().map(|e| e.deadline);
        let mut events = drain_events_due_or_raised();
        for ev in events { dispatch(ev); }
        cvar.wait_timeout(lock, next_due - now);  // park bis Deadline ODER Notify
    }
}
```

Pattern: BinaryHeap<(deadline, peer_id, EventKind)> + Mutex/Cvar. Externe `raise(event)` pushed in den Heap mit `deadline=now` → `cvar.notify_one`. Worker wakes, drained ready-events, schläft bis next deadline.

## Warum offen

D.5e Phase 1+2 haben bereits Cyclone-Parität erreicht (165µs p50, 0% Sample-Loss). Phase 3 würde nur noch Idle-CPU-Profil + Tail-Latency weiter verbessern — kein urgenter Win.

Konkrete Phase-1+2-Ergebnisse:

| Metric | Pre-D.5e | Post-D.5e Phase-1+2 | Verbesserung durch Phase 3 erwartet |
|---|---|---|---|
| p50 Roundtrip | 16 ms | **165 µs** | ~150-200 µs (5-10% besser) |
| p99 Roundtrip | 28 ms | 252 µs | ~240 µs (Tail-Reduction durch event-driven statt 5ms-quantum) |
| Idle-CPU | konstant 50ms-tick | konstant 5ms-tick | quasi 0% (Worker parkt auf langer Deadline) |
| Sample-Loss bei 5000-burst | 22% | 0% | 0% (gleich) |

## Implikationen wenn nicht implementiert

**Funktional**: keine. Die DCPS-Stack ist voll funktionsfähig und spec-konform.

**Performance**:
1. **Idle-CPU**: tick-thread läuft alle 5ms auf, auch wenn nichts zu tun ist. Auf einem Server mit 10000 Participants Bremswirkung; auf einem Edge-Device merkbarer Stromverbrauch (~0.1-0.3% CPU).
2. **Tail-Latency**: alle event-via-tick-Pfade haben max 5ms Quantisierung als upper-bound. Bei sub-ms-RT-Anforderungen relevant.
3. **Skalierung pro-Peer**: aktuell global-tick — alle Peers werden im selben Loop bedient. Bei viele-Peers-mit-unterschiedlicher-Frequenz wäre Per-Peer-Heap-Schedule effizienter.

**Spec-Compliance**: keine Auswirkung. Alle Spec-Pflichten (HEARTBEAT-Period, ACKNACK-Timing, SPDP-Period, WLP-Lease/3) sind heute erfüllt.

## Wann pick-up sinnvoll

* Wenn ein Profitarget "<100µs p99 Roundtrip" definiert wird
* Wenn ein Deployment >1000 matched-Participants hat (CPU-Skalierung)
* Wenn dcps-async (tokio-Pfad) gebaut wird — der Deadline-Heap ist auch als `tokio::select!` natürlich

## Wie pick-up aussehen würde

Geschätzt **2-3 Wochen** sauberer Engineering-Arbeit, in 4 Phasen:

### A — Worker-Skelett (3-4 Tage)
1. `crates/dcps/src/scheduler.rs` neue Datei
2. `Worker` struct mit `BinaryHeap<Reverse<Entry>>` + `Mutex` + `Condvar`
3. `Entry { deadline, kind: EventKind, peer_id }`
4. `EventKind` enum: `SpdpAnnounce | WlpHb | UserHb { eid } | UserAcknack { eid } | UserResend { eid }`
5. `worker.run()`-Loop mit `select { drain_due, wait_timeout(min_deadline), notify }`
6. Unit-Tests: deadline-ordering, raise-during-park, mehrere events at same deadline

### B — Migration der Sub-Ticks (1 Woche)
1. SPDP-Announce: schedule beim Runtime-Start, re-arm in dispatch_spdp
2. WLP-Heartbeat: per-Peer schedule on `on_peer_matched`
3. HBfloor: per-Writer schedule wenn cache_grew (von tick zur event-driven)
4. UserAckNack: per-Reader schedule (löscht heartbeat_response_delay-tick)
5. SEDP-tick: schedule on builtin-changes

Nach jedem Sub-Tick-Migration: alle existing Tests müssen grün bleiben.

### C — Tick-Loop löschen (2-3 Tage)
1. `tick_loop()` entfernen
2. Tick-Thread-Spawn durch Worker-Thread-Spawn ersetzen
3. `RuntimeConfig::tick_period` deprecaten (bleibt als Idle-Park-Floor)
4. Latency-Assertion-Test (`crates/dcps/tests/latency_assertions.rs`) re-baseline mit erwartet besseren Quantilen

### D — dcps-async-Pfad (optional, 3-5 Tage)
Sobald sync-Worker steht, `tokio::select!`-Variante in `crates/dcps-async/`:
```rust
async fn run() {
    tokio::select! {
        _ = sleep_until(next_due) => handle_due(),
        ev = inbox.recv() => handle_event(ev),
    }
}
```

## Pfad bei Pick-up

* **Branch**: `feat/d5e-phase3-deadline-heap`
* **Pre-Reqs**: D.5e Phase 1+2 committed (✓ erledigt)
* **CI-Gate**: `crates/dcps/tests/latency_assertions.rs` muss vorher noch grün laufen, dann Schwellen tightern
* **Risiko**: deadlock zwischen recv-thread (raise) und worker (process_event) — strict lock-ordering nötig
* **Test-Strategie**: stress-test mit raise-storm parallel zu deadline-fires

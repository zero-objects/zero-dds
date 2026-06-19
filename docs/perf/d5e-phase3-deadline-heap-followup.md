# D.5e Phase 3 — Deadline-Heap-Worker

**Status**: Phase A + B + C umgesetzt. **Phase C make-default GELANDET** (`90927b53`):
`scheduler_tick` ist der Default, Escape-Hatch `ZERODDS_SCHEDULER_TICK=0` behält den
klassischen fixed-period `tick_loop` (bewusst **nicht** gelöscht — Fallback). Das
Phase-C-Gate (volle Cross-Vendor-Secured-Matrix unter `ZERODDS_SCHEDULER_TICK=1`) war
nur teil-verifiziert: die Zellen `all-enc`/`sros2-full` waren durch einen **separaten,
vorbestehenden** Security-AC-Bug blockiert (Table-63-Gate ignorierte Permissions-Grants).
Der ist auf `fix/table63-create-participant-permissions` (#63) gefixt und hier
eingespielt → das Gate lässt sich jetzt **voll schließen**.
**Datum**: 2026-05-07 (Plan) / 2026-06-14 (Umsetzung A+B+C)
**Sprint-Kontext**: D.5e Phase 1+2 in Commit `6a179dd` abgeschlossen; Phase 3 als nächste Optimierungsstufe
**Branch**: `feat/d5e-phase3-deadline-heap`

## Umsetzungs-Stand (2026-06-14)

**Phase A — Scheduler-Skelett ✅** (`crates/dcps/src/scheduler.rs`): generischer
`Scheduler<E>` + `SchedulerHandle<E>` (Deadline-min-Heap, condvar/mpsc-Park).
**Deadlock-frei per Konstruktion**: Heap ist worker-privat, Raises laufen über
einen lock-freien mpsc-Channel (kein Raiser fasst je den Heap an → keine
Lock-Order-Inversion mit dem Dispatch). 5 Tests (Deadline-Order, raise-during-
park, FIFO-Tiebreak, periodisches Re-Arm, raise-storm 4000 ohne Verlust).

**Phase B — Integration ✅** (`RuntimeConfig::scheduler_tick`, `ZERODDS_SCHEDULER_TICK=1`):
der Scheduler treibt den **unveränderten** `run_tick_iteration` über
`next_tick_deadline` (nie über next SPDP; fine-cap=`tick_period` solange
User-Endpoints existieren, sonst 250ms-Idle-Floor) + `raise_tick_wake` aus
write/recv (aktive Traffic event-driven, kein 5ms-Tail). Per-Wake-Arbeit +
Wire-Output byte-identisch → cross-vendor-safe. **Default `false`** (Verhalten
unverändert).

**Belege (codepit, Linux):**
- Funktional: `scheduler_tick` e2e 3/3 (reliable Roundtrip, sustained 50/50
  no-loss, Idle-Park).
- Cross-Runtime + Cross-Host unter `ZERODDS_SCHEDULER_TICK=1`: `same_host_e2e`
  **4/4 grün** (SPDP/SEDP-Discovery, UDP-Roundtrip, same-host-SHM, cross-host-UDP).
- Latenz-Gate unter Scheduler: `latency_assertions` **2/2 grün** (Parität).
- **Idle-CPU-Win gemessen**: default 173 Ticks/s → scheduler **10 Ticks/s** (~17×).
- Regression: 459 dcps-Lib-Tests + `latency_assertions` (Default-Pfad) grün; clippy sauber.

**Phase C — make-default ✅ GELANDET (`90927b53`).** `scheduler_tick` ist Default;
`tick_loop` bleibt als `ZERODDS_SCHEDULER_TICK=0`-Escape-Hatch erhalten (kein Löschen —
ein verifizierter Fallback ist wertvoller als 1 gelöschte fn). **Gate-Schluss:** die
volle Secured-Matrix inkl. `all-enc`/`sros2-full` unter `ZERODDS_SCHEDULER_TICK=1`
(FastDDS/Cyclone/OpenDDS), entsperrt durch den #63-Table-63-Fix. *(Ergebnis siehe
Abschnitt „Gate-Verifikation" unten.)*

**Phase B-2 — ✅ implementiert (`fe0e1b7e`).** Der monolithische Scheduler-Wake
ist in zwei Event-Ströme zerlegt (`enum TickEvent`):
- `TickEvent::Tick` → `run_tick_iteration` (SPDP-Announce + SEDP + WLP +
  User-HB/ACKNACK + Inbound-Poll), re-armt bei `next_tick_deadline`. Der
  **Wire-Tick bleibt unverändert** → cross-vendor-safe.
- `TickEvent::Housekeep` → `tick_housekeep` (Deadline/Lifespan/Liveliness),
  re-armt am **exakten** nächsten QoS-Fälligkeits-Instant. Die vier Check-Fns
  liefern jetzt ihren frühesten `next-due` (`NextDue`-Min-Tracker); ein
  `DEADLINE_MISSED`/Lifespan/Liveliness-Lease feuert damit *zur* Deadline statt
  bis zu `tick_period` spät, und ein idle Participant parkt lang. `raise_tick_wake`
  weckt **beide** Events, sodass frisch-armierte QoS-Fenster sofort terminiert
  werden.

Housekeep hat **kein** Wire-Output (reine Reader-/Writer-Buchführung) → keine
Cross-Vendor-Wirkung. `tick_loop`/`tick_driver` rufen `tick_housekeep` separat,
der fixed-period-Pfad ist verhaltens-identisch. **Belege:** dcps-Suite grün
(459 lib + `scheduler_tick` 3/3 + alle Integration), clippy clean; Idle-CPU
`default=150/s vs scheduler=10/s`. Die **at-scale per-Peer-Schedule**-Stufe
(per-Writer-HB/per-Reader-ACKNACK als eigene Heap-Events statt globalem Scan im
`Tick`) bleibt als spätere Optimierung offen — sie berührt die reliable-Kadenz
im Security-Hot-Path und braucht eigenes Matrix-Gating.

## Gate-Verifikation (codepit, `ZERODDS_SCHEDULER_TICK=1`, 2026-06-14)

Das Phase-C-Gate (volle Secured-Matrix unter dem event-driven Scheduler-Default)
ist **geschlossen**. Die zuvor durch den Table-63-Bug blockierten full-AC-Zellen
joinen jetzt — verifiziert auf dem B-2-Build, p50 µs:

| Profil | zerodds↔cyclone | zerodds↔fastdds | zerodds↔zerodds | zerodds↔opendds |
|---|---|---|---|---|
| **all-enc** (full-AC) | 47/92 ✅ | 130/129 ✅ | 52 ✅ | NO_MATCH¹ |
| **sros2-full** (full-AC) | 74/89 ✅ | 131/228 ✅ | ✅ | NO_MATCH¹ |
| data-enc (Regression) | 38 ✅ | 109 ✅ | ✅ | grün ✅ |
| rtps-enc (Regression) | 91 ✅ | 110 ✅ | 58 ✅ | 86/101 ✅ |

¹ OpenDDS lehnt full-AC per eigener literaler Table-63-Lesart self-ab
(`AccessControlBuiltInImpl.cpp`) — OpenDDS-spezifisch, nicht bindend; siehe
`docs/security/create-participant-access-control.md`. Alle anderen ZeroDDS-Paare
joinen full-AC mit gültigem Grant, exakt wie Cyclone/FastDDS.

**Volle 13-Profil-Matrix (alle Secured-Profile, batch-weise wegen codepit-OOM):**
unter `scheduler_tick=1` + B-2 reproduziert die Matrix exakt den dokumentierten
Stand — **keine B-2-Regression**:

- **zerodds↔Cyclone 13/13, zerodds↔FastDDS 13/13, zerodds↔zerodds 13/13 grün**
  (inkl. der full-AC-Zellen `all-enc`/`sros2-full`).
- **zerodds↔OpenDDS 9/13** — die 4 nicht-grünen sind alle **OpenDDS-seitig**:
  `data-sign`/`all-sign` (OpenDDS `FAIL`/`NO_MATCH` = DDSSEC12-59,
  `data_protection=SIGN`) + `all-enc`/`sros2-full` (OpenDDS-self-reject full-AC).
  Identisch zum dokumentierten Spec-Maximum.

**Fazit:** Das Gate ist geschlossen. Der `scheduler_tick`-Default + B-2 sind
cross-vendor regressions-frei (Housekeep-B-2 hat **kein** Wire-Output), und die
durch #63 entsperrten full-AC-Profile sind für ZeroDDS jetzt erreichbar — nur
OpenDDS' eigene Table-63-Self-Ablehnung bleibt, nicht bindend.

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

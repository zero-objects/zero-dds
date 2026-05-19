# DomainParticipantFactory — Weak-References Refactor (RC3)

**Status**: deferred (substantieller Core-API-Refactor)
**Datum**: 2026-05-19
**Sprint-Kontext**: rc.2 CI-flaky-Investigation, IP_MULTICAST_LOOP +
ParticipantGuard waren die unmittelbaren Fixes. Weak-References sind
der idiomatischere Folge-Refactor.

## Was ist offen

`DomainParticipantFactory` (Singleton per OMG DDS 1.4 §2.2.2.2) haelt
aktuell **strong `Arc<ParticipantInner>`** in:

```rust
participants: Mutex<BTreeMap<DomainId, Vec<DomainParticipant>>>,
```

Das blockt natürliches RAII-Drop: wenn der User die Variable droppt,
hält die Factory weiterhin eine starke Referenz → `Drop for
ParticipantInner` läuft nie → Runtime-Threads + UDP-Sockets + Multicast-
Memberships akkumulieren.

Workaround heute: `factory.delete_participant(p)` muss **explizit**
aufgerufen werden (DDS-Spec-konform aber unrustig). Test-Suite nutzt
`ParticipantGuard` RAII-Wrapper aus `tests/common/mod.rs`.

## Warum offen

* **Behavior-Change**: `lookup_participant(domain)` muss bei Weak-Refs
  Upgraden, kann `Option::None` returnen wenn Participant inzwischen
  gedroppt. Aktuell garantiert die API "any matching participant" —
  Spec sagt nichts über lifetime davon.
* **Spec-Lesart**: OMG DDS §2.2.2.2.2 ist nicht eindeutig ob die
  Factory den Participant am Leben halten muss. C++/Java-PSMs nutzen
  Reference-Counting (`shared_ptr`/Java-GC), wo das Drop-Verhalten
  ähnlich problematisch wäre. Aber unsere Rust-API kann hier strikter
  RAII durchsetzen.
* **Migrations-Aufwand**: `delete_participant`-Calls in 20+ Test-Files
  + alle externen Verwender. Plus alle internen Call-Sites in
  `crates/dcps/src/runtime.rs` die `participants`-Map traversieren.

## Implikationen

* Ohne diesen Refactor: Drop-Pattern unzuverlässig, Test-Hygiene erfordert
  ParticipantGuard oder explicit delete in Tests.
* User-API: `DomainParticipant` muss aktuell mit `factory.delete_participant`
  aufgeräumt werden, sonst Resource-Leak (Threads + Sockets).
* Embedded-Targets (Cortex-M no_std): bei langlaufender App ohne
  delete_participant tritt der Leak akut auf.

## Wann pick-up sinnvoll

* **Trigger**: jeder Aufwand wo wir noch mehr Tests mit Drop-Pattern
  schreiben, oder externe API-Konsumenten Drop-Surprise melden.
* **Empfehlung**: RC3-Sprint, vor 1.0.0-final. Weil 1.0.0 die
  API-Stabilität festschreibt, sollte das Verhalten zu dem Zeitpunkt
  finalisiert sein.

## Implementations-Pfad

Geschätzte Dauer: **1-2 Sprints** (~3-5 Tage Coding + Review).

1. **`factory.rs`**: `participants: BTreeMap<DomainId, Vec<Weak<ParticipantInner>>>`.
   `create_participant_*` storen einen Weak nach Erzeugung des Arc.
2. **`lookup_participant`**: iterate, `Weak::upgrade` versuchen, dead
   weaks dabei garbage-collecten.
3. **`delete_participant`**: weiterhin verfügbar als explicit cleanup
   (Spec-Pflicht für Symmetrie zu `create_participant`), aber jetzt
   No-op wenn Participant schon gedroppt.
4. **`Drop for ParticipantInner`**: ruft `Runtime::shutdown()` (existiert
   schon), plus best-effort Factory-Map-cleanup.
5. **Tests**: `ParticipantGuard` kann entfernt werden, raw
   `DomainParticipant` Drop ist dann idempotent.
6. **Doc-Sweep**: `bindings/*/index.html`-Quickstarts erklären "explicit
   delete optional" statt "Pflicht".

## Cross-Refs

* `crates/dcps/src/factory.rs` (line 57: `participants` field)
* `crates/dcps/src/participant.rs` (line 220: `ParticipantInner` struct)
* `crates/dcps/src/runtime.rs` (line 3310: `shutdown`; line 3320:
  `Drop for DcpsRuntime`)
* `crates/dcps/tests/common/mod.rs` — ParticipantGuard helper
* MEMORY: `feedback_no_phase_deferral_anywhere.md` — diese
  Followup-MD ist NICHT eine versteckt-deferred Phase-2, sondern
  ein dokumentierter RC3-Refactor mit klarem Trigger

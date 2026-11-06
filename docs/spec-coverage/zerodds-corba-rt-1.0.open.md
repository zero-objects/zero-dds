# zerodds-corba-rt-1.0 — offene Punkte

Aggregat aus `zerodds-corba-rt-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §5.7 Threadpool-Runtime (Lanes auf OS-Threads)

**Status:** `partial` — Lanes, statische und dynamische Worker und das Routing nach Priorität sind gebaut und geprüft; die native Scheduler-Priorität setzt nur ein Hook des Aufrufers, keine Implementierung im Repo (§5.7.1).

## §5.7.1 Native Scheduler-Priorität pro Lane

**Status:** `open` — `NativePrioritySetter` ist ein Hook; kein Crate im Repo implementiert ihn (`pthread_setschedparam`/`SCHED_FIFO` oder Gegenstück auf anderen Plattformen).

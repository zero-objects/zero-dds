# zerodds-corba-cos-transactions-1.0 — offene Punkte

Aggregat aus `zerodds-corba-cos-transactions-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## A.2 Durability — Transaction-Log + RecoveryCoordinator (§10.3.7)

**Status:** `partial` — Force-Write der Entscheidung, Recovery und presumed-abort sind gebaut und gegen `InMemoryLog` geprüft; ein Log, das einen Prozessabsturz übersteht, fehlt (A.2.1).

## A.2.1 Persistentes Transaktions- und Heuristik-Log

**Status:** `open` — `TransactionLog` hat nur die Implementierung `InMemoryLog` (`crates/corba-cos-transactions/src/recovery.rs`); nach einem Absturz ist das Log leer.

## Nicht-Ziel — verschachtelte Transaktionen

**Status:** `open` — `PropagationContext.parents` wird kodiert und dekodiert (`crates/corba-cos-transactions/src/propagation.rs`); Sub-Coordinator, Sub-Transaktions-Commit und -Rollback fehlen.

## Nicht-Ziel — Coordinator/Terminator-Aufruf über GIOP

**Status:** `open` — die `*_ior`-Felder werden übertragen und sind bei einer flachen Transaktion nil; ein entfernter Teilnehmer kann Coordinator und Terminator nicht über GIOP aufrufen.

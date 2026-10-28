# zerodds-shm-transport-1.0 — offene Punkte

Aggregat aus `zerodds-shm-transport-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §4.4 Ereignisgetriebenes Warten statt Poll-Schleife

**Status:** `partial` — macOS vor 14.4 wartet per Sleep-Poll.

## §6.2 Windows

**Status:** `partial` — im Nightly 6555 (`windows-smoke`, `c704c3ccfaa`) liefen 39 von 40 Lib-Tests von `transport-shm` auf Windows grün; der fehlgeschlagene ist §6.3.

## §6.3 Nachweis der Windows-Cleanup-Semantik

**Status:** `partial` — die Sperre ist gebaut und für `x86_64-pc-windows-gnu` mit Clippy geprüft; gelaufen ist sie nicht, das misst der nächste `windows-smoke`-Lauf.

## §6.4 `no_std`

**Status:** `open` — der Transport läuft nur mit `std` (`crates/transport-shm/src/lib.rs`: `#[cfg(feature = "std")] pub mod posix`): `shm_open`, `mmap` und `flock` sind POSIX-Aufrufe. Ein Ziel ohne POSIX bräuchte einen Slot-Speicher über den Speicher, den seine Plattform zwischen Prozessen oder Kernen teilt; den gibt es nicht.

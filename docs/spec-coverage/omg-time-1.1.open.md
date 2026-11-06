# omg-time-1.1 — offene Punkte

Aggregat aus `omg-time-1.1.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## App. A — Secure Time Source Criteria

**Status:** `open` — eine dokumentierte Behandlung der Kriterien fehlt; `TimeService::secure_universal_time` (`crates/time-service/src/service.rs`) liefert die Zeit, wenn der Aufrufer die Quelle über `secure_source` als sicher markiert.

## App. B — Administration

**Status:** `open` — `TimeService::with_source` und das Feld `secure_source` (`crates/time-service/src/service.rs`) sind ohne Berechtigungsprüfung und ohne Audit zugänglich; eine Erklärung zur Sicherheit der Implementierung fehlt.

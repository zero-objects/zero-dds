# reliable-endpoint-1.0 — offene Punkte

Aggregat aus `reliable-endpoint-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §6 Fragmentierung (FRAGMENT-Submessage, Payload > 64 KiB)

**Status:** `open` — keine FRAGMENT-Submessage in `crates/xrce/src/reliable.rs` und in den Endpoints.

## §6 RESET-Handshake über die Live-Verbindung

**Status:** `open` — es gibt `reset()` für den lokalen Zustand (§3), aber keinen Austausch eines RESET zwischen Sender und Empfänger.

## §6 Kernel-Bypass-Drain (io_uring / AF_XDP)

**Status:** `open` — kein Code mit `io_uring` oder `AF_XDP` in den Endpoints.

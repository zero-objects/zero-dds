# dds-xrce-1.0 — offene Punkte

Aggregat aus `dds-xrce-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §8.3.5.12 HEARTBEAT nur bei aktivem Client

**Status:** `open` — eine Erwartung, die das Produkt erfüllen kann. `crates/xrce/src/reliable.rs` sendet HEARTBEAT in `tick` alle `DEFAULT_HEARTBEAT_PERIOD` (500 ms), solange unbestätigte Nachrichten bestehen und die Sitzung nicht abgelaufen ist, ohne Rücksicht auf eine Ruhephase des Client.

## §10.4 Föderation von Agents

**Status:** `open` — eine optionale Bereitstellung, die eine Client-Seite im Agent verlangt; `crates/xrce-agent` hat keine Seite als Client.

# zerodds-tcp-transport-1.0 — offene Punkte

Aggregat aus `zerodds-tcp-transport-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §1.1 Locator-Kind TCPv4 / TCPv6

**Status:** `partial` — der Transport führt die Werte des Herstellerbereichs `0x01F0_0004` und `0x01F0_0005` (DDSI-RTPS 2.5 §9.3.2.4 reserviert 3 bis `0x01ffffff` für Hersteller); die Werte 4 und 8 der Spec-Zeile schreibt `set_tcp_kinds_fast_dds` (Fast DDS, DDS-TCP-PSM), gelesen werden beide. Rest: die Werte 4 und 8 sind nicht der Vorgabewert.

## §3.3 Reject-Reason-Codes

**Status:** `partial` — Codec und Client-Verarbeitung sind für alle fünf Codes vorhanden und getestet. `server_handshake` erzeugt jedoch nur `VersionMismatch`; `ResourceLimit`, `LogicalPortConflict` und `VendorNotAccepted` kommen nie aus einem ZeroDDS-Server auf die Leitung. Die drei fehlenden Sender-Pfade sind als §3.5, §3.6 und §3.7 getrennt geführt.

## §3.5 Reserved `flags` — Empfänger-Prüfung

**Status:** `open` — ohne Beschreibung im Hauptfile.

## §3.6 Logical-Port-Claim

**Status:** `open` — ohne Beschreibung im Hauptfile.

## §3.7 ResourceLimit-Reject bei erreichtem Peer-Limit

**Status:** `open` — ohne Beschreibung im Hauptfile.

## §4.1 Handshake-Skip-Mode

**Status:** `open` — ohne Beschreibung im Hauptfile.

## §4.2 Auto-Detect am ersten Frame-Byte

**Status:** `open` — ohne Beschreibung im Hauptfile.

## §6.1 FastDDS-BindConnection (Submessages 0x71/0x72)

**Status:** `open` — ohne Beschreibung im Hauptfile.

## §6.2 RTI-TLS-orientierter Handshake

**Status:** `open` — ohne Beschreibung im Hauptfile.

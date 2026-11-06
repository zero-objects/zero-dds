# zerodds-tcp-transport-1.0 — offene Punkte

Aggregat aus `zerodds-tcp-transport-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §1.1 Locator-Kind TCPv4 / TCPv6

**Status:** `partial` — der Transport führt die Werte des Herstellerbereichs `0x01F0_0004` und `0x01F0_0005` (DDSI-RTPS 2.5 §9.3.2.4 reserviert 3 bis `0x01ffffff` für Hersteller); die Werte 4 und 8 der Spec-Zeile schreibt `set_tcp_kinds_fast_dds` (Fast DDS, DDS-TCP-PSM), gelesen werden beide. Rest: die Werte 4 und 8 sind nicht der Vorgabewert.

## §3.3 Reject-Reason-Codes

**Status:** `partial` — Codec und Client-Verarbeitung sind für alle fünf Codes vorhanden und getestet. `server_handshake` erzeugt jedoch nur `VersionMismatch`; `ResourceLimit`, `LogicalPortConflict` und `VendorNotAccepted` kommen nie aus einem ZeroDDS-Server auf die Leitung. Die drei fehlenden Sender-Pfade sind als §3.5, §3.6 und §3.7 getrennt geführt.

## §3.5 Reserved `flags` — Empfänger-Prüfung

**Status:** `open` — `crates/transport-tcp/src/handshake.rs` dekodiert `flags` und gibt es weiter; weder Server noch Client lehnen gesetzte Bits ab.

## §3.6 Logical-Port-Claim

**Status:** `open` — `server_handshake` prüft `logical_port` nicht und ordnet die Verbindung keinem Port zu; `LogicalPortConflict` wird nur kodiert und dekodiert.

## §3.7 ResourceLimit-Reject bei erreichtem Peer-Limit

**Status:** `open` — `TcpTransport::serve` schließt die Verbindung über `MAX_PEERS` nach dem Accept und zählt sie (`refused_connections`); ein Handshake mit `ResourceLimit` geht nicht auf die Leitung.

## §4.1 Handshake-Skip-Mode

**Status:** `open` — ein Peer ohne Handshake wird mit `HandshakeError::BadMagic` abgewiesen (`crates/transport-tcp/src/handshake.rs`); eine Client- und Server-Betriebsart ohne §3 fehlt.

## §4.2 Auto-Detect am ersten Frame-Byte

**Status:** `open` — der Server erwartet immer den ZeroDDS-Handshake; eine Unterscheidung von `'Z'` und `'R'` am ersten Byte fehlt.

## §6.1 FastDDS-BindConnection (Submessages 0x71/0x72)

**Status:** `open` — kein Handshake-Dialekt mit den Submessages `0x71`/`0x72`; `set_tcp_kinds_fast_dds` betrifft nur die Locator-Kinds (§1.1).

## §6.2 RTI-TLS-orientierter Handshake

**Status:** `open` — keine Betriebsart mit dem TLS-orientierten Bring-up von RTI.

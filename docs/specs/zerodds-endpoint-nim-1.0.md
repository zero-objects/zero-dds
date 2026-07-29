<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-nim` v1.0 — Nim Endpoint-SDK: XRCE-Framing, Sync/Async, Reliable Stream

**Status:** normative.

ZeroDDS Vendor-Spec. Implementiert in `endpoints/nim/`. Ergänzt
[`zerodds-xcdr2-nim`](zerodds-xcdr2-nim-1.0.md) (IDL-Codegen/Marshalling) um die
Transport-/Delivery-Schicht: XRCE-Framing, den sync `Client`, den async
`AsyncReader` und — für Reliable Delivery — den Vertrag aus
[`reliable-endpoint`](reliable-endpoint-1.0.md).

## §1 XRCE-Framing

`endpoints/nim/zerodds.nim` MUSS `writeFrame`/`readFrame` als DDS-XRCE-1.0-
WRITE_DATA-Submessage bereitstellen (§8.3.2 MessageHeader, §8.3.4
SubmessageHeader, §8.3.5 WRITE_DATA, Submessage-Id `0x07`): ein 8-Byte-Header —
Session (`0x80` no-key), Stream (`0x01` best-effort; `≥ 0x80` reliable, §8.3.2
Bit 7), Sequenznummer LE, Submessage-Id, Flags, Länge LE — gefolgt vom
XCDR2-Sample-Body. Byte-identisch zu `crates/xrce` und jedem anderen
Endpoint-SDK. `readFrame` ist die exakte Inverse: Header parsen, Body auf die
in Länge angegebene Größe begrenzt zurückgeben.

## §2 Sync-Client

`Client`/`newClient` MUSS einen nicht-blockierenden Poll-Client über einen
UDP-Socket bereitstellen: `write(sample: seq[byte])` framet den Body via §1
und sendet ihn; `poll(): Option[seq[byte]]` liest nicht-blockierend und
liefert `some(body)` bei einem vollständigen Frame, sonst `none`. Die
Sequenznummer ist ein monotoner `uint16`-Zähler, der bei `0xffff` umläuft
(kein Fehler, kein Reset).

## §3 Async-Reader

`AsyncReader`/`newAsyncReader` MUSS den idiomatischen Nim-`async`/`await`-Pfad
bereitstellen: `recv(): Future[seq[byte]]` ist eine `{.async.}`-Prozedur, die
erst resolved, sobald ein vollständiges Frame empfangen und entrahmt wurde.
Intern MUSS das Polling über `sleepAsync` erfolgen (kooperatives Yielding via
`asyncdispatch`), nicht über einen dem Aufrufer sichtbaren Busy-Spin.

## §4 Reliable Stream

Reliable Delivery MUSS dem Vertrag aus [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md)
folgen:

- **State-Machine (§3):** Sender `submit`/`pendingHeartbeat`/`recvAcknack`/
  `getInFlight`; Receiver `recvData`/`drainInOrder`/`pendingAcknack`/`reset`
  — idiomatische Nim-Namen, identische Semantik zur Referenz
  `crates/xrce/src/reliable.rs`.
- **Konstanten (§3.1):** `SENDER_WINDOW = 16`, `RECEIVER_BUFFER = 64`,
  `HEARTBEAT_PERIOD = 500ms`, `MAX_PAYLOAD = 65535`, RFC-1982-16-Bit-
  Sequenznummern, reliable Stream-Id `≥ 128` (Bit 7 gesetzt).
- **Wire (§4):** HEARTBEAT (`0x0B`) und ACKNACK (`0x0A`) byte-identisch zu den
  Referenz-Goldens (`golden_heartbeat_le.bin`/`golden_acknack_le.bin`).
- **Async-Entkopplung (§2/§5):** der Producer MUSS nie in den Kernel — er
  enqueued wait-free oder lock-basiert in eine Queue; ein dedizierter
  Drain-Thread besitzt den Socket und den Sender-State (WRITE_DATA senden,
  periodisches HEARTBEAT, ACKNACK-Empfang, Retransmit aus der History). Die
  konkrete Queue-Implementierung (wait-free Ring vs. lock-basiertes
  stdlib-`Channel`) ist nicht vorgeschrieben; ein Latenz-Messwert
  (`enqueue` vs. inline `sendto`), der die Entkopplung zeigt, ist Pflicht
  (§5 Punkt 4 der Referenz-Spec).
- **Test-/Beleg-Pflicht (§5):** Unit (State-Machine-Spiegel), Byte-Golden
  (HEARTBEAT/ACKNACK), E2E Loss-Recovery live gegen den geteilten Rust-Peer
  mit injiziertem Drop, Latenz-Bench, ein lauffähiges
  `example_reliable_*` — alle fünf gelten unverändert für `endpoints/nim`.

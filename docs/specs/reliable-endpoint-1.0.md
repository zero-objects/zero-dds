# `reliable-endpoint` v1.0 — Reliable Delivery als Endpoint-Fähigkeit

ZeroDDS Vendor-Spec. Implementiert in `crates/xrce/src/reliable.rs` (Referenz-State-Machine)
und `crates/xrce/src/submessages/acknack.rs` / `heartbeat.rs` (Wire-Codec). Baut auf
DDS-XRCE 1.0 §8.4.10/§8.4.11 (Reliable Streams) und §8.3.2 (Stream-IDs) auf.

## §1 Entscheidung — Reliable Delivery liegt im Endpoint, nicht am Hub

Der Endpoint ist der Datenproduzent. Reliable Delivery am Rand (Quelle → Aggregator,
Aggregator → Bus) existiert nur, wenn der reliable **Writer-State** im Endpoint sitzt:
ein History-Cache bis zum ACK, ein HEARTBEAT-Announce, ACKNACK-Empfang, Retransmit.
Das an einen zentralen Hub auszulagern macht den Endpoint→Hub-Sprung best-effort und
bricht genau den einen Sprung, der nicht verlieren darf.

Der Schnitt:

- **Hub:** Discovery (SPDP/SEDP), QoS-Matching, Routing.
- **Endpoint (Pflicht):** reliable *Delivery* — ein stateful Writer und Reader. Ein
  begrenzter, komponierbarer Baustein (XRCE Reliable Stream, `stream_id ≥ 128`), kein
  Voll-Stack. Kollidiert nicht mit einem dünnen Endpoint: Discovery bleibt draußen.

## §2 Zwei Achsen, ein Bauteil

Async-Write und Reliability sind dasselbe Bauteil: der **drain-Task** des Async-Writers
hält den reliable State.

Das Motiv für Async-Write ist nicht das Warten auf ACKNACK — es ist die **Syscall-Latenz
in einem engen Producer-Loop**. Ein `sendto` ist ein Kernelübergang (hunderte ns bis µs);
auf einem Multi-Core-Aggregator, der im ns-Takt produziert, ist das Faktor 100–1000 zu
teuer. Der Producer darf nie in den Kernel.

```
producer   →  enqueue(sample)               // wait-free, ns, kein Kernel
drain-task →  submit(sample) → seq, history  // reliable Writer-State
           →  sendmmsg(WRITE_DATA…)           // gebündelt, ein Syscall pro N
           →  HEARTBEAT (periodisch)
           ←  ACKNACK  →  retransmit(missing) aus history
           →  prune(acked)
Backpressure = history/window voll (Producer entscheidet lokal: drop / spin / count)
```

Ein inline arbeitender „AsyncWriter", der den Transport-Deliver direkt auf dem
Producer-Thread aufruft — ein Syscall, kein Ring, kein Drain, keine History — liefert
weder Latenz-Entkopplung noch reliable State und ist mit diesem Bauteil nicht konform.

## §3 Kanonischer State-Machine-Kontrakt

Jede Sprache spiegelt diese Oberfläche (idiomatische Namen, identische Semantik).
RFC-1982 16-bit Sequenznummern.

### §3.1 Konstanten (verbindlich)

| Konstante | Wert | Begründung |
|---|---|---|
| `HEARTBEAT_PERIOD` | 500 ms | Spec empfiehlt 100 ms; 500 ms konservativ ohne Tx-Pacing-Schicht |
| `SENDER_WINDOW` | 16 | entspricht der 16-bit-ACKNACK-Bitmap |
| `RECEIVER_BUFFER` | 64 | DoS-Grenze gegen eine Reorder-Flut |
| `MAX_PAYLOAD` | 65535 | u16-Submessage-Längenlimit |
| reliable stream id | Bit 7 gesetzt (`≥ 128`) | Spec §8.3.2 |

### §3.2 Sender

- `submit(payload) -> seq` — Fehler `PayloadTooLarge` (`len > MAX_PAYLOAD`) und
  `WindowFull` (`in_flight ≥ SENDER_WINDOW`); weist eine monotone `seq` zu, buffert in
  `in_flight: seq→payload`.
- `pending_heartbeat(now) -> HEARTBEAT?` — `None` wenn `in_flight` leer; feuert beim
  ersten Aufruf, danach erst nach `HEARTBEAT_PERIOD`; Body `{ first_unacked, last_unacked, stream_id }`.
- `recv_acknack(payload)` — `base = first_unacked`; alles `< base` (RFC-1982) ist
  acknowledged und wird entfernt; im Fenster `[base, base+16)` bedeutet ein gesetztes Bit
  missing (behalten), ein gelöschtes Bit acked (entfernen).
- `get_in_flight(seq)` — Retransmit-Lookup.

### §3.3 Receiver

- `recv_data(seq, payload)` — Duplikat (`seq < expected`) still verworfen; schon
  gebuffert ist ein No-op; Fehler `BufferFull` bei `RECEIVER_BUFFER`; sonst in den
  Out-of-Order-Buffer `received: seq→payload`.
- `drain_in_order() -> [(seq, payload)]` — liefert lückenlos ab `expected`, advanced
  `expected`.
- `pending_acknack(hint_last_seen) -> ACKNACK` — Bitmap der fehlenden Slots in
  `[expected, expected+16)`; Slots jenseits `hint_last_seen` werden nicht als missing markiert.
- `reset()` — kompletter State-Clear (nach einer RESET-Submessage).

## §4 Wire-Format (byte-golden)

`HEARTBEAT` und `ACKNACK` sind byte-identisch zur Rust-Referenz:

- `AckNack { first_unacked_seq_num: i16, nack_bitmap: [u8;2] LE, stream_id: u8 }` —
  XRCE-Submessage-ID `0x0A`, Flags `0x01` (nur E-Flag = LE gesetzt).
- `Heartbeat { first_unacked_seq_nr: i16, last_unacked_seq_nr: i16, stream_id: u8 }` —
  XRCE-Submessage-ID `0x0B`, Flags `0x01` (nur E-Flag = LE gesetzt).
- `WriteData` (Nutzdaten-Submessage des reliable Streams) — XRCE-Submessage-ID `0x07`,
  Flags `0x03` (E-Flag = LE plus `DataFormat::Sample`-Bits) im typischen Reliable-Case.
- Jede Submessage trägt XRCE-Submessage-Header (ID, Flags, Len LE) + die reliable
  Stream-ID in der Session/Stream-Zeile (`stream_id ≥ 128`, Bit 7 gesetzt, Spec §8.3.2).

Golden-Quellen: `golden_heartbeat_le.bin` / `golden_acknack_le.bin` des C-SDKs
(`endpoints/c/test/test_reliable.c`) und `crates/xrce`. Jede Sprache assertet dagegen.

## §5 Test- & Beleg-Pflicht (pro Sprache)

1. **Unit** — spiegelt die Referenz-Tests: monotone seq, window-full, heartbeat
   first/silence/empty, acknack-clear, reorder, duplicate-drop, buffer-full,
   pending-acknack-Bitmap, reset.
2. **Byte-golden** — HEARTBEAT + ACKNACK identisch zu `golden_*.bin`.
3. **E2E Loss-Recovery** — live gegen den Rust reliable Peer (`zerodds-endpoint-e2e`),
   `stream_id ≥ 128`, mit injiziertem Drop (jedes n-te Datagramm / `netem drop`); Assert:
   **alle** Samples lückenlos in-order geliefert trotz Loss.
4. **Latenz-Bench** — Producer `write→return` im async-entkoppelten Pfad vs. inline
   Deliver; ein Messwert (ns/µs), der die Entkopplung zeigt.
5. **Example** — ein lauffähiges `example_reliable_*` (Aggregator sendet N reliable
   Samples, Reader gibt die lückenlose Sequenz aus). Generiert, kein Stub.

Kein false-green: jeder Test läuft auf realer Toolchain, lauter Skip nur bei fehlender
Toolchain.

## §6 Außerhalb des Scopes (getrennte Runden)

- Fragmentierung (FRAGMENT-Submessage, Payload > 64 KiB).
- RESET-Handshake über die Live-Verbindung.
- Kernel-Bypass-Drain (io_uring / AF_XDP) — eine Optimierung nach der `sendmmsg`-Baseline.

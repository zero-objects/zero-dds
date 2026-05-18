# Phase 1 Performance- und Komplexitaets-Audit

Stand 2026-04-20. Scope: Protokoll-Hot-Path (`rtps`, `discovery`, `types`,
`cdr`, `qos`, `transport-{udp,tcp,shm}`). Keine Micro-Opts (Inlining,
SIMD, Branch-Hints) — die sind Phase 3.

Legende: **C**ritical (DoS/Skalierung), **H**igh (>5% Overhead erwartet),
**M**edium, **L**ow.

---

## Versteckte Schleifen / Super-linear Complexity

### F1 [H] Mutable-Struct-Assignability ist O(n·m)
`crates/types/src/assignability.rs:362-391`. Fuer jedes Reader-Member wird
per `find` linear ueber `writer.member_seq` gesucht. Bei 100 Membern:
10 000 Member-Compares **und** rekursive `is_assignable`-Calls pro Paar.
TypeMatcher wird bei jedem SEDP-Event getriggert → skaliert schlecht.
**Fix:** einmalig `BTreeMap<member_id, &wm>` vor der Schleife bauen
(O(n log n) pre-pass), dann O(m log n) Lookups.

### F2 [M] Enum-Assignability ist O(n·m)
`crates/types/src/assignability.rs:424-429`. `any(|rl| rl.value == wl.value)`
pro Writer-Literal. Bei einem 256-Literal-Enum: 65 k Compares.
**Fix:** Reader-Literal-Werte in `BTreeSet<i32>` pre-sammeln.

### F3 [H] SEDP-Cache-Insert ist O(N) linear ueber alle Eintraege
`crates/discovery/src/sedp/cache.rs:167-184` und `:209-224`. Pro Insert
zwei lineare Scans (count per prefix + LRU-min). Bei 10 k Discoveries
→ 10 k² = 100 M Compares beim Fuellen. SPDP-Fan-in skaliert bei grossen
Deployments quadratisch.
**Fix:** Index `BTreeMap<GuidPrefix, BTreeMap<SequenceNumber, Guid>>`
fuer pro-Participant LRU, dann O(log n) Insert/Evict.

### F4 [M] ReaderProxy-Lookup ist O(P)
`crates/rtps/src/reliable_writer.rs:191`, `:203`, `:347`. `iter().position()`
bei jedem `add/remove_reader_proxy` und **bei jedem AckNack/NackFrag**.
Bei grossen Fan-outs (P>>50) merkbar, weil AckNack-Rate pro Reader laeuft.
**Fix:** Paralleler `BTreeMap<Guid, usize>`-Index auf den Vec.

### F5 [M] ReliableReader `proxy_index_by_writer_id` ist O(W)
`crates/rtps/src/reliable_reader.rs:412`. Wird pro eingehender DATA/
DATA_FRAG/HEARTBEAT/GAP aufgerufen — d.h. pro empfangenem Paket. Bei
W Writern skaliert das linear pro Rx-Paket.
**Fix:** `BTreeMap<EntityId, usize>`.

### F6 [L] FragmentBuffer::missing scannt komplette Fragment-Range
`crates/rtps/src/fragment_assembler.rs:107-112`. Bei 16k-Fragment-Samples
iteriert er 16 k Mal pro NACK_FRAG. BTreeSet::contains ist O(log n), also
O(n log n). Akzeptabel bei DoS-Cap, aber Fix ist billig:
**Fix:** ueber `received` iterieren (sortiert) und Luecken direkt
emittieren — O(n).

---

## Hot-Path-Allocations

### F7 [H] Doppelte Payload-Allocations beim Writer-Send
`crates/rtps/src/reliable_writer.rs:231` (cache-insert `payload.clone()`),
`:412, :583, :655` (`to_vec()` pro DATA/DATA_FRAG-Submessage).
Pro `write()` auf N Readers entstehen 1 Cache-Copy + N Submessage-Copies.
Beim Fragment-Resend dito jede Runde.
**Fix:** `CacheChange::payload` auf `Rc<[u8]>`/`Arc<[u8]>` ziehen und als
Borrowed-View in Submessages durchreichen (refactor über `write_body`-Sig
— geht alloc-frei).

### F8 [H] `change.payload.clone()` im tick()
`crates/rtps/src/reliable_writer.rs:270, :288`. Fuer jedes resend-Request
ein Clone des ganzen Sample-Bodys, auch wenn das Submessage-Build eine
Borrowed-View nehmen koennte.
**Fix:** Rc-Share (siehe F7) oder `&payload`-Call in `append_data`/
`build_data_frag_datagram`.

### F9 [M] ReliableReader kopiert Payload 3x
`crates/rtps/src/reliable_reader.rs:287` (DATA into cache),
`:426` (DeliveredSample aus cache), und pro NACK-Runde. Kein Zero-Copy.
**Fix:** `Arc<[u8]>` auf `CacheChange.payload`; Delivery-API erlaubt
Ref-Counted-Slice.

### F10 [M] DATA-Datagramm-Build allokiert DataSubmessage-Struct auf Stack
mit `.to_vec()`-Payload pro Datagram. Bei SEDP-burst (z.B. announce
100 Topics an 50 Peers) sind das 5000 transient Vecs.
**Fix:** `DataSubmessage::write_body_into(&mut Vec<u8>, payload: &[u8])`
statt Struct mit owned payload.

### F11 [L] Cdr-Extensibility-Encoder: Per-Member-Inner-Buffer
`crates/cdr/src/struct_enc.rs:41-54` (`encode_appendable`) und
`:104-133` (`encode_mutable_member`). Jedes Nested-Struct bzw. jedes
Mutable-Member allokiert einen eigenen `BufferWriter` (=Vec). Bei tief
genesteten Strukturen compoundiert das.
**Fix (Phase 2):** Single-Buffer + Placeholder-Offsets, Length patchen
nach Body-Encode (klassisches "write-ahead with back-patch").

### F12 [L] ParticipantData `targets_for` klont Locator-Vecs
`crates/rtps/src/reliable_writer.rs:389-395`. `Rc::new(Vec::clone)` pro
Reader pro Submessage-Run. Rc wird erst innerhalb `tick()` mit anderen
Aufrufen geshared. Ergibt O(P) Vec-Clones pro tick.
**Fix:** Locator-Liste direkt als `Rc<Vec<Locator>>` im ReaderProxy
speichern, dann ist `targets_for` ein `Rc::clone` statt Vec-Clone.

---

## Lock-Contention / Scope-Mismatch

### F13 [M] TcpTransport haelt Inbound-Mutex ueber Condvar-wait
`crates/transport-tcp/src/tcp_transport.rs:384-396`. Korrekt laut
Condvar-Semantik (wait gibt Lock frei), aber die Wait-Schleife checkt
nicht auf Shutdown. Sender weckt mit `notify_one()` (:306) — OK, weil
SPSC-Pattern. **Kein Fix noetig**, aber ein `shutdown`-Flag (wie bei
SHM) macht Drop sauberer.

### F14 [M] ShmTransport `send` klont Payload innerhalb des Peer-Locks
`crates/transport-shm/src/shm_transport.rs:106-110`. `buf.push(data.to_vec())`
unter `peer.buffer.lock()`. Bei grossen Payloads verlaengert sich der
Lock-Held-Zeitraum unnoetig.
**Fix:** `let owned = data.to_vec(); let mut buf = ...; buf.push(owned);`
— klont vor Lock.

### F15 [L] Shared peer-pool in TcpTransport: OK, aber Eviction ist
nicht-LRU
`crates/transport-tcp/src/tcp_transport.rs:354-363`. Bei MAX_PEERS wird
**der kleinste Key** evicted (Kommentar erkennt das an). Funktional
korrekt, aber bei chatty-Angreifer loescht das legitime Peers. Phase-2
sollte IndexMap + Insertion-Order.

### F16 [L] UDP `recv()` allokiert 64 KiB Stack-Array pro Call
`crates/transport-udp/src/udp_transport.rs:171-194`. Stack-Alloc ist
billig, aber `buf[..len].to_vec()` bei jedem Paket allokiert Heap.
**Fix (spaeter):** Recv-Buffer-Pool, Ring-Buffer oder `recvmmsg`.

---

## Backpressure-Gaps / DoS-Caps

### F17 [C] ParameterList::from_bytes ohne Parameter-Count-Cap
`crates/rtps/src/parameter_list.rs:144-177`. Schleife bis Sentinel ohne
`max_parameters` und ohne `max_total_bytes`. Ein Angreifer kann ein
SEDP-Packet mit 10 000 winzigen Parametern senden → 10 000 Vec-Allocs
im Decoder-Pfad pro empfangenem Datagramm. Kombiniert mit Multicast-
SPDP = ideales Amplification-Target.
**Fix:** `DEFAULT_MAX_PARAMETERS = 256` + Fruehabort bei `pos >
MAX_PLIST_BYTES (z.B. 64 KiB)`. Analog zu
`FragmentAssembler::AssemblerCaps` als struct konfigurierbar.

### F18 [H] TypeLookup Registry-Poisoning ohne Count-Cap
`crates/discovery/src/type_lookup/mod.rs:99-117`. Jedes Reply haengt
alle mitgesendeten TypeObjects in die Registry. Kein
`max_registry_size`, kein Pending-Request-Match. Jeder Peer kann die
Registry fluten.
Zusaetzlich werden TypeObjects `clone()`ed nur um den Hash zu berechnen
(Zeile 106/111 `m.clone()`). Hash-Computation serialisiert das
TypeObject — doppelt teuer.
**Fix:** (a) Cap auf Registry-Groesse, (b) `compute_hash(&m)` statt
`compute_hash(&TypeObject::Minimal(m.clone()))`, (c) Pending-Requests-
Tabelle fuer Match-gated Inserts (schon im Modul-Doc als TODO markiert).

### F19 [M] HistoryCache remove_up_to ist O(n log n)
`crates/rtps/src/history_cache.rs:186-191`. `split_off` erzeugt eine
neue BTreeMap, dann wird `self.changes = keep` ueberschrieben — doppelter
Move. Funktional OK, Cost akzeptabel, aber: Logik ist invertiert lesbar
(Map wird zwei Mal getauscht). **Fix (Cosmetic):** `self.changes.retain(|k, _| k > &sn)`
oder `split_off` direkt zurueckweisen. Perf-Unterschied minimal; Clarity
gewinnt.

---

## Async-Unfriendly Sync-Spots (Info fuer Phase 2)

### F20 [L] `Transport::recv` ist Blocking
Sowohl TCP (Condvar::wait) als auch SHM (Condvar::wait) und UDP
(socket.recv_from) blockieren. Fuer tokio-Portierung in Phase 2 muessen
die Transport-Traits `poll_recv` / `async fn recv` bekommen.
Kein Fix jetzt — nur als Marker.

---

## Zusammenfassung

- **Critical (sofort Phase-1-Hotfix):** F17 (PList-Cap) — reine
  Hygiene, klein.
- **High (naechstes Review):** F1, F3, F7, F8, F18 — Skalierung +
  Hot-Path-Allocs + DoS.
- **Medium/Low:** Groesstenteils Micro-Refactor oder Phase-2-Scope.

Geschaetzter Gesamt-Gain bei F7+F8+F10 (Zero-Copy-Payload):
~30-50% Throughput fuer Reliable-Writer-Tick unter Last (N Readers,
Resend-lastig), basierend auf Profil-Erfahrungen bei Cyclone/Fast-DDS.

F1+F3+F5 zahlen erst bei grossen Deployments (>100 Endpoints). Heute
nicht blocking, aber fuer Sales-Demo "Wir skalieren auf 10 k Topics"
muss F3 adressiert werden.

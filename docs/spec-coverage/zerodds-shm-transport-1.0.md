# ZeroDDS-SHM-Transport 1.0 — Spec-Coverage

**Quelle:** dieses Dokument ist zugleich die Spec. Für Shared Memory
normiert OMG nichts außer dem vendor-reservierten Locator-Kind
(DDSI-RTPS 2.5 §9.4.5); Segment-Layout, Synchronisation und
Cleanup-Protokoll sind vendor-spezifisch. Die normativen Anforderungen
stehen in den `##`-Sektionen §2 bis §7; jedes `###`-Item darunter trägt
Spec-Zitat, Repo-Anker, Test und Status.

Implementation: `crates/transport-shm/src/ring.rs` — Segment-Layout und
SpSc-Ringbuffer in `core`; `crates/transport-shm/src/posix.rs` — benannte
Segmente, Sperren, Crash-Recovery; `crates/transport-shm/src/region.rs` —
der Transport über einem Bereich des Integrators ohne Betriebssystem
(§6.4). Der Zero-Copy-Loan-Pfad aus §7 liegt in
`crates/flatdata/` und `crates/zerodds-c-api/src/shm_loan_ffi.rs`; siehe
§7.2 zur Abgrenzung.

Offene Punkte: `zerodds-shm-transport-1.0.open.md`.

## §1 Scope und Spec-Status

### §1.1 Vendor-reservierter Locator-Kind

**Spec:** DDSI-RTPS 2.5 §9.3.2.4 — `0x00000003` bis `0x01ffffff` sind für
vendor-spezifische Locator-Kinds reserviert. ZeroDDS belegt `0x01F0_0001` für
Shared Memory. Das Port-Feld ist für SHM bedeutungslos und MUSS `0` sein;
die 16 Adress-Bytes tragen die Segment-ID.

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind::Shm`,
`Locator::shm`, `Locator::SHM_ANY`); gebunden in
`crates/transport-shm/src/posix.rs` (`PosixShmTransport::local_locator`,
`peer_locator`, Kind-Prüfung in `Transport::send`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::rtps_9_3_2_4_locator_kind_is_the_vendor_reserved_shm_value`
(prüft den Zahlwert `0x01F0_0001`, die Lage im Bereich `0x00000003` bis `0x01ffffff` und `port == 0`),
`crates/transport-shm/src/posix.rs::tests::shm_1_1_local_locator_is_shm_with_own_id`,
`crates/transport-shm/src/posix.rs::tests::shm_1_1_send_rejects_non_shm_locator`,
`crates/rtps/src/wire_types.rs::tests::locator_kind_roundtrip`.

**Status:** done

### §1.2 Was OMG für Shared Memory nicht normiert

**Spec:** DDSI-RTPS 2.5 kennt kein SHM-Wire-Format, kein Segment-Layout,
kein Synchronisations-Modell und kein Cleanup-Protokoll. Cyclone DDS
bindet iceoryx als Fremdkomponente ein, FastDDS und RTI haben je eigene
SHM-Transporte. Die Sektion hält den Scope fest und stellt keine
Implementierungs-Anforderung.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Abgrenzung des Geltungsbereichs; die Anforderungen stehen in den Items ab §2.

## §2 Segment-Layout

```text
  offset 0:   magic: u32 BE   "ZSHM"
  offset 4:   version: u32 LE
  offset 8:   capacity: u64 LE         (Daten-Region, ohne Header)
  offset 16:  head: AtomicU64          (nächster Schreib-Offset, Writer-owned)
  offset 24:  tail: AtomicU64          (nächster Lese-Offset, Reader-owned)
  offset 32:  shutdown: AtomicU32      (0=active, 1=owner-gone)
  offset 36:  consumer_attached: AtomicU32 (Consumer, die das Segment offen haben)
  offset 40:  owner_epoch: AtomicU32   (jeder Owner, der öffnet, zählt hoch)
  offset 44:  reserved
  offset 48:  session_head: u64        (§6.4; in einem benannten Segment 0)
  offset 56:  reserved (Padding zur 64-Byte-Cache-Line)
  offset 64:  data-region [capacity bytes]
```

### §2.1 Header-Felder

**Spec:** §2 — der Ersteller MUSS Magic `"ZSHM"` big-endian auf Offset 0,
die Layout-Version little-endian auf Offset 4 und die Kapazität der
Daten-Region little-endian auf Offset 8 schreiben. Der Header ist 64 Byte
lang, damit `head` und `tail` nicht mit der Daten-Region in dieselbe
Cache-Line fallen. Auf Offset 36 zählt `consumer_attached` die
Consumer-Handles, die das Segment offen haben: jeder Consumer erhöht beim
Öffnen um 1 und senkt beim Drop um 1; der Owner liest daraus, ob ein
Consumer angehängt ist (`peer_attached`). Auf Offset 40 erhöht jeder
Owner, der das Segment öffnet, `owner_epoch` um 1 und merkt sich den
neuen Wert; welcher Owner das Segment beim Drop beendet, regelt §5.3. Beide
Felder sind `u32` in der Byte-Reihenfolge des Hosts. Auf Offset 48 steht
`session_head` (`u64`), das nur ein Owner über einem Bereich des
Integrators schreibt (§6.4); in einem benannten Segment bleibt es null,
ebenso Offset 44 bis 48 und 56 bis 64. Die Kapazität liegt zwischen 32 Byte
und 2³¹ Byte: jede Seite liest die Position der anderen nur in ihren
unteren 32 Bit (§6.4). Ein Anhänger übernimmt die Kapazität aus Offset 8 statt der
eigenen Konfiguration und senkt sein `max_datagram` auf
`(capacity - 16) / 2`, wenn es größer ist; die Konfiguration verlangt
`capacity >= 2 * max_datagram + 16`. Beide Felder kamen vor dem ersten
Release der Version 2 hinzu und gehören zu jeder Version ab 2; die
aktuelle Version 3 kennzeichnet den Ort der Sperren (§5.4).

**Repo:** `crates/transport-shm/src/ring.rs` (`SHM_MAGIC`,
`SHM_VERSION`, `HEADER_BYTES = 64`, `MAX_CAPACITY`,
`Ring::create_in_zeroed`, `Ring::attach`, `Ring::consumer_attached`,
`Ring::owner_epoch`), `crates/transport-shm/src/posix.rs`
(Initialisierung im `is_creator`-Zweig von `PosixShmTransport::open`,
Übernahme der Kapazität im Anhänger-Zweig, `peer_attached`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_2_1_owner_writes_magic_and_version_into_the_header`
(liest Magic, Version und Kapazität direkt aus dem Mapping und vergleicht
das Magic gegen `b"ZSHM"`),
`crates/transport-shm/src/posix.rs::tests::shm_2_1_attach_and_epoch_words_sit_at_offsets_36_and_40`
(liest beide Wörter roh aus dem Mapping: ein Owner ergibt Epoche 1, zwei
Consumer-Handles ergeben 2, ein zweiter Owner Epoche 2, Offset 44 bis 64
ist null),
`crates/transport-shm/src/posix.rs::tests::shm_2_1_the_owner_sees_the_consumer_attach_and_leave`,
`crates/transport-shm/src/posix.rs::tests::shm_2_1_sides_with_different_ring_sizes_share_the_creators_ring`
(Owner und Consumer mit verschiedener Ringgröße teilen den Ring des
Erstellers, in beiden Reihenfolgen),
`crates/transport-shm/src/posix.rs::tests::shm_2_1_an_attacher_lowers_its_max_datagram_to_the_creators_ring`
(ein Ring von 4096 Byte, der Anhänger mit `max_datagram` 32 752: als Owner
meldet er `max_message_size` 2040, weist 2041 Byte mit `PayloadTooLarge`
ab und schickt 2040 Byte; als Consumer meldet er ebenfalls 2040),
`crates/transport-shm/src/posix.rs::tests::shm_2_1_invalid_config_capacity_too_small_for_max_datagram`.

**Status:** done

### §2.2 Zurückweisung fremder Segmente

**Spec:** §2 — beim Attach MUSS der Opener Magic und Version prüfen und
ein fremdes oder neueres Segment ablehnen, statt seine Bytes als Ring zu
interpretieren. Eine Layout-Änderung erhöht die Version.

**Repo:** `crates/transport-shm/src/posix.rs` (`else`-Zweig von
`PosixShmTransport::open`, `PosixShmError::InvalidHeader`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_2_2_attach_rejects_foreign_magic`
(Segment mit fremdem Magic, angelegt über dieselben
`shared_memory`-Primitiven, wird mit `InvalidHeader` abgelehnt),
`crates/transport-shm/src/posix.rs::tests::shm_2_2_attach_rejects_unknown_version`
(`SHM_VERSION + 1` wird abgelehnt).

**Status:** done

### §2.3 Memory-Ordering von `head` und `tail`

**Spec:** §2 — der Writer MUSS `head` mit `Release` speichern, nachdem er
die Frame-Bytes geschrieben hat; der Reader MUSS `head` mit `Acquire`
laden, bevor er sie liest. Sichtbar wird ein Frame damit genau dann, wenn
der Reader den neuen `head`-Wert sieht — auch über Prozessgrenzen hinweg.

**Repo:** `crates/transport-shm/src/ring.rs` (`store_position`,
`load_position`, `low`, die `Ordering::Release`- und
`Ordering::Acquire`-Stellen in `Ring::try_push` und `Ring::next`; ohne
64-Bit-Atomics das Modul `split`).

**Tests:** `crates/transport-shm/tests/l1_cross_process.rs::shm_2_3_l1_shm_datagram_crosses_process_boundary`
(zwei OS-Prozesse, ein Segment; der Empfänger sieht die Bytes des
Senders),
`crates/transport-shm/src/posix.rs::tests::shm_2_3_open_concurrent_two_threads_both_bound`,
`crates/transport-shm/src/posix.rs::tests::shm_2_3_many_frames_roundtrip_with_wraparound`.

**Status:** done

## §3 Frame-Format

Length-Prefix-Format innerhalb der Daten-Region:

```text
+---------+---------+---------+---------+----- ...
| len: u32 LE                           | bytes [len]
+---------+---------+---------+---------+----- ...
```

### §3.1 Length-Prefix-Frame im Ring

**Spec:** §3 — jeder Frame beginnt mit seiner Länge als `u32` little-endian,
gefolgt von genau so vielen Bytes. `head` und `tail` sind monoton
wachsende Zähler; die Position im Ring ist `head % capacity`. Die
Belegung ist `head - tail`.

**Repo:** `crates/transport-shm/src/ring.rs` (`Ring::write_u32`,
`Ring::read_u32`, `Ring::try_push`, `Ring::next`, `Ring::occupied`),
`crates/transport-shm/src/posix.rs` (`push_frame`, `pop_frame`,
`occupied_bytes`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_3_1_open_owner_then_consumer_roundtrip_single_frame`,
`crates/transport-shm/src/posix.rs::tests::shm_3_1_occupied_bytes_reflects_send_state`,
`crates/transport-shm/src/posix.rs::tests::shm_3_1_recv_after_send_drains_occupied_bytes`.

**Status:** done

### §3.2 Padding-Frame am Ring-Ende

**Spec:** §3 — ein Frame wird nie über das Ring-Ende hinweg gesplittet.
Reicht der zusammenhängende Platz am Ende nicht, MUSS der Writer einen
Padding-Frame mit `len = 0xFFFF_FFFE` setzen und an den Anfang springen;
der Reader überspringt ihn und zählt ihn zur Diagnose mit. Bleiben unter
4 Byte übrig, wird der Rest ohne Marker übersprungen.

**Repo:** `crates/transport-shm/src/ring.rs` (`PADDING_FRAME_LEN`,
Wrap-Zweig in `Ring::try_push` mit Padding und Überspringen eines Rests
unter 4 Byte, Padding- und `tail_space < 4`-Zweig in `Ring::next`),
`crates/transport-shm/src/posix.rs` (`padding_frames_seen`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_3_2_fill_ring_triggers_wraparound_padding`
(300-Byte-Ring, fünf 60-Byte-Frames — der Wrap ist erzwungen und jeder
Frame kommt unverändert an; vor dem Lesen des fünften steht auf Offset
256 der Marker `0xFFFF_FFFE`, danach zählt der Consumer genau einen
Padding-Frame),
`crates/transport-shm/src/posix.rs::tests::shm_3_2_a_padding_frame_is_counted_once_per_wrap`
(201 Frames zu 64 Byte im 300-Byte-Ring überqueren das Ring-Ende 50-mal;
`padding_frames_seen()` steht danach auf 50),
`crates/transport-shm/src/posix.rs::tests::shm_3_2_a_remainder_too_small_for_a_padding_header_is_skipped`
(ein Frame endet 2 Byte vor dem Ring-Ende; der nächste geht durch),
`crates/transport-shm/src/posix.rs::tests::shm_3_2_frames_of_every_length_cross_the_ring_end_whole_and_in_order`
(5 000 Runden mit Längen 1–200 Byte; ohne das Überspringen des Rests
brach der Lauf in Runde 16 mit „shm ring full near wraparound“ ab),
`crates/transport-shm/src/posix.rs::tests::shm_2_3_many_frames_roundtrip_with_wraparound`,
`crates/transport-shm/src/posix.rs::tests::shm_3_2_corrupt_frame_counter_default_zero`.

**Status:** done

### §3.3 Längen-Sanity-Check mit Zähler

**Spec:** §3 — der Reader MUSS eine Länge oberhalb der halben Ringgröße
verwerfen, statt die angekündigte Größe zu allokieren, und den Vorgang
zählen. Die Schranke ist die des Rings, nicht das eigene `max_datagram`:
ein Owner, der ein größeres konfiguriert hat, schickt gültige Frames bis
dorthin. Ebenso verwirft und zählt er einen Frame, dessen `4 + len` über
das Ring-Ende oder über den publizierten `head` hinausreicht, einen
Padding-Marker oder einen Rest unter 4 Byte, die über `head` hinaus
springen würden, und einen `head`, der mehr als eine Ringgröße vor `tail`
oder hinter `tail` steht. Kein Byte außerhalb der Daten-Region wird
gelesen oder geschrieben; die Prüfung gilt in jedem Build. Nach dem
Verwerfen setzt der Reader `tail` auf `head`: hinter einer falschen Länge
ist die nächste Frame-Grenze unbekannt, und was der Owner bis `head`
publiziert hat, geht verloren. Ein wartendes `recv` kehrt dann mit
`Io` („shm corrupt frame dropped“) zurück, statt weiter zu warten. Ein
stiller Drop ist nicht zulässig: der Zähler ist das Ops-Signal für einen
defekten oder bösartigen Owner, und die DCPS-Empfangspfade melden jeden
Drop als Ereignis `shm.corrupt_frame` (Level error, Attribute `peer` und
`count`).

**Repo:** `crates/transport-shm/src/ring.rs` (`Ring::next` mit den
Schranken, `Ring::drop_published`, `Ring::in_ring` für jeden Zugriff auf
die Daten-Region), `crates/transport-shm/src/posix.rs` (`pop_frame`,
`corrupt_frame_counter`, `corrupt_frames_seen`, `CORRUPT_FRAME_DROPPED`
in `wait_parked`);
`crates/dcps/src/same_host_channel.rs` (`SameHostChannels::receive`,
`corrupt_frames`, aufgerufen in der Empfangsschleife der Same-Host-Kanäle
in `crates/dcps/src/runtime.rs`); `crates/dcps/src/shm_user.rs`
(`report_corrupt_frames` im Empfangs-Thread je Peer,
`ShmUserTransport::with_observability`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_3_3_corrupt_length_is_dropped_and_counted`
(schreibt halbe Ringgröße + 1 als Längen-Header in den Ring; `pop_frame`
liefert nichts und `corrupt_frames_seen()` steht auf 1),
`crates/transport-shm/src/posix.rs::tests::shm_3_3_a_length_past_the_ring_end_is_dropped_and_counted`
(Länge 200, 100 Byte vor dem Ring-Ende; vorher las der Reader im
Release-Build über das Mapping hinaus),
`crates/transport-shm/src/posix.rs::tests::shm_3_3_a_length_past_head_is_dropped_and_counted`
(Länge 100 bei 8 publizierten Byte; vorher reichte der Reader
unpublizierte Bytes hoch und setzte `tail` hinter `head`),
`crates/transport-shm/src/posix.rs::tests::shm_3_3_skips_and_heads_beyond_what_was_published_are_dropped`,
`crates/transport-shm/src/posix.rs::tests::shm_3_3_the_ring_carries_frames_again_after_a_corrupt_one`
(das wartende `recv` meldet den Drop, er zählt einmal, danach laufen 200
Frames durch),
`crates/dcps/src/same_host_channel.rs::tests::shm_3_3_a_corrupt_frame_is_reported_as_an_event`,
`crates/dcps/src/shm_user.rs::tests::shm_3_3_a_corrupt_frame_on_the_user_transport_is_reported_as_an_event`
(beide schreiben die falsche Länge über die Datei des Segments in
`/dev/shm`, wie ein anderer Prozess sie schriebe; nur Linux),
`crates/transport-shm/src/posix.rs::tests::shm_3_2_corrupt_frame_counter_default_zero`,
`crates/transport-shm/src/posix.rs::tests::shm_3_3_send_rejects_oversize_payload`.

**Status:** done

## §4 Synchronisations-Modell

### §4.1 Ein Segment pro (Owner, Consumer)-Paar

**Spec:** §4 — ein Segment gehört genau einem Paar, nicht einem Owner mit
Multi-Reader-Fan-out. Lock-freies SpSc skaliert linear mit der
Reader-Zahl und hat keine globale Contention; SpmC wie bei iceoryx würde
den Writer am langsamsten Reader bremsen. Preis: N Segmente bei N Readern.

**Repo:** `crates/transport-shm/src/posix.rs` (`segment_os_id` und
`segment_flink` sind Funktionen von `(owner_id, consumer_id)`;
`ShmRole`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_3_1_open_owner_then_consumer_roundtrip_single_frame`,
`crates/transport-shm/src/posix.rs::tests::shm_4_1_send_rejects_wrong_peer`
(ein Locator, der nicht der Peer dieses Segments ist, wird abgewiesen),
`crates/transport-shm/tests/l1_cross_process.rs::shm_2_3_l1_shm_datagram_crosses_process_boundary`.

**Status:** done

### §4.2 Rollen-Trennung von `send` und `recv`

**Spec:** §4 — der Owner schreibt, der Consumer liest. Ein `send` auf der
Consumer-Seite und ein `recv` auf der Owner-Seite MÜSSEN mit einem Fehler
scheitern; sonst würde eine zweite Schreibrichtung die SpSc-Annahme des
Rings brechen.

**Repo:** `crates/transport-shm/src/posix.rs` (Rollen-Prüfung in
`Transport::send` und `Transport::recv`, `ShmRole`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_4_2_consumer_cannot_send`,
`crates/transport-shm/src/posix.rs::tests::shm_4_2_owner_cannot_recv`.

**Status:** done

### §4.3 Idempotentes Öffnen unabhängig von der Rolle

**Spec:** §4 — die SEDP-Match-Reihenfolge ist nicht vorhersagbar: die
Reader-Seite kann vor der Writer-Seite öffnen. Wer zuerst kommt, legt das
Segment an; wer später kommt, hängt sich an. Die Rolle bestimmt nur
send/recv, nicht wer erzeugt. Der SHM-User-Transport der DCPS-Runtime
öffnet beide Segmente eines Peers, sobald SPDP ihn meldet, nicht erst bei
seinem ersten Senden an ihn: ein Best-Effort-Reader sendet nichts
(DDSI-RTPS 2.5 §8.4.2.3).

**Repo:** `crates/transport-shm/src/posix.rs` (`open` — flock, Attach-
Versuch, sonst Create; `open_owner` und `open_consumer` teilen sich
denselben Pfad); `crates/dcps/src/shm_user.rs` (`add_destination`),
`crates/dcps/src/runtime.rs` (`note_user_transport_peer`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_4_3_open_consumer_first_then_owner_roundtrip`,
`crates/transport-shm/src/posix.rs::tests::shm_4_3_consumer_open_succeeds_without_owner_creates_segment`,
`crates/transport-shm/src/posix.rs::tests::shm_4_3_a_consumer_that_made_the_segment_leaves_it_to_the_owner`
(ein Consumer, der das Segment vor dem Owner angelegt hat, entfernt es
beim Drop nicht; ein erneut öffnender Consumer findet den Ring des
Owners),
`crates/transport-shm/src/posix.rs::tests::shm_2_3_open_concurrent_two_threads_both_bound`,
`crates/dcps/tests/shm_user_transport.rs::shm_4_3_a_best_effort_reader_takes_what_a_process_it_never_sent_to_writes`.

**Status:** done

### §4.4 Ereignisgetriebenes Warten statt Poll-Schleife

**Spec:** §4 — ein wartender Consumer MUSS vom Writer geweckt werden,
statt in einer Schleife zu pollen. Der `head`-Wert wird vor `pop_frame`
gelesen, damit ein zwischenzeitlich publizierter Frame das Parken sofort
scheitern lässt (kein verlorener Wakeup). Das gilt auf jeder Plattform:
wo es kein Warten auf eine geteilte Adresse gibt, wartet der Consumer
auf einen FIFO je Segment, `<name>.wake` im Paar-Verzeichnis (§5.4), in
den der Writer nach jedem Frame ein Byte schreibt; ein Byte, das zwischen
dem Blick auf den Ring und dem Parken kommt, bleibt in der Pipe. Die
Türglocke eines Teilnehmers wartet ebenso. Beide Seiten eines Segments
laufen auf einem Host und nutzen denselben Weg: einen Futex auf Linux,
`os_sync_wait_on_address` auf macOS ab 14.4, `__ulock_wait` mit
`UL_COMPARE_AND_WAIT_SHARED` auf macOS ab 10.15, den FIFO auf den übrigen
Unix-Systemen und auf einem älteren macOS, ein benanntes
Auto-Reset-Event auf Windows. Der Owner, der ein Segment beendet, weckt
den wartenden Consumer und entfernt den FIFO.

**Repo:** `crates/transport-shm/src/wait.rs` (`Backend`, `Waiter` mit
`open`, `park`, `wake`, `remove_files`; die Wege `futex`, `os_sync` und
`ulock`, beide zur Laufzeit über `dlsym` aufgelöst, `ulock` erst ab
Darwin 19, `Fifo` und `Event`); `crates/transport-shm/src/posix.rs`
(`wait_parked` und `wake_ring` auf den unteren 32 Bit von `head`, das
Wecken in `push_frame` und im Drop des Owners, der `Waiter` unter dem
Lock des Paares geöffnet); `crates/transport-shm/src/bell.rs`
(`ShmDoorbell::park` und `ring` über denselben `Waiter`, der FIFO unter
dem Lock der Glocke angelegt); `sweep_pair_dir` entfernt einen FIFO,
dessen Segment fort ist (`is_pair_file_type`: `.wake` als FIFO, `.lock`
und `.hold` als reguläre Datei; vorher nahm der Sweep nur reguläre
Dateien und ließ jeden FIFO stehen, den Prozesse ohne ihren Drop
hinterließen).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_4_4_the_consumer_parks_instead_of_polling`
(auf jedem Weg des Hosts: ein untätiger Consumer mit 300 ms Timeout parkt
höchstens dreimal; der Sleep-Poll sah einige hundert Mal auf den Ring),
`crates/transport-shm/src/posix.rs::tests::shm_4_4_a_push_wakes_a_parked_consumer`
(auf jedem Weg: der Consumer hat geparkt, der Push weckt ihn in unter 5 s
gegen ein Timeout von 20 s),
`crates/transport-shm/src/posix.rs::tests::shm_4_4_no_wakeup_is_lost_between_the_look_and_the_park`
(auf jedem Weg 20 000 Runden, in denen der Writer den nächsten Frame erst
schickt, wenn der Consumer den letzten genommen hat; kein `recv` dauert
1,5 s, ein verlorener Wakeup kostete sein Timeout von 3 s),
`crates/transport-shm/src/posix.rs::tests::shm_4_4_owner_drop_wakes_a_waiting_consumer`
(auf jedem Weg: der Drop des Owners weckt den geparkten Consumer, der in
unter 5 s gegen ein Timeout von 20 s den Fehler „owner“ liefert),
`crates/transport-shm/tests/l1_cross_process.rs::shm_4_4_l1_a_push_wakes_a_consumer_in_another_process`
(der Owner in einem Kindprozess schickt 300 ms, nachdem der Consumer
dieses Prozesses geparkt hat; der Consumer wacht in unter 5 s gegen ein
Timeout von 20 s auf),
`crates/transport-shm/src/bell.rs::tests::shm_4_4_a_ring_wakes_the_parked_receiver_and_none_is_lost`
(auf jedem Weg; ein untätiges Parken dauert seine 200 ms),
`crates/transport-shm/src/posix.rs::tests::shm_4_4_owner_drop_signals_consumer_via_shutdown_flag`
(der Owner wird vor dem `recv` gedroppt; es kehrt in unter 200 ms zurück,
obwohl das `recv_timeout` 10 s beträgt),
`crates/transport-shm/src/posix.rs::tests::shm_4_4_interrupt_recv_returns_a_waiting_recv_at_once`,
`crates/transport-shm/src/posix.rs::tests::shm_4_4_interrupt_recv_reaches_a_recv_about_to_park`,
`crates/transport-shm/src/posix.rs::tests::shm_4_4_recv_times_out_when_idle`,
`crates/flatdata/src/posix.rs::tests::flatdata_4_1_futex_notify_wakes_consumer_across_mappings`.
„Auf jedem Weg des Hosts“: die Tests erzwingen nacheinander jeden Weg, den
der Host hat (Linux: Futex und FIFO; macOS 26: os_sync, ulock und FIFO;
Windows: das Event).
Gemessen auf macOS 26 vor den Wegen ulock und FIFO (Mac unter Last,
Lastmittel 97), drei Läufe: 100 wartende Consumer 0,3 % eines Kerns;
Wecklatenz p50 22 bis 24 µs gegen 63 bis 69 µs über UDP-Loopback im
selben Lauf. Mit dem Sleep-Poll hatte eine Messung am selben Mac 4,45 %
eines Kerns je Consumer und p50 177 µs ergeben.

Big-Endian-Linux parkt auf denselben unteren 32 Bit von `head`, die dort
auf Byte 20 liegen (`head_word`); auf s390x unter qemu-user grün (alle
`transport-shm`-Tests auf dem Stand `8f94dbd7d6e`, Linux-Testhost; das
war vor `wait.rs` und vor den Tests je Weg). Windows wartet auf einem benannten
Auto-Reset-Event je Segment (`Event` in `wait.rs`, vorher `RingEvent`),
das der Owner nach jedem Frame setzt; im Nightly 6555 (`windows-smoke`,
`81d8500d495`) liefen
`shm_4_4_interrupt_recv_returns_a_waiting_recv_at_once` und
`crates/transport-shm/src/bell.rs::tests::shm_4_4_a_ring_wakes_the_parked_receiver_and_none_is_lost` auf
Windows grün; das war der Code vor `3861278cf5e`, der das Event nach
`wait.rs` verlegte und den Glocken-Test je Weg neu schrieb.

**Status:** partial — gebaut; gemessen auf dem Linux-Testhost mit Futex
und FIFO (`cargo nextest run --profile merge -p zerodds-transport-shm
--all-targets`). Die macOS-Wege (os_sync, ulock, der FIFO auf macOS)
liefen grün auf einem Mac (macOS 26.2, arm64: derselbe Befehl, 92 Tests,
in der Sandbox von `ci/macos-loopback.sb`); der Job
`merge-macos-transport-shm` auf Runner 8 scheiterte in Pipeline 9916 am
Paar-Verzeichnis (§5.4), sein grüner Lauf dort steht aus. Ein macOS
zwischen 10.15 und 14.3, auf dem ulock der eigene Weg ist,
ist nicht gemessen; auf macOS 26 erzwingen die Tests ihn. Der Weg über das
Event auf Windows und die je Weg neu geschriebenen Tests sind seit
`3861278cf5e` für `x86_64-pc-windows-gnu` mit Clippy übersetzt und unter
Wine 10.0 auf dem Linux-Testhost grün (§6.2); auf Windows sind sie nicht
gelaufen: das tut der Job `platform-windows-transport-shm`.
Der FIFO als eigener Weg der übrigen Unix-Systeme (`native()` in
`wait.rs`) ist für `x86_64-unknown-freebsd` mit Clippy übersetzt, mit libc
auf dem ABI von FreeBSD 11 (`RUST_LIBC_UNSTABLE_FREEBSD_VERSION=11`): mit
dem Standard-ABI der libc (FreeBSD 12) übersetzt `nix` 0.23.2, das
`shared_memory` 0.12 mitbringt, nicht, und damit das Crate für FreeBSD
nicht. Auf einem solchen System ist er nicht gelaufen; auf Linux läuft der
FIFO in den Tests oben als erzwungener Weg.

### §4.5 Backpressure bei vollem Ring

**Spec:** §4 — ist der Ring voll, weil der Reader nicht nachkommt, MUSS
`send` nach einer begrenzten Spin-Phase mit einem benannten Fehler
zurückkehren. Blockieren ohne Ende und stilles Verwerfen sind beide nicht
zulässig.

**Repo:** `crates/transport-shm/src/posix.rs` (`SPIN_LIMIT = 1024`, die
beiden `SendError::Io`-Zweige in `push_frame` für `Push::Full` und
`Push::FullAtWrap` aus `crates/transport-shm/src/ring.rs`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_4_5_full_ring_returns_an_error_instead_of_dropping`
(512-Byte-Ring, 128-Byte-Frames, kein Consumer-Drain; der Fehlertext
nennt „ring full"),
`crates/transport-shm/src/posix.rs::tests::shm_3_3_send_rejects_oversize_payload`.

**Status:** done

## §5 Cleanup-Semantik

### §5.1 Vorhersagbare `os_id`

**Spec:** §5 — der Segment-Name MUSS eine reine Funktion des Paares sein,
damit ein Zombie-Segment nach einem Crash wiedergefunden werden kann. Der
Name beginnt mit `/` und bleibt unter dem Plattform-Limit (macOS
`PSHMNAMLEN` = 31); die Richtung `(A,B)` ist von `(B,A)` verschieden.
Der Name MUSS von jedem Byte beider IDs abhängen: wo er kürzer sein muss
als die IDs in Hex (macOS), trägt er einen festen Hash über beide. Ein
Ausschnitt der IDs reicht nicht, denn die Entity-IDs wiederholen sich
zwischen Teilnehmern: jeder Teilnehmer zählt seine Endpunkte vom selben
Anfang.

**Repo:** `crates/transport-shm/src/posix.rs` (`segment_os_id`,
`segment_os_id_in`, `fnv1a_128`, `segment_flink`, `DEFAULT_FLINK_DIR`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_5_1_segment_os_id_is_deterministic_and_within_platform_limit`
(gleiches Paar liefert denselben Namen, umgekehrtes Paar einen anderen,
Länge unter 31 auf macOS),
`crates/transport-shm/src/posix.rs::tests::shm_5_1_short_names_of_pairs_that_differ_only_in_the_prefix_differ`
(die kurze Form, auf jeder Plattform: zwei Paare mit gleichen Entity-IDs
und anderen Teilnehmern bekommen verschiedene Namen),
`crates/transport-shm/src/posix.rs::tests::shm_3_1_open_owner_then_consumer_roundtrip_single_frame`.

**Status:** done

### §5.2 Crash-Recovery

**Spec:** §5 — vor dem Anlegen eines Segments MUSS ein Zombie desselben
Namens entfernt werden (`shm_unlink`, idempotent — `ENOENT` wird
ignoriert), sonst leckt `/dev/shm` beim N-ten Neustart. Eine
zurückgebliebene Flink-Datei darf das Öffnen nicht scheitern lassen.

**Repo:** `crates/transport-shm/src/posix.rs` (`shm_unlink_by_os_id` auf
jedem Unix-System, vorher nur auf Linux und macOS; auf Windows entfernt
es den Namen der Segment-Datei (`crate::mapping::remove_name`, §6.2),
vorher war es dort leer; Create-Zweig in `open` mit vorherigem Entfernen
des Flinks).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_5_2_a_zombie_segment_without_a_flink_is_unlinked_before_the_create`
(ein Segment unter dem Namen des Paares mit gültigem Header und einem
publizierten Frame der beendeten Sitzung, ohne Flink und ohne Halter;
das Paar öffnet mit leerem Ring und trägt nur den Frame der neuen
Sitzung; `open` zählt dabei kein verwaistes Segment:
das Entfernen vor dem Anlegen nahm den Zombie, nicht der Weg von §5.5,
der beim selben leeren Ring ankäme; auf Windows ebenso, wo vorher der Weg
von §5.5 den Zombie nahm),
`crates/transport-shm/src/posix.rs::tests::shm_5_2_open_recovers_from_stale_flink_of_crashed_owner`
(Flink-Datei eines abgestürzten Owners zeigt auf ein nicht mehr
existentes Segment; `open_owner` räumt auf, und ein Frame läuft danach
durch),
`crates/transport-shm/src/posix.rs::tests::shm_4_3_consumer_open_succeeds_without_owner_creates_segment`.

**Status:** done — gemessen auf dem Linux-Testhost. Ohne das Entfernen
vor dem Anlegen scheitert der erste Test (der Zähler steht auf 1, eine
Probe mit entferntem Aufruf auf dem Linux-Testhost). Auf macOS und den
übrigen Unix-Systemen nur übersetzt (§4.4), nicht gelaufen; der
Windows-Zweig ist unter Wine auf dem Linux-Testhost grün (§6.2), auf
Windows nicht gelaufen.

### §5.3 Shutdown-Flag

**Spec:** §5 — der Owner MUSS beim Drop `shutdown = 1` setzen
(Release-Store). Der Consumer prüft das Flag nach jedem leeren Poll und
kehrt mit einem gezielten Fehler zurück, statt bis zum `recv_timeout` zu
warten. Frames, die vor dem Drop publiziert wurden, kommen noch an — die
Prüfung steht hinter `pop_frame`. Hat nach diesem Owner ein weiterer
Owner dasselbe Segment geöffnet (`owner_epoch` im Header ist nicht mehr
die eigene, §2.1), setzt der Drop des früheren weder das Flag noch
entfernt er den Namen: das Segment gehört dem späteren, und erst dessen
Drop beendet es. Der Drop eines Owners prüft die Epoche und beendet das
Segment unter dem Lock des Paares (§5.4); die Lock-Datei entfernt er
zuletzt. Ein Öffner läuft so ganz vor oder ganz nach dem Drop: entweder
übernimmt er das Segment, oder er findet keinen Namen mehr und legt ein
neues an, dem der Consumer nach dem Shutdown-Flag folgt. Ein abgelöster
Owner sendet nicht mehr: sein `send` liefert einen Fehler („superseded“),
denn der Ring hat einen Producer. Die Owner eines Segments in einem
Prozess teilen sich die Producer-Sperre, und die Epoche steigt unter
ihr; ein Push des früheren endet vor dem Anstieg oder sieht ihn. Die
Same-Host-Kanäle von DCPS schicken einen Frame, den der abgelöste Owner
ablehnt, durch den Kanal, der jetzt offen ist.

**Repo:** `crates/transport-shm/src/ring.rs` (`Ring::shutdown`),
`crates/transport-shm/src/posix.rs` (`Drop for PosixShmTransport` unter `lock_pair` mit dem Vergleich der
`owner_epoch` und dem Drop des Mappings vor dem Entfernen der
Lock-Datei, `is_superseded`, `owner_producer_lock`, die Ablehnung in
`send_with_prefix`, Shutdown-Prüfung in `wait_parked` und `try_recv`);
`crates/dcps/src/same_host_channel.rs` (`SameHostChannels::send_on`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_5_3_frames_published_before_the_drop_still_arrive`
(drei Frames vor dem Drop kommen in Reihenfolge an, der vierte Aufruf
liefert sofort den Fehler mit „owner“),
`crates/transport-shm/src/posix.rs::tests::shm_4_4_owner_drop_signals_consumer_via_shutdown_flag`
(misst die Rückkehrzeit gegen ein 10-s-Timeout und prüft den
Fehlertext auf „owner"),
`crates/transport-shm/src/posix.rs::tests::shm_5_3_a_superseded_owner_leaves_the_segment_to_its_successor`
(der Drop des ersten Owners lässt das Segment dem zweiten; erst dessen
Drop liefert dem Consumer den Fehler),
`crates/transport-shm/src/posix.rs::tests::shm_5_3_a_drop_racing_the_next_owner_leaves_it_a_ring_that_is_read`
(500 Runden: ein Thread droppt den Owner, während der Test hinter einer
Barriere den nächsten öffnet; in jeder Runde liest der Consumer den Ring
des nächsten Owners; vor dem Lock im Drop scheiterten 16 von 20 Läufen,
mit einem Lock, das die Lock-Datei früh entfernte, 5 von 30),
`crates/transport-shm/src/posix.rs::tests::shm_5_3_a_superseded_owner_refuses_to_send`,
`crates/transport-shm/src/posix.rs::tests::shm_5_3_two_owners_of_one_segment_never_push_at_once`
(der frühere Owner sendet ohne Pause, der spätere öffnet und sendet 5000
Frames; jeder gesendete Frame kommt ganz und in Reihenfolge an, kein
Frame wird als defekt verworfen, die Sendungen des früheren enden mit dem
Öffnen),
`crates/dcps/src/same_host_channel.rs::tests::shm_5_3_the_peer_follows_a_channel_made_anew`,
`crates/dcps/src/same_host_channel.rs::tests::shm_5_3_a_send_on_a_superseded_channel_goes_through_the_new_one`,
`crates/dcps/src/runtime.rs::tests::a_same_host_peer_keeps_receiving_when_its_peer_opens_the_channel_again`
(zwei Laufzeiten; a schließt den Kanal zu b und öffnet ihn neu; b liest
den Ring des neuen Owners, auf welchem der beiden Wege auch immer, und
das nächste Sample kommt durch den Kanal).

**Status:** done

### §5.4 Race-Schutz beim Anlegen

**Spec:** §5 — parallele Öffner derselben `os_id` MÜSSEN über Threads und
Prozesse hinweg serialisiert werden, sonst entfernt der eine das gerade
angelegte Segment des anderen. Der Lock wird beim Schließen des Handles
oder beim Prozess-Tod automatisch frei (gleiche Crash-Resilienz wie das
Segment selbst). Der Lock hängt nur am Namen des Segments, nicht am
Flink-Verzeichnis: `<name>.lock` im Paar-Verzeichnis, auf Linux
`/dev/shm` neben den Segmenten, auf macOS `zerodds-shm` im temporären
Verzeichnis des Benutzers, wie das System es nennt
(`confstr(_CS_DARWIN_USER_TEMP_DIR)`, nicht `TMPDIR`), auf den übrigen
Unix-Systemen `/tmp/zerodds-shm-<euid>`, auf Windows `zerodds-shm` im
temporären Verzeichnis. Ein Segment hat Modus 0600, die beiden Seiten
eines Paares laufen also unter einem Benutzer; ein vorhandenes
Paar-Verzeichnis auf macOS und den übrigen Unix-Systemen muss diesem
Benutzer gehören, darf kein Symlink sein und für Gruppe und andere nicht
schreibbar sein, sonst scheitert das Öffnen; Lese- und Suchrecht von
Gruppe und anderen werden entzogen (Modus 0700). Auf macOS ist das
Paar-Verzeichnis auch die Wurzel der Flink-Verzeichnisse, die
`zerodds-dcps` in `TMPDIR` anlegt (`zerodds-shm/<host>/d<domain>`), und
`create_dir_all` legt eine fehlende Wurzel mit dem Modus an, den die
umask lässt, 0755 bei 022. Auf Unix werden die
Paar-Dateien mit Modus 0600 und ohne Folgen eines Symlinks geöffnet. Auf
Windows darf weder das Paar-Verzeichnis noch eine Paar-Datei ein
Analysepunkt sein (symbolischer Link, Junction), sonst scheitert das
Öffnen; Eigentümer und Zugriffsliste des Verzeichnisses werden dort nicht
geprüft: es erbt die des temporären Verzeichnisses, im Normalfall des
eigenen im Benutzerprofil, das nur der Benutzer, `SYSTEM` und die
Administratoren betreten. Findet ein Öffner unter dem Lock keinen
Flink in seinem Verzeichnis, aber einen gehaltenen Segmentnamen (§5.5),
hängt er sich daran. Version 3 kennzeichnet diesen Ort der Sperren: ein
Nutzer der Version 2 sperrte `<flink>.lock` und hielt auf macOS den Flink,
und ein Öffner der Version 3 hielte sein lebendes Segment für verwaist.

**Repo:** `crates/transport-shm/src/posix.rs` (`pair_dir`, `pair_file`,
`private_dir` — öffnet das Verzeichnis mit `O_NOFOLLOW | O_DIRECTORY`,
prüft Eigentümer, Typ und die Schreibbits von Gruppe und anderen an
diesem Deskriptor und setzt dort 0700 —, `plain_dir` und
`is_reparse_point` auf Windows,
`darwin_user_temp_dir`, `open_pair_file`, `lock_pair`,
`acquire_flock_excl` — `flock(LOCK_EX)` auf Unix, `LockFileEx` mit
`LOCKFILE_EXCLUSIVE_LOCK` auf Windows —, `FlockGuard`, `segment_in_use`
auf jeder Plattform, `SHM_VERSION = 3`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_2_3_open_concurrent_two_threads_both_bound`
(zwei Threads öffnen dieselbe `os_id` gleichzeitig; beide sind danach
gültig gebunden),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_users_in_different_flink_directories_share_the_segment`
(Owner und Consumer suchen das Segment in verschiedenen Flink-Verzeichnissen,
etwa unter verschiedenem `TMPDIR`; der zweite hängt sich an das gehaltene
Segment, statt es zu entfernen und ein eigenes anzulegen; vorher nur auf
Linux),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_openers_in_different_flink_directories_race_to_one_segment`
(200 Runden: Owner und Consumer in je eigenem Flink-Verzeichnis öffnen
hinter einer Barriere gleichzeitig; in jeder Runde kommt der Frame an),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_two_processes_open_the_pair_at_once`
(10 Runden: ein Kindprozess öffnet den Owner in seinem Verzeichnis zum
selben Zeitpunkt wie der Test den Consumer in einem anderen; der Frame des
Kindprozesses kommt an),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_the_pair_lock_lies_in_the_pair_dir_not_in_the_flink_directory`
(der Lock liegt im Paar-Verzeichnis, nicht im Flink-Verzeichnis; außerhalb
von Linux hält der Owner `<name>.hold`, und das Paar-Verzeichnis ist
privat; der Drop des Owners entfernt den Lock),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_a_pair_file_behind_a_symlink_is_refused`
(Unix: ein Symlink unter dem Namen der Lock-Datei lässt das Öffnen
scheitern, seine Zieldatei bleibt unverändert),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_a_pair_dir_others_may_only_read_is_made_private_one_they_may_write_is_refused`
(`private_dir` auf jedem Unix: ein fehlendes Verzeichnis wird mit 0700
angelegt, eines mit 0755 angenommen und auf 0700 gesetzt; 0720, 0702,
0775 und 1777 werden mit `PermissionDenied` abgewiesen und behalten ihren
Modus, ebenso ein Symlink auf ein privates Verzeichnis und eine Datei),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_an_open_takes_a_pair_dir_made_0755_and_makes_it_private`
(macOS und die übrigen Unix-Systeme: das Paar-Verzeichnis auf 0755
gesetzt, wie `create_dir_all` es für ein Flink-Verzeichnis unter
`TMPDIR` anlegt; Owner und Consumer öffnen, ein Frame kommt an, das
Verzeichnis hat danach 0700; mit der früheren Prüfung scheiterte ein
Öffnen so auf einem Mac mit „… is not a directory private to user 501“,
wie jedes Öffnen auf Runner 8, Job 238161),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_a_pair_dir_that_is_a_junction_is_refused`
(Windows: ein Paar-Verzeichnis, das eine Junction ist, wird abgewiesen,
ein gewöhnliches angenommen).

**Status:** done — gemessen auf dem Linux-Testhost (`cargo nextest run
--profile merge -p zerodds-transport-shm --all-targets`, als root und
als `nobody`). Der Zweig für macOS lief grün auf einem Mac (macOS 26.2,
arm64; derselbe Befehl, 92 Tests, in der Sandbox von
`ci/macos-loopback.sb`, mit einem vorher auf 0755 gesetzten
Paar-Verzeichnis). Der Zweig für Windows ist für `x86_64-pc-windows-gnu`
mit Clippy übersetzt, dort aber nicht gelaufen: das tut der Job
`platform-windows-transport-shm` (§6.2), auch für
`shm_5_4_a_pair_dir_that_is_a_junction_is_refused`. Unter Wine 10.0 auf
dem Linux-Testhost laufen die Tests dieses Items grün bis auf diesen
einen: das `cmd` von Wine legt mit `mklink /J` keine Junction an
(„Failed to open“). Der Zweig der übrigen
Unix-Systeme (`/tmp/zerodds-shm-<euid>`, `private_dir`) ist für
`x86_64-unknown-freebsd` mit Clippy übersetzt, mit libc auf dem ABI von
FreeBSD 11; mit dem Standard-ABI übersetzt die Abhängigkeit `nix` 0.23.2
(über `shared_memory` 0.12) nicht (§4.4). Gelaufen ist er auf keinem
solchen System; `private_dir` selbst läuft im Unit-Test oben auch auf
Linux und macOS.

### §5.5 Liveness-Lock und verwaiste Segmente

**Spec:** §5 — jeder Nutzer eines Segments, Owner wie Consumer, MUSS für
die Dauer der Nutzung einen geteilten `flock` auf dem Segment halten; wo
das Segment keine Datei ist (alle Plattformen außer Linux), auf dessen
Hold-Datei `<name>.hold` im Paar-Verzeichnis (§5.4). Der
Kernel gibt ihn frei, wenn der Prozess endet, auch durch `SIGKILL`; ein
Segment ohne Lock-Halter ist verwaist. Ein Opener, der unter dem Namen
seines Paares ein verwaistes Segment findet, MUSS es entfernen und neu
anlegen, statt es zu übernehmen: sein Ring enthält die Frames der
beendeten Sitzung. Verwaiste Segmente, die niemand wieder öffnet, entfernt
ein Sweep über `/dev/shm`. Er lässt Segmente stehen, die jünger als ein
Mindestalter sind (das deckt den Abstand zwischen dem Anlegen und dem Lock
des Erstellers). Den Lock-Test wendet er nur auf Segmente dieser
Layout-Version an. Ab Version 2 gilt dieses Protokoll: ein Nutzer der
Version 1 hält keinen Lock, seine lebenden Segmente sähen verwaist aus.
Ringe einer früheren Layout-Version (Magic `ZSHM`, andere Version)
entfernt der Sweep auf Linux deshalb nur, wenn kein Prozess sie mappt: `/proc/<pid>/maps` nennt jedes Mapping
mit seinem Pfad in `/dev/shm`. Kann der Sweep die Maps eines Prozesses
von root nicht lesen, lässt er alle stehen. Auf macOS bleiben sie stehen:
dort lassen sich die Mappings eines Prozesses nicht auflisten.

**Repo:** `crates/transport-shm/src/posix.rs` (`hold_segment`,
`SegmentHold`, `sweep_orphaned_segments`, `SweepReport`,
`sweep_stale_pair_files` für die `.lock`-Dateien und, auf Linux, die
Flinks der Paare, deren Segment fort ist — sie lagen nach jedem nicht
sauber beendeten Prozess (eine Testkette auf einem MacBook Air: 3 504 `.lock` in einem
Domain-Verzeichnis); `lock_pair` prüft nach dem Sperren, dass der Pfad
noch die gesperrte Datei nennt, `SHM_VERSION`; ein Ring mit leerem Kopf, dessen Ersteller vor dem
Schreiben des Kopfs endete, gilt als entfernbar, weil kein Anhänger ihn
annimmt; `sweep_pair_dir` für die Dateien im Paar-Verzeichnis: auf Linux
die `.lock`-Dateien, deren Segment fort ist, sonst die `.hold`-Dateien,
die niemand hält, mit dem verwaisten Segment, das sie nennen, und
`.lock`-Dateien ohne `.hold`; überall die Warte-FIFOs `.wake` (§4.4),
deren Segment fort ist; jeweils unter dem Lock des Paares;
`is_segment_os_id`: ein Name aus einer Flink geht nur in der Form, die
`segment_os_id` erzeugt (`/zd-` und die beiden IDs in Hex oder die 26
Hex-Ziffern der kurzen Form), in die Pfade der Paar-Dateien ein, eine
Flink mit einem anderen Inhalt zählt als `foreign` und bleibt; vorher
prüfte der Linux-Zweig von `sweep_stale_pair_files` nur den Anfang `/zd-`,
und eine Flink mit `/zd-x/../../../<dir>/<name>` ließ ihn
`<dir>/<name>.lock` außerhalb von `/dev/shm` entfernen),
`crates/transport-shm/src/bell.rs` (die Türglocke eines
Teilnehmers: geteilter `flock` auf ihrer Datei in `/dev/shm`,
`sweep_orphaned_doorbells`); `crates/dcps/src/same_host_shm.rs`
(`sweep_orphaned_segments_once`: einmal pro Prozess beim Start eines
Teilnehmers mit `transport.same_host_shm`, Ringe, Türglocken und das
Paar-Verzeichnis, danach je Flink-Verzeichnis die Paar-Dateien,
Mindestalter 60 s, Ereignis `shm.orphaned_segments_removed`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_5_5_an_orphaned_segment_is_made_anew_not_attached`
(ein eigener Prozess öffnet den Owner, publiziert einen Frame und endet
ohne Drop; der Consumer findet danach einen leeren Ring und empfängt nur
den Frame der neuen Sitzung),
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_sweep_removes_orphaned_segments_and_keeps_held_ones`
(das verwaiste Segment bleibt unter dem Mindestalter stehen und wird
darüber entfernt; ein gehaltenes Segment bleibt, und ein Frame läuft
danach durch; ein Segment mit leerem Kopf wird entfernt),
`crates/transport-shm/src/bell.rs::tests::shm_5_5_the_sweep_removes_orphaned_bells_and_keeps_held_ones`,
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_pair_file_sweep_removes_lone_lock_files_and_keeps_the_rest`,
`crates/transport-shm/src/posix.rs::tests::shm_5_5_a_lock_taken_on_a_removed_lock_file_is_taken_anew`,
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_flink_sweep_leaves_a_flink_whose_name_points_outside_dev_shm`
(Linux: eine Flink mit `/zd-<dir>/../../../<pfad>/secret` bleibt, und
`<pfad>/secret.lock` bleibt; ohne die Prüfung scheitert der Test, der
Sweep meldet `removed: 1` und die Datei ist fort),
`crates/transport-shm/src/posix.rs::tests::shm_5_5_a_name_from_a_flink_has_the_shape_segment_os_id_gives`
(die lange und die kurze Form gelten; leer, ohne `/`, Großbuchstaben, eine
Ziffer zu viel oder zu wenig, ein Zeilenende, `/`, `.` und `..` nicht),
`crates/transport-shm/src/posix.rs::tests::shm_5_5_a_ring_of_an_earlier_layout_goes_unless_a_process_maps_it`
(Linux: ein Ring der Version 1, den niemand mappt, wird entfernt, ein
gemappter bleibt, bis das Mapping endet),
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_pair_dir_sweep_removes_what_gone_segments_left`
(Linux: der Lock eines fortgegangenen Segments geht, der eines vorhandenen
und ein gehaltener bleiben, unter dem Mindestalter bleibt alles; sonst:
die Hold-Datei eines beendeten Prozesses geht mit ihrem Segment, die eines
offenen Paares bleibt; auf jedem Unix: ein mit `mkfifo` angelegter
`.wake`-FIFO bleibt, solange sein Segment da ist, und geht danach; vorher
legte der Test `.wake` als reguläre Datei an und konnte nicht scheitern,
mit einem FIFO scheitert er gegen den früheren Sweep auf dem
Linux-Testhost: `young: 2` statt 3, der FIFO bleibt).
Gemessen auf dem Linux-Testhost: zwei Teilnehmer per `kill -9` beendet; der Start
eines dritten nach 60 s entfernte ihre 2 Ringe und 2 Türglocken
(`removed: 4`). Vorher blieben Türglocken jedes nicht sauber beendeten
Prozesses liegen (138 nach einem Tag Proben).

Auf macOS und den übrigen Unix-Systemen ohne Shared-Memory im
Dateisystem liegt der Lock auf der Hold-Datei des Segments im
Paar-Verzeichnis (§5.4; vorher auf der Flink-Datei des Paares, die ein
Nutzer in einem anderen Flink-Verzeichnis nicht sah).
`sweep_orphaned_flinks` findet verwaiste Segmente über das
Flink-Verzeichnis (dcps: einmal je Verzeichnis, also je Host und Domain)
und entfernt sie unter dem Lock des Paares mit Flink, Hold- und
Lock-Datei; ein Segment einer anderen Layout-Version bleibt, ebenso eine
Flink, deren Inhalt `is_segment_os_id` nicht annimmt (Test
`shm_5_5_the_flink_sweep_leaves_a_flink_whose_name_points_outside_the_pair_dir`:
eine Flink mit `/zd-<dir>/../../<opfer>/secret` bleibt, `secret.hold`
bleibt, und `secret.lock` entsteht nicht; ohne die Prüfung legte der
Sweep `secret.lock` außerhalb des Paar-Verzeichnisses an und sperrte sie,
bevor die Abbildung den Namen ablehnte; unter Wine gemessen, mit und ohne
die Prüfung).
`sweep_pair_dir` findet über die Hold-Dateien auch die, deren Flink fort
ist (dcps: einmal pro Prozess).
Eine Türglocke legt dort eine Markierung `<name>.bell` an, auf der ihr
Teilnehmer die Sperre hält; der Sweep entfernt Glocke und Markierung,
wenn niemand sie hält. Tests
`shm_5_5_the_flink_sweep_removes_orphaned_segments_and_keeps_held_ones`
(mit einem Ring der Version 1, der bleibt) und
`bell.rs::tests::shm_5_5_the_flink_sweep_removes_orphaned_bells_and_keeps_held_ones`;
die Glocken-Markierung ist unverändert und auf macOS 26 grün, der
Segment-Teil mit der Hold-Datei ist für `aarch64-apple-darwin` mit Clippy
übersetzt und nicht gelaufen. `shm_5_5_an_orphaned_segment_is_made_anew_not_attached`
läuft auf jedem Unix. Windows: §6.3.

**Status:** done — gemessen auf dem Linux-Testhost; der macOS-Zweig der
Sweeps über die Hold-Dateien und die `.wake`-FIFOs lief grün auf einem
Mac (macOS 26.2, arm64, §5.4); der Zweig für FreeBSD ist für
`x86_64-unknown-freebsd` mit Clippy übersetzt und nicht gelaufen (für
FreeBSD gibt es keinen Job); die Flink mit einem Namen außerhalb des
Paar-Verzeichnisses ist unter Wine 10.0 auf dem Linux-Testhost grün, auf
Windows nicht gelaufen (§6.2).

## §6 Plattform-Support

### §6.1 Linux und macOS

**Spec:** §6 — Linux ist die primäre Plattform, macOS wird unterstützt.
Auf macOS gilt das `PSHMNAMLEN`-Limit von 31 Zeichen für den
Segment-Namen; der Name wird dort entsprechend gekürzt.

**Repo:** `crates/transport-shm/src/posix.rs` (`#[cfg(target_os = "macos")]`-Zweig
in `segment_os_id`, `#[cfg(any(target_os = "linux", target_os = "macos"))]`
für `shm_unlink_by_os_id`, `#[cfg(unix)]` für `acquire_flock_excl`),
`crates/transport-shm/src/mapping.rs` (ein Mapping hält keinen
Deskriptor: `shm_open`, `ftruncate`, `mmap`, dann `close`. Mit
`shared_memory` blieb je Mapping einer offen, zwei Ringe und eine
Türglocke je Peer; ein Prozess einer Testkette aus mehreren Prozessen lief am macOS-Standardlimit
von 256 Deskriptoren aus, 62 davon Shared-Memory-Objekte. Je Peer bleiben
die Lebendsperren der beiden Ringe).

**Tests:** `crates/transport-shm/src/mapping.rs::tests::shm_6_1_a_mapping_holds_no_file_descriptor`
(100 Mappings, Zahl der offenen Deskriptoren in einem eigenen Prozess
unverändert; macOS 26),
`crates/transport-shm/src/mapping.rs::tests::shm_6_1_create_open_share_and_the_owner_unlinks`,
`crates/transport-shm/src/posix.rs::tests::shm_5_1_segment_os_id_is_deterministic_and_within_platform_limit`
(prüft das 31-Zeichen-Limit unter `cfg(target_os = "macos")`),
`crates/transport-shm/tests/l1_cross_process.rs::shm_2_3_l1_shm_datagram_crosses_process_boundary`,
`crates/transport-shm/src/posix.rs::tests::shm_3_1_open_owner_then_consumer_roundtrip_single_frame`.

**Status:** done

### §6.2 Windows

**Spec:** §6 — auf Windows liefert `CreateFileMapping` die Mappings, der
Create-Race wird über `LockFileEx` serialisiert. Der Transport MUSS dort
mit derselben Test-Suite laufen wie auf Unix.

**Repo:** `crates/transport-shm/src/mapping.rs` (Windows: je Segment eine
Datei in `%TEMP%\shared_memory-rs`, wo `shared_memory` sie anlegte,
gemappt über ein Mapping-Objekt ohne Namen (`CreateFileMappingW`,
`MapViewOfFile`); die Sichten einer Datei sind über Prozesse hinweg
kohärent, aus welchem Mapping-Objekt sie auch stammen. `remove_name`
benennt eine Datei in `<name>.<pid>-<n>.gone` um und entfernt sie dann:
der Name ist sofort frei, auch wo Windows auf die alte Art löscht und
unter Wine. Vorher mappte `shared_memory` über ein Mapping-Objekt mit dem
Namen des Segments, und Windows hält ein benanntes Objekt, solange ein
Handle darauf offen ist: wer ein Segment unter dem Namen eines beendeten
Rings neu anlegte, den ein Consumer noch mappte, bekam diesen beendeten
Ring (Kopf mit gesetztem Shutdown-Flag, altes `head` und `tail`) über
einer neuen, leeren Datei);
`crates/transport-shm/src/posix.rs` (`file_lock`: `flock` auf Unix,
`LockFileEx` auf Windows, für Paar-Sperre, Halt und Sweeps; `still_named`
vergleicht auf Windows Volume-Seriennummer und Dateiindex
(`GetFileInformationByHandle`) und erkennt eine zum Löschen markierte
Datei (`delete_pending`), vorher antwortete es dort ohne Blick mit ja;
`lock_pair` versucht das Öffnen einer zum Löschen markierten Lock-Datei
bis zu 5 s erneut; `hold_segment`, `segment_in_use`, `sweep_pair_dir`,
`sweep_orphaned_flinks` und `sweep_stale_pair_files` gelten für Windows
wie für macOS, `sweep_backing_files` für die Segment-Dateien;
`shm_unlink_by_os_id` entfernt auf Windows den Namen der Segment-Datei,
vorher war es leer), `crates/transport-shm/src/bell.rs` (eine Türglocke
hat auf Windows wie auf macOS eine Markierung im Flink-Verzeichnis, auf
der ihr Teilnehmer die Sperre hält), `crates/dcps/src/same_host_shm.rs`
(`sweep_orphaned_segments_once` auch auf Windows; vorher räumte dort
nichts auf). CI: Job `platform-windows-transport-shm` in
`ci/platforms.yml` (auf dem Windows-Runner der CI, wöchentlich, bei einem
Release-Kandidaten und mit `PLATFORMS=1`); Job `platform-windows-dcps-shm`
(`ci/platform_jobs.py`, `WINDOWS_FEATURE_RUNS`): baut `zerodds-dcps` mit
dem Feature `same-host-shm` für Windows und führt dessen Vertragstest mit
`zerodds-transport-shm` aus, darin den Sweep, den ein Teilnehmer bei
seinem Start ausführt; vorher baute kein Job die Laufzeit mit
`same-host-shm` für Windows.

**Tests:** `ci/test_platform_jobs.py::Gate::test_transport_shm_runs_on_windows_what_it_runs_on_linux`
(der Windows-Job führt jeden Test des Crates aus, den der Lauf auf Linux
ausführt, bis auf 10; 8 davon nennt der Test mit ihrem Gegenstück auf
Windows (`/dev/shm` und `/proc/<pid>/maps`, der Name aus einer Flink, der
aus `/dev/shm` hinausführt, ein Datei-Symlink, das Unix-Mapping und ein
noch nicht vergrößertes Objekt), 2 mit dem Grund:
ein Unix-Mapping schließt seinen Deskriptor sofort, ein Windows-Mapping
hält Datei und Mapping-Objekt; ein später von Windows ausgenommener Test
lässt ihn scheitern),
`crates/transport-shm/src/mapping.rs::windows_tests::shm_6_2_create_open_share_and_the_owner_unlinks`,
`crates/transport-shm/src/mapping.rs::windows_tests::shm_6_2_a_segment_made_anew_while_the_old_one_is_mapped_is_new`
(ein Segment, das unter dem Namen eines noch gemappten neu angelegt wird,
ist neu und genullt, das alte Mapping behält seine Bytes; mit einem
Mapping-Objekt, das den Namen des Segments trägt, liest der Test 119
statt 0: Probe unter Wine auf dem Linux-Testhost),
`crates/transport-shm/src/mapping.rs::windows_tests::shm_6_2_an_unsized_segment_file_does_not_open`
(die Segment-Datei zwischen `create_new` und `set_len` öffnet nicht,
`MapOpenFailed(ERROR_INVALID_PARAMETER)`; vergrößert öffnet sie),
`crates/transport-shm/src/bell.rs::tests::shm_6_2_a_bell_its_creator_has_not_sized_yet_is_waited_for`
(ein Sender wartet auf die Größe einer Türglocke, deren Datei ihr
Ersteller noch nicht vergrößert hat),
`crates/transport-shm/src/posix.rs::tests::shm_5_4_a_removed_pair_file_frees_its_name_at_once`,
`crates/transport-shm/src/posix.rs::tests::shm_5_5_a_lock_taken_on_a_removed_lock_file_is_taken_anew`,
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_pair_file_sweep_removes_lone_lock_files_and_keeps_the_rest`,
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_pair_dir_sweep_removes_what_gone_segments_left`
(diese drei vorher nur auf Unix),
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_flink_sweep_removes_orphaned_segments_and_keeps_held_ones`,
`crates/transport-shm/src/bell.rs::tests::shm_5_5_the_flink_sweep_removes_orphaned_bells_and_keeps_held_ones`
(diese zwei vorher nur auf Unix außer Linux),
`crates/transport-shm/src/posix.rs::tests::shm_5_5_the_flink_sweep_leaves_a_flink_whose_name_points_outside_the_pair_dir`
(das Gegenstück zum Linux-Test über `/dev/shm`, §5.5),
`crates/transport-shm/src/posix.rs::tests::shm_2_3_open_concurrent_two_threads_both_bound`,
`crates/transport-shm/src/posix.rs::tests::shm_5_3_a_drop_racing_the_next_owner_leaves_it_a_ring_that_is_read`,
`crates/dcps/tests/contract_transport_shm.rs::the_participants_start_sweep_runs_without_a_failure`
(der Sweep beim Start eines Teilnehmers meldet kein
`shm.orphan_sweep_failed`),
`crates/dcps/tests/contract_transport_shm.rs::a_sweep_of_a_fresh_or_missing_flink_directory_removes_nothing`
(vorher nur auf Unix).

**Status:** partial — gemessen auf dem Linux-Testhost: `cargo nextest run
--profile merge -p zerodds-transport-shm --all-targets` 91 von 91 grün;
derselbe Lauf mit `--target x86_64-pc-windows-gnu` unter Wine 10.0 als
Target-Runner 89 von 90 grün (nextest meldet
einen davon als `leaky`); der eine ist
`shm_5_4_a_pair_dir_that_is_a_junction_is_refused`, weil das `cmd` von
Wine keine Junction anlegt. Der Lauf des Jobs
`platform-windows-dcps-shm` unter Wine, `cargo nextest run --profile merge
-p zerodds-dcps --features same-host-shm --test contract_transport_shm
--target x86_64-pc-windows-gnu`: 5 von 5 grün. Wine ist nicht Windows: es gibt
den Namen einer entfernten Datei erst mit ihrem letzten Handle frei und
meldet sie nicht als zum Löschen markiert (gemessen; daher
`remove_name`). Auf Windows selbst ist dieser Stand nicht gelaufen: die
Jobs `platform-windows-transport-shm` und `platform-windows-dcps-shm`
laufen auf dem Windows-Runner der CI, den dieser Lauf nicht erreichte.
Ein früherer Stand des Crates lief dort in Pipeline 8782 (44 Tests
grün).

### §6.3 Nachweis der Windows-Cleanup-Semantik

**Spec:** §6 — dass ein abgestürzter Owner auf Windows kein Mapping mit
den Frames seiner Sitzung hinterlässt, MUSS ein Test auf dem
Windows-Runner belegen, nicht nur die Dokumentation behaupten.

**Repo:** `crates/transport-shm/src/posix.rs` (`hold_segment` mit
`LockFileEx` auf der Hold-Datei des Segments im Paar-Verzeichnis (§5.4),
wie die macOS-Sperre; ein Öffner, der ein Segment ohne Halter findet,
verwirft es mit `discard_orphaned_segment`; öffnet niemand das Paar
wieder, entfernen `sweep_pair_dir` mit `sweep_backing_files` und
`sweep_orphaned_flinks` beim Start eines Teilnehmers
(`crates/dcps/src/same_host_shm.rs`, `sweep_orphaned_segments_once`; für
Windows gebaut und ausgeführt im Job `platform-windows-dcps-shm`), was der
beendete Owner hinterließ: die Segment-Datei mit den Frames seiner Sitzung, Hold-,
Lock- und Flink-Datei; vorher lief auf Windows kein Sweep, und jede
solche Datei blieb), `crates/transport-shm/src/mapping.rs` (das Segment
ist eine Datei in `%TEMP%\shared_memory-rs`, die den Prozess überlebt).
Die frühere Annahme, Windows gebe das Mapping mit dem letzten Handle
frei, traf schon für `shared_memory` nicht zu (Datei windows.rs der
Crate, Funktion `new_map`); gemessen im Nightly 6555: der Test fand 29
Byte der beendeten Sitzung im Ring.

**Tests:** `crates/transport-shm/src/posix.rs::tests::shm_5_5_an_orphaned_segment_is_made_anew_not_attached`
(ein Kindprozess öffnet den Owner, publiziert einen Frame und endet ohne
Drop; derselbe Name öffnet danach mit leerem Ring),
`crates/transport-shm/src/posix.rs::tests::shm_6_3_the_sweep_removes_what_an_ended_owner_left_on_windows`
(was ein ohne Drop beendeter Owner hinterließ, gehen mit dem Sweep des
Paar-Verzeichnisses die Segment-Datei mit dem Frame, Hold- und
Lock-Datei, mit dem Flink-Sweep der Flink; ein Ring, dessen Ersteller vor
seinem Halt endete, geht mit dem Sweep der Segment-Dateien, unter dem
Mindestalter bleibt er; ein gehaltenes Paar bleibt und trägt danach einen
Frame).

**Status:** partial — beide Tests unter Wine 10.0 auf dem Linux-Testhost
grün (§6.2); auf Windows nicht gelaufen: das misst der Job
`platform-windows-transport-shm` auf dem Windows-Runner der CI, den
dieser Lauf nicht erreichte.

### §6.4 `no_std`

**Spec:** §6 — der Ring aus §2 bis §4 und das Shutdown-Flag aus §5.3
gelten auf jedem Ziel, auch ohne Betriebssystem und ohne `std`, bis auf
§4.3: über einem Bereich legt allein der Owner den Ring an. Ein Consumer,
der vor ihm kommt, erhält `RingError::NotReady` und öffnet später erneut.
Ohne Betriebssystem gibt es keine Sperre, die zwei Seiten ordnet, die
zugleich den Kopf schreiben, und der Owner legt den Ring über jedem
fremden Inhalt neu an. Ohne
Betriebssystem gibt der Integrator den Speicherbereich vor: Speicher, den
zwei Kerne eines Chips teilen, oder eine Seite, die ein Hypervisor zwei
Partitionen gibt; je Paar ein Bereich, ausgerichtet auf 8 Byte, an einer
Adresse, die beide Seiten sehen, mit höchstens einem Owner und einem
Consumer zugleich. Namen, Flinks, das Entfernen vor dem Anlegen und die
Sperren (§5.1, §5.2, §5.4, §5.5) sind Dienste eines Betriebssystems und
entfallen dort. An ihre Stelle tritt die Sitzungs-Regel: Ein Owner legt
den Ring beim Start neu an, wo der Bereich keinen Ring dieses Layouts und
dieser Kapazität hält. Hält er einen, setzt der Owner ihn fort und
schreibt den Beginn seiner Sitzung nach `session_head` (Offset 48): auf
`head`, wenn der vorige Owner ohne Drop endete (`shutdown` 0, etwa ein
zurückgesetzter Kern), damit dessen Frames nicht ausgeliefert werden (wie
§5.5); auf das `tail` des Consumers, wenn der vorige Owner seine Seite
beendete (`shutdown` 1), damit dessen Frames noch ankommen (wie §5.3).
Danach zählt er `owner_epoch` hoch; ein Owner, nach dem ein weiterer
öffnete, sendet nicht mehr (§5.3). Der Drop des Owners setzt das
Shutdown-Flag und weckt den Consumer: ein `recv`, das wartet, kehrt mit
dem Fehler aus §5.3 zurück, mit und ohne Timeout. Ein Consumer springt beim Anhängen und
bei jeder Änderung von `owner_epoch` auf den Beginn der Sitzung. Jede
Seite liest die Position der anderen nur in ihren unteren 32 Bit; die
Kapazität ist deshalb höchstens 2³¹ Byte, und ein Kern ohne
64-Bit-Atomics nimmt teil, indem er seine Position als zwei 32-Bit-Wörter
schreibt, das untere zuletzt mit `Release`. Wie eine Seite auf die andere
wartet, wählt der Integrator (ein Ereignis zwischen Kernen, ein
Interrupt); das Warten endet mit einem Frame und mit dem Drop des Owners,
sonst erst mit seiner Frist. Ein Warten, das vorher zurückkehrt, beendet
ein `recv` mit Frist mit `Timeout`, weil die Seite keine Uhr für den Rest
der Frist hat; ein `recv` ohne Frist wartet erneut. Ohne eigene Wahl liest die Seite das untere Wort von `head` und das
Shutdown-Flag in einer Schleife. Der
Crate baut ohne `std` für ein Ziel ohne Betriebssystem: der Ring ohne
`alloc`, der Transport mit `alloc`.

**Repo:** `crates/transport-shm/src/ring.rs` (`Ring` in `core` allein:
Header, `create`, `create_in_zeroed`, `attach`, `try_push`, `next`,
`release`, `drop_published`, `skip_to_session`, die Positionen über
`store_position`, `load_position` und `low`, ohne 64-Bit-Atomics das Modul
`split`), `crates/transport-shm/src/region.rs` (`RegionShmTransport`:
`open_owner` und `open_consumer` über den Bereich des Aufrufers, die
Sitzungs-Regel, `Transport`; `RingWait`, `Parked` (worauf ein `recv`
wartet: das untere Wort von `head` und das Shutdown-Flag; vorher wartete
es nur auf das Wort, und der Drop des Owners, der `head` nicht ändert,
beendete das Warten nicht; `RingWait::park` kehrt mit `true` nur zurück,
nachdem `Parked::is_over` wahr war, und sonst mit `false` nach seiner
Frist, und `Parked` merkt sich, ob `is_over` wahr war; vorher durfte
`park` früher zurückkehren, und `recv` begann bei jeder Runde die volle
Frist neu), `SpinWait`, `RegionConfig`,
`RegionShmError`), `crates/transport-shm/src/posix.rs` (der Transport der
benannten Segmente nutzt denselben `Ring`; vorher lag der Ring dort
allein und war an `Mapping`, `std::sync::Mutex` und `Vec` gebunden),
`crates/transport-shm/src/lib.rs` (`#![cfg_attr(not(feature = "std"),
no_std)]`; `ring` immer, `region` mit `alloc` auf Zielen mit
32-Bit-Compare-and-Swap, `posix`, `bell`, `mapping` und `wait` mit `std`),
`crates/transport-shm/Cargo.toml` (`shared_memory` und `libc` optional
unter `std`, `zerodds-rtps` und `zerodds-transport` ohne
Default-Features, Kategorie `no-std`: damit baut
`tools/no-std-claims-gate.sh` im Job `no-std` (`ci/weekly-checks.yml`)
den Crate für `thumbv7em-none-eabihf` ohne Default-Features mit
`alloc`; derselbe Job baut ihn ohne `alloc`),
`crates/transport-shm/bare-metal/two-cores` (ein Programm ohne
Betriebssystem für `aarch64-unknown-none`, ein Arbeitsbereich für sich:
Kern 0 einer QEMU-Maschine `virt` mit zwei Cortex-A53 ist Owner eines Rings
in einem Bereich des RAM, Kern 1, den Kern 0 über PSCI startet, sein
Consumer; beide mit MMU und Caches, warten mit `WFE`/`SEV`, ein
Bump-Allokator; ein eigenes `Cargo.lock`), `tools/shm-region-two-cores.sh`
(baut es mit `--locked` und startet es unter `qemu-system-aarch64 -smp
2`), CI-Job `no-std-shm-two-cores` in `ci/weekly-checks.yml`;
`tools/shm-ring-armv5te.sh` (führt die Tests von `ring` und `region`,
ohne `std` gebaut, für `armv5te-unknown-linux-gnueabi` unter `qemu-arm`
aus; dieses Ziel hat nur 32-Bit-Atomics, also schreibt der Ring seine
Positionen als zwei Wörter über `split`; das Skript prüft das mit
`rustc --print cfg`), CI-Job `no-std-shm-armv5te` in
`ci/weekly-checks.yml`; `ci/Dockerfile.rust` bringt `qemu-system-arm`,
`qemu-user`, `gcc-arm-linux-gnueabi`, `libc6-dev-armel-cross` und die
Ziele `aarch64-unknown-none` und `armv5te-unknown-linux-gnueabi` ins
Image.

**Tests:** `crates/transport-shm/src/region.rs::tests::shm_6_4_a_ring_over_a_caller_supplied_region_carries_frames`
(Owner und Consumer auf zwei Threads tragen 5 000 Frames von 4 bis 603
Byte durch einen Ring von 4 KiB in einem Bereich, den der Test vorgibt,
in Reihenfolge und ganz, über das Ring-Ende und über 2³², wo das untere
Wort einer Position umläuft),
`crates/transport-shm/src/region.rs::tests::shm_6_4_a_new_owner_session_discards_the_frames_of_an_ended_one`
(ein Owner ohne Drop hinterlässt zwei Frames, der nächste Owner beginnt
dahinter, der Consumer erhält nur dessen Frame; ein Owner mit Drop
hinterlässt seine Frames dem Consumer; ein abgelöster Owner sendet
nicht mehr),
`crates/transport-shm/src/region.rs::tests::shm_6_4_the_owner_makes_the_ring_anew_where_the_region_holds_none`,
`crates/transport-shm/src/region.rs::tests::shm_6_4_recv_waits_with_the_integrators_wait_up_to_its_timeout`,
`crates/transport-shm/src/region.rs::tests::shm_6_4_the_owners_drop_ends_a_parked_recv`
(der Drop des Owners beendet ein `recv`, das in `SpinWait` wartet, ohne
Timeout und mit 30 s, mit `shm owner terminated`; mit dem Warten nur auf
das Wort scheitert der Test nach 10 s),
`crates/transport-shm/src/region.rs::tests::shm_6_4_a_park_that_returns_early_does_not_restart_the_timeout_without_end`
(ein Warten, das nach 1 ms `true` zurückgibt, ohne dass das Warten vorbei
ist: ein `recv` mit 50 ms endet nach einem Warten mit `Timeout`, eines
ohne Frist wartet erneut und erhält den Frame, der danach kommt; mit dem
früheren `recv` war das `recv` mit 50 ms nach 3 s nicht zurückgekehrt,
und der Test scheiterte),
`crates/transport-shm/src/ring.rs::tests::shm_6_4_positions_cross_the_low_word_wrap_and_split_words_match_whole_ones`
(Positionen über 2³², wo das untere Wort eines 32-Bit-Kerns umläuft:
400 Frames ganz und in Reihenfolge; eine Position, als zwei 32-Bit-Wörter
geschrieben, hat die Bytes des ganzen Worts),
`crates/transport-shm/src/ring.rs::tests::shm_6_4_create_and_attach_check_the_header_and_the_region`,
`crates/transport-shm/src/posix.rs::tests::shm_6_4_region_and_posix_transport_share_one_layout`
(ein Consumer über einem Bereich liest die Frames eines POSIX-Owners aus
dessen Mapping, ein POSIX-Consumer die eines Owners über dem Bereich),
`tools/shm-region-two-cores.sh` (zwei Kerne ohne Betriebssystem: 10 000
Frames von 4 bis 203 Byte durch einen Ring von 8 KiB, jeder Byte für Byte
geprüft; danach wartet Kern 1 in einem `recv` ohne Timeout, und Kern 0
gibt den Owner frei; QEMU endet mit Status 0 nur, wenn alle Frames
ankamen und der Drop das Warten mit dem Fehler aus §5.3 beendete),
`tools/shm-ring-armv5te.sh` (die acht Tests von `ring` und `region` auf
einem Kern ohne 64-Bit-Atomics).

**Status:** done — gemessen auf dem Linux-Testhost: `cargo nextest run
--profile merge -p zerodds-transport-shm --all-targets` 91 von 91 grün;
`tools/shm-ring-armv5te.sh` 8 von 8 grün (`qemu-arm` 10.0.13);
`tools/shm-region-two-cores.sh` unter QEMU 10.0.13 fünfmal hintereinander
grün („sent 10000, received 10000, …, the owner's drop ended the parked
recv“, 4 bis 18 Sendungen trafen einen vollen Ring). Mit dem früheren
Warten nur auf das Wort scheitert
`shm_6_4_the_owners_drop_ends_a_parked_recv` nach 10 s, und das Programm
der zwei Kerne endet nicht (Probe; QEMU nach 120 s beendet).
`cargo build -p zerodds-transport-shm --no-default-features --target
thumbv7em-none-eabihf` mit und ohne `--features alloc` übersetzt; Clippy
für `thumbv7em-none-eabihf` (mit und ohne `alloc`), für
`armv5te-unknown-linux-gnueabi` und für das Bare-Metal-Programm ohne
Befund. Der Ring ohne 64-Bit-Atomics lief als Linux-Prozess unter
`qemu-arm`, nicht auf einem Kern ohne Betriebssystem. Die Jobs
`no-std-shm-two-cores` und `no-std-shm-armv5te` sind in CI noch nicht
gelaufen: sie brauchen das Image, das die Änderung an
`ci/Dockerfile.rust` beim Merge neu baut.

## §7 Zero-Copy-Loan (same-host)

### §7.1 Loan-Pfad Writer → Reader

**Spec:** §7 — für Samples auf demselben Host MUSS ein Pfad existieren,
der ohne Staging-Kopie auskommt: der Writer erhält einen Zeiger **in
einen Shared-Memory-Slot**, füllt ihn, und der Commit finalisiert den
Slot an Ort und Stelle. Ein Reader auf demselben Host mappt dasselbe
Segment und liest den Slot über seinen eigenen Zeiger — die Bytes
überqueren nie das Netz und werden nie umkopiert.

**Repo:** `crates/flatdata/src/posix.rs` (`PosixSlotAllocator`,
`reserve_slot`, `slot_data_ptr`, `commit_in_place`, `slot_read_ptr`,
`next_unread_slot`, `mark_read`);
`crates/zerodds-c-api/src/shm_loan_ffi.rs` (`try_loan`, `try_commit`,
`try_take`, `zerodds_dw_enable_shm_loan`, `zerodds_dr_enable_shm`,
`zerodds_dr_take_shm`, `zerodds_dr_release_shm`).

**Tests:** `crates/zerodds-c-api/tests/shm_loan_e2e.rs::capi_2_6_shm_loan_writer_to_reader_zero_copy`
(misst den Loan-Pfad selbst, nicht seinen Aufbau: der Test schreibt die
Nutzlast direkt in den geliehenen Zeiger und committet in place; der
Lesezeiger des Readers trägt eine andere Adresse als der Writer-Zeiger —
eigenes Mapping desselben Segments — und die Bytes sind gleich. Dass
beide Zeiger die Bytes des Segments adressieren und keine Kopie, zeigt
der Test mit einem dritten Mapping des Segments, das er selbst anlegt: es
enthält die Nutzlast; ein Muster, das er nach dem Commit über den
Loan-Zeiger des Writers schreibt, liest er über den Zeiger des Readers
und über das dritte Mapping, und eines, das er über das dritte Mapping
schreibt, über beide. Ein Commit, der einen Heap-Puffer in den Slot
kopierte, oder ein Take, der den Slot herauskopierte, scheitert an diesen
Lesungen. Danach Release, `NoData` beim nächsten Take und ein zweites
Sample durch denselben Ring),
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::capi_2_6_shm_loan_runtime_path_writer_to_reader_zero_copy`
(derselbe Pfad mit demselben Nachweis über ein drittes Mapping, über die
Runtime-FFI, die das ROS-2-RMW nutzt),
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::capi_2_6_loan_without_shm_falls_back_to_heap`
(ohne aktivierten Loan bleibt der Heap-Pfad),
`crates/flatdata/src/posix.rs::tests::flatdata_3_2_write_read_through_shm`
(Slot-Backend ohne FFI). Das Feature `flatdata-loan` steht in `default`
von `crates/zerodds-c-api/Cargo.toml`; die Tests laufen im Default-Lauf
und nicht hinter einem Schalter.

**Status:** done

### §7.2 Abgrenzung: `crates/transport-shm` kopiert

**Spec:** §7 — der Ring aus §2 bis §5 ist **kein** Zero-Copy-Pfad:
`push_frame` kopiert die Nutzlast in die Ring-Region, `pop_frame` kopiert
sie in einen `Vec` heraus. Der Ring transportiert RTPS-Datagramme
zwischen Prozessen; der Loan-Pfad aus §7.1 transportiert Sample-Bytes
ohne Kopie. Beide Mechanismen sind getrennt und werden nicht
gegeneinander ausgetauscht. Die Sektion hält die Abgrenzung fest und
stellt keine eigene Implementierungs-Anforderung.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Abgrenzung zwischen Ring und Loan-Pfad; der Loan-Pfad steht als Item §7.1.

---

## Audit-Status

19 done / 3 partial / 0 open / 0 blocked / 2 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-shm-transport-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-c-api --all-targets` (Job `merge-rust-c-api`) — 3 Tests grün.
* `cargo nextest run --profile merge -p zerodds-flatdata --all-targets` (Job `merge-rust-flatdata`) — 2 Tests grün.
* `cargo nextest run --profile merge -p zerodds-rtps --all-targets` (Job `merge-rust-rtps`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-transport-shm --all-targets` (Job `merge-rust-transport-shm`) — 41 Tests grün.
* `cargo nextest run --profile merge -p zerodds-dcps --all-targets --features same-host-shm` (Job `merge-rust-dcps-features-same-host-shm`) — 1 Test grün.
* Job `merge-macos-transport-shm` — 2 Tests grün.

50 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `zerodds-shm-transport-1.0.open.md`.

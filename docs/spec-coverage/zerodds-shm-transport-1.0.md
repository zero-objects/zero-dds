# ZeroDDS-SHM-Transport 1.0 — Spec-Coverage

**Quelle:** dieses Dokument ist zugleich die Spec. Für Shared Memory
normiert OMG nichts außer dem vendor-reservierten Locator-Kind
(DDSI-RTPS 2.5 §9.4.5); Segment-Layout, Synchronisation und
Cleanup-Protokoll sind vendor-spezifisch. Die normativen Anforderungen
stehen in den `##`-Sektionen §2 bis §7; jedes `###`-Item darunter trägt
Spec-Zitat, Repo-Anker, Test und Status.

Implementation: `crates/transport-shm/src/posix.rs` — Segment-Layout,
SpSc-Ringbuffer, Crash-Recovery. Der Zero-Copy-Loan-Pfad aus §7 liegt in
`crates/flatdata/` und `crates/zerodds-c-api/src/shm_loan_ffi.rs`; siehe
§7.2 zur Abgrenzung.

Offene Punkte und Decision-Records: [`zerodds-shm-transport-1.0.open.md`](zerodds-shm-transport-1.0.open.md).

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

**Tests:** `crates/transport-shm/src/posix.rs::tests::locator_kind_is_the_vendor_reserved_shm_value`
(prüft den Zahlwert `0x01F0_0001`, die Lage im Bereich `0x00000003` bis `0x01ffffff` und `port == 0`),
`crates/transport-shm/src/posix.rs::tests::local_locator_is_shm_with_own_id`,
`crates/transport-shm/src/posix.rs::tests::send_rejects_non_shm_locator`,
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

**Status:** n/a (informative)

## §2 Segment-Layout

```text
  offset 0:   magic: u32 BE   "ZSHM"
  offset 4:   version: u32 LE
  offset 8:   capacity: u64 LE         (Daten-Region, ohne Header)
  offset 16:  head: AtomicU64          (nächster Schreib-Offset, Writer-owned)
  offset 24:  tail: AtomicU64          (nächster Lese-Offset, Reader-owned)
  offset 32:  shutdown: AtomicU32      (0=active, 1=owner-gone)
  offset 36:  reserved (Padding zur 64-Byte-Cache-Line)
  offset 64:  data-region [capacity bytes]
```

### §2.1 Header-Felder

**Spec:** §2 — der Ersteller MUSS Magic `"ZSHM"` big-endian auf Offset 0,
die Layout-Version little-endian auf Offset 4 und die Kapazität der
Daten-Region little-endian auf Offset 8 schreiben. Der Header ist 64 Byte
lang, damit `head` und `tail` nicht mit der Daten-Region in dieselbe
Cache-Line fallen.

**Repo:** `crates/transport-shm/src/posix.rs` (`SHM_MAGIC`,
`SHM_VERSION`, `HEADER_BYTES = 64`, Initialisierung im `is_creator`-Zweig
von `PosixShmTransport::open`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::owner_writes_magic_and_version_into_the_header`
(liest Magic, Version und Kapazität direkt aus dem Mapping und vergleicht
das Magic gegen `b"ZSHM"`),
`crates/transport-shm/src/posix.rs::tests::invalid_config_capacity_too_small_for_max_datagram`.

**Status:** done

### §2.2 Zurückweisung fremder Segmente

**Spec:** §2 — beim Attach MUSS der Opener Magic und Version prüfen und
ein fremdes oder neueres Segment ablehnen, statt seine Bytes als Ring zu
interpretieren. Eine Layout-Änderung erhöht die Version.

**Repo:** `crates/transport-shm/src/posix.rs` (`else`-Zweig von
`PosixShmTransport::open`, `PosixShmError::InvalidHeader`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::attach_rejects_foreign_magic`
(Segment mit fremdem Magic, angelegt über dieselben
`shared_memory`-Primitiven, wird mit `InvalidHeader` abgelehnt),
`crates/transport-shm/src/posix.rs::tests::attach_rejects_unknown_version`
(`SHM_VERSION + 1` wird abgelehnt).

**Status:** done

### §2.3 Memory-Ordering von `head` und `tail`

**Spec:** §2 — der Writer MUSS `head` mit `Release` speichern, nachdem er
die Frame-Bytes geschrieben hat; der Reader MUSS `head` mit `Acquire`
laden, bevor er sie liest. Sichtbar wird ein Frame damit genau dann, wenn
der Reader den neuen `head`-Wert sieht — auch über Prozessgrenzen hinweg.

**Repo:** `crates/transport-shm/src/posix.rs`
(`SegmentLayout::head`, `SegmentLayout::tail`, die `Ordering::Release`-
und `Ordering::Acquire`-Stellen in `push_frame` und `pop_frame`).

**Tests:** `crates/transport-shm/tests/l1_cross_process.rs::l1_shm_datagram_crosses_process_boundary`
(zwei OS-Prozesse, ein Segment; der Empfänger sieht die Bytes des
Senders),
`crates/transport-shm/src/posix.rs::tests::open_concurrent_two_threads_both_bound`,
`crates/transport-shm/src/posix.rs::tests::many_frames_roundtrip_with_wraparound`.

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

**Repo:** `crates/transport-shm/src/posix.rs`
(`SegmentLayout::write_u32`, `read_u32`, `push_frame`, `pop_frame`,
`occupied_bytes`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::open_owner_then_consumer_roundtrip_single_frame`,
`crates/transport-shm/src/posix.rs::tests::occupied_bytes_reflects_send_state`,
`crates/transport-shm/src/posix.rs::tests::recv_after_send_drains_occupied_bytes`.

**Status:** done

### §3.2 Padding-Frame am Ring-Ende

**Spec:** §3 — ein Frame wird nie über das Ring-Ende hinweg gesplittet.
Reicht der zusammenhängende Platz am Ende nicht, MUSS der Writer einen
Padding-Frame mit `len = 0xFFFF_FFFE` setzen und an den Anfang springen;
der Reader überspringt ihn und zählt ihn zur Diagnose mit. Bleiben unter
4 Byte übrig, wird der Rest ohne Marker übersprungen.

**Repo:** `crates/transport-shm/src/posix.rs` (`PADDING_FRAME_LEN`,
Wrap-Zweig in `push_frame` mit Padding und Überspringen eines Rests unter
4 Byte, Padding- und `tail_space < 4`-Zweig in `pop_frame`,
`padding_frames_seen`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::fill_ring_triggers_wraparound_padding`
(300-Byte-Ring, fünf 60-Byte-Frames — der Wrap ist erzwungen und jeder
Frame kommt unverändert an),
`crates/transport-shm/src/posix.rs::tests::a_remainder_too_small_for_a_padding_header_is_skipped`
(ein Frame endet 2 Byte vor dem Ring-Ende; der nächste geht durch),
`crates/transport-shm/src/posix.rs::tests::frames_of_every_length_cross_the_ring_end_whole_and_in_order`
(5 000 Runden mit Längen 1–200 Byte; ohne das Überspringen des Rests
brach der Lauf in Runde 16 mit „shm ring full near wraparound“ ab),
`crates/transport-shm/src/posix.rs::tests::many_frames_roundtrip_with_wraparound`,
`crates/transport-shm/src/posix.rs::tests::corrupt_frame_counter_default_zero`.

**Status:** done

### §3.3 Längen-Sanity-Check mit Zähler

**Spec:** §3 — der Reader MUSS eine Länge oberhalb der halben Ringgröße
verwerfen, statt die angekündigte Größe zu allokieren, und den Vorgang
zählen. Die Schranke ist die des Rings, nicht das eigene `max_datagram`:
ein Owner, der ein größeres konfiguriert hat, schickt gültige Frames bis
dorthin. Ein stiller Drop ist nicht zulässig: der Zähler ist das
Ops-Signal für einen defekten oder bösartigen Owner.

**Repo:** `crates/transport-shm/src/posix.rs` (DoS-Zweig in `pop_frame`,
`corrupt_frame_counter`, `corrupt_frames_seen`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::corrupt_length_is_dropped_and_counted`
(schreibt halbe Ringgröße + 1 als Längen-Header in den Ring; `pop_frame`
liefert nichts und `corrupt_frames_seen()` steht auf 1),
`crates/transport-shm/src/posix.rs::tests::corrupt_frame_counter_default_zero`,
`crates/transport-shm/src/posix.rs::tests::send_rejects_oversize_payload`.

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

**Tests:** `crates/transport-shm/src/posix.rs::tests::open_owner_then_consumer_roundtrip_single_frame`,
`crates/transport-shm/src/posix.rs::tests::send_rejects_wrong_peer`
(ein Locator, der nicht der Peer dieses Segments ist, wird abgewiesen),
`crates/transport-shm/tests/l1_cross_process.rs::l1_shm_datagram_crosses_process_boundary`.

**Status:** done

### §4.2 Rollen-Trennung von `send` und `recv`

**Spec:** §4 — der Owner schreibt, der Consumer liest. Ein `send` auf der
Consumer-Seite und ein `recv` auf der Owner-Seite MÜSSEN mit einem Fehler
scheitern; sonst würde eine zweite Schreibrichtung die SpSc-Annahme des
Rings brechen.

**Repo:** `crates/transport-shm/src/posix.rs` (Rollen-Prüfung in
`Transport::send` und `Transport::recv`, `ShmRole`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::consumer_cannot_send`,
`crates/transport-shm/src/posix.rs::tests::owner_cannot_recv`.

**Status:** done

### §4.3 Idempotentes Öffnen unabhängig von der Rolle

**Spec:** §4 — die SEDP-Match-Reihenfolge ist nicht vorhersagbar: die
Reader-Seite kann vor der Writer-Seite öffnen. Wer zuerst kommt, legt das
Segment an; wer später kommt, hängt sich an. Die Rolle bestimmt nur
send/recv, nicht wer erzeugt.

**Repo:** `crates/transport-shm/src/posix.rs` (`open` — flock, Attach-
Versuch, sonst Create; `open_owner` und `open_consumer` teilen sich
denselben Pfad).

**Tests:** `crates/transport-shm/src/posix.rs::tests::open_consumer_first_then_owner_roundtrip`,
`crates/transport-shm/src/posix.rs::tests::consumer_open_succeeds_without_owner_creates_segment`,
`crates/transport-shm/src/posix.rs::tests::open_concurrent_two_threads_both_bound`.

**Status:** done

### §4.4 Ereignisgetriebenes Warten statt Poll-Schleife

**Spec:** §4 — ein wartender Consumer MUSS vom Writer geweckt werden,
statt in einer Schleife zu pollen. Der `head`-Wert wird vor `pop_frame`
gelesen, damit ein zwischenzeitlich publizierter Frame das Parken sofort
scheitern lässt (kein verlorener Wakeup).

**Repo:** `crates/transport-shm/src/posix.rs` (`ring_park` / `ring_wake`
auf den unteren 32 Bit von `head`, aufgerufen in `wait_parked` und
`push_frame`: auf Linux `ring_futex`, auf macOS ab 14.4 `ring_os_sync`
mit `os_sync_wait_on_address` und `OS_SYNC_WAIT_ON_ADDRESS_SHARED`, zur
Laufzeit über `dlsym` aufgelöst; sonst `wait_polling`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::the_consumer_parks_instead_of_polling`
(Linux und macOS),
`crates/transport-shm/src/posix.rs::tests::owner_drop_signals_consumer_via_shutdown_flag`
(der wartende Consumer kehrt in unter 200 ms zurück, obwohl das
`recv_timeout` 10 s beträgt),
`crates/transport-shm/src/posix.rs::tests::interrupt_recv_returns_a_waiting_recv_at_once`,
`crates/transport-shm/src/posix.rs::tests::recv_times_out_when_idle`,
`crates/flatdata/src/posix.rs::tests::futex_notify_wakes_consumer_across_mappings`.
Gemessen auf macOS 26 (Mac unter Last, Lastmittel 97), drei Läufe: 100
wartende Consumer 0,3 % eines Kerns; Wecklatenz p50 22 bis 24 µs gegen
63 bis 69 µs über UDP-Loopback im selben Lauf. Mit dem Sleep-Poll hatte
eine Messung am selben Mac 4,45 % eines Kerns je Consumer und p50 177 µs
ergeben.

Big-Endian-Linux parkt auf denselben unteren 32 Bit von `head`, die dort
auf Byte 20 liegen (`head_word`); auf s390x unter qemu-user grün (alle
`transport-shm`-Tests, Linux-Testhost). Windows wartet auf einem benannten
Auto-Reset-Event je Segment (`RingEvent`), das der Owner nach jedem Frame
setzt; im Nightly 6555 (`windows-smoke`, `c704c3ccfaa`) liefen
`interrupt_recv_returns_a_waiting_recv_at_once` und
`bell::tests::a_ring_wakes_the_parked_receiver_and_none_is_lost` auf
Windows grün.

**Status:** partial — macOS vor 14.4 wartet per Sleep-Poll.

### §4.5 Backpressure bei vollem Ring

**Spec:** §4 — ist der Ring voll, weil der Reader nicht nachkommt, MUSS
`send` nach einer begrenzten Spin-Phase mit einem benannten Fehler
zurückkehren. Blockieren ohne Ende und stilles Verwerfen sind beide nicht
zulässig.

**Repo:** `crates/transport-shm/src/posix.rs` (`SPIN_LIMIT = 1024`, die
beiden `SendError::Io`-Zweige in `push_frame`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::full_ring_returns_an_error_instead_of_dropping`
(512-Byte-Ring, 128-Byte-Frames, kein Consumer-Drain; der Fehlertext
nennt „ring full"),
`crates/transport-shm/src/posix.rs::tests::send_rejects_oversize_payload`.

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

**Tests:** `crates/transport-shm/src/posix.rs::tests::segment_os_id_is_deterministic_and_within_platform_limit`
(gleiches Paar liefert denselben Namen, umgekehrtes Paar einen anderen,
Länge unter 31 auf macOS),
`crates/transport-shm/src/posix.rs::tests::short_names_of_pairs_that_differ_only_in_the_prefix_differ`
(die kurze Form, auf jeder Plattform: zwei Paare mit gleichen Entity-IDs
und anderen Teilnehmern bekommen verschiedene Namen),
`crates/transport-shm/src/posix.rs::tests::open_owner_then_consumer_roundtrip_single_frame`.

**Status:** done

### §5.2 Crash-Recovery

**Spec:** §5 — vor dem Anlegen eines Segments MUSS ein Zombie desselben
Namens entfernt werden (`shm_unlink`, idempotent — `ENOENT` wird
ignoriert), sonst leckt `/dev/shm` beim N-ten Neustart. Eine
zurückgebliebene Flink-Datei darf das Öffnen nicht scheitern lassen.

**Repo:** `crates/transport-shm/src/posix.rs` (`shm_unlink_by_os_id`,
Create-Zweig in `open` mit vorherigem `remove_file` auf den Flink).

**Tests:** `crates/transport-shm/src/posix.rs::tests::open_recovers_from_stale_flink_of_crashed_owner`
(Flink-Datei eines abgestürzten Owners zeigt auf ein nicht mehr
existentes Segment; `open_owner` räumt auf, und ein Frame läuft danach
durch),
`crates/transport-shm/src/posix.rs::tests::consumer_open_succeeds_without_owner_creates_segment`.

**Status:** done

### §5.3 Shutdown-Flag

**Spec:** §5 — der Owner MUSS beim Drop `shutdown = 1` setzen
(Release-Store). Der Consumer prüft das Flag nach jedem leeren Poll und
kehrt mit einem gezielten Fehler zurück, statt bis zum `recv_timeout` zu
warten. Frames, die vor dem Drop publiziert wurden, kommen noch an — die
Prüfung steht hinter `pop_frame`.

**Repo:** `crates/transport-shm/src/posix.rs` (`SegmentLayout::shutdown`,
`Drop for PosixShmTransport`, Shutdown-Prüfung in `wait_for_frame`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::owner_drop_signals_consumer_via_shutdown_flag`
(misst die Rückkehrzeit gegen ein 10-s-Timeout und prüft den
Fehlertext auf „owner"),
`crates/transport-shm/src/posix.rs::tests::posix_shm_error_display_covers_all_variants`.

**Status:** done

### §5.4 Race-Schutz beim Anlegen

**Spec:** §5 — parallele Öffner derselben `os_id` MÜSSEN über Threads und
Prozesse hinweg serialisiert werden, sonst entfernt der eine das gerade
angelegte Segment des anderen. Der Lock wird beim Schließen des Handles
oder beim Prozess-Tod automatisch frei (gleiche Crash-Resilienz wie das
Segment selbst).

**Repo:** `crates/transport-shm/src/posix.rs` (`acquire_flock_excl` —
`flock(LOCK_EX)` auf Unix, `LockFileEx` mit `LOCKFILE_EXCLUSIVE_LOCK` auf
Windows —, `FlockGuard`, Sentinel-Datei `<flink>.lock`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::open_concurrent_two_threads_both_bound`
(zwei Threads öffnen dieselbe `os_id` gleichzeitig; beide sind danach
gültig gebunden),
`crates/transport-shm/src/posix.rs::tests::users_in_different_flink_directories_share_the_segment`
(Owner und Consumer suchen das Segment in verschiedenen Flink-Verzeichnissen,
etwa unter verschiedenem `TMPDIR`; der zweite hängt sich an das gehaltene
Segment, statt es zu entfernen und ein eigenes anzulegen).

**Status:** done

### §5.5 Liveness-Lock und verwaiste Segmente

**Spec:** §5 — jeder Nutzer eines Segments, Owner wie Consumer, MUSS für
die Dauer der Nutzung einen geteilten `flock` auf dem Segment halten. Der
Kernel gibt ihn frei, wenn der Prozess endet, auch durch `SIGKILL`; ein
Segment ohne Lock-Halter ist verwaist. Ein Opener, der unter dem Namen
seines Paares ein verwaistes Segment findet, MUSS es entfernen und neu
anlegen, statt es zu übernehmen: sein Ring enthält die Frames der
beendeten Sitzung. Verwaiste Segmente, die niemand wieder öffnet, entfernt
ein Sweep über `/dev/shm`. Er lässt Segmente stehen, die jünger als ein
Mindestalter sind (das deckt den Abstand zwischen dem Anlegen und dem Lock
des Erstellers) oder eine andere Layout-Version tragen. Version 2
kennzeichnet dieses Protokoll: ein Nutzer der Version 1 hält keinen Lock,
seine lebenden Segmente sähen verwaist aus.

**Repo:** `crates/transport-shm/src/posix.rs` (`hold_segment`,
`SegmentHold`, `sweep_orphaned_segments`, `SweepReport`,
`sweep_stale_pair_files` für die `.lock`-Dateien und, auf Linux, die
Flinks der Paare, deren Segment fort ist — sie lagen nach jedem nicht
sauber beendeten Prozess (eine Testkette auf einem MacBook Air: 3 504 `.lock` in einem
Domain-Verzeichnis); `lock_pair` prüft nach dem Sperren, dass der Pfad
noch die gesperrte Datei nennt, `SHM_VERSION = 2`; ein Ring mit leerem Kopf, dessen Ersteller vor dem
Schreiben des Kopfs endete, gilt als entfernbar, weil kein Anhänger ihn
annimmt), `crates/transport-shm/src/bell.rs` (die Türglocke eines
Teilnehmers: geteilter `flock` auf ihrer Datei in `/dev/shm`,
`sweep_orphaned_doorbells`); `crates/dcps/src/same_host_shm.rs`
(`sweep_orphaned_segments_once`: einmal pro Prozess beim Start eines
Teilnehmers mit `transport.same_host_shm`, Ringe und Türglocken, danach
je Flink-Verzeichnis die Paar-Dateien, Mindestalter 60 s, Ereignis
`shm.orphaned_segments_removed`).

**Tests:** `crates/transport-shm/src/posix.rs::tests::an_orphaned_segment_is_made_anew_not_attached`
(ein eigener Prozess öffnet den Owner, publiziert einen Frame und endet
ohne Drop; der Consumer findet danach einen leeren Ring und empfängt nur
den Frame der neuen Sitzung),
`crates/transport-shm/src/posix.rs::tests::the_sweep_removes_orphaned_segments_and_keeps_held_ones`
(das verwaiste Segment bleibt unter dem Mindestalter stehen und wird
darüber entfernt; ein gehaltenes Segment bleibt, und ein Frame läuft
danach durch; ein Segment mit leerem Kopf wird entfernt),
`crates/transport-shm/src/bell.rs::tests::the_sweep_removes_orphaned_bells_and_keeps_held_ones`,
`crates/transport-shm/src/posix.rs::tests::the_pair_file_sweep_removes_lone_lock_files_and_keeps_the_rest`,
`crates/transport-shm/src/posix.rs::tests::a_lock_taken_on_a_removed_lock_file_is_taken_anew`.
Gemessen auf dem Linux-Testhost: zwei Teilnehmer per `kill -9` beendet; der Start
eines dritten nach 60 s entfernte ihre 2 Ringe und 2 Türglocken
(`removed: 4`). Vorher blieben Türglocken jedes nicht sauber beendeten
Prozesses liegen (138 nach einem Tag Proben).

Auf macOS und den übrigen Unix-Systemen ohne Shared-Memory im
Dateisystem liegt der Lock auf der Flink-Datei des Paares, und
`sweep_orphaned_flinks` findet verwaiste Segmente über das
Flink-Verzeichnis (dcps: einmal je Verzeichnis, also je Host und Domain).
Eine Türglocke legt dort eine Markierung `<name>.bell` an, auf der ihr
Teilnehmer die Sperre hält; der Sweep entfernt Glocke und Markierung,
wenn niemand sie hält. Tests
`the_flink_sweep_removes_orphaned_segments_and_keeps_held_ones` und
`bell.rs::tests::the_flink_sweep_removes_orphaned_bells_and_keeps_held_ones`,
auf macOS 26 grün; `an_orphaned_segment_is_made_anew_not_attached` läuft auf
jedem Unix. Windows: §6.3.

**Status:** done

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

**Tests:** `crates/transport-shm/src/mapping.rs::tests::a_mapping_holds_no_file_descriptor`
(100 Mappings, Zahl der offenen Deskriptoren in einem eigenen Prozess
unverändert; macOS 26),
`crates/transport-shm/src/mapping.rs::tests::create_open_share_and_the_owner_unlinks`,
`crates/transport-shm/src/posix.rs::tests::segment_os_id_is_deterministic_and_within_platform_limit`
(prüft das 31-Zeichen-Limit unter `cfg(target_os = "macos")`),
`crates/transport-shm/tests/l1_cross_process.rs::l1_shm_datagram_crosses_process_boundary`,
`crates/transport-shm/src/posix.rs::tests::open_owner_then_consumer_roundtrip_single_frame`.

**Status:** done

### §6.2 Windows

**Spec:** §6 — auf Windows liefert `CreateFileMapping` die Mappings, der
Create-Race wird über `LockFileEx` serialisiert. Der Transport MUSS dort
mit derselben Test-Suite laufen wie auf Unix.

**Repo:** `crates/transport-shm/src/posix.rs`
(`#[cfg(windows)]`-Variante von `acquire_flock_excl` mit
`LockFileEx`/`LOCKFILE_EXCLUSIVE_LOCK`,
`#[cfg(not(any(target_os = "linux", target_os = "macos")))]`-No-op von
`shm_unlink_by_os_id`); CI-Job `windows-smoke` in `.gitlab-ci.yml` ruft
`cargo test -p zerodds-transport-shm` auf dem Windows-Runner.

**Tests:** `crates/transport-shm/src/posix.rs::tests::open_concurrent_two_threads_both_bound`
(der Test, der den `LockFileEx`-Pfad auf Windows fährt),
`crates/transport-shm/src/posix.rs::tests::open_recovers_from_stale_flink_of_crashed_owner`.

**Status:** partial — im Nightly 6555 (`windows-smoke`, `c704c3ccfaa`)
liefen 39 von 40 Lib-Tests von `transport-shm` auf Windows grün; der
fehlgeschlagene ist §6.3.

### §6.3 Nachweis der Windows-Cleanup-Semantik

**Spec:** §6 — dass ein abgestürzter Owner auf Windows kein Mapping mit
den Frames seiner Sitzung hinterlässt, MUSS ein Test auf dem
Windows-Runner belegen, nicht nur die Dokumentation behaupten.

**Repo:** `crates/transport-shm/src/posix.rs` (`#[cfg(windows)]`-Variante
von `hold_segment` mit `LockFileEx` auf dem Flink, wie die
macOS-Sperre; `discard_orphaned_segment` entfernt die Backing-Datei).
Die frühere Annahme, Windows gebe das Mapping mit dem letzten Handle
frei, trifft für `shared_memory` nicht zu: Es legt jedes Mapping auf eine
Datei `%TEMP%\shared_memory-rs\<name>`, die den Prozess überlebt
(Datei windows.rs der Crate, Funktion `new_map`). Gemessen im Nightly 6555: Der Test fand 29
Byte der beendeten Sitzung im Ring.

**Tests:** `crates/transport-shm/src/posix.rs::tests::an_orphaned_segment_is_made_anew_not_attached`
(ein Kindprozess öffnet den Owner, publiziert einen Frame und endet ohne
Drop; derselbe Name öffnet danach mit leerem Ring). Läuft auf jeder
Plattform, auf Windows im Job `windows-smoke`.

**Status:** partial — die Sperre ist gebaut und für
`x86_64-pc-windows-gnu` mit Clippy geprüft; gelaufen ist sie nicht, das
misst der nächste `windows-smoke`-Lauf.

### §6.4 `no_std`

**Spec:** §6 — der SHM-Transport ist `std`-only. `shm_open`, `mmap`,
`flock` und die Flink-Dateien sind OS-Aufrufe; ein `no_std`-Ziel ohne
Betriebssystem hat kein Äquivalent. Der Crate ist deshalb hinter
`#[cfg(feature = "std")]` gestellt.

**Repo:** —

**Tests:** —

**Status:** `open` — der Transport läuft nur mit `std`
(`crates/transport-shm/src/lib.rs`: `#[cfg(feature = "std")] pub mod posix`):
`shm_open`, `mmap` und `flock` sind POSIX-Aufrufe. Ein Ziel ohne POSIX bräuchte einen Slot-Speicher
über den Speicher, den seine Plattform zwischen Prozessen oder Kernen teilt;
den gibt es nicht.

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

**Tests:** `crates/zerodds-c-api/tests/shm_loan_e2e.rs::shm_loan_writer_to_reader_zero_copy`
(misst den Loan-Pfad selbst, nicht seinen Aufbau: der Test schreibt die
Nutzlast direkt in den geliehenen Zeiger, committet in place und prüft
beim Reader mit `assert_ne!`, dass dessen Lesezeiger eine andere Adresse
trägt als der Writer-Zeiger — eigenes Mapping desselben Segments — und
dass die Bytes gleich sind; danach Release, `NoData` beim nächsten Take
und ein zweites Sample durch denselben Ring),
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::shm_loan_runtime_path_writer_to_reader_zero_copy`
(derselbe Pfad über die Runtime-FFI, die das ROS-2-RMW nutzt),
`crates/zerodds-c-api/tests/shm_loan_e2e.rs::loan_without_shm_falls_back_to_heap`
(ohne aktivierten Loan bleibt der Heap-Pfad),
`crates/flatdata/src/posix.rs::tests::write_read_through_shm`
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

**Status:** n/a (informative)

---

## Audit-Status

18 done / 3 partial / 1 open / 2 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-transport-shm` — 27 Tests (lib) + 1
Test (`tests/l1_cross_process.rs`) grün, 0 failed;
`cargo test -p zerodds-c-api --test shm_loan_e2e` — 5 Tests grün, 0
failed. Gemessen auf macOS 15 (Rust-Host `aarch64-apple-darwin`); der
Windows-Lauf aus §6.2 und der Linux-Futex-Pfad aus §4.4 sind darin nicht
enthalten.

Offene Punkte und Decision-Records: [`zerodds-shm-transport-1.0.open.md`](zerodds-shm-transport-1.0.open.md).

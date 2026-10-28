# ZeroDDS-UDS-Transport 1.0 — Spec-Coverage

**Quelle:** dieses Dokument ist zugleich die Spec. Für Unix-Domain-Sockets
normiert OMG nichts außer dem vendor-reservierten Locator-Kind
(DDSI-RTPS 2.5 §9.4.5); Pfad-Auflösung, Wire-Format und Cleanup sind
vendor-spezifisch. Cyclone DDS und FastDDS haben keinen offiziellen
UDS-Transport. Die normativen Anforderungen stehen in den `##`-Sektionen
§2 bis §6; jedes `###`-Item darunter trägt Spec-Zitat, Repo-Anker, Test
und Status.

Implementation: `crates/transport-uds/src/lib.rs` — `SOCK_DGRAM` über
Filesystem-Sockets; `crates/transport-uds/src/abstract_dgram.rs` —
Linux-Abstract-Namespace, `#[cfg(target_os = "linux")]`.

Offene Punkte und Decision-Records: [`zerodds-uds-transport-1.0.open.md`](zerodds-uds-transport-1.0.open.md).

## §1 Scope und Spec-Status

### §1.1 Vendor-reservierter Locator-Kind

**Spec:** DDSI-RTPS 2.5 §9.3.2.4 — `0x00000003` bis `0x01ffffff` sind für
vendor-spezifische Locator-Kinds reserviert. ZeroDDS belegt `0x01F0_0002` für
UDS. Das Port-Feld ist bedeutungslos und MUSS `0` sein; die 16
Adress-Bytes tragen die Endpoint-ID.

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind::Uds`,
`Locator::uds`); gebunden in `crates/transport-uds/src/lib.rs`
(`UdsTransport::local_locator`, Kind-Prüfung in `Transport::send`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::locator_kind_is_the_vendor_reserved_uds_value`
(prüft den Zahlwert `0x01F0_0002`, die Lage im Bereich `0x00000003` bis `0x01ffffff` und `port == 0`),
`crates/transport-uds/src/lib.rs::tests::local_locator_reflects_bind_id`,
`crates/transport-uds/src/lib.rs::tests::send_rejects_non_uds_locator`,
`crates/rtps/src/wire_types.rs::tests::locator_uds_layout`,
`crates/rtps/src/wire_types.rs::tests::locator_kind_roundtrip`.

**Status:** done

### §1.2 Was OMG für UDS nicht normiert

**Spec:** DDSI-RTPS 2.5 kennt kein Pfad-Schema, kein UDS-Wire-Format und
kein Cleanup-Protokoll. Cyclone DDS und FastDDS haben keinen offiziellen
UDS-Transport; sie decken denselben Anwendungsfall über iceoryx oder
POSIX-SHM ab. Die Sektion hält den Scope fest und stellt keine
Implementierungs-Anforderung.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 Pfad-Auflösung

### §2.1 Filesystem-Modus (Default)

**Spec:** §2 — die 16-Byte-Locator-Adresse wird als
`<base_dir>/<lowercase-hex32>.sock` aufgelöst. Default-`base_dir` ist
`/tmp/zerodds/uds`. Der Hex-String ist die Adresse in 32 Kleinbuchstaben,
ohne Trenner.

**Repo:** `crates/transport-uds/src/lib.rs` (`socket_path`,
`DEFAULT_BASE_DIR`, `parse_hex_id`, `source_locator` als Rückrichtung).

**Tests:** `crates/transport-uds/src/lib.rs::tests::socket_path_is_hex_under_base_dir`
(prüft Präfix und `.sock`-Endung an einer Adresse mit gesetzten
Randbytes),
`crates/transport-uds/src/lib.rs::tests::parse_hex_id_roundtrip`,
`crates/transport-uds/src/lib.rs::tests::parse_hex_id_rejects_wrong_length`,
`crates/transport-uds/src/lib.rs::tests::parse_hex_id_rejects_non_hex`.

**Status:** done

### §2.2 Basis-Verzeichnis: lazy, user-privat, TOCTOU-fest

**Spec:** §2 — das Basis-Verzeichnis wird beim Bind angelegt, wenn es
fehlt, und trägt dann Mode `0700`. Ein bestehender Pfad, der ein Symlink
oder keine Verzeichnis ist, MUSS abgelehnt werden, statt ihn zu
chmodden — sonst könnte ein untergeschobener Symlink fremde Pfade auf
`0700` setzen. Auf ein bestehendes fremdes Verzeichnis wird kein Mode
gesetzt.

**Repo:** `crates/transport-uds/src/lib.rs` (`ensure_base_dir` —
`symlink_metadata` statt `exists`, Ablehnung von Symlink und
Nicht-Verzeichnis, `create_dir_all` plus `set_permissions(0o700)` mit
Wiederholungs-Prüfung).

**Tests:** `crates/transport-uds/src/lib.rs::tests::ensure_base_dir_creates_dir_with_mode_0700`
(liest den Mode zurück und vergleicht ihn gegen `0o700`),
`crates/transport-uds/src/lib.rs::tests::ensure_base_dir_rejects_symlink`,
`crates/transport-uds/src/lib.rs::tests::ensure_base_dir_rejects_file_path`,
`crates/transport-uds/src/lib.rs::tests::ensure_base_dir_creates_missing_dir`.

**Status:** done

### §2.3 Abstract-Namespace-Modus (Linux)

**Spec:** §2 — auf Linux gibt es die Alternative ohne Filesystem-Inode:
der Socket-Name beginnt mit einem Null-Byte, gefolgt von
`<prefix>-<lowercase-hex32>`. Kein Cleanup nötig, keine
Inode-Permissions, kein Cross-Mount-Volume. Der Name passt in
`sun_path` (108 Byte, Byte 0 ist das Null-Präfix).

**Repo:** `crates/transport-uds/src/abstract_dgram.rs`
(`UdsAbstractDgramTransport`, `UdsAddress::Abstract`,
`MAX_ABSTRACT_NAME = 107`, `UdsAddress::abstract_default`); Modul
deklariert in `crates/transport-uds/src/lib.rs` unter
`#[cfg(target_os = "linux")]`.

**Tests:** `crates/transport-uds/src/abstract_dgram.rs::tests::abstract_bind_does_not_create_file`,
`crates/transport-uds/src/abstract_dgram.rs::tests::abstract_send_recv_roundtrip`,
`crates/transport-uds/src/abstract_dgram.rs::tests::abstract_preserves_message_boundaries`,
`crates/transport-uds/src/abstract_dgram.rs::tests::local_locator_reflects_bind_id`.
Diese Tests fahren auf macOS **0 Fälle**: das Modul steht hinter
`#[cfg(target_os = "linux")]` und wird dort nicht übersetzt. Gemessen
wurden sie auf dem Linux-Testhost gegen dieselbe, unveränderte
`abstract_dgram.rs` (Kommando `cargo test -p zerodds-transport-uds`) —
11 Fälle aus `abstract_dgram::tests` grün, 0 failed.

**Status:** done

### §2.4 Filesystem-Modus im Abstract-Transport (A/B-Vergleich)

**Spec:** §2 — derselbe Transport MUSS auch Filesystem-Adressierung
können, damit Abstract- und Filesystem-Modus mit identischem Socket-Code
gegeneinander gemessen werden können. Der Unterschied ist eine
Config-Zeile.

**Repo:** `crates/transport-uds/src/abstract_dgram.rs`
(`UdsAddress::Filesystem`, `UdsAddress::filesystem_default`, `fs_path`).

**Tests:** `crates/transport-uds/src/abstract_dgram.rs::tests::filesystem_bind_creates_socket_file`
(Socket-Datei entsteht beim Bind und verschwindet beim Drop),
`crates/transport-uds/src/abstract_dgram.rs::tests::filesystem_send_recv_roundtrip`.
Auch diese Tests sind `#[cfg(target_os = "linux")]`-gated und fahren auf
macOS 0 Fälle; gemessen auf dem Linux-Testhost im selben Lauf wie §2.3.

**Status:** done

## §3 Wire-Format

### §3.1 `SOCK_DGRAM` mit erhaltenen Message-Grenzen

**Spec:** §3 — eine Datagram-Message pro RTPS-Frame. Der Kernel erhält
die Message-Grenzen, anders als bei einem TCP-Stream; es braucht deshalb
kein Framing-Layer darüber. Zwei Sends MÜSSEN als zwei Receives mit den
ursprünglichen Längen ankommen.

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsTransport` über
`std::os::unix::net::UnixDatagram`, `Transport::send` mit `send_to`,
`Transport::recv` mit `recv_from`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::preserves_message_boundaries_two_datagrams`
(zwei Sends unterschiedlicher Länge, zwei Receives mit genau diesen
Längen),
`crates/transport-uds/src/lib.rs::tests::send_and_recv_roundtrip_same_process`,
`crates/transport-uds/tests/l1_cross_process.rs::l1_datagram_crosses_process_boundary`
(dieselbe Zustellung über eine echte Prozessgrenze).

**Status:** done

### §3.2 Absender-Locator aus der Socket-Adresse

**Spec:** §3 — der Empfänger MUSS aus der Absender-Adresse den
Locator zurückgewinnen, damit RTPS antworten kann. Ist die Adresse
unbenannt (anonymer Client ohne Bind) oder nicht auflösbar, MUSS
`Locator::INVALID` zurückgegeben werden — nicht geraten.

**Repo:** `crates/transport-uds/src/lib.rs` (`source_locator`,
`parse_hex_id`, `Locator::INVALID`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::send_and_recv_roundtrip_same_process`
(prüft `got.source` gegen den Locator des Senders),
`crates/transport-uds/src/lib.rs::tests::preserves_message_boundaries_two_datagrams`,
`crates/transport-uds/tests/l1_cross_process.rs::l1_datagram_crosses_process_boundary`.

**Status:** done

### §3.3 Datagram-Obergrenze

**Spec:** §3 — `max_datagram` (Default 65 536) begrenzt, was gesendet und
was in den Empfangspuffer gelesen wird. Eine größere Nutzlast MUSS mit
`PayloadTooLarge` abgelehnt werden; eine stille Kürzung ist nicht
zulässig, weil sie auf RTPS-Ebene von einer kurzen Nachricht nicht zu
unterscheiden wäre.

**Repo:** `crates/transport-uds/src/lib.rs` (`DEFAULT_MAX_DATAGRAM`,
Längenprüfung in `Transport::send`, Puffergröße in `Transport::recv`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::send_rejects_oversize_payload`
(Nutzlast über dem konfigurierten Limit wird mit `PayloadTooLarge`
abgelehnt),
`crates/transport-uds/src/lib.rs::tests::oversized_for_the_kernel_fails_loudly_and_never_truncates`
(1 KiB kommt unverkürzt an; bei `max_datagram` gilt entweder
unverkürzte Zustellung oder ein Fehler, der EMSGSIZE benennt — und dann
liegt nichts Teilweises in der Queue).

**Status:** done

### §3.4 Kernel-Obergrenze gegen konfigurierte Obergrenze

**Spec:** §3 — die tatsächliche Obergrenze pro Message setzt der Kernel:
Linux über `wmem_max` (Default 212 992), macOS über
`net.local.dgram.maxdgram` (Default 2048). Der Transport MUSS die
konfigurierte Grenze gegen die des Betriebssystems abgleichen, sie beim
Bind sichtbar machen oder die Socket-Puffer entsprechend setzen — sonst
scheitert ein Sample zwischen beiden Grenzen erst zur Laufzeit.

**Repo:** `crates/transport-uds/src/lib.rs` (`size_buffers`: Sendepuffer
auf das Doppelte, Empfangspuffer auf das Achtfache von `max_datagram`,
aufgerufen in `UdsTransport::bind` und
`UdsAbstractDgramTransport::bind`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::a_datagram_near_the_configured_limit_arrives_whole`
(vier Datagramme zu `max_datagram` − 512 Byte kommen ganz an; auf macOS
26 gemessen, vorher dort EMSGSIZE über 2048 Byte),
`crates/transport-uds/src/lib.rs::tests::oversized_for_the_kernel_fails_loudly_and_never_truncates`.

**Status:** done

## §4 Cleanup-Semantik

### §4.1 Filesystem-Modus: Cleanup beim Drop, Übernahme beim Bind

**Spec:** §4 — der Bind legt die Socket-Datei an, der Drop entfernt sie
best-effort (ein abgehängtes Container-Volume darf den Drop nicht
scheitern lassen). Findet ein Bind eine Restdatei eines abgestürzten
Vorgängers, MUSS er sie entfernen und neu binden: `UnixDatagram::bind`
kann nicht über eine bestehende Datei binden und meldet sonst
`EADDRINUSE`.

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsTransport::bind` mit
`remove_file` bei bestehendem Pfad, `Drop for UdsTransport`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::bind_creates_socket_file`
(Datei existiert nach dem Bind und ist nach dem Drop weg),
`crates/transport-uds/src/lib.rs::tests::bind_reuses_path_after_stale_leftover`
(eine Restdatei mit fremdem Inhalt blockiert den Bind nicht).

**Status:** done

### §4.2 Abstract-Modus: kein Cleanup nötig

**Spec:** §4 — im Abstract-Namespace gibt es keinen Inode; Linux gibt den
Namen frei, sobald der letzte Filedeskriptor schließt. Ein
Crash-Cleanup-Pfad ist deshalb nicht erforderlich, und ein Bind darf
keine Datei anlegen.

**Repo:** `crates/transport-uds/src/abstract_dgram.rs`
(`UdsAbstractDgramTransport::bind` im `Abstract`-Zweig ohne
Filesystem-Zugriff, `Drop` entfernt nur im `Filesystem`-Zweig).

**Tests:** `crates/transport-uds/src/abstract_dgram.rs::tests::abstract_bind_does_not_create_file`,
`crates/transport-uds/src/abstract_dgram.rs::tests::filesystem_bind_creates_socket_file`
(die Gegenprobe: im Filesystem-Zweig entsteht und verschwindet die
Datei). Beide `#[cfg(target_os = "linux")]`-gated, auf macOS 0 Fälle;
gemessen auf dem Linux-Testhost wie §2.3.

**Status:** done

### §4.3 Differenzierte Fehlerklassifikation beim Senden

**Spec:** §4 — ein Sendefehler MUSS seine Ursache benennen: fehlender
Ziel-Socket, verweigerte Permission, voller Kernel-Puffer, abgelehnte
Verbindung und überschrittene Message-Grenze sind unterschiedliche
Betriebszustände. Ein Sammel-`Io` macht sie ununterscheidbar.

**Repo:** `crates/transport-uds/src/lib.rs` (`classify_send_error` —
`NotFound`, `PermissionDenied`, `WouldBlock`, `ConnectionRefused`,
`EMSGSIZE` über `raw_os_error`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::classify_send_error_maps_kinds`
(alle benannten `io::ErrorKind`-Werte auf ihre Meldung),
`crates/transport-uds/src/lib.rs::tests::send_to_missing_peer_is_io_error`,
`crates/transport-uds/src/lib.rs::tests::oversized_for_the_kernel_fails_loudly_and_never_truncates`
(der EMSGSIZE-Zweig, sofern der Kernel die Message ablehnt).

**Status:** done

### §4.4 Leerlauf-Verhalten von `recv`

**Spec:** §4 — mit gesetztem `recv_timeout` MUSS ein leerer Socket nach
Ablauf `RecvError::Timeout` liefern, nicht einen Io-Fehler. Ohne Timeout
blockiert `recv`.

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsConfig::recv_timeout`,
`set_read_timeout` im Bind, `WouldBlock`/`TimedOut`-Zweig in
`Transport::recv`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::recv_times_out_when_idle`,
`crates/transport-uds/src/abstract_dgram.rs::tests::recv_times_out_when_idle`
(derselbe Nachweis im Abstract-Transport, Linux-gated).

**Status:** done

## §5 Container-Anwendungsfall

### §5.1 Volume-Mount als Permission-Grenze

**Spec:** §5 — das Muster für Docker und Kubernetes ist ein gemeinsam
gemountetes Volume unter `base_dir`; jeder Container bindet seinen
eigenen 16-Byte-Locator als Socket-Datei, der Kernel routet zwischen den
Filedeskriptoren. UDS ist dort die Wahl, wenn Multicast durch eine
Cluster-Network-Policy blockiert ist und POSIX-SHM cross-Container
unpraktisch ist (UID-Mapping, `/dev/shm`-Sichtbarkeit, SELinux-Profile).

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsConfig::base_dir` als
freier Pfad, `socket_path` unterhalb davon; Drop-Cleanup ist
best-effort, damit ein abgehängtes Volume den Drop nicht scheitern
lässt).

**Tests:** `crates/transport-uds/tests/l1_cross_process.rs::l1_datagram_crosses_process_boundary`
(zwei OS-Prozesse über ein gemeinsames Verzeichnis — dieselbe Topologie
wie zwei Container über ein gemeinsames Volume, ohne den
Container-Runtime-Anteil),
`crates/transport-uds/src/lib.rs::tests::send_and_recv_roundtrip_same_process`.

**Status:** partial — die Prozessgrenze ist belegt, die
Container-Grenze nicht: es gibt keinen Test, der zwei Container mit
gemountetem Volume startet und die UID-Mapping- und SELinux-Fragen
tatsächlich durchspielt. Der fehlende Nachweis ist als §5.2 offen
geführt.

### §5.2 Nachweis über eine echte Container-Grenze

**Spec:** §5 — dass das Muster aus §5.1 unter einer Container-Runtime
trägt, MUSS ein Test mit zwei Containern und einem gemounteten Volume
belegen: unterschiedliche UID-Mappings auf beiden Seiten, Socket-Datei
auf dem Volume, Datagram in beide Richtungen.

**Repo:** —

**Tests:** —

**Status:** open

## §6 Plattform-Support

### §6.1 Linux und macOS

**Spec:** §6 — Linux ist die primäre Plattform mit beiden Modi
(Filesystem und Abstract-Namespace), macOS unterstützt den
Filesystem-Modus. Der Crate ist als Ganzes `#[cfg(unix)]`.

**Repo:** `crates/transport-uds/src/lib.rs` (`#![cfg(unix)]`,
`#[cfg(target_os = "linux")] pub mod abstract_dgram`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::send_and_recv_roundtrip_same_process`
und `crates/transport-uds/tests/l1_cross_process.rs::l1_datagram_crosses_process_boundary`
(auf beiden Plattformen gemessen: macOS in diesem Audit, Linux auf
Linux-Testhost), `crates/transport-uds/src/abstract_dgram.rs::tests::abstract_send_recv_roundtrip`
(nur Linux).

**Status:** done

### §6.2 Windows

**Spec:** §6 — Unix-Domain-Sockets sind Unix-spezifisch. Auf Windows
übersetzt der Crate zu einer leeren Bibliothek; Konsumenten schalten ihre
UDS-Nutzung mit `#[cfg(unix)]`. Die naheliegende Analogie wären
Named Pipes — ein anderes Protokoll mit anderer Adressierung, das nicht
Teil dieser Spec ist.

**Repo:** —

**Tests:** —

**Status:** `open` — auf Windows übersetzt der Crate zu einer leeren
Bibliothek (`crates/transport-uds/src/lib.rs`: `#![cfg(unix)]`). Windows bietet seit Windows 10 (Build 17063) Sockets der Familie
`AF_UNIX` an (Angabe von Microsoft, hier nicht geprüft); der Transport nutzt
sie nicht.

### §6.3 Cross-Vendor-Interop

**Spec:** §6 — UDS ist Intra-Container- und Intra-Host-IPC. Cross-Vendor-
Interop mit Cyclone DDS oder FastDDS bleibt in der UDP-, TCP- und
SHM-Domäne; beide Vendoren haben keinen UDS-Transport, mit dem sich ein
Wire-Vergleich führen ließe. Die Sektion hält den Scope fest und stellt
keine Implementierungs-Anforderung.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

14 done / 1 partial / 2 open / 2 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-transport-uds` — auf macOS 15
(Rust-Host `aarch64-apple-darwin`) 20 Tests (lib) + 1 Test
(`tests/l1_cross_process.rs`) grün, 0 failed. Der
`#[cfg(target_os = "linux")]`-gated `abstract_dgram`-Modul fährt dort 0
Fälle; derselbe Befehl auf dem Linux-Testhost gegen die unveränderte
`abstract_dgram.rs` — 11 `abstract_dgram::tests`-Fälle grün, 0 failed.

Offene Punkte und Decision-Records: [`zerodds-uds-transport-1.0.open.md`](zerodds-uds-transport-1.0.open.md).

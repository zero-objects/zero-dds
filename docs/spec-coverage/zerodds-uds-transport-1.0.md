# ZeroDDS-UDS-Transport 1.0 — Spec-Coverage

**Quelle:** dieses Dokument ist zugleich die Spec. Für Unix-Domain-Sockets
normiert OMG nichts außer dem vendor-reservierten Locator-Kind
(DDSI-RTPS 2.5 §9.3.2.4); Pfad-Auflösung, Wire-Format und Cleanup sind
vendor-spezifisch. Cyclone DDS und FastDDS haben keinen offiziellen
UDS-Transport. Die normativen Anforderungen stehen in den `##`-Sektionen
§2 bis §6; jedes `###`-Item darunter trägt Spec-Zitat, Repo-Anker, Test
und Status.

Implementation: `crates/transport-uds/src/lib.rs` — `SOCK_DGRAM` über
Filesystem-Sockets; `crates/transport-uds/src/abstract_dgram.rs` —
Linux-Abstract-Namespace, `#[cfg(target_os = "linux")]`;
`crates/transport-uds/src/stream.rs` — Stream-Modus über `SOCK_STREAM`
(§3.5), der UDS-Transport auf Windows; `crates/transport-uds/src/windows.rs`
— Basis-Verzeichnis mit Zugriffsliste und Ablehnung der Modes auf Windows
(§6.2).

Offene Punkte: `zerodds-uds-transport-1.0.open.md`.

## §1 Scope und Spec-Status

### §1.1 Vendor-reservierter Locator-Kind

**Spec:** DDSI-RTPS 2.5 §9.3.2.4 — `0x00000003` bis `0x01ffffff` sind für
vendor-spezifische Locator-Kinds reserviert. ZeroDDS belegt `0x01F0_0002` für
UDS. Das Port-Feld ist bedeutungslos und MUSS `0` sein; die 16
Adress-Bytes tragen die Endpoint-ID.

**Repo:** `crates/rtps/src/wire_types.rs` (`LocatorKind::Uds`,
`Locator::uds`); gebunden in `crates/transport-uds/src/lib.rs`
(`UdsTransport::local_locator`, Kind-Prüfung in `Transport::send`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::rtps_9_3_2_4_locator_kind_is_the_vendor_reserved_uds_value`
(prüft den Zahlwert `0x01F0_0002`, die Lage im Bereich `0x00000003` bis `0x01ffffff` und `port == 0`),
`crates/transport-uds/src/lib.rs::tests::uds_1_1_local_locator_reflects_bind_id`,
`crates/transport-uds/src/lib.rs::tests::uds_1_1_send_rejects_non_uds_locator`,
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

**Status:** n/a (informative) — Abgrenzung des Geltungsbereichs; die Anforderungen stehen in den Items ab §2.

## §2 Pfad-Auflösung

### §2.1 Filesystem-Modus (Default)

**Spec:** §2 — die 16-Byte-Locator-Adresse wird als
`<base_dir>/<lowercase-hex32>.sock` aufgelöst. Default-`base_dir` ist
`/tmp/zerodds/uds`. Der Hex-String ist die Adresse in 32 Kleinbuchstaben,
ohne Trenner.

**Repo:** `crates/transport-uds/src/lib.rs` (`socket_path`,
`DEFAULT_BASE_DIR`, `parse_hex_id`, `source_locator` als Rückrichtung).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_2_1_socket_path_is_hex_under_base_dir`
(prüft Präfix und `.sock`-Endung an einer Adresse mit gesetzten
Randbytes),
`crates/transport-uds/src/lib.rs::tests::uds_2_1_parse_hex_id_roundtrip`,
`crates/transport-uds/src/lib.rs::tests::uds_2_1_parse_hex_id_rejects_wrong_length`,
`crates/transport-uds/src/lib.rs::tests::uds_2_1_parse_hex_id_rejects_non_hex`.

**Status:** done

### §2.2 Basis-Verzeichnis: lazy, user-privat, TOCTOU-fest

**Spec:** §2 — das Basis-Verzeichnis wird beim Bind angelegt, wenn es
fehlt, und trägt dann den Mode `base_dir_mode` (Default `0700`, §5.1).
Ein bestehender Pfad, der ein Symlink oder kein Verzeichnis ist, MUSS
abgelehnt werden, statt ihn zu chmodden — sonst könnte ein
untergeschobener Symlink fremde Pfade auf diesen Mode setzen. Auf ein
bestehendes Verzeichnis wird kein Mode gesetzt. Setzt der Kernel den
Mode eines angelegten Verzeichnisses nicht wie verlangt (er verwirft das
Set-Group-ID-Bit ohne Fehler, wenn die Gruppe des Verzeichnisses keine
Gruppe des Prozesses ist, und macOS verwirft es für jeden Prozess in
einer Sandbox: `sandbox-exec`, Operation `file-write-setugid`), MUSS der
Bind scheitern und das Verzeichnis
wieder entfernen; sonst nähme der nächste Bind es als bestehend hin. Das
gilt für beide Transporte, auch für den Filesystem-Modus von
`UdsAbstractDgramTransport`.

**Repo:** `crates/transport-uds/src/lib.rs` (`ensure_base_dir` —
`symlink_metadata` statt `exists`, Ablehnung von Symlink und
Nicht-Verzeichnis, `create_dir_all` plus `set_permissions(base_dir_mode)`
mit Wiederholungs-Prüfung, danach Rücklesen des Modes: weicht er ab,
`PermissionDenied` mit verlangtem und gesetztem Mode und `remove_dir`;
ein bestehendes Verzeichnis bleibt unverändert);
`crates/transport-uds/src/abstract_dgram.rs`
(`UdsAbstractDgramTransport::bind` ruft im Filesystem-Modus dieselbe
`ensure_base_dir`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_2_2_ensure_base_dir_creates_dir_with_mode_0700`
(liest den Mode zurück und vergleicht ihn gegen `0o700`),
`crates/transport-uds/src/lib.rs::tests::uds_2_2_ensure_base_dir_rejects_symlink`,
`crates/transport-uds/src/lib.rs::tests::uds_2_2_ensure_base_dir_rejects_file_path`,
`crates/transport-uds/src/lib.rs::tests::uds_2_2_ensure_base_dir_creates_missing_dir`,
`crates/transport-uds/src/lib.rs::tests::uds_2_2_ensure_base_dir_leaves_the_mode_of_an_existing_dir`
(ein Verzeichnis mit `0755` behält `0755`, auch nach einem Bind mit
`base_dir_mode` `0o3770`),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_2_filesystem_mode_creates_base_dir_with_mode_0700`
(`0700` als Default, `0o3770` mit `base_dir_mode`),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_2_filesystem_mode_rejects_a_symlink_base_dir`
(Symlink und Datei werden mit `InvalidInput` abgelehnt, das Ziel des
Symlinks behält seinen Mode),
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_a_set_group_id_bit_the_kernel_drops_fails_the_bind`
(UID 61001 ohne Zusatzgruppen legt das Verzeichnis mit `base_dir_mode`
`3770` in einem Elternverzeichnis der Gruppe 61099 mit
Set-Group-ID-Bit an; der Kernel setzt `1770`, der Bind scheitert mit
`PermissionDenied` „uds base_dir_mode 3770: the kernel set 1770“, das
Verzeichnis ist danach entfernt).

**Status:** done

### §2.3 Abstract-Namespace-Modus (Linux)

**Spec:** §2 — auf Linux gibt es die Alternative ohne Filesystem-Inode:
der Socket-Name beginnt mit einem Null-Byte, gefolgt von
`<prefix>-<lowercase-hex32>`. Kein Cleanup nötig, keine
Inode-Permissions, kein Cross-Mount-Volume. Der Name passt in
`sun_path` (108 Byte, Byte 0 ist das Null-Präfix); ein längerer Name
wird beim Bind mit `InvalidInput` abgelehnt.

**Repo:** `crates/transport-uds/src/abstract_dgram.rs`
(`UdsAbstractDgramTransport`, `UdsAddress::Abstract`, `abstract_name`,
`build_abstract_sockaddr` über `socket2::SockAddr::unix` mit führendem
Null-Byte, `MAX_ABSTRACT_NAME = 107`, `UdsAddress::abstract_default`);
Modul deklariert in `crates/transport-uds/src/lib.rs` unter
`#[cfg(target_os = "linux")]`.

**Tests:** `crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_bind_does_not_create_file`
(`/proc/net/unix` führt den Namen als `@<name>` und keinen
Filesystem-Socket dieses Namens, unter dem Arbeitsverzeichnis und
unter `/tmp` liegt keine Datei, nach dem Drop ist der Eintrag weg),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_name_is_nul_prefix_dash_lowercase_hex32`
(der mit `getsockname` zurückgelesene Name ist byte-genau
`<prefix>-ab000000000000cd00000000000000ef`, Länge
`offsetof(sun_path) + 1 + Name` ohne Terminator),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_name_above_107_bytes_is_refused`
(107 Byte binden, 108 Byte ergeben `InvalidInput`),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_send_recv_roundtrip`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_preserves_message_boundaries`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_local_locator_reflects_bind_id`.
Das Modul steht hinter `#[cfg(target_os = "linux")]` und wird auf macOS
nicht übersetzt; gemessen auf dem Linux-Testhost.

**Status:** done

### §2.4 Filesystem-Modus im Abstract-Transport (A/B-Vergleich)

**Spec:** §2 — derselbe Transport MUSS auch Filesystem-Adressierung
können, damit Abstract- und Filesystem-Modus mit identischem Socket-Code
gegeneinander gemessen werden können. Der Unterschied ist eine
Config-Zeile. Der Filesystem-Modus adressiert wie §2.1
(`<base_dir>/<lowercase-hex32>.sock`, Default-`base_dir`
`/tmp/zerodds/uds`), sodass ein `UdsTransport` und dieser Modus unter
einem `base_dir` einander erreichen.

**Repo:** `crates/transport-uds/src/abstract_dgram.rs`
(`UdsAddress::Filesystem`, `UdsAddress::filesystem_default` mit
`DEFAULT_BASE_DIR`, `build_sockaddr` und `Drop` über
`crate::socket_path`, `decode_source` über `crate::source_locator`).

**Tests:** `crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_4_filesystem_bind_creates_socket_file`
(Socket-Datei entsteht beim Bind und verschwindet beim Drop),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_4_filesystem_send_recv_roundtrip`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_4_filesystem_mode_binds_the_path_of_2_1`
(der gebundene Pfad ist wörtlich
`<tmp>/c000000000000000000000000000000c.sock`, auch per `getsockname`;
`filesystem_default` ist `/tmp/zerodds/uds` mit `0700`),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_4_filesystem_mode_exchanges_datagrams_with_uds_transport`
(ein `UdsTransport` und der Filesystem-Modus unter einem `base_dir`, in
beide Richtungen, jeweils mit dem Locator des anderen als Absender).
Linux-gated wie §2.3.

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

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_3_1_preserves_message_boundaries_two_datagrams`
(zwei Sends unterschiedlicher Länge, zwei Receives mit genau diesen
Längen),
`crates/transport-uds/src/lib.rs::tests::uds_3_1_send_and_recv_roundtrip_same_process`,
`crates/transport-uds/tests/l1_cross_process.rs::uds_3_1_l1_datagram_crosses_process_boundary`
(dieselbe Zustellung über eine echte Prozessgrenze).

**Status:** done

### §3.2 Absender-Locator aus der Socket-Adresse

**Spec:** §3 — der Empfänger MUSS aus der Absender-Adresse den
Locator zurückgewinnen, damit RTPS antworten kann. Auflösbar ist nur ein
Name, den der Transport selbst vergibt: im Filesystem-Modus
`<base_dir>/<lowercase-hex32>.sock` unter dem eigenen `base_dir`, im
Abstract-Modus `<prefix>-<lowercase-hex32>` mit dem eigenen Präfix —
eine Antwort geht an genau diesen Namen. Ist die Adresse unbenannt
(anonymer Client ohne Bind) oder nicht auflösbar, MUSS
`Locator::INVALID` zurückgegeben werden — nicht geraten.

**Repo:** `crates/transport-uds/src/lib.rs` (`source_locator` —
Elternverzeichnis gleich `base_dir`, Endung `.sock`, `parse_hex_id` nur
mit Kleinbuchstaben; `Locator::INVALID`);
`crates/transport-uds/src/abstract_dgram.rs` (`decode_source`,
`decode_abstract_name` über `SockAddr::as_abstract_namespace`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_3_1_send_and_recv_roundtrip_same_process`
(prüft `got.source` gegen den Locator des Senders),
`crates/transport-uds/src/lib.rs::tests::uds_3_2_unbound_sender_yields_locator_invalid`
(ein Socket ohne Bind sendet),
`crates/transport-uds/src/lib.rs::tests::uds_3_2_sender_outside_base_dir_yields_locator_invalid`,
`crates/transport-uds/src/lib.rs::tests::uds_3_2_a_name_socket_path_does_not_give_yields_locator_invalid`
(kein Hex, Hex in Großbuchstaben, Endung `.seqp`, Unterverzeichnis —
alle `INVALID`; der Kleinbuchstaben-Name als Gegenprobe löst auf),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_3_2_abstract_unbound_sender_yields_locator_invalid`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_3_2_abstract_foreign_prefix_yields_locator_invalid`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_3_2_filesystem_mode_sender_outside_base_dir_yields_locator_invalid`,
`crates/transport-uds/tests/l1_cross_process.rs::uds_3_1_l1_datagram_crosses_process_boundary`.

**Status:** done

### §3.3 Datagram-Obergrenze

**Spec:** §3 — `max_datagram` (Default 65 536) begrenzt in beiden
Transporten, was gesendet und was empfangen wird. Eine größere Nutzlast
MUSS beim Senden mit `PayloadTooLarge` abgelehnt werden; ein größeres
Datagramm eines Peers mit höherer Grenze MUSS beim Empfang als
`RecvError::BufferTooSmall` gemeldet werden. Eine stille Kürzung ist
nicht zulässig, weil sie auf RTPS-Ebene von einer kurzen Nachricht nicht
zu unterscheiden wäre. Die DCPS-Runtime zählt einen solchen Fehler ihres
User-Transports und meldet ihn als Ereignis, statt ihn zu verwerfen.

**Repo:** `crates/transport-uds/src/lib.rs` (`DEFAULT_MAX_DATAGRAM`,
Längenprüfung in `Transport::send`, `recv_datagram` — Puffer
`max_datagram + 1`, auf Linux `MSG_TRUNC` für die wahre Länge, ein
Datagramm über `max_datagram` ergibt `BufferTooSmall` mit beiden
Größen); `crates/transport-uds/src/abstract_dgram.rs`
(`AbstractDgramConfig::max_datagram`, Default `DEFAULT_MAX_DATAGRAM`,
`recv` über `recv_datagram`); `crates/dcps/src/runtime.rs`
(`recv_user_data_loop` zählt `BufferTooSmall` in
`USER_DATAGRAMS_ABOVE_LIMIT`, abzufragen mit
`DcpsRuntime::user_datagrams_above_limit`, und meldet das Ereignis
`transport.user_recv.datagram_above_limit` mit Größe und Grenze).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_3_3_send_rejects_oversize_payload`
(Nutzlast über dem konfigurierten Limit wird mit `PayloadTooLarge`
abgelehnt),
`crates/transport-uds/src/lib.rs::tests::uds_3_3_recv_of_a_datagram_above_max_datagram_is_an_error_not_a_short_message`
(Empfänger 1024, Sender 8192, 4000 Byte: `BufferTooSmall { datagram_size:
4000, buffer_size: 1024 }` auf Linux; das nächste Datagramm kommt
ganz an),
`crates/transport-uds/src/lib.rs::tests::uds_3_3_oversized_for_the_kernel_fails_loudly_and_never_truncates`
(1 KiB kommt unverkürzt an; bei `max_datagram` gilt entweder
unverkürzte Zustellung oder ein Fehler, der EMSGSIZE benennt — und dann
liegt nichts Teilweises in der Queue),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_3_3_abstract_recv_above_max_datagram_is_an_error`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_3_3_abstract_default_max_datagram_is_65536`,
`crates/dcps/src/runtime.rs::tests::uds_3_3_a_datagram_above_the_transport_limit_is_counted_by_the_user_receive_loop`
(ein Transport meldet einmal `BufferTooSmall`: der Zähler steigt, die
Schleife empfängt weiter, und die Senke der Runtime erhält genau ein
Ereignis `transport.user_recv.datagram_above_limit` mit `size` 4000 und
`limit` 1024).

**Status:** done — auf macOS (ohne `MSG_TRUNC`) meldet `datagram_size`
`max_datagram + 1` statt der wahren Länge; dieser Zweig lief grün auf
einem Mac (macOS 26.2, arm64) und auf Runner 8 (Job 238162, §6.1).

### §3.4 Kernel-Obergrenze gegen konfigurierte Obergrenze

**Spec:** §3 — die tatsächliche Obergrenze pro Message setzt der Kernel:
Linux über `wmem_max` (Default 212 992), macOS über
`net.local.dgram.maxdgram` (Default 2048). Der Transport MUSS die
konfigurierte Grenze gegen die des Betriebssystems abgleichen, sie beim
Bind sichtbar machen oder die Socket-Puffer entsprechend setzen — sonst
scheitert ein Sample zwischen beiden Grenzen erst zur Laufzeit. Der Bind
setzt die Puffer und liest zurück, was der Kernel gewährt hat; reicht
das für `max_datagram` nicht, scheitert der Bind mit `InvalidInput`,
das die Grenze, die gewährte Größe und die begrenzende Einstellung
nennt. Linux weist ein Datagramm über `SO_SNDBUF − 32` ab und verdoppelt
eine auf `wmem_max` begrenzte Anfrage, die größte mögliche Grenze ist
dort `2 · wmem_max − 32`; macOS braucht `SO_SNDBUF` und `SO_RCVBUF` von
mindestens `max_datagram`.

**Repo:** `crates/transport-uds/src/lib.rs` (`size_buffers`: Sendepuffer
auf das Doppelte, Empfangspuffer auf das Achtfache von `max_datagram`,
bei einer abgewiesenen Anfrage auf das Nötige, danach Rücklesen und
Prüfung gegen `max_datagram + SEND_BUFFER_OVERHEAD`; aufgerufen in
`UdsTransport::bind` und `UdsAbstractDgramTransport::bind`, ein Fehler
entfernt die Socket-Datei wieder).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_3_4_a_datagram_near_the_configured_limit_arrives_whole`
(vier Datagramme zu `max_datagram` − 512 Byte kommen ganz an),
`crates/transport-uds/src/lib.rs::tests::uds_3_4_bind_refuses_a_max_datagram_the_kernel_does_not_grant`
(1 GiB: `InvalidInput` mit `max_datagram 1073741824` und
`net.core.wmem_max`, keine Socket-Datei bleibt),
`crates/transport-uds/src/lib.rs::tests::uds_3_4_bind_accepts_exactly_the_largest_max_datagram_the_kernel_grants`
(Linux: `2 · wmem_max − 32` bindet, ein Byte mehr nicht; die Meldung
nennt `SO_SNDBUF 2 · wmem_max`),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_3_4_abstract_bind_refuses_a_max_datagram_the_kernel_does_not_grant`
(beide Modi),
`crates/transport-uds/src/lib.rs::tests::uds_3_3_oversized_for_the_kernel_fails_loudly_and_never_truncates`.

**Status:** done — gemessen auf Linux; der macOS-Zweig (Prüfung auch
von `SO_RCVBUF`, `kern.ipc.maxsockbuf` in der Meldung) lief grün auf
einem Mac (macOS 26.2, arm64) und auf Runner 8 (Job 238162, §6.1).

### §3.5 Stream-Modus: Frames über `SOCK_STREAM`

**Spec:** §3 — Neben dem Datagramm-Modus gibt es den Stream-Modus über
`AF_UNIX`-Sockets vom Typ `SOCK_STREAM`; er ist der einzige Modus auf
Windows (§6.2). Jeder Transport lauscht an `<base_dir>/<hex32>.sock`
(§2.1). Eine Verbindung beginnt mit dem Hello: die vier Oktette `ZUDS`
und die 16-Byte-ID des Senders. Die ID ist der Absender-Locator jeder
Nachricht der Verbindung (`Locator::uds(id)`); der verbindende Socket ist
nicht gebunden, seine Adresse nennt den Sender nicht. Danach trägt die
Verbindung jede RTPS-Nachricht als Frame: die Länge als `u32` little
endian, dann die Oktette. Ein Frame über `max_datagram` wird beim
Empfang als `BufferTooSmall` mit seiner Länge gemeldet und übersprungen;
die folgenden Frames der Verbindung kommen an. Eine Verbindung, die
binnen 5 s nach dem Annehmen nicht das ganze Hello gesendet hat, wird
geschlossen und gezählt; die 5 s gelten für alle Lesevorgänge des Hellos
zusammen. Eine Verbindung, die in einem Frame endet, wird als abgeschnittener Frame
gezählt, der Teil-Frame wird nicht weitergegeben. Eine Verbindung, deren
Peer geschrieben und geschlossen hat, bevor sie angenommen wurde, wird
wie jede andere gelesen: ihre Frames kommen an. Der Sender hält eine
Verbindung je Ziel und baut sie beim ersten Senden auf; ein Senden
wartet höchstens `send_timeout` auf Platz und scheitert dann mit „uds:
kernel send buffer full (EAGAIN)“ (§4.3). Ein Frame, der nur zum Teil
hinausging, schließt die Verbindung, damit der Peer den Rest nicht als
Beginn des nächsten liest; eine Verbindung, die der Peer geschlossen hat
(Neustart), ersetzt dasselbe Senden einmal durch eine neue. Die
Empfangswarteschlange fasst 128 Frames; ist sie voll, lesen die
Verbindungen nicht weiter, die Kernel-Puffer füllen sich, und das Senden
des Peers scheitert nach seinem `send_timeout` wie ein Datagramm an eine
volle Warteschlange (§4.3); nichts wird verworfen. Pfad, Basis-Verzeichnis
(§2.2), Mode der Socket-Datei (§5.1) und Fehlerklassen (§4.3) sind die
des Datagramm-Modus; die Kernel-Obergrenze von §3.4 gilt nicht, ein
Stream kennt keine Obergrenze je Nachricht. Im Datagramm-Modus belegt der
gebundene Absenderpfad die ID; im Stream-Modus gibt der Sender sie im
Hello an, und wer an die Socket-Datei schreiben darf, kann jede ID
angeben. Die Runtime wählt den Modus mit `transport.uds.mode`
(`datagram` oder `stream`; Default `datagram`, auf Windows `stream`).

**Repo:** `crates/transport-uds/src/stream.rs` (`UdsStreamTransport`,
`STREAM_MAGIC`, `HELLO_TIMEOUT`, `STREAM_QUEUE_FRAMES`; Accept-Thread,
ein Lese-Thread je Verbindung, Empfangswarteschlange mit
Gegendruck; `read_connection` wartet vor jedem Lesen des Hellos mit
`poll` (auf Windows `WSAPoll`) höchstens die verbleibende Zeit bis 5 s
nach dem Annehmen und lehnt ab, wenn sie abgelaufen ist; `accept` setzt
auf einer angenommenen Verbindung keine Socket-Option (auf
Apple-Systemen `accept_raw` und `set_cloexec`: macOS lehnt jede
Socket-Option mit `EINVAL` ab, wenn der Peer der Verbindung geschlossen
hat, und `Socket::accept` setzt dort `SO_NOSIGPIPE`); der Accept-Thread
wartet mit `poll` (auf
Windows `WSAPoll`) je höchstens `ACCEPT_POLL` (50 ms) auf eine
Verbindung und prüft dazwischen, ob der Transport geschlossen ist, sodass
das Schließen nicht über den Socket-Pfad geht, den inzwischen ein
anderer Transport gebunden haben kann; Zähler `accepted_connections`, `truncated_frames`,
`refused_hellos`, `connection_errors`; `recv_batch`, `close`,
`forget_destination`); `crates/transport-uds/src/lib.rs`
(`classify_send_error`: `TimedOut` wie `WouldBlock`, eine vom Peer
geschlossene Verbindung als „uds: the peer closed the connection
(EPIPE)“); `crates/dcps/src/runtime.rs` (`UdsMode`, `DEFAULT_UDS_MODE`,
`UdsUserConfig::mode`, Zeile `transport.uds.mode` in `SETTINGS`,
`uds_config_from`, `select_user_transport` bindet im Modus `stream` den
Stream-Transport, das Ereignis `transport.uds.bind_failed` nennt den
Modus); `man/man5/zerodds.yaml.5`;
`packaging/linux/configs/zerodds.yaml.example`;
`docs/doc-map/config.md`; `tests/integration/src/kernel.rs`
(`unix_stream_sockets`); `ci/integration_scenarios.py` (Szenario
`dcps-uds-stream`).

**Tests:** `crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_frames_keep_message_boundaries`
(Nachrichten von 0, 1 und 8192 Oktetten kommen ganz und in Reihenfolge
an),
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_source_locator_is_the_senders_id_and_reaches_it`,
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_a_frame_above_max_datagram_is_reported_and_the_next_arrives`
(ein roher Client sendet 8193 Oktette an einen Transport mit 8192:
`BufferTooSmall { 8193, 8192 }`, der nächste Frame kommt an),
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_send_refuses_a_payload_above_max_datagram`,
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_a_connection_without_the_hello_or_ending_inside_a_frame_is_counted`
(dazu: kein Verbindungsfehler; auf macOS ließ der Client, der vor dem
Annehmen schloss, das Annehmen mit `EINVAL` scheitern, und der Test
scheiterte nach 5 s, Job 238162),
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_a_peer_that_closed_before_the_accept_is_read_whole`
(ein roher Client verbindet sich, schreibt das Hello, einen ganzen Frame
und den Anfang eines zweiten und schließt; danach wird die Verbindung
angenommen und gelesen: der ganze Frame kommt mit der Id des Clients als
Quelle an, der abgeschnittene wird gezählt, kein Verbindungsfehler; auf
macOS scheitert der Test mit dem früheren `accept` (`EINVAL`) und mit
dem früheren `SO_RCVTIMEO` für das Hello (abgelehnt)),
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_a_connection_without_a_hello_within_5_s_is_refused`
(eine Verbindung ohne ein Oktett und eine, die das Hello in Stücken im
Abstand von 1,5 s sendet, den Rest nach 6 s: nach 4,5 s ist keine
abgelehnt, nach 5 s sind beide abgelehnt und gezählt, der Frame nach dem
späten Hello kommt nicht an; mit dem vorigen Timeout je Lesevorgang
schlug der Test fehl),
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_reconnects_after_the_peer_restarts`,
`crates/transport-uds/src/stream.rs::tests::uds_3_5_stream_recv_batch_takes_the_queued_messages`,
`crates/transport-uds/src/stream.rs::tests::uds_4_1_stream_bind_replaces_a_leftover_file_and_drop_removes_it`,
`crates/transport-uds/src/stream.rs::tests::uds_4_1_stream_a_transport_whose_path_another_took_is_dropped`
(zwei Transporte binden dieselbe ID im selben Verzeichnis; der Drop des
ersten kehrt binnen 1 s zurück, der zweite nimmt keine Verbindung an;
mit dem vorigen Aufwecken über den Pfad blockierte der Drop),
`crates/transport-uds/src/stream.rs::tests::uds_4_4_stream_recv_times_out_when_idle_and_close_ends_it`,
`crates/transport-uds/src/stream.rs::tests::uds_4_3_stream_send_names_a_missing_socket_and_a_refusal`,
`crates/transport-uds/src/stream.rs::tests::uds_4_3_stream_a_peer_that_does_not_read_fails_the_send_with_the_full_buffer_error`
(nach mindestens 128 angenommenen Frames scheitert ein Senden mit „uds:
kernel send buffer full (EAGAIN)“ nach weniger als 1 s bei
`send_timeout` 50 ms; liest der Peer, geht das nächste Senden hinaus),
`crates/transport-uds/src/stream.rs::tests::uds_5_1_stream_socket_mode_is_applied_to_the_socket_file`,
`crates/transport-uds/tests/l1_cross_process.rs::uds_3_5_l1_stream_frames_cross_the_process_boundary`,
`crates/dcps/tests/runtime_config_layering.rs::uds_5_1_the_uds_directory_modes_and_datagram_limit_are_settings`
(`transport.uds.mode` aus Datei und Umgebung),
`crates/dcps/tests/runtime_config_layering.rs::uds_5_1_a_bad_uds_value_is_refused_instead_of_falling_back`
(`seqpacket`, `Stream`),
`crates/dcps/tests/contract_transport_uds.rs::uds_3_5_a_refused_stream_bind_is_reported_with_its_mode`
(ein Teilnehmer im Modus `stream`, dessen `base_dir` eine Datei ist: der
Start scheitert mit `TransportError` „user unicast bind (UDS)“, genau
ein Ereignis `transport.uds.bind_failed` der Stufe `Error` mit `mode`
`stream`, `base_dir` und „uds base_dir exists but is not a directory“;
auf Linux und im Windows-Job `platform-windows-dcps-uds`),
`tests/integration/tests/dcps_transports.rs::dcps_over_uds_a_refused_bind_is_reported_with_its_cause`
(`mode` `datagram` im Ereignis),
`tests/integration/tests/dcps_transports.rs::dcps_over_uds_in_stream_mode`
(Writer- und Reader-Prozess über die DCPS-API mit
`transport.uds.mode` `stream`: alle Samples kommen an; jeder Prozess
lauscht mit einem Stream-Socket im konfigurierten Verzeichnis und hält
eine angenommene Verbindung des anderen; kein UDP-Socket auf den
Ports des Nutzverkehrs).

**Status:** done — gemessen auf dem Linux-Testhost: `cargo nextest run --profile
merge -p zerodds-transport-uds --all-targets` 91 Tests grün, die 15
Stream-Tests von `src/stream.rs` in 10 Läufen hintereinander grün; das Szenario
`dcps_over_uds_in_stream_mode` grün. Auf einem Mac (macOS 26.2, arm64)
die 15 Stream-Tests in 10 Läufen hintereinander grün, in der Sandbox von
`ci/macos-loopback.sb` (§6.1); Windows §6.2.

### §4.1 Filesystem-Modus: Cleanup beim Drop, Übernahme beim Bind

**Spec:** §4 — der Bind legt die Socket-Datei an, der Drop entfernt sie
best-effort (ein abgehängtes Container-Volume darf den Drop nicht
scheitern lassen). Findet ein Bind eine Restdatei eines abgestürzten
Vorgängers, MUSS er sie entfernen und neu binden: `UnixDatagram::bind`
kann nicht über eine bestehende Datei binden und meldet sonst
`EADDRINUSE`.

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsTransport::bind` mit
`remove_file` bei bestehendem Pfad, `Drop for UdsTransport`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_4_1_bind_creates_socket_file`
(Datei existiert nach dem Bind und ist nach dem Drop weg),
`crates/transport-uds/src/lib.rs::tests::uds_4_1_bind_reuses_path_after_stale_leftover`
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

**Tests:** `crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_bind_does_not_create_file`
(der Name steht in `/proc/net/unix` als `@<name>`, keine Datei entsteht,
nach dem Drop ist der Eintrag weg),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_4_2_abstract_name_is_free_after_drop`
(ein zweiter Bind desselben Namens scheitert mit `AddrInUse`, nach dem
Drop des ersten gelingt er ohne Aufräumen),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_4_filesystem_bind_creates_socket_file`
(die Gegenprobe: im Filesystem-Zweig entsteht und verschwindet die
Datei). Linux-gated wie §2.3.

**Status:** done

### §4.3 Differenzierte Fehlerklassifikation beim Senden

**Spec:** §4 — ein Sendefehler MUSS seine Ursache benennen: fehlender
Ziel-Socket, verweigerte Permission, voller Kernel-Puffer, abgelehnte
Verbindung und überschrittene Message-Grenze sind unterschiedliche
Betriebszustände. Ein Sammel-`Io` macht sie ununterscheidbar. Das gilt
für beide Transporte; im Abstract-Namespace heißt `ECONNREFUSED`, dass
kein Socket den Namen hält.

Ein voller Kernel-Puffer MUSS als Sendefehler ankommen und darf den
sendenden Thread nicht festhalten, bis der Peer liest. Linux reiht für
einen Unix-Datagram-Socket Datagramme ein, bis seine Queue
`net.unix.max_dgram_qlen + 1` hält (10 in einem neuen Netzwerk-Namespace,
512 im Namespace des Hosts auf dem Linux-Testhost und im CI-Container)
oder der Sendepuffer des Senders voll ist: jedes eingereihte Datagramm
belastet den `SO_SNDBUF` des Senders mit seiner Puffergröße, bis der
Peer es liest (bei `SO_SNDBUF` 212 992 sind es 278 kurze Datagramme).
Ist eines von beiden voll, wartet ein blockierendes Senden an einen
Peer, der nicht liest, ohne Zeitgrenze. Ein Senden wartet deshalb
höchstens `send_timeout` (Default 100 ms) auf Platz in der Queue des
Peers oder im eigenen Sendepuffer und scheitert dann mit
„uds: kernel send buffer full (EAGAIN)“. Ein Peer, der liest, leert
seine Queue innerhalb dieser Zeit, und das Datagramm geht hinaus; ein
Peer, der nicht mehr liest, kostet jedes Senden an ihn höchstens
`send_timeout`. `send_timeout` null wartet nicht. macOS reiht ein
Datagramm bei vollem Empfangspuffer nicht ein und meldet sofort
`ENOBUFS`.

**Repo:** `crates/transport-uds/src/lib.rs` (`classify_send_error` —
`NotFound`, `PermissionDenied`, `WouldBlock`, `ConnectionRefused`,
`EMSGSIZE` und `ENOBUFS` über `raw_os_error`; `UdsConfig::send_timeout`,
`DEFAULT_SEND_TIMEOUT`, `set_send_timeout` setzt beim Bind
`SO_SNDTIMEO`, `send_flags` gibt bei `send_timeout` null `MSG_DONTWAIT`
und auf Linux `MSG_NOSIGNAL`, `Transport::send` sendet mit
`send_to_with_flags`); `crates/transport-uds/src/abstract_dgram.rs`
(`AbstractDgramConfig::send_timeout`, derselbe Bind- und Sendeweg;
`classify_send_error`: `ECONNREFUSED` im Abstract-Modus mit eigener
Meldung, sonst die des Crate-Wurzelmoduls); `crates/dcps/src/runtime.rs`
(`select_user_transport` bindet den UDS-User-Transport mit
`DEFAULT_SEND_TIMEOUT`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_4_3_classify_send_error_maps_kinds`
(alle benannten `io::ErrorKind`-Werte auf ihre Meldung),
`crates/transport-uds/src/lib.rs::tests::uds_4_3_classify_send_error_names_emsgsize_and_enobufs`
(rohe Fehlernummern `EMSGSIZE`, `ENOBUFS`, `EAGAIN`, `EACCES`, `ENOENT`,
`ECONNREFUSED` auf ihre Meldung),
`crates/transport-uds/src/lib.rs::tests::uds_4_3_send_to_missing_peer_names_the_missing_socket`,
`crates/transport-uds/src/lib.rs::tests::uds_4_3_send_to_a_leftover_socket_file_names_the_refusal`
(eine Socket-Datei ohne Socket ergibt die `ECONNREFUSED`-Meldung),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_4_3_abstract_send_to_missing_peer_names_the_cause`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_4_3_filesystem_mode_send_errors_name_the_cause`,
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_without_socket_mode_the_other_uid_is_refused_with_permission_denied`
(die Permission-Meldung über eine echte UID-Grenze, §5.1),
`crates/transport-uds/src/lib.rs::tests::uds_4_3_a_peer_that_does_not_read_fails_the_send_with_the_full_buffer_error`
(ein Empfänger, der nie liest: auf Linux gehen Datagramme hinaus, bis
die Queue des Peers oder der Sendepuffer des Senders voll ist, höchstens
`max_dgram_qlen + 1`; das nächste Senden wartet mindestens
`send_timeout` (50 ms) und scheitert mit der `EAGAIN`-Meldung; ein
Datagramm eines zweiten Senders wird ebenfalls abgewiesen, wenn die
Queue voll war, und geht hinein, wenn der Sendepuffer des ersten Senders
voll war; die eingereihten Datagramme kommen ganz an, vom gescheiterten
nichts; auf macOS erwartet derselbe Test die `ENOBUFS`-Meldung),
`crates/transport-uds/src/lib.rs::tests::uds_4_3_with_send_timeout_zero_a_full_queue_fails_the_send_at_once`
(`send_timeout` null: das Senden, das keinen Platz findet, scheitert ohne
Warten mit `EAGAIN`, ebenso das nächste),
`crates/transport-uds/src/lib.rs::tests::uds_4_3_a_send_to_a_full_queue_goes_out_when_the_peer_reads_within_send_timeout`
(Queue oder Sendepuffer mit `MSG_DONTWAIT` bis `EAGAIN` gefüllt, der
Peer beginnt nach 200 ms zu lesen: das Senden wartet und geht hinaus,
alle Datagramme kommen in Reihenfolge an),
`crates/transport-uds/src/lib.rs::tests::uds_4_3_the_default_send_timeout_is_100_ms`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_4_3_a_full_peer_queue_fails_the_send_after_send_timeout_in_both_modes`
(Abstract- und Filesystem-Modus, gefüllt bis zum ersten `EAGAIN`).

**Status:** done — die Zustände des vollen Kernel-Puffers gemessen auf
dem Linux-Testhost, als root und als `nobody`, mit
`net.unix.max_dgram_qlen` 10 (die Queue zuerst voll) und 512 (der
Sendepuffer zuerst voll, nach 278 Datagrammen, wie im CI-Container: dort
scheiterten die früheren Tests, die `max_dgram_qlen + 1` erwarteten, Job
238176). Der macOS-Zweig (`ENOBUFS` bei vollem Empfangspuffer) lief grün
auf einem Mac (macOS 26.2, arm64) und auf Runner 8 (Job 238162, §6.1).

### §4.4 Leerlauf-Verhalten von `recv`

**Spec:** §4 — mit gesetztem `recv_timeout` MUSS ein leerer Socket nach
Ablauf `RecvError::Timeout` liefern, nicht einen Io-Fehler. Ohne Timeout
blockiert `recv`.

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsConfig::recv_timeout`,
`set_read_timeout` im Bind, `WouldBlock`/`TimedOut`-Zweig in
`Transport::recv`).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_4_4_recv_times_out_when_idle`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_4_4_an_abstract_socket_recv_times_out_when_idle`
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

Rechte-Modell: Wer an einen Socket sendet, braucht Schreibrecht auf die
Socket-Datei und Suchrecht auf das Verzeichnis; wer bindet, braucht
Schreib- und Suchrecht auf das Verzeichnis. Laufen beide Seiten unter
derselben UID, genügt `base_dir`. Unter verschiedenen UIDs brauchen sie
eine gemeinsame Gruppe, Socket-Dateien mit Gruppen-Schreibrecht (`0660`)
und ein Verzeichnis dieser Gruppe mit dem Mode `3770`. Das
Set-Group-ID-Bit gibt neuen Socket-Dateien die Gruppe des Verzeichnisses;
ohne es trägt eine Socket-Datei die primäre Gruppe des Binders, und
Container mit verschiedenen primären Gruppen, die die gemeinsame Gruppe
nur als Zusatzgruppe haben, weisen einander ab. Das Sticky-Bit lässt nur
den Eigentümer einer Socket-Datei (und den des Verzeichnisses) sie
entfernen oder umbenennen; ohne es kann jede UID der Gruppe die
Socket-Datei einer anderen löschen und an ihren Pfad einen eigenen
Socket oder einen Symlink legen. Die Gruppe bekommt das Verzeichnis,
wenn der Betreiber es vorbereitet (`chgrp`, `chmod 3770`) oder wenn die
Runtime es in einem Elternverzeichnis der Gruppe mit Set-Group-ID-Bit
anlegt: ein neues Verzeichnis übernimmt dessen Gruppe. Der Transport
setzt den Mode der Socket-Datei direkt nach dem Bind (`socket_mode`;
ohne ihn gilt die umask, bei `022` also `0755`, und eine andere UID wird
mit „permission denied on destination“ abgewiesen), und zwar nur auf den
Socket, den er gebunden hat, nie durch einen Symlink hindurch; liegt am
Pfad etwas anderes als ein Socket des eigenen effektiven UID, scheitert
der Bind. Den Mode eines Verzeichnisses, das er anlegt, setzt er auf
`base_dir_mode` (Default `0700`, der jede andere UID ausschließt) und
prüft ihn (§2.2). Die Runtime liest beides mit dem Verzeichnis und
der Datagramm-Grenze aus `transport.uds.base_dir`, `transport.uds.socket_mode`
(`umask` oder oktal), `transport.uds.base_dir_mode` (oktal) und
`transport.uds.max_datagram`. Unter User-Namespaces gilt die Regel für
die IDs, die das Volume auf dem Host sieht; unter SELinux braucht das
Volume ein Label, das beide Container nutzen dürfen (`:z` bei Docker und
Podman) — beides ist Sache der Container-Runtime. Discovery (SPDP/SEDP)
läuft weiter über UDPv4: ohne Multicast brauchen die Container
Unicast-Peers (`discovery.peers`).

**Repo:** `crates/transport-uds/src/lib.rs` (`UdsConfig::base_dir`,
`UdsConfig::socket_mode`, `UdsConfig::base_dir_mode`, `check_modes`,
`apply_socket_mode` — auf Linux `O_PATH | O_NOFOLLOW`, Prüfung des
Deskriptors mit `check_own_socket` (Socket, Eigentümer der effektive
UID), `chmod` über `/proc/self/fd`; sonst `lstat`-Prüfung und `fchmodat`
mit `AT_SYMLINK_NOFOLLOW` —, `ensure_base_dir`; Drop-Cleanup
best-effort, damit ein abgehängtes Volume den Drop nicht scheitern
lässt);
`crates/transport-uds/src/abstract_dgram.rs` (`UdsAddress::Filesystem`
mit `socket_mode` und `base_dir_mode`); `crates/dcps/src/runtime.rs`
(`UdsUserConfig`, `RuntimeConfig::uds`, die Zeilen `transport.uds.*` in
`SETTINGS`, `uds_config_from`, `parse_octal_mode`,
`parse_uds_socket_mode`, Prüfung in `valid_runtime_value`,
`select_user_transport` bindet damit und meldet einen Bind-Fehler als
Ereignis `transport.uds.bind_failed`); `man/man5/zerodds.yaml.5`;
`packaging/linux/configs/zerodds.yaml.example`;
`docs/doc-map/config.md`.

**Tests:** In `crates/transport-uds/tests/l2_cross_uid.rs` laufen die
Prozesse unter den UIDs 61001 und 61002 mit je eigener primärer Gruppe
(61001, 61002) und der Gruppe 61042 als Zusatzgruppe; das Verzeichnis
legt der erste Binder in einem Elternverzeichnis der Gruppe 61042 mit
Set-Group-ID-Bit an.
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_two_uids_with_a_shared_group_exchange_datagrams_in_both_directions`
(`socket_mode` `0660`, `base_dir_mode` `3770`: Datagramm hin und
zurück, Absender-Locator je der des anderen, Socket-Dateien `0660` mit
Eigentümer der jeweiligen UID und der Gruppe 61042, die keiner der
beiden Prozesse als primäre Gruppe hat; Verzeichnis `3770`),
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_without_the_set_group_id_bit_a_socket_file_keeps_the_binders_group_and_is_refused`
(Verzeichnis `0770` der Gruppe 61042: die Socket-Datei trägt die
primäre Gruppe 61001 des Binders, die zweite UID bindet, ihr Senden wird
mit „uds: permission denied on destination“ abgewiesen),
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_under_3770_the_other_uid_cannot_remove_a_socket_file_it_can_remove_under_2770`
(unter `3770` scheitert das Löschen der Socket-Datei von UID 61001
durch UID 61002 mit `PermissionDenied` und die Datei bleibt; unter
`2770` gelingt es),
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_a_set_group_id_bit_the_kernel_drops_fails_the_bind`
(§2.2),
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_without_socket_mode_the_other_uid_is_refused_with_permission_denied`
(Socket-Datei `0755` aus der umask, die zweite UID erhält „uds:
permission denied on destination“),
`crates/transport-uds/tests/l2_cross_uid.rs::uds_5_1_a_base_dir_created_with_the_default_mode_shuts_the_other_uid_out`
(Verzeichnis `0700`, der Bind der zweiten UID scheitert mit
`PermissionDenied`),
`crates/transport-uds/src/lib.rs::tests::uds_5_1_apply_socket_mode_refuses_a_symlink_and_a_file_that_is_no_socket`
(ein Symlink auf eine Datei mit `0600` und eine Datei, die kein Socket
ist, werden mit `InvalidInput` abgelehnt, das Ziel des Symlinks behält
`0600`; ein eigener Socket erhält den Mode),
`crates/transport-uds/src/lib.rs::tests::uds_5_1_socket_mode_is_applied_to_the_socket_file`,
`crates/transport-uds/src/lib.rs::tests::uds_5_1_base_dir_mode_is_applied_to_a_created_dir`,
`crates/transport-uds/src/lib.rs::tests::uds_5_1_a_mode_outside_the_permission_bits_is_refused`,
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_5_1_filesystem_mode_applies_socket_mode_and_base_dir_mode`,
`crates/dcps/tests/runtime_config_layering.rs::uds_5_1_the_uds_directory_modes_and_datagram_limit_are_settings`
(Default, Datei und Umgebung),
`crates/dcps/tests/runtime_config_layering.rs::uds_5_1_a_bad_uds_value_is_refused_instead_of_falling_back`,
`crates/dcps/tests/contract_transport_uds.rs::uds_5_1_the_runtimes_uds_settings_default_to_the_transports_defaults`,
`tests/integration/tests/dcps_transports.rs::dcps_over_uds_in_a_configured_base_dir`
(Writer- und Reader-Prozess über die DCPS-API mit
`transport.uds.base_dir`, `socket_mode` `0660` und `base_dir_mode`
`3770`: alle Samples kommen an, beide Sockets liegen im konfigurierten
Verzeichnis mit `0660`, das Verzeichnis trägt `3770`),
`tests/integration/tests/dcps_transports.rs::dcps_over_uds_a_refused_bind_is_reported_with_its_cause`
(ein Teilnehmer mit UDS-User-Transport und `transport.uds.max_datagram`
1 GiB: der Start scheitert mit `TransportError` „user unicast bind
(UDS)“, die Senke erhält genau ein Ereignis `transport.uds.bind_failed`
der Stufe `Error` mit `base_dir`, `mode` `datagram` und der Meldung des
Transports „uds max_datagram 1073741824 exceeds what the kernel grants …
(capped by net.core.wmem_max)“),
`crates/transport-uds/tests/l1_cross_process.rs::uds_3_1_l1_datagram_crosses_process_boundary`.

**Status:** partial — Rechte-Modell, Schlüssel und Runtime-Anbindung
sind gebaut und auf dem Linux-Testhost über eine echte UID-Grenze
gemessen (als root, der die beiden Prozesse unter anderen UIDs
startet), ebenso das Volume unter Docker und unter Podman mit je einem
User-Namespace pro Container (§5.2). Nicht gemessen: das SELinux-Label;
es braucht einen Host mit SELinux im Modus enforcing, auf dem die
Container-Tests von §5.2 laufen (ihre Mounts tragen `:z`).

### §5.2 Nachweis über eine echte Container-Grenze

**Spec:** §5 — dass das Muster aus §5.1 unter einer Container-Runtime
trägt, MUSS ein Test mit zwei Containern und einem gemounteten Volume
belegen: unterschiedliche UID-Mappings auf beiden Seiten, Socket-Datei
auf dem Volume, Datagram in beide Richtungen.

**Repo:** `tools/isolation-smoke/src/main.rs` (Rollen `echo` — sendet
jedes Datagramm an den Absender-Locator zurück — und `ping` — sendet
`--count` Datagramme einzeln und prüft jedes Echo auf Inhalt und
Absender; `--socket-mode`, `--hold-ms`);
`tools/isolation-smoke/tests/container_volume.rs` (startet die beiden
Rollen in zwei Containern aus `debian:trixie-slim` mit dem gebauten
Binary, ohne Netzwerk, das Volume bei beiden unter `/run/zerodds-uds`
mit `:z`; die Wurzel des Volumes gehört root und der Gruppe 61042 mit
`3770`; ein Thread des Tests liest die Socket-Dateien vom Host aus);
`ci/manual-tests.tsv` (die drei Container-Tests);
`testing/isolation/docker/docker-compose.uds.yml` und
`testing/isolation/docker/Dockerfile` (dasselbe Muster mit Docker
Compose). Podman gibt einem neuen benannten Volume beim ersten Mount
Eigentümer und Gruppe des Container-Benutzers und entfernt damit die
gemeinsame Gruppe (gemessen mit Podman 5.4.2); der Podman-Test nutzt
deshalb ein Host-Verzeichnis als Volume.

**Tests:** `tools/isolation-smoke/tests/container_volume.rs::uds_5_2_two_containers_under_different_uids_exchange_datagrams_over_a_volume_in_both_directions`
(Docker, benanntes Volume; Echo unter UID 61001, Ping unter UID 61002,
je mit eigener primärer Gruppe und der Zusatzgruppe 61042, beide mit
dem Modus `0660` der Socket-Datei: 50 Datagramme hin und zurück, jedes Echo
unverändert und vom Locator des Echos, das Echo antwortet dem Locator
des Pings; auf dem Host liegen beide Socket-Dateien auf dem Volume,
Sockets mit `0660`, Eigentümer 61001 und 61002, Gruppe 61042),
`tools/isolation-smoke/tests/container_volume.rs::uds_5_2_without_socket_mode_the_container_of_the_other_uid_is_refused`
(Echo ohne eigenen Modus der Socket-Datei: sie hat weder Gruppen- noch
Fremd-Schreibrecht, das erste Senden des Pings wird mit „uds:
permission denied on destination“ abgewiesen, Exit-Code 3),
`tools/isolation-smoke/tests/container_volume.rs::uds_5_2_two_containers_with_different_uid_mappings_exchange_datagrams_in_both_directions`
(Podman, je Container ein eigener User-Namespace: UID 1000 ist auf dem
Host 101000 beim Echo und 201000 beim Ping, die Gruppe 61042 ist in
beiden 61042; 50 Datagramme in beide Richtungen; auf dem Host tragen die
Socket-Dateien die Eigentümer 101000 und 201000, die Gruppe 61042 und
`0660`),
`tools/isolation-smoke/tests/roundtrip.rs::uds_5_2_ping_and_echo_exchange_datagrams_in_both_directions`,
`tools/isolation-smoke/tests/roundtrip.rs::uds_5_2_socket_mode_and_the_echo_roles_are_refused_where_they_do_not_apply`.

**Status:** done — die drei Container-Tests sind manuelle Tests auf dem
Linux-Testhost (`ci/manual-tests.tsv`, Entscheidung 11). Sie brauchen
root auf dem Rechner, auf dem der Container-Daemon läuft: der Test setzt
Eigentümer, Gruppe 61042 und Modus der Volume-Wurzel und liest die
Socket-Dateien auf dem Volume vom Host aus; der dritte Test braucht
zudem Podman. Die GitLab-Runner haben keine Administratorrechte
(Entscheidung 11); Runner 8 startet Docker-Container in der VM von
colima, dort liegt auch das Volume, nicht auf dem Rechner des Jobs; kein
Runner hat Podman. Als root mit Docker 26.1.5 und
Podman 5.4.2 grün (`cargo test -p zerodds-isolation-smoke --test
container_volume -- --ignored`: 3 passed). Die beiden Tests aus
`roundtrip.rs` laufen in der Krate-Ebene. Ein Host mit SELinux im Modus
enforcing war nicht verfügbar; die Mounts tragen `:z`, ohne SELinux
ignorieren beide Runtimes es.

## §6 Plattform-Support

### §6.1 Linux und macOS

**Spec:** §6 — Linux ist die primäre Plattform mit beiden Modi
(Filesystem und Abstract-Namespace), macOS unterstützt den
Filesystem-Modus. Auf beiden gibt es den Datagramm- und den Stream-Modus
(§3.5); der Datagramm-Transport ist `#[cfg(unix)]`, Windows regelt §6.2.

**Repo:** `crates/transport-uds/src/lib.rs` (Datagramm-Transport unter
`cfg(unix)`,
`#[cfg(target_os = "linux")] pub mod abstract_dgram`; die
macOS-Zweige `SEND_BUFFER_OVERHEAD`, `BUFFER_CAP_SYSCTL`,
`RECV_TRUNC_FLAG` und die `SO_RCVBUF`-Prüfung in `size_buffers` unter
`cfg(not(any(target_os = "linux", target_os = "android")))`);
`ci/platform_jobs.py` und `ci/platforms.yml` (Job
`platform-macos-transport-uds` auf Runner 8, wöchentlich, zum
Release-Kandidaten und als `merge-macos-transport-uds`, wenn sich das
Paket ändert); `ci/test_platform_jobs.py`
(`test_a_change_to_transport_uds_runs_its_macos_job`). Die Tests von
`src/lib.rs` und `tests/l1_cross_process.rs` legen ihre temporären
Verzeichnisse unter `/tmp` an (`short_tempdir`): `sun_path` hat auf
macOS 104 Byte, und unter dem per-User-`TMPDIR`
(`/var/folders/<2>/<30>/T/`, 49 Byte) hätte ein Socket in einem
Unterverzeichnis eines temporären Verzeichnisses 104 Byte, die der Bind
ablehnt; unter `/tmp` ist der längste Pfad 59 Byte. Das Verzeichnis
erhält die Gruppe des Prozesses, weil es auf macOS die Gruppe `wheel`
von `/tmp` erbt und ein Set-Group-ID-Bit eine Gruppe des Prozesses
braucht. `ci/macos-loopback.sb`, die Sandbox, in der der Job die Tests
ausführt, erlaubt die Operation `file-write-setugid`: ohne sie verwirft
macOS in der Sandbox das Set-User-ID- und das Set-Group-ID-Bit eines
`chmod` ohne Fehler (gemessen: `chmod 3770` auf ein Verzeichnis der
Gruppe des Prozesses ergibt in der Sandbox `1770`, außerhalb `3770`),
und `uds_5_1_base_dir_mode_is_applied_to_a_created_dir` scheiterte mit
der Ablehnung aus §2.2 (Job 238162).

**Tests:** `crates/transport-uds/src/lib.rs::tests::uds_3_1_send_and_recv_roundtrip_same_process`,
`crates/transport-uds/tests/l1_cross_process.rs::uds_3_1_l1_datagram_crosses_process_boundary`,
`crates/transport-uds/src/lib.rs::tests::uds_3_4_a_datagram_near_the_configured_limit_arrives_whole`,
`crates/transport-uds/src/lib.rs::tests::uds_3_3_recv_of_a_datagram_above_max_datagram_is_an_error_not_a_short_message`
(auf beiden Plattformen; der macOS-Job führt alle Tests des Pakets aus),
`crates/transport-uds/src/abstract_dgram.rs::tests::uds_2_3_abstract_send_recv_roundtrip` (nur Linux).

**Status:** partial — Linux gemessen auf dem Linux-Testhost. macOS
gemessen auf einem Mac (macOS 26.2, arm64): `cargo nextest run
--profile merge -p zerodds-transport-uds --all-targets` 56 Tests grün,
ohne Sandbox und in der von `ci/macos-loopback.sb`, wie der Job sie
ausführt; die Sockets der Tests liegen unter `/tmp`, ihre Pfadlängen
sind damit gemessen. Runner 8 (`TMPDIR` `/var/folders/80/<30>/T/`,
49 Byte) lief den Job `merge-macos-transport-uds` in Pipeline 9916:
55 Tests, 53 grün; die beiden Fehlschläge,
`uds_3_5_stream_a_connection_without_the_hello_or_ending_inside_a_frame_is_counted`
(§3.5) und `uds_5_1_base_dir_mode_is_applied_to_a_created_dir` (die
Sandbox oben), sind behoben. Offen: ein grüner Lauf dieses Jobs auf
Runner 8.

### §6.2 Windows

**Spec:** §6 — Windows bietet seit Windows 10 (Build 17063) Sockets der
Familie `AF_UNIX`, aber nur vom Typ `SOCK_STREAM`, und kennt keine
Mode-Bits; eine Socket-Datei trägt die Zugriffsliste (ACL) ihres
Verzeichnisses, und verbinden darf, wer Schreibrecht auf die Datei hat.
Auf Windows ist der UDS-Transport daher der Stream-Modus von §3.5
(`UdsStreamTransport`); die Runtime hat dort `transport.uds.mode`
`stream` als Default, `datagram` lässt den Start scheitern. Pfad wie
§2.1; der Default `/tmp/zerodds/uds` liegt auf dem aktuellen Laufwerk
(`\tmp\zerodds\uds`), `transport.uds.base_dir` nimmt einen Pfad ab der
Wurzel eines Laufwerks, und der Pfad der Socket-Datei muss in
`sun_path` (108 Oktette) passen. Ein Basis-Verzeichnis, das der Bind
anlegt, erhält eine geschützte ACL mit Vollzugriff für den Benutzer des
Prozesses und `SYSTEM` (`D:P(A;OICI;FA;;;<SID>)(A;OICI;FA;;;SY)`), die
Socket-Dateien darin erben sie: das Verzeichnis ist privat wie `0700`
unter Unix (§2.2). Ein bestehendes Verzeichnis behält seine ACL; ein
Reparse-Point (symbolischer Link, Junction) wird abgelehnt.
`socket_mode` und ein `base_dir_mode` außer `0700` werden mit
`Unsupported` abgelehnt; ein Verzeichnis für mehrere Benutzer bereitet
der Betreiber mit einer ACL vor. Die Fehlerklassen sind die von §4.3; ein
abgelaufener `SO_SNDTIMEO` (`WSAETIMEDOUT`) gilt als voller Sendepuffer,
und die Verbindung wird danach nicht weiter benutzt, weil Microsoft ihren
Zustand nach einem Timeout als unbestimmt beschreibt. Named Pipes sind
nicht Teil dieser Spec.

**Repo:** `crates/transport-uds/src/lib.rs` (`#![cfg(any(unix,
windows))]`; Datagramm-Transport, Modes und `abstract_dgram` unter
`cfg(unix)`; `socket_path` schreibt das Verzeichnis auf Windows mit den
Trennzeichen der Plattform; `classify_send_error` mit `WSAENOBUFS`);
`crates/transport-uds/src/windows.rs` (`check_modes`,
`ensure_base_dir`, `create_private_dir` mit `CreateDirectoryW` und der
ACL aus `private_dacl_sddl`, `current_user_sid`);
`crates/transport-uds/src/stream.rs` (§3.5);
`crates/transport-uds/Cargo.toml` (`windows-sys` für Windows, `libc`
nur für Unix); `crates/dcps/src/runtime.rs` (`UserTransportKind::Uds`
unter `any(unix, windows)`, `DEFAULT_UDS_MODE`, im Modus `datagram`
auf Windows ein `Unsupported`-Fehler mit dem Ereignis
`transport.uds.bind_failed`, `transport.uds.base_dir` auf Windows ab der
Wurzel eines Laufwerks); `ci/platform_jobs.py` (`WINDOWS_EXTRA`,
`WINDOWS_FEATURE_RUNS`) und `ci/platforms.yml` (auf dem Windows-Runner,
wöchentlich und zum Release-Kandidaten: Job
`platform-windows-transport-uds` mit allen Tests des Pakets, Job
`platform-windows-dcps-uds` mit dem Ziel `contract_transport_uds` von
`zerodds-dcps` und dem Feature `same-host-uds`, also dem UDS-Zweig der
Runtime); `ci/platform-jobs.yml` (die Windows-Vorlage übergibt
`PLATFORM_FEATURES` als `--features` und `PLATFORM_TESTS` als
`--test`); `ci/cutover_gate.py` (`platform_runs` zählt mit den Features
des Jobs); `ci/test_platform_jobs.py`
(`test_windows_runs_the_uds_transport_after_the_shm_transport`,
`test_windows_runs_the_runtimes_uds_arm_with_its_feature`).

**Tests:** `crates/transport-uds/src/windows.rs::tests::uds_6_2_a_created_base_dir_is_private_to_the_user`
(die ACL des angelegten Verzeichnisses ist `private_dacl_sddl` des
Benutzers, die Socket-Datei erbt Vollzugriff für Benutzer und `SYSTEM`),
`crates/transport-uds/src/windows.rs::tests::uds_6_2_mode_bits_are_refused_on_windows`,
`crates/transport-uds/src/windows.rs::tests::uds_6_2_a_junction_as_base_dir_is_refused`,
`crates/transport-uds/src/windows.rs::tests::uds_6_2_the_stream_transport_carries_messages_on_windows`,
`crates/transport-uds/tests/l1_cross_process.rs::uds_3_5_l1_stream_frames_cross_the_process_boundary`
(auf Windows der UDS-Transport über eine Prozessgrenze),
`crates/dcps/tests/contract_transport_uds.rs::uds_6_2_the_runtimes_default_mode_is_the_platforms`,
`crates/dcps/tests/contract_transport_uds.rs::uds_6_2_the_mode_datagram_fails_the_start_on_windows`
(nur Windows: ein Teilnehmer im Modus `datagram` scheitert mit
`TransportError` „user unicast bind (UDS)“, genau ein Ereignis
`transport.uds.bind_failed` mit `mode` `datagram` und dem Grund, im
`base_dir` entsteht nichts),
`crates/dcps/tests/contract_transport_uds.rs::uds_3_5_a_refused_stream_bind_is_reported_with_its_mode`
(auf Windows über die Prüfung des Basis-Verzeichnisses von `windows.rs`),
`crates/dcps/tests/runtime_config_layering.rs::uds_5_1_the_uds_directory_modes_and_datagram_limit_are_settings`
(Default `stream` auf Windows; `/run/zerodds-uds` gilt dort als Pfad ab
der Wurzel); die Tests von `src/stream.rs` (§3.5) laufen im Windows-Job
`platform-windows-transport-uds` mit, die von `contract_transport_uds`
im Job `platform-windows-dcps-uds`.

**Status:** partial — gebaut. Gemessen auf dem Linux-Testhost: der
Stream-Modus (§3.5), die Runtime im Modus `stream` und
`contract_transport_uds` mit `same-host-uds` (7 Tests, davon 6 auf Linux
übersetzt und grün). Für `x86_64-pc-windows-gnu` übersetzt: `cargo
clippy --target x86_64-pc-windows-gnu -p zerodds-transport-uds
--all-targets -- -D warnings` und `cargo clippy --target
x86_64-pc-windows-gnu -p zerodds-dcps --features same-host-uds
--all-targets -- -D warnings` ohne Befund. Auf Windows nicht gelaufen:
die Jobs `platform-windows-transport-uds` (der Transport) und
`platform-windows-dcps-uds` (der UDS-Zweig der Runtime: Default
`stream`, `datagram` lässt den Start scheitern, das Ereignis
`transport.uds.bind_failed` mit dem Modus) brauchen den Windows-Runner
und eine Pipeline, die sie startet; ob der zweite Job mit dem Bau von
`zerodds-dcps` unter `same-host-uds` in 5 Minuten passt, ist nicht
gemessen. Die Angaben zu Windows (nur `SOCK_STREAM`,
Rechte über die ACL der Datei, `SO_SNDTIMEO`, die Fehlercodes eines
fehlenden Pfads) stammen aus der Dokumentation von Microsoft und sind
nicht gemessen.

### §6.3 Cross-Vendor-Interop

**Spec:** §6 — UDS ist Intra-Container- und Intra-Host-IPC. Cross-Vendor-
Interop mit Cyclone DDS oder FastDDS bleibt in der UDP-, TCP- und
SHM-Domäne; beide Vendoren haben keinen UDS-Transport, mit dem sich ein
Wire-Vergleich führen ließe. Die Sektion hält den Scope fest und stellt
keine Implementierungs-Anforderung.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — weder Cyclone DDS noch FastDDS hat einen UDS-Transport, gegen den ein Wire-Vergleich möglich wäre.

---

## Audit-Status

15 done / 3 partial / 0 open / 0 blocked / 2 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-uds-transport-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-rtps --all-targets` (Job `merge-rust-rtps`) — 2 Tests grün.
* `cargo nextest run --profile merge -p zerodds-transport-uds --all-targets` (Job `merge-rust-transport-uds`) — 29 Tests grün.

31 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `zerodds-uds-transport-1.0.open.md`.

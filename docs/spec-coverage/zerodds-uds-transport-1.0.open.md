# zerodds-uds-transport-1.0 — offene Punkte

Aggregat aus `zerodds-uds-transport-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §5.1 Volume-Mount als Permission-Grenze

**Status:** `partial` — Rechte-Modell, Schlüssel und Runtime-Anbindung sind gebaut und auf dem Linux-Testhost über eine echte UID-Grenze gemessen (als root, der die beiden Prozesse unter anderen UIDs startet), ebenso das Volume unter Docker und unter Podman mit je einem User-Namespace pro Container (§5.2). Nicht gemessen: das SELinux-Label; es braucht einen Host mit SELinux im Modus enforcing, auf dem die Container-Tests von §5.2 laufen (ihre Mounts tragen `:z`).

## §6.1 Linux und macOS

**Status:** `partial` — Linux gemessen auf dem Linux-Testhost. macOS gemessen auf einem Mac (macOS 26.2, arm64): `cargo nextest run --profile merge -p zerodds-transport-uds --all-targets` 56 Tests grün, ohne Sandbox und in der von `ci/macos-loopback.sb`, wie der Job sie ausführt; die Sockets der Tests liegen unter `/tmp`, ihre Pfadlängen sind damit gemessen. Runner 8 (`TMPDIR` `/var/folders/80/<30>/T/`, 49 Byte) lief den Job `merge-macos-transport-uds` in Pipeline 9916: 55 Tests, 53 grün; die beiden Fehlschläge, `uds_3_5_stream_a_connection_without_the_hello_or_ending_inside_a_frame_is_counted` (§3.5) und `uds_5_1_base_dir_mode_is_applied_to_a_created_dir` (die Sandbox oben), sind behoben. Offen: ein grüner Lauf dieses Jobs auf Runner 8.

## §6.2 Windows

**Status:** `partial` — gebaut. Gemessen auf dem Linux-Testhost: der Stream-Modus (§3.5), die Runtime im Modus `stream` und `contract_transport_uds` mit `same-host-uds` (7 Tests, davon 6 auf Linux übersetzt und grün). Für `x86_64-pc-windows-gnu` übersetzt: `cargo clippy --target x86_64-pc-windows-gnu -p zerodds-transport-uds --all-targets -- -D warnings` und `cargo clippy --target x86_64-pc-windows-gnu -p zerodds-dcps --features same-host-uds --all-targets -- -D warnings` ohne Befund. Auf Windows nicht gelaufen: die Jobs `platform-windows-transport-uds` (der Transport) und `platform-windows-dcps-uds` (der UDS-Zweig der Runtime: Default `stream`, `datagram` lässt den Start scheitern, das Ereignis `transport.uds.bind_failed` mit dem Modus) brauchen den Windows-Runner und eine Pipeline, die sie startet; ob der zweite Job mit dem Bau von `zerodds-dcps` unter `same-host-uds` in 5 Minuten passt, ist nicht gemessen. Die Angaben zu Windows (nur `SOCK_STREAM`, Rechte über die ACL der Datei, `SO_SNDTIMEO`, die Fehlercodes eines fehlenden Pfads) stammen aus der Dokumentation von Microsoft und sind nicht gemessen.

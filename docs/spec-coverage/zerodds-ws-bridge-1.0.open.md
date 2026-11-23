# zerodds-ws-bridge-1.0 — offene Punkte

Aggregat aus `zerodds-ws-bridge-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §6 DDS-QoS → WS-Behavior Map

**Status:** `partial` — die Tests liefen auf dem Linux-Testrechner grün. Offen: Der Daemon berechnet den KeyHash nicht aus dem Payload. Die Reader eines Topics mit Schlüssel bekommen keine Typunterstützung für den Schlüssel (dafür fehlt in `zerodds-dcps` das Gegenstück zu `set_user_writer_key_support` für Reader, und der Daemon müsste aus der `idl_file` eine `SerializedTypeSupport` bauen). Ein Sample ohne `PID_KEY_HASH` auf einem solchen Topic nennt deshalb keine Instanz: Es geht an die Clients, wird aber nicht für spätere Abonnenten gehalten und startet keine Deadline; es wird in `zerodds_ws_bridge_errors_total` gezählt und einmal je Topic protokolliert (Spezifikation §6). Ein Topic ohne `idl_file` gilt als Topic ohne Schlüssel. Jeder Verlust zählt in `zerodds_ws_bridge_send_queue_drops_total{connection_id}` (§8.2).

## §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Status:** `partial` — die Tests liefen grün auf dem Linux-Testrechner. Offen: Auf Windows setzt nichts das Flag des Beendens (`install_signal_watcher` ist dort ein leerer Thread; es fehlen der Konsolen-Handler für Ctrl+C und der Stopp des Windows-Dienstes `ZeroDDSWSBridge`). Der Linux-Testrechner hat kein Windows-Target, und es gibt keinen Windows-Runner; ungeprüfter Code wurde nicht geschrieben.

## §11 Linux/macOS/Windows/Docker Layout

**Status:** `partial` — die Tests liefen grün auf dem Linux-Testrechner; der Windows-Code wurde dort mit `cargo clippy --target x86_64-pc-windows-gnu -p zerodds-websocket-bridge --all-targets -- -D warnings` und ebenso für `zerodds-dcps` (`--lib --tests`) übersetzt, nicht ausgeführt. Es fehlt: (1) der Lauf des Dienstes `ZeroDDSWSBridge` unter `sc.exe` (Start, `READY=1` als `SERVICE_RUNNING`, Stopp mit dem Drain, `paramchange` als Reload); er braucht einen Windows-Rechner mit dem registrierten Dienst, der Linux-Testrechner ist keiner; der Windows-Job von `zerodds-dcps` (Windows-Runner, Benutzer SYSTEM) führt die Tests von `cli` aus, darunter den Konsolenweg von `run_as_service`, und ist auf dieser Änderung noch nicht gelaufen. (2) Kein Job startet das Image `packaging/docker/ws-bridged/` und spricht es mit einem Client an; `release.yml` baut es nur beim Release, und auf dem Messrechner (unprivilegierter LXC-Container) scheitert `docker build` laut `ci/release-packaging-check.yml` (gelesen, nicht gemessen).

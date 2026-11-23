# zerodds-mqtt-bridge-1.0 — offene Punkte

Aggregat aus `zerodds-mqtt-bridge-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §11 Linux/macOS/Windows/Docker Layout

**Status:** `partial` — die Tests liefen grün auf dem Linux-Testrechner; der Windows-Code wurde dort mit `cargo clippy --target x86_64-pc-windows-gnu -p zerodds-mqtt-bridge --all-targets -- -D warnings` und ebenso für `zerodds-dcps` (`--lib --tests`) übersetzt, nicht ausgeführt. Es fehlt der Lauf des Dienstes `ZeroDDSMQTTBridge` unter `sc.exe` (Start, `READY=1` als `SERVICE_RUNNING`, Stopp mit dem Drain, `paramchange` als Reload, das Log in `%PROGRAMDATA%\ZeroDDS\logs\mqtt-bridged.log`); er braucht einen Windows-Rechner mit dem registrierten Dienst, der Linux-Testrechner ist keiner. Es fehlt ein CI-Job, der den Windows-Code dieses Pakets übersetzt (`install_signal_watcher` unter `cfg(not(unix))`, der Windows-Weg von `run_as_service`, `running_as_service`): die Windows-Jobs der CI (`ci/platform_jobs.py`, `CORE` und `WINDOWS_EXTRA`) laufen nur für die Kern-Crates und `zerodds-foundation`, `zerodds-qos`, `zerodds-transport-shm`, und das CI-Image (`ci/Dockerfile.rust`) hat das Target `x86_64-pc-windows-gnu` nicht; ein solcher Job braucht einen der beiden Wege und einen Lauf, der ihn grün zeigt. Das Image von `packaging/docker/mqtt-bridged/Dockerfile` wurde auf dem Linux-Testrechner einmal von Hand gebaut (Docker 26.1.5, Exit 0) und gestartet: `--check-config` mit der Konfiguration des Images Exit 0; mit `--network host` und `--broker` gegen Mosquitto 2.0.21 die Zeilen `connect`, `subscribed`, `catalog_published`, der Katalog auf `$zerodds/zerodds-mqtt-bridge/catalog` von `mosquitto_sub` gelesen, nach `docker stop` `catalog_cleared`, `disconnect` mit 0x00 und Exit 0. Kein Job außer `release.yml` beim Release baut das Image.

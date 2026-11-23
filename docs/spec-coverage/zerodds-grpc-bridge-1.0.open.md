# zerodds-grpc-bridge-1.0 — offene Punkte

Aggregat aus `zerodds-grpc-bridge-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §11 Linux/macOS/Windows/Docker Layout

**Status:** `partial` — die Tests liefen grün auf dem Linux-Testrechner; der Windows-Code wurde dort mit `cargo clippy --target x86_64-pc-windows-gnu --all-targets` übersetzt, nicht ausgeführt. Der wöchentliche Job `platform-windows-grpc-bridge-build` (`ci/platform_jobs.py` `WINDOWS_BUILD`, `ci/platforms.yml`) baut alle Targets von `zerodds-grpc-bridge` auf dem Windows-Runner, darunter den Watcher des Dienstes und den Aufruf von `run_as_service`; er ist noch nicht gelaufen. Es fehlt der Lauf des Dienstes `ZeroDDSGrpcBridge` unter `sc.exe` (Start, `READY=1` als `SERVICE_RUNNING`, Stopp mit dem Drain, `paramchange` als Reload); er braucht einen Windows-Rechner mit dem registrierten Dienst.

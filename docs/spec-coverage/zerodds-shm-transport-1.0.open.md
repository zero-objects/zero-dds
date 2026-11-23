# zerodds-shm-transport-1.0 — offene Punkte

Aggregat aus `zerodds-shm-transport-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §4.4 Ereignisgetriebenes Warten statt Poll-Schleife

**Status:** `partial` — gebaut; gemessen auf dem Linux-Testhost mit Futex und FIFO (`cargo nextest run --profile merge -p zerodds-transport-shm --all-targets`). Die macOS-Wege (os_sync, ulock, der FIFO auf macOS) liefen grün auf einem Mac (macOS 26.2, arm64: derselbe Befehl, 92 Tests, in der Sandbox von `ci/macos-loopback.sb`); der Job `merge-macos-transport-shm` auf Runner 8 scheiterte in Pipeline 9916 am Paar-Verzeichnis (§5.4), sein grüner Lauf dort steht aus. Ein macOS zwischen 10.15 und 14.3, auf dem ulock der eigene Weg ist, ist nicht gemessen; auf macOS 26 erzwingen die Tests ihn. Der Weg über das Event auf Windows und die je Weg neu geschriebenen Tests sind seit `3861278cf5e` für `x86_64-pc-windows-gnu` mit Clippy übersetzt und unter Wine 10.0 auf dem Linux-Testhost grün (§6.2); auf Windows sind sie nicht gelaufen: das tut der Job `platform-windows-transport-shm`. Der FIFO als eigener Weg der übrigen Unix-Systeme (`native()` in `wait.rs`) ist für `x86_64-unknown-freebsd` mit Clippy übersetzt, mit libc auf dem ABI von FreeBSD 11 (`RUST_LIBC_UNSTABLE_FREEBSD_VERSION=11`): mit dem Standard-ABI der libc (FreeBSD 12) übersetzt `nix` 0.23.2, das `shared_memory` 0.12 mitbringt, nicht, und damit das Crate für FreeBSD nicht. Auf einem solchen System ist er nicht gelaufen; auf Linux läuft der FIFO in den Tests oben als erzwungener Weg.

## §6.2 Windows

**Status:** `partial` — gemessen auf dem Linux-Testhost: `cargo nextest run --profile merge -p zerodds-transport-shm --all-targets` 91 von 91 grün; derselbe Lauf mit `--target x86_64-pc-windows-gnu` unter Wine 10.0 als Target-Runner 89 von 90 grün (nextest meldet einen davon als `leaky`); der eine ist `shm_5_4_a_pair_dir_that_is_a_junction_is_refused`, weil das `cmd` von Wine keine Junction anlegt. Der Lauf des Jobs `platform-windows-dcps-shm` unter Wine, `cargo nextest run --profile merge -p zerodds-dcps --features same-host-shm --test contract_transport_shm --target x86_64-pc-windows-gnu`: 5 von 5 grün. Wine ist nicht Windows: es gibt den Namen einer entfernten Datei erst mit ihrem letzten Handle frei und meldet sie nicht als zum Löschen markiert (gemessen; daher `remove_name`). Auf Windows selbst ist dieser Stand nicht gelaufen: die Jobs `platform-windows-transport-shm` und `platform-windows-dcps-shm` laufen auf dem Windows-Runner der CI, den dieser Lauf nicht erreichte. Ein früherer Stand des Crates lief dort in Pipeline 8782 (44 Tests grün).

## §6.3 Nachweis der Windows-Cleanup-Semantik

**Status:** `partial` — beide Tests unter Wine 10.0 auf dem Linux-Testhost grün (§6.2); auf Windows nicht gelaufen: das misst der Job `platform-windows-transport-shm` auf dem Windows-Runner der CI, den dieser Lauf nicht erreichte.

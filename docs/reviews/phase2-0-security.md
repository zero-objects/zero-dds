# Phase-2.0 Security-Audit — ZeroDDS

**Datum:** 2026-04-20
**Scope:** Commits 0c19d7d, 5eda933, a00f75c, 45d2b45, dc39ed7, 700b07d,
b9fac67. Erstmaliger Bruch mit `forbid(unsafe_code)` in zwei
Transport-Crates + zwei neue Deps (`shared_memory 0.12.4`, `socket2 0.5`).
Baseline: `phase1-post-fix-security.md`.

## Findings

| # | Sev | Bereich | Titel |
|---|-----|---------|-------|
| 1 | **High** | transport-shm | SpSc-Annahme nicht runtime-erzwungen |
| 2 | **High** | transport-tcp | Slow-Read-DoS im Handshake (kein read-timeout) |
| 3 | **Med**  | transport-shm | `PADDING_FRAME_LEN`-Kollision via mutierter Peer |
| 4 | **Med**  | isolation-smoke | `--base-dir` ohne Symlink-Validierung |
| 5 | **Low**  | transport-shm | `read_u32` vertraut attacker-writable Ring |
| 6 | **Low**  | transport-uds | Abstract-Name NUL-Injection theoretisch |
| 7 | **Info** | TCP-Handshake | Cyclone/Fast-DDS-Inkompat doc-ok, API-stumm |
| 8 | **Info** | Deps | neue Transitive: `nix`, `rand`, `win-sys` |

### 1 — SpSc-Annahme nur per Doc (High)

`crates/transport-shm/src/posix.rs:182-183` deklariert
`unsafe impl Send/Sync for SegmentLayout`. Das ist korrekt, solange
**genau ein** Writer und **genau ein** Reader das Segment mappen.
Zwei `PosixShmTransport::open_owner`-Aufrufe **im selben Prozess**
mit denselben IDs sind durch `remove_file(&flink)` + `Shmem::create`
defacto race-frei (zweiter create schlaegt OS-seitig fehl), aber zwei
Owner in **verschiedenen Prozessen** koennen in der Luecke zwischen
`remove_file` und `create` parallel binden. Race: beide Owner
schreiben konkurrierend auf `AtomicU64::head` mit `Relaxed`-Loads →
Data-Race auf Frame-Payload (die Atomics sind tearing-free, die
`ptr::copy_nonoverlapping`-Bodies aber nicht).
**Fix:** `posix.rs:328`: `Shmem::create_with_lock()` oder
`flock(flink_dir)` in Zeile 325 vor `remove_file`. Alternativ
`RoleBind`-Token in den Header (offset 56) mit `compare_exchange`.

### 2 — TCP-Handshake ohne Read-Timeout (High, DoS)

`crates/transport-tcp/src/tcp_transport.rs:286-304`: accepted Stream
bekommt **nie** `set_read_timeout` gesetzt. `server_handshake` macht
`read_exact(&mut [0u8; 16])`. Angreifer connected, schreibt 0 Bytes,
haelt TCP-Halfopen → Thread parkt unendlich.
`MAX_PEERS=256` cappt den Pool erst *nach* erfolgreichem Handshake —
die Pre-Handshake-Accept-Loop ist ungedeckelt.
**Fix:** `stream.set_read_timeout(Some(handshake_timeout))?` direkt
nach `accept`; Default ~5s. Zusaetzlich Semaphor auf max pending
Handshakes.

### 3 — Padding-Frame-Injection (Medium)

`posix.rs:79, 495`: `PADDING_FRAME_LEN = 0xFFFF_FFFE`. Owner
erzeugt diese Markierung nur beim Wrap. Schreibt aber ein anderer
Prozess (Fehlkonfiguration aus Finding 1, oder eine kaputte Owner-
Impl) `0xFFFF_FFFE` als reale Laenge, ueberspringt der Reader den
kompletten Rest der Region — kein Crash, aber Message-Loss ohne
Log. Offensichtlich kein Cross-Trust-Boundary, aber Diagnostik-Loch.
**Fix:** `occupied_bytes`-Telemetry + warn-log wenn
`len==PADDING_FRAME_LEN` bei `tail_space == capacity` (impossible-
state).

### 4 — isolation-smoke Symlink-TOCTOU (Medium)

`tools/isolation-smoke/src/main.rs:139` akzeptiert
`--base-dir=/any/path` ungeprueft. `run_shm:229` + `run_uds:269`
reichen den Pfad direkt an `create_dir_all`/`bind` weiter. Symlink
`/tmp/zds-test -> /etc` fuehrt zu `remove_file`/`create` in
fremden Verzeichnissen (begrenzt auf die Rechte des aufrufenden
Users). Kein privilege-escalation, aber ein fuss-gun beim Lauf
unter einem Service-Account.
**Fix:** `base_dir.canonicalize()?` + Prefix-Check gegen eine
Allowlist (`/tmp/`, `/var/run/zerodds/`). Analoge Haertung in
`posix.rs:325` und `abstract_dgram.rs:160`.

### 5 — `read_u32` auf beschriebenem Ring (Low)

`posix.rs:242, 494` liest Laenge ungeprueft aus dem Ring.
Single-Trust-Pair, daher kein Angriff, aber ein crashender Owner
mit partiell geschriebenem Frame (`head` noch nicht advanced) lasst
den Reader einen `len > capacity` sehen und `vec![0u8; len]`
allozieren. `tail_space < 4`-Check deckt das nur halb ab.
**Fix:** `if len as usize > self.config.max_datagram { return None; }`
in `pop_frame` vor `vec!`-Alloc.

### 6 — Abstract-Name-NUL-Injection (Low, theoretisch)

`abstract_dgram.rs:304-342`: `name` wird aus `prefix + hex(id)`
gebaut, `id: [u8; 16]` liefert nur `[0-9a-f]`. Attacker-controlled
Input landet nicht im Namen. **Kein Bug, dokumentarisch.** Wenn
spaeter `prefix` aus User-Config gezogen wird, muss
`prefix.contains('\0')` gekappt werden.

### 7 — Cyclone-Inkompat nicht am Error-Typ sichtbar (Info)

`handshake.rs:28-37` doc-string nennt Inter-Vendor-Interop explizit
nicht gegeben. `HandshakeError::BadMagic` kommt aber nur an die
accept-Loop, nicht an den `TcpTransport::new`-Caller. Konsumenten
einer `ZeroDdsTcp`-Konfig bekommen erst beim ersten Peer
Fehlermeldungen. **Fix (nice-to-have):** `TcpConfig::strict_zerodds:
bool` + `#[must_use]` Warn-Logger beim Bind.

### 8 — Neue Deps (Info)

`Cargo.lock`: `shared_memory 0.12.4` pullt `nix`, `rand`,
`cfg-if`, `win-sys` (alle transitiv bekannt, keine exotics).
`socket2 0.5.10` pullt `libc` + `windows-sys 0.52`. Lokal keine
`cargo audit`-Ausfuehrung moeglich (nicht installiert, Baseline
Finding #1 unveraendert). CI-Runner `glr1` wird beim naechsten
Push beide Crates durch die `advisory-db`-Policy schicken — keine
bekannten aktiven RUSTSEC-Advisories fuer diese Versionen per
2026-04.

## Verdict

Zwei **High**-Findings (1, 2) blockieren Phase-2.0-Public-Merge.
Beide sind mit <=20 LOC fixbar (flock + `set_read_timeout`). Rest
ist Medium/Low und passt in ein WP-2.0d-Hygiene-Batch.
`unsafe`-Code selbst ist korrekt begruendet und durch Debug-Asserts
+ AtomicU64-Semantik abgedeckt; die Risiken liegen in der
Prozess-Koordination, nicht in der Rust-Safety.

**Wortanzahl (Fliesstext ohne Tabellen/Code):** ~395.

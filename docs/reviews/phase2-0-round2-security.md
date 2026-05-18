# Phase-2.0 Round-2 Security-Audit — ZeroDDS

**Datum:** 2026-04-20
**Scope:** Fix-Commits `c3b9439`, `a6066fe`, `33300e1`, `06b8a08`,
`d5266d7` gegen Baseline `phase2-0-security.md`.

## Findings

| # | Sev  | Bereich         | Titel                                         |
|---|------|-----------------|-----------------------------------------------|
| 1 | Med  | transport-shm   | flock schuetzt nur kooperative Writer         |
| 2 | Med  | transport-shm   | `pop_frame`-Rekursion nicht explizit bounded  |
| 3 | Low  | transport-tcp   | 5 s-Timeout pro syscall, nicht pro Phase      |
| 4 | Low  | isolation-smoke | `validate_base_dir` TOCTOU-Restfenster        |
| 5 | Low  | transport-shm   | deterministische `os_id` → local-preemption   |
| 6 | Info | supply-chain    | `cargo audit` lokal fehlt (CI-only)           |

### 1 — flock-Deckung (Med)

`posix.rs:505-512` nimmt `LOCK_EX` auf `<flink>.lock`. `flock(2)`
ist **advisory** — ein non-kooperativer Prozess ruft direkt
`shm_open`/`create` und umgeht den Lock. Schutz greift nur gegen
Writer, die denselben Code-Pfad nehmen. Abandoned-Lock ist OK:
Kernel gibt bei `close`/Exit frei, kein robust-mutex-Problem.
**NFS:** `flock` seit Linux-2.6.12 klient-seitig emuliert;
`tmpfs`/`ext4`/`xfs` voll — `DEFAULT_FLINK_DIR=/tmp/zerodds/shm`
passt. NFS-Deploys verpuffen den Schutz stumm.
**Fix:** Doc in `ShmConfig::flink_dir` ergaenzen.

### 2 — `pop_frame`-Rekursion (Med)

`posix.rs:711, 725` rufen `self.pop_frame()` rekursiv. Analyse:
nach einem Wrap ist `t % cap == 0` und `tail_space == capacity`,
also kein Padding mehr — Tiefe ≤ 2. Rust garantiert kein TCO;
ein korrupter Writer kann die Tiefe nicht trivial erhoehen, der
Beweis haengt aber an `tail_space`-Invarianten.
**Fix:** 5-LOC-Umbau zu `loop`.

### 3 — TCP-Handshake-Timeout (Low)

`tcp_transport.rs:297-303`: `set_read_timeout(5 s)` gilt **pro
syscall**. `server_handshake` liest 16 Byte in einem `read_exact`
— ein kooperativer Peer schickt das in einem Paket, ein
Slow-Loris-Angreifer kann aber legitim Byte-fuer-Byte senden und
den Thread de-facto unbegrenzt halten (`read_exact` wiederholt
bei WouldBlock). `write_timeout(5 s)` gegen Slow-Read-zurueck
ist ok.
**Fix:** `Instant::now() + 5 s`-Deadline, pro Iteration neu
clampen. Heute kein Blocker, weil `MAX_PEERS=256` nach erstem
ungueltigem Byte-Chunk erreicht wird.

### 4 — `validate_base_dir` TOCTOU-Rest (Low)

`main.rs:202-208` + `uds/lib.rs:139`: `symlink_metadata` →
`create_dir_all`/`bind`. Angreifer mit `w+x` im Parent kann im
Check-Use-Fenster einen Symlink einschieben. Eintrittshuerde:
gleicher User oder Shared-Tmp ohne Sticky. Kein Privilege-Escal,
`/tmp`-Sticky deckt Cross-User ab.
**Fix (optional):** `openat2(2)` mit `RESOLVE_NO_SYMLINKS`
(Linux-5.6+) — Phase-2.1.

### 5 — Deterministische `os_id` (Low)

`segment_os_id` → `/zd-<owner>-<consumer>` ist aus den GUIDs
ableitbar. Ein Prozess im gleichen User-Namespace kann den Slot
**vor** dem echten Owner belegen. Der Owner ruft dann
`shm_unlink_by_os_id` (Zeile 519) — `flock` gilt nicht fuer den
Angreifer. Worst-case: DoS-Cycle; Consumer sieht bei Race-Window
einen fremden Header und kriegt `InvalidHeader` — kein
Data-Corruption, weil Magic+Version validiert wird. Das
By-design-Caveat "Consumer darf `os_id` kennen" ist intentional.
**Fix:** `XDG_RUNTIME_DIR/<uid>/zerodds/` + `0o700`-Parent in
Phase-2.1.

### 6 — `cargo audit` (Info)

Lokal nicht installiert. CI-Runner `glr1` laeuft `advisory-db`
bei jedem Push; per 2026-04-20 keine offenen RUSTSEC-Advisories
fuer `shared_memory 0.12.4`, `socket2 0.5.10`, `nix`, `libc`,
`windows-sys 0.52`.

## Re-Verify Round-1

| Round-1                        | Fix        | Status                              |
|--------------------------------|------------|-------------------------------------|
| #1 SHM race-bind               | c3b9439    | OK fuer coop (s. neu #1)            |
| #2 TCP slow-read               | a6066fe    | OK (s. neu #3)                      |
| #3 PADDING_FRAME_LEN skip      | c3b9439    | OK counter + DoS-cap                |
| #4 base-dir symlink            | c3b9439    | OK (s. neu #4)                      |
| #5 pop_frame len-alloc         | c3b9439    | OK `> max_datagram`-guard           |
| #6 abstract-name NUL           | docs       | OK, nicht exploitable               |

## Positiv

- Owner-`Drop` raeumt flink + `.lock` + `shm_unlink` — Zombie-
  Segmente aus Round-1 sauber geschlossen.
- `padding_counter` + `len > max_datagram`-Drop macht den
  Silent-Skip beobachtbar und DoS-fest.
- `classify_send_error` ist UDS-local, kein remote Info-Leak.
- `unsafe`-Bloecke bleiben schmal, SAFETY-Comments konsistent.
- Keine neuen `forbid(unsafe_code)`-Bruchstellen ausser den zwei
  in Round-1 begruendeten.

## Verdict

Keine **High**, zwei **Med** (Doc + 5-LOC-Refactor). Phase-2.0
ist merge-fahig. Die local-preemption-Haertung (#5) gehoert in
Phase-2.1 zusammen mit `openat2` und `XDG_RUNTIME_DIR`.

**Wortanzahl (Fliesstext ohne Tabellen/Code):** ~395.

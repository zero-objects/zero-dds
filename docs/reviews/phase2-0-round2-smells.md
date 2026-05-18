# Phase-2.0 Round-2 Smell-Audit — Post-Consolidation

**2026-04-20** — c3b9439 a6066fe 33300e1 06b8a08 d5266d7.

## Severity-Tabelle

| # | Sev | Kategorie | File:Line |
|---|-----|-----------|-----------|
| R1 | **High** | `pop_frame` rekurs. Self-Call — Stack-Overflow bei all-padding Ring | `transport-shm/src/posix.rs:711,725` |
| R2 | **High** | Owner-Drop `shm_unlink` — Consumer timeout'd statt `OwnerGone` | `transport-shm/src/posix.rs:421-431` |
| R3 | Med  | `validate_base_dir` nur im Parse; SHM-Open ohne symlink-Guard (UDS ok) | `main.rs:200`, `posix.rs:490` |
| R4 | Med  | Malformed-len-Drop ohne Counter — silent data loss | `transport-shm/src/posix.rs:732-741` |
| R5 | Med  | `pub fn padding_frames_seen()` exponiert Atomic — Encapsulation-Bruch | `transport-shm/src/posix.rs:688-691` |
| R6 | Low  | `parse_hex_id` silent-left-pad bleibt (`--local-id=01` → `00…01`); `--help` fehlt | `main.rs:185-192` |
| R7 | Low  | Shell `wait $pid` nach `kill TERM` — hang falls child SIGTERM-ignore | `host/l1.sh:20-23` |
| R8 | Low  | `TransportKind` nicht `#[non_exhaustive]` (internal, OK) | `main.rs:50` |

## Details

**R1** `pop_frame` nutzt `return self.pop_frame()` bei Padding + `tail_space<4`.
Kein Rust-Tailcall-Guarantee. All-Padding-Ring (regressiver wrap-Rhythmus oder
malicious): bis 256K Calls bei 1 MiB → Stack-Overflow. **Fix:** `loop/continue`.

**R2** `shm_unlink` entfernt Namen, Consumer-Mapping bleibt via Kernel-refcount
(crasht nicht, gut). Aber `head`/`tail` stehen still → Consumer wartet nur
`recv_timeout` ab statt gezielt `OwnerGone`-Err. **Fix:** Sentinel-Flag im
Header (`shutdown: AtomicU32`), `wait_for_frame` liest ihn.

**R3** `validate_base_dir` checkt nur beim Parse; `PosixShmTransport::open` ruft
`std::fs::create_dir_all` ohne Symlink-Guard. **Fix:** analog
`transport-uds/src/lib.rs:ensure_base_dir` auf SHM-Seite.

**R4** DoS-Guard wirft Frame weg + `tail += tail_space` ohne Zaehler — silent in
Prod. `corrupt_frame_counter` analog `padding_counter` einziehen.

## Re-Check Round-1-Highs

- **#1 wrap-branch:** `needed + tail_space <= free` korrekt (tail_space wird
  publizierter Padding-Frame). ✓
- **#2 unsafe-Doku:** L281-307 AcqRel happens-before explizit aarch64-aware. ✓
- **#3 offset_of!:** `core::mem::offset_of!` in build + decode. ✓
- **#6 classify_send_error:** NotFound/PermissionDenied/WouldBlock/
  ConnectionRefused differenziert. ✓
- **#7 accept_one→Err:** `Reject(reason)` jetzt Err. ✓
- **#9 SHM-Leak:** unlink-before-create + Owner-Drop raeumt alles. ✓

## Antworten

- **Drop vs Consumer-recv:** POSIX-refcount haelt Mapping; Consumer crasht
  nicht, sieht kein Progress → R2.
- **Lock-File-Recovery:** Kernel released `flock` bei Crash; `.lock`-Datei bleibt,
  neuer Owner oeffnet mit `create(true)`+`flock`. Kein abandoned-state. ✓
- **malformed-len-Loop:** `pop_frame` single-shot (tail-advance + `None`); kein
  Endlos-Loop. Risiko ist R1.

## Positives

- Happens-before-Doku reviewer-friendly, aarch64-explizit.
- Cleanup-Trap-Arrays generisch wiederverwendbar fuer L3/L4.
- `classify_send_error` + `padding_counter` + max_backoff=1 ms geben gute
  Ops-Visibility + Tail-Latency.
- TOCTOU-Fix `ensure_base_dir` (symlink_metadata + re-check vor chmod) hart
  genug fuer /tmp-shared.
- `#[non_exhaustive]` konsequent auf allen neuen Public-Enums.

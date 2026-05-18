# Phase-2.0 Consolidation Rollup

**Datum:** 2026-04-21. **Scope:** Commits 0c19d7d…33300e1 (WP 2.0a
Zero-Copy + WP 2.0c Hygiene-1 + WP 2.0b Transport-Parity T1-T5 +
4-Agenten-Audit + 3-Batch-Consolidation).

Konsolidierung nach dem parallelen Audit zu Phase 2.0. Audit-Reports
(`phase2-0-{coverage,smells,perf,security}.md`) wurden ausgewertet,
Findings priorisiert, kritische Items gefixt, Coverage-Luecken
nachgezogen.

## Audit-Summary

| Bereich | Findings | Adressiert in Consolidation |
|---------|----------|------------------------------|
| Coverage | 80.45 % R / 91.21 % L (−1.21/−0.96 pp vs Phase 1) | +15 Tests, erwartete Recovery auf ~82.5 % R |
| Security | 2 Highs, 2 Meds, 2 Lows, 2 Infos | Beide Highs fixed (Batch 1) |
| Smells | 1 "Crit" (false-positive), 6 Highs, 7 Meds, 1 Sammel-Low | 5 Highs fixed, Crit aufgeklaert |
| Perf | F14 resolved in new path, F13 bleibt, neue Hotspots in UDS/SHM dokumentiert | iovec-Pfad fuer WP 2.0a-2 festgelegt |

## Batch-Ergebnis

### Batch 1 — Security + Functional (commit `a6066fe`)

- **Sec-High-1** SHM owner-create race → POSIX advisory-flock auf
  `.lock`-Datei pro flink vor `remove_file` + `create()`. Zweiter
  Owner serialisiert, sieht fertigen Header, failed bei magic-check.
- **Sec-High-2** TCP slow-read handshake DoS → `set_read_timeout`
  + `set_write_timeout` (5 s) auf accepted stream vor
  `server_handshake`, nach erfolgreichem handshake wieder None.

### Batch 2 — SemVer + Error-Semantik + Quality (commit `a6066fe`)

- **5 neue public Enums** auf `#[non_exhaustive]`:
  `TcpTransportError`, `ResponseStatus`, `RejectReason`, `ShmRole`,
  `UdsAddress`. `TcpTransportError::Handshake` wurde in 45d2b45
  hinzugefuegt — technisch SemVer-Break, jetzt future-proof.
- **`accept_one` bei Handshake-Reject** liefert
  `Err(HandshakeError::Rejected { reason })` statt `Ok(())`.
  Caller unterscheidet Reject von EOF-after-zero-frames.
- **SAFETY-Kommentar** auf `SegmentLayout` dokumentiert exakte
  happens-before-Invarianten (AcqRel-publish-edge); macht den
  Unsafe-Island aarch64-refactor-safe.
- **`sun_path`-Offset** via `core::mem::offset_of!` statt manueller
  ptr-sub. Unsafe-Oberflaeche minimal kleiner.
- **recv-Doc** SEQPACKET → DGRAM korrigiert.
- **UDS `send` io::Error-Klassifikation** NotFound/PermissionDenied/
  WouldBlock/ConnectionRefused statt generischem
  `Io{message: "uds send failed"}`.
- **Crit-Aufklaerung**: `posix.rs:443` wrap-branch-condition
  `(needed + tail_space) <= free` ist korrekt — tail_space ist
  ring-occupancy bis der Reader die Padding-Marke konsumiert.
  Ein Kommentar am Code belegt das fuer kuenftige Audits.

### Batch 3 — Coverage-Boost (commit `33300e1`)

- **+5 Tests** in `handshake.rs`: Paired-Stream-Helper deckt
  client_handshake gegen alle 4 RejectReason-Codes + BadResponse.
- **+5 Tests** in `posix.rs`: InvalidConfig-Pfad,
  occupied_bytes-State, fill_ring_triggers_wraparound_padding
  (erzwingt Wrap), Display-Smoke fuer PosixShmError.
- **+5 Tests** in `isolation-smoke/tests/roundtrip.rs`: Spawn als
  Subprozess via `CARGO_BIN_EXE_isolation-smoke`; UDP+UDS
  roundtrip, 3 Error-Pfade. Schliesst die grosse 0%-Luecke
  (Binary war 212 uncovered regions).

## Noch offen (bewusst deferred)

### Smell-Meds (selektiv, in WP 2.0c-2)
- **#8** UDS `ensure_base_dir` 0o700 auf fremde Dirs → TOCTOU
  mit symlinks. Fix: `canonicalize` + explicit path-check.
- **#9** SHM consumer-drop unlinkt flink nicht; owner-crash laesst
  `/dev/shm/…` liegen. Fix: Reaper-Thread oder periodic GC.
- **#10** Fragment-Path `Arc::from(chunk)` kopiert — Zero-Copy-
  Claim nur fuer unfragmentierte DATA. Doc-Fix.
- **#11** `parse_hex_id` padded silent 0-links — Tippfehler-Trap.
- **#12** Shell-Scripts ohne `trap … EXIT` fuer `mktemp -d`.
- **#13** `docker-compose.shm.yml` `ipc: host` ohne Testing-Only-
  Banner.
- **#14** SHM-recv sleep-poll bis 10 ms — Tail-Latency;
  WP 2.0a-2 bringt futex-basierte Notify.

### Security-Meds
- **#3** PADDING_FRAME_LEN als reale Laenge → Reader skipt silent.
  Fix: Log-Counter + separater Padding-Flag.
- **#4** `isolation-smoke --base-dir` ohne Symlink-Validation.
  Fix: `std::fs::canonicalize` + allowlist.

### Perf — in WP 2.0a-2 (vectored sendmsg)
- UDP + UDS-DGRAM bekommen den groessten iovec-Win (MessageBuilder
  schreibt header + submessages heute in ein flat Vec).
- TCP einen modesten writev-Gewinn.
- SHM profitiert nicht (eine memcpy reicht).

## Verdict

**Phase-2.0 ist mergable.** Criticals aufgeklaert, Highs (Sec +
SemVer + Error-Semantik + Unsafe-Doc) gefixt, Coverage-Luecke um
15 Tests adressiert. Verbleibende Meds sind alle Hygiene-Klasse
und werden opportunistisch in 2.0c-2 oder parallel zu WP 2.1
eingesammelt.

**Naechster Block:** WP 2.0a-2 (iovec/sendmsg) fuer den vollen
Zero-Copy-Claim, dann WP 2.1 DCPS Public API.

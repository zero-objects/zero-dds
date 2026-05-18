# Phase-2.0 Smell/Bug-Audit — Transport-Parity + Arc-Migration

**Datum:** 2026-04-20. **Scope:** Commits 0c19d7d…b9fac67.

## Severity-Tabelle

| # | Sev | Kategorie | File:Line |
|---|-----|-----------|-----------|
| 1 | **Crit** | SpSc-Ring Wrap-Branch starve near-full | `transport-shm/src/posix.rs:443` |
| 2 | **High** | Unsafe SAFETY-Kommentar ungenau; happens-before nicht dokumentiert | `transport-shm/src/posix.rs:181-244` |
| 3 | **High** | `sun_path`-Offset via ptr-sub gegen `&sockaddr_un`; `offset_of!` nutzen | `transport-uds/src/abstract_dgram.rs:355-358` |
| 4 | **High** | Doc/Code — `recv()`-Kommentar sagt SEQPACKET, Transport ist DGRAM | `transport-uds/src/abstract_dgram.rs:238` |
| 5 | **High** | Missing `#[non_exhaustive]` auf 6 neuen Enums | `tcp_transport.rs:38`, `handshake.rs:125,135`, `posix.rs:249`, `abstract_dgram.rs:81`, `main.rs:51` |
| 6 | **High** | Silent Fail — UDS `send` kollabiert alle io::Error auf `Io{message}` | `transport-uds/src/lib.rs:156-169` |
| 7 | **High** | `accept_one` liefert `Ok(())` bei Handshake-Reject | `transport-tcp/src/tcp_transport.rs:298-301` |
| 8 | Med | TOCTOU — `ensure_base_dir` setzt 0o700 auf fremde Dirs | `transport-uds/src/lib.rs:127-142` |
| 9 | Med | Leak — Consumer-Drop unlinkt flink nicht; Crash laesst `/dev/shm/...` | `transport-shm/src/posix.rs:330-340` |
| 10 | Med | Fragment-Path `Arc::from(chunk)` kopiert — Zero-Copy nur fuer DATA | `rtps/src/reliable_writer.rs:680` |
| 11 | Med | `parse_hex_id` padded silent 0-links — Tippfehler unentdeckt | `tools/isolation-smoke/src/main.rs:147-160` |
| 12 | Med | Shell — kein `trap … EXIT` fuer `mktemp -d` | `host/l1.sh:26,38`, `l2_different_user.sh:39` |
| 13 | Med | YAML — `ipc: host`+`/dev/shm`-bind ohne Testing-Only-Banner | `docker-compose.shm.yml:23-44` |
| 14 | Med | SHM-`recv` sleep-polling bis 10 ms, kein futex — Tail-Latency | `transport-shm/src/posix.rs:514-527` |
| 15 | Low | Sammel: Silent-Lock `dropped_frames`, TODOs ohne Issue, Marker-Test, `eprintln!`-Spam, unused var | `tcp_transport.rs:340,933`, `handshake.rs:30`, `posix.rs:3`, `main.rs:58,329`, `cross_host.sh:54` |

## Details

**#1** L443: `(needed + tail_space) <= free` fordert `free >= needed + tail_space`; korrekt ist `needed <= free` (tail_space ist separat publiziertes Padding). Bei `free == needed` spin-loopt Writer statt zu wrappen.

**#2** SAFETY behauptet „nur Atomics". Raw copy ist korrekt *weil* durch `head` Acquire/Release publiziert — explizit dokumentieren, sonst UB-Risiko bei aarch64-Refactor.

**#5** 45d2b45 addierte `Handshake(..)` zu `TcpTransportError` ohne `#[non_exhaustive]` — technisch SemVer-Break. Vor Release nachziehen.

**#7** Server-Reject faellt in `Ok(())`; Caller sieht „EOF nach 0 frames" statt „rejected".

**#10** Fragment-Pfad alloziert pro Chunk — Zero-Copy-Positioning praezise trennen (DATA vs. DATA_FRAG).

**#12** `set -euo pipefail` + Err → `rm -rf` unreached. `trap 'rm -rf "$tmp"' EXIT` nach `mktemp`.

## Positives

- `PosixShmError`, `HandshakeError`, `InvalidLocator` sind `#[non_exhaustive]`.
- Arc-Migration durchgaengig in DATA/DATA_FRAG/CacheChange/writer-tick.
- SHM-Magic+Version-Check catched fremde Segmente; Handshake hat Bad-Magic/Version-Mismatch/Roundtrip-Tests.

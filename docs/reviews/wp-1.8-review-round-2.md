# WP 1.8 TCP-Transport — Review Round 2

**Datum:** 2026-04-20. **Scope:** Follow-up der 20 Findings aus `wp-1.8-code-review.md`.
**Dateien:** `crates/transport-tcp/src/{lib,framing,tcp_transport}.rs`, `crates/transport-tcp/tests/loopback.rs`, `crates/rtps/src/wire_types.rs`.
**Verdict:** Ready to unblock WP 1.9. Keine Merge-Blocker. Keine Regressionen.

## Per-Finding Verification

| # | Prev Sev | Status | Evidence |
|---|----------|--------|----------|
| 1 | Crit | ✅ Fixed | `tcp_transport.rs:321–372` — `impl Transport for TcpTransport`; Tests gehen ueber Trait-Pfad. |
| 2 | High | ✅ Fixed | `tcp_transport.rs:205, 325–343` — Pool-Lock nur fuer Entry-Lookup, `peer_arc` gecloned, danach Peer-Lock. |
| 3 | High | ✅ Fixed | `MAX_INBOUND_QUEUE=1024`, FIFO-Drop, `dropped_frames()`; Test `inbound_overflow_drops_oldest_and_counts`. |
| 4 | High | ✅ Fixed | `MAX_PEERS=256` + Eviction. Nit: Eviction ist BTreeMap-key-order, nicht chronologisch — Kommentar angepasst werden. |
| 5 | High | ✅ Fixed | `TcpTransportError::{Accept, FrameTooLarge, PeerIo}`; `accept_one` gibt konkrete Fehler zurueck. |
| 6 | Med | ✅ Fixed | `SendError::PayloadTooLarge { size, limit }` aus `FrameTooLarge`. |
| 7 | Med | ✅ Fixed | `set_accept_timeout` entfernt. |
| 8 | Med | ✅ Fixed | Accept-Fehler auf `TcpTransportError::Accept`. |
| 9 | Med | 📋 Deferred (dok.) | `accept_one` noch single-shot; explizit als Phase-1 dokumentiert. |
| 10 | Med | ✅ Fixed | Backoff auch bei `flush`/`write_frame`-Fehlern. |
| 11 | Med | ⚠ Offen | `u16::try_from(loc.port)` kollabiert Port-Overflow auf `UnsupportedLocator` (Debug-Papercut). |
| 12 | Med | ✅ Fixed | `recv` blockiert auf Condvar; `try_recv` inherente Methode. |
| 13 | Low→Test | ✅ Fixed | `concurrent_senders_to_one_server` (2 × 50 Frames, 100 erwartet). |
| 14 | Low | ✅ Fixed | "Phase-1 Scope vs DDS-TCP-PSM" Section im Modul-Doc. |
| 15 | Low | ⚠ Offen | `new_tcp_v4` vs `udp_v4` — Namenschema inkonsistent. |
| 16 | Low | ✅ Fixed | `last_attempt: Option<Instant>`. |
| 17 | Low | ✅ Fixed | `drop_writer()` ruft `stream.shutdown(Both)`. |
| 18 | Low | ⚠ Offen | Beziehung zu WP-1.2 Fragmentation nicht dokumentiert. |
| 19 | Low | ✅ Fixed | `unsupported_locator_udpv4` + `unsupported_locator_invalid` Tests. TCPv6 noch nicht. |
| 20 | Low | ⚠ Offen | `new_tcp_v4` dupliziert `udp_v4`-Layout. |

## Neue Beobachtungen (keine Regressionen)

- **#4 Nit:** Eviction via `pool.keys().next()` entfernt niedrigste `SocketAddrV4`, nicht aeltesten Peer. FIFO-Kommentar irrefuehrend. Cleanup: `IndexMap` oder Sidecar-Insertion-Queue.
- **`dropped_frames()` auf poisoned Mutex** liefert 0 via `unwrap_or_default()` — koennte Probleme maskieren.
- **`try_recv` races mit `recv`** — beide drainen dieselbe Queue; OK fuer Tests/Polling, Doc-Note sinnvoll.

## Zusammenfassung

- Critical: 1/1 ✅
- High: 4/4 ✅
- Medium: 7/8 ✅, 1 deferred (#9)
- Low: 4/7 ✅, 3 cosmetic/doc remain (#11, #15, #18, #20)

**Gesamt-Verdikt:** Grüner Pfad fuer WP 1.9.

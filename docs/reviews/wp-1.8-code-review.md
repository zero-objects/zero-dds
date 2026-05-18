# WP 1.8 — TCP-Transport Code Review

**Scope:** `crates/transport-tcp/` + `LocatorKind::Tcpv4/Tcpv6`.
**Overall:** Good (solide Phase-1-Basis; 1 Critical + 4 High vor Phase-2-Merge).
**Security/Robustness:** C+. **Maintainability:** B. **Tests:** B- (Happy-Path stark, Concurrency-Edges fehlen).

## Highlights
- `LocatorKind::Tcpv4=4`, `Tcpv6=8` **spec-konform** (DDS-RTPS 2.5 §9.3.2, DDS-TCP-PSM §4).
- 16-MiB-DoS-Cap + `UnexpectedEof`-Handling im Framer sauber.
- Reconnect-Backoff (50 ms → 5 s) mit `WouldBlock`-Short-Circuit ist lehrbuchhaft.

## Findings

| # | Sev | Theme | File:Line | Description | Fix |
|---|-----|-------|-----------|-------------|-----|
| 1 | Critical | Trait-Gap | `tcp_transport.rs:150-265` | `TcpTransport` implementiert `Transport` **nicht**, obwohl Signaturen 1:1 passen. RTPS-Layer kann nicht polymorph über UDP/TCP dispatchen. | `impl Transport for TcpTransport` hinzufügen, inherente Methoden entfernen oder re-routen. |
| 2 | High | Concurrency | `tcp_transport.rs:243-252` | `send()` hält `peers.lock()` über blocking `connect` + `write_all` + `flush` — serialisiert alle Sends global. | Pool-Lock nur für Entry-Lookup; pro Peer ein eigenes `Arc<Mutex<PeerConn>>`, Release Pool-Lock vor I/O. |
| 3 | High | Resource-Limit | `tcp_transport.rs:141` | `inbound: VecDeque` unbounded — schneller Peer füllt RAM vor `recv()`. | `MAX_INBOUND_QUEUE`; bei Overflow Frame droppen + `tracing::warn`. |
| 4 | High | Resource-Limit | `tcp_transport.rs:140` | `peers: BTreeMap` wächst unbegrenzt; fehlgeschlagene Conns bleiben drin. | Max-Peer-Cap + LRU-Eviction oder Idle-Timeout-Housekeeping. |
| 5 | High | Error-Swallow | `tcp_transport.rs:231-233` | `accept_one` verschluckt `FrameTooLarge` + `Io` stillschweigend mit `break Ok(())` — Telemetrie weg. | Eigene `AcceptError::{FrameTooLarge, PeerIo}` + Result; Caller loggt. |
| 6 | Medium | Spec | `tcp_transport.rs:243-252` | Kein `SendError::PayloadTooLarge`-Pfad; `FramingError::FrameTooLarge` kollabiert zu generic `Io`. | Match auf `FrameTooLarge { announced }` → `SendError::PayloadTooLarge`. |
| 7 | Medium | API-Lie | `tcp_transport.rs:179-195` | `set_accept_timeout(Some(d))` droppt `d` mit `let _ = d;` — API verspricht Timeout, liefert None-Blocking. | Nonblocking + Retry-Loop in `accept_one`, oder Methode umbenennen. |
| 8 | Medium | Error-Mapping | `tcp_transport.rs:213-218` | Accept-Fehler werden auf `TcpTransportError::Bind` gemappt — semantisch falsch. | Neue Variante `TcpTransportError::Accept(io::Error)`. |
| 9 | Medium | Concurrency | `tcp_transport.rs:212-236` | `accept_one` akzeptiert **eine** Connection, blockt dann bis EOF — weitere Peers stranden im Backlog. | Background-Accept-Thread + Reader-Thread pro Connection; sonst TODO(Phase-2) dokumentieren. |
| 10 | Medium | Lost-Frames | `tcp_transport.rs:107-121` | Write-Fehler verwirft Writer, aber halbes Frame auf dem Wire ist weg und Backoff **nicht** inkrementiert (anders als connect-Fail). | Backoff auch bei Write-Fail hochsetzen; Doc: `send` ist Best-Effort, Reliable-Writer WP 1.1 retried. |
| 11 | Medium | Silent-Conversion | `tcp_transport.rs:275` | `u16::try_from(loc.port).ok()?` mappt Port-Overflow auf `UnsupportedLocator` — Debug hart. | Explizite Variante `InvalidPort` oder `tracing`. |
| 12 | Medium | Poisoned-Mutex | `tcp_transport.rs:245-247, 260-262` | Poisoning → `SendError::Io`; Transport bleibt unbrauchbar, kein Recovery. | Minimum: `tracing::error!`; Phase 2 `parking_lot::Mutex`. |
| 13 | Medium | Test-Gap | `tests/loopback.rs` | Keine Tests für partial writes, concurrent Sender, oversized-Frame von Peer, Peer-Disconnect mid-frame. | Multi-Thread-Stress (2 Sender × 1 Receiver × 1000 Frames); Mock via `mio`-Pipe für EAGAIN. |
| 14 | Low | Doc-Gap | `lib.rs:1-19` | Phase-2-Deferrals (IDENTITY_BIND_REQUEST, CONTROL-Channel, Endianness, Port 7400/7410) nur kurz in Framing-Doc. | `lib.rs`-Abschnitt "Phase-1 Scope vs DDS-TCP-PSM" mit Referenz §5.2.1, §6.3. |
| 15 | Low | Consistency | `tcp_transport.rs:166` / `wire_types.rs:555` | `Locator::new_tcp_v4` vs. `Locator::udp_v4` — Namensschema inkonsistent. | `tcp_v4` ohne `new_`-Prefix (Rust-idiomatisch). |
| 16 | Low | Readability | `tcp_transport.rs:67-70` | `checked_sub(3600s)`-Trick für "last_attempt lang her" nicht selbsterklärend. | `Option<Instant>` für `last_attempt`. |
| 17 | Low | Cleanup | `tcp_transport.rs:112-118` | Kein `stream.shutdown(Both)` vor Drop — FIN verzögert. | `let _ = stream.shutdown(Shutdown::Both);` vor `self.writer = None`. |
| 18 | Low | Spec | `framing.rs:30` | Relation zu WP-1.2-Fragmentation nicht dokumentiert. | Doc-Kommentar: Submessage-Ebene ≤ 16 MiB ausreichend, Reassembly über DATA_FRAG. |
| 19 | Low | Test-Gap | `tests/loopback.rs:101-109` | `unsupported_locator` testet nur UDPv4 — TCPv6/Invalid ungetestet. | Parametrisierter Test über alle `LocatorKind` ausser `Tcpv4`. |
| 20 | Low | Duplication | `wire_types.rs:555-563` | `new_tcp_v4` dupliziert Layout von `udp_v4`; wächst mit v6-Varianten. | Private Helper `fn with_address(kind, addr, port) -> Self`. |

## Delegation
- **#1 blockt WP 1.9** (Participant-Transport-Dispatch) — vor nächstem Merge schliessen.
- **#2–#5** sind Phase-1-Exit-Kriterien für Multi-Peer-Szenarien; für PoC/Interop reicht's.
- Control-Channel-Design (Phase 2) jetzt parallel skizzieren, damit Security-Plugins (WP 2+) Framer nicht erneut umbauen.

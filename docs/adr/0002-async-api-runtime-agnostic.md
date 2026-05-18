# 0002 — async-DDS-API runtime-agnostic mit Tokio-Glue als Optional

- **Status:** accepted
- **Datum:** 2026-05-04
- **Autoren:** @sandra
- **Kontext:** crates/dcps-async, docs/specs/zerodds-async-1.0.md

## Kontext

Das Tokio-Oekosystem dominiert (~85 % des Rust-async-Marktes). Eine
naive Lösung wäre `tokio` als Hard-Dep im async-Crate zu hinterlegen.
Das schliesst aber alle Caller aus, die `async-std`, `smol`,
`embassy` (no_std) oder eigene Reactor-Loops nutzen.

Auf der anderen Seite: ohne Tokio-spezifische Optimierungen
(`tokio::sync::Notify`, `tokio::time::sleep` mit Reactor-Wakeup statt
detached-thread) ist der Wakeup-Pfad messbar langsamer.

## Entscheidung

**`crates/dcps-async` ist runtime-agnostic by Default.**

- Public-API liefert `impl Future`/`impl Stream` ohne Tokio-Pin.
- Wakeup-Pfad nutzt einen detached-thread-basierten Sleep, der
  `cx.waker().clone()` aufruft. Das funktioniert unter jedem
  poll-basierten Reactor.
- Optional Feature `tokio-glue` schaltet auf `tokio::time::sleep`-
  basierte Wakeups um (kein detached-thread-Overhead).
- Keine `tokio`-Hard-Dep im Default-Cargo.toml — Workspace-CI bleibt
  schlank.

## Alternativen

1. **Tokio-Hard-Dep** — einfachste Implementation, aber excluded
   ~15 % des async-Markts. Verworfen.
2. **Feature-Set für mehrere Runtimes** (tokio + smol + async-std) —
   3-fache Maintenance + 3-fache Test-Surface. Verworfen.
3. **Pure futures-core + detached threads** (gewählt) — funktioniert
   überall, mit kleinem Performance-Tax bei jedem Wakeup; Tokio-Glue
   schaltet das ab.

## Konsequenzen

**Positiv**:
- `dcps-async` läuft mit jedem async-Reactor ohne Anpassung.
- Caller behält volle Kontrolle über seine Runtime.
- Workspace bleibt frei von tokio-Transitive-Deps im Default.

**Negativ**:
- Performance-Tax (detached-thread-Spawn) bei jedem Wakeup. Im
  Polling-Pfad (5-20 ms Tick) vernachlässigbar (~µs); im Hot-Path
  (1000+ writes/s) sichtbar.
- Caller, der maximalen Throughput will, muss `--features tokio-glue`
  explizit anschalten.

**Folge-Aufgaben**:
- Phase-2-A: `tokio-glue`-Feature aktiv für `yield_for` (bereits live).
- Phase-2-B: `spawn_in_tokio` für die DCPS-Tick-Loop.
- Bench: `criterion`-Vergleich mit/ohne Tokio-Glue.

## Referenzen

- `docs/specs/zerodds-async-1.0.md` D-1, D-2
- `crates/dcps-async/src/lib.rs::yield_for`
- Tokio-Doku: <https://tokio.rs/tokio/topics/bridging>

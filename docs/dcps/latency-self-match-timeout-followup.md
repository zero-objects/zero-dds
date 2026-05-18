# F-DCPS-latency-self-match-timeout — wait_for_matched_subscription Timeout auf CI

- **Status**: blocking (CI-Tests ignored)
- **Datum**: 2026-05-08
- **Sprint-Kontext**: D.5e Phase-1+2 perf-Commit (6a179dd) hat
  `crates/dcps/tests/latency_assertions.rs` eingefuehrt.
- **Reproduktion**: GitLab Pipeline 917 test-job 11737, Linux x86_64.
  `wait_for_matched_subscription(1, 5s)` timeoutet im
  Self-Match-Pfad eines einzigen `DomainParticipant` mit Pub+Sub
  intra-Process.

## Was ist offen

Beide Tests in `latency_assertions.rs` (`single_roundtrip_under_50ms`
und `sustained_roundtrip_no_loss_p99_under_100ms`) timeouten beim
ersten Sync-Punkt nach Endpoint-Erstellung. Tests sind via
`#![cfg(target_os = "linux")]` Linux-only — lokales
macOS-Reproducing nicht moeglich.

Vermutete Ursachen:
1. SEDP-Self-Match-Pfad (intra-Participant) braucht laenger als 5 s
   auf GitLab-Runner unter Last (cargo-Build laeuft zeitgleich,
   `--test-threads=1` serialisiert nichts gegen build-Threads).
2. Discovery-Tick-Period (im Test-Pfad: `tick_period 5ms`) kommt
   nicht zum Zug wegen Runner-CPU-Throttling.
3. `ignore_local_subscriptions/publications` aus Default-QoS koennte
   Self-Match unterdruecken.

## Warum offen

CDR-Agent-Scope deckt nicht die DCPS-Discovery-Self-Match-Pfade ab.
Memory K3a bestaetigt DCPS 1.4 als "voll abgeschlossen" mit 5044
gruenen Tests — der D.5e-Commit ist danach hinzugekommen und nie
auf CI durchgelaufen (Pipeline-Lints davor immer rot).

## Implikationen

- Latency-Regression-Gate ist nicht aktiv auf CI.
- Echte Performance wird nur via `roundtrip-typed`-Bench gemessen.
- Falls der Self-Match-Bug echt ist (nicht nur CI-timing), waeren
  intra-Process-PSM-Use-Cases (eingebettete Anwendungen) auch live
  betroffen.

## Wann pick-up sinnvoll

- Vor naechstem K3a-DCPS-Release-Tag.
- Sobald ein DCPS/Discovery-Owner-Sprint laeuft.

## Implementations-Pfad

1. Lokal in Linux-Container reproduzieren (`docker run rust:1.88-bookworm`).
2. RUST_LOG=trace + `tick_period`/`spdp_period` runter, sehen ob der
   match nur "spaet" oder gar nicht eintritt.
3. Wenn timing: tighter scheduling (`spdp_period 50ms`) und
   wait-timeout 30 s in CI; Open-Item closen.
4. Wenn echter Bug: SEDP-Self-Match-Pfad Repro im
   `crates/discovery/`-Test isolieren, Fix dort.
5. `#[ignore]` aus den beiden Tests entfernen.

Geschaetzt: 0.5–1.5 Tage (DCPS/Discovery-Owner-Domaene).

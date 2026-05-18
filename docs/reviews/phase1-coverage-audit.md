# Phase-1 Coverage Audit — Status vs. 99 % L / 95 % R

**Datum:** 2026-04-20 (Tag nach Round-3-Push).
**Quelle:** `cargo llvm-cov --workspace --summary-only` (161 Zeilen).

## Workspace-TOTAL

| Metric    | Round 3 (2026-04-20) | Heute       | Delta    | Ziel      | Gap        |
|-----------|----------------------|-------------|----------|-----------|------------|
| Regions   | 81.55 %              | **81.67 %** | +0.12 pp | 95 %      | **13.3 pp**|
| Functions | 92.27 %              | **92.34 %** | +0.07 pp | —         | —          |
| Lines     | 92.10 %              | **92.17 %** | +0.07 pp | 99 %      | **6.8 pp** |

Das Workspace-TOTAL hat sich seit Round 3 nicht nennenswert bewegt — die
Round-3-Tests sind stabil, neue WP-1.5 XTypes-Files (assignability, type_lookup,
builder, type_object/*) wurden in den Gesamt-Snapshot hineingewachsen und ziehen
R-Coverage zurueck. Der verbleibende Lines-Gap von 1 900 Lines verteilt sich
auf ~35 Files unterhalb der 95 %-L-Marke.

## Crates unterhalb 95 % R oder 99 % L (sortiert nach Lines-Missed)

| Crate                | Datei                                    | R %   | L %   | Top-3-Gap-Lines | Klassif. |
|----------------------|------------------------------------------|-------|-------|-----------------|----------|
| idl                  | `ast/builder.rs`                         | 66.29 | 76.51 | 389             | T+S      |
| idl                  | `ast/print.rs`                           | 57.98 | 82.26 | 110             | T        |
| rtps                 | `reliable_writer.rs`                     | 81.45 | 91.61 | 83              | T+S      |
| tools/idlc           | `main.rs`                                | 67.86 | 77.19 | 26              | T        |
| lint                 | `runner.rs`                              | 0     | 0     | 85              | T        |
| lint                 | `bin/zerodds-lint.rs`                        | 0     | 0     | 39              | T        |
| rtps                 | `subscription_data.rs`                   | 52.88 | 69.95 | 61              | T        |
| rtps                 | `reliable_reader.rs`                     | 79.38 | 88.28 | 58              | T+S      |
| rtps                 | `publication_data.rs`                    | 81.04 | 87.07 | 57              | T        |
| types                | `builder.rs`                             | 92.02 | 93.18 | 57              | T        |
| idl                  | `parser.rs`                              | 69.32 | 72.47 | 49              | T        |
| lint                 | `lints/bounded_recursion.rs`             | 66.67 | 77.99 | 46              | T        |
| types                | `type_identifier/mod.rs`                 | 81.46 | 96.17 | 24              | T        |
| types                | `resolve.rs`                             | 72.83 | 90.93 | 37              | T+S      |
| types                | `type_object/minimal/*`, `complete/*`    | 66–77 | 93–100| ~0 L-miss       | S (genericity) |
| transport-udp        | `udp_transport.rs`                       | 75.42 | 87.30 | 24              | T+S      |
| transport-tcp        | `framing.rs`                             | 77.19 | 83.70 | 15              | S (Mutex) |
| transport-shm        | `shm_transport.rs`                       | 81.61 | 87.12 | 21              | T+S      |
| rtps                 | `participant_data.rs`                    | 76.27 | 89.26 | 32              | T        |
| rtps                 | `datagram.rs`                            | 79.49 | 94.43 | 19              | T        |
| discovery/sedp       | `stack.rs`, `reader.rs`, `writer.rs`     | 52–79 | 77–89 | 77+59+29        | T+S      |
| tools                | `admin/dashboard/perf/traceability/xmlc` | 0     | 0     | 24              | D (stubs)|

**Legende:** T = mit Test erreichbar · S = strukturell unerreichbar (OS-Fehler,
Mutex-Poisoning, V6-auf-V4-Listener, `unreachable!`-Guards, Generic-Mono) ·
D = Dead/Stub-Code (Tool-`main.rs` mit leerem Body).

## Quick-Win-Tests (nicht geschrieben — nur skizziert)

1. `rtps/subscription_data.rs` — Roundtrip BE, DATA_REPRESENTATION non-empty,
   UnsupportedEncapsulation, zu kurzes Preamble, TYPE_NAME fehlt → **+ ~40 L**.
2. `lint/runner.rs` + `lints/mod.rs` — Dispatch-Test mit je 1 Lint-Regel, der
   `lint::runner::run_all()` gegen in-memory-File aufruft → **+85 L**.
3. `types/type_object/flags.rs` — `has()` + `empty()` fuer alle 10 Flag-Typen
   (UnionTypeFlag, EnumTypeFlag, BitmaskTypeFlag etc.) mit 1 Assertion je
   Variante → **+9 R, +9 L**.
4. `rtps/publication_data.rs` — RTI-Quirks (PID_TOPIC_DATA short-read,
   TYPE_INFORMATION Oversize, duplicate PID), **+ ~30 L**.
5. `idl/ast/print.rs` — `Display` fuer alle AST-Node-Varianten (Union mit
   default case, Enum mit default_literal, Bitmask, Typedef-Array-Dims),
   **+ ~80 L** (Tabellen-basierter Fixture-Roundtrip).

## Strukturell unerreichbar (Kandidaten fuer `#[coverage(off)]`)

- `transport-tcp/framing.rs` — `inbound lock poisoned`-Arms (3 Stellen),
  bereits in Round-3 als strukturell dokumentiert.
- `transport-udp/udp_transport.rs` — `SocketAddr::V6`-Arm nach V4-Bind,
  `set_multicast_loop_v4` auf Loopback-only-Listener.
- `types/type_object/{minimal,complete}/*.rs` — Serde-Wrapper-Regions, die
  durch Generic-Instantiation je Typ-Kategorie erzeugt werden, aber mit den
  existierenden Roundtrip-Tests bereits inhaltlich gedeckt sind. Lines sind
  i. d. R. 100 %, nur Regions zaehlen Mehrfach-Instanzen.
- `rtps/reliable_writer.rs` + `reliable_reader.rs` — Heartbeat-Timer-
  Double-Fire, `unreachable!()`-Guards fuer ReliabilityKind=BestEffort in
  Reliable-Pfaden, Timeout-Nanosekunden-Overflow.
- `idl/ast/builder.rs` — Engine-Invariant-Guards (`unreachable!` wenn
  CST leer, obwohl Grammar minlen>0 garantiert).

## Dead-Code-Kandidaten

- `tools/{admin,dashboard,perf,traceability,xmlc}/src/main.rs` — alle mit
  `fn main() { println!("stub"); }`-artigen Dummies. Entweder auf `no_run`
  `#[cfg(feature="bin")]`-Gate oder via `#[coverage(off)]` neutralisieren,
  sonst dauerhaft 0 %.
- `crates/lint/src/bin/zerodds-lint.rs` — echtes Binary, braucht Integration-Test
  mit `assert_cmd`, **nicht** dead.

## Realistisches Phase-1-Erreichbar-Szenario

Mit Quick-Wins (Punkte 1–5 oben) + `#[coverage(off)]` fuer strukturelle
Unerreichbarkeiten + Tool-Stubs hinter `cfg(...)`:

| Metric   | Heute   | Plausibles Phase-1-Ziel | Restliche Luecke |
|----------|---------|-------------------------|------------------|
| Regions  | 81.67 % | **~91 %**               | ~4 pp (generic mono) |
| Lines    | 92.17 % | **~97 %**               | ~2 pp (OS-errors)    |

Die **99 % L / 95 % R**-Messlatte ist in Phase 1 nur erreichbar, wenn
Generic-Monomorphisierungs-Regions via `#[coverage(off)]` am
Serde-Blanket-`impl` deaktiviert werden und die 6 Tool-`main.rs`-Stubs
entweder gelöscht oder mit Integrations-Tests versehen werden. Ohne diese
Struktur-Massnahmen landet der realistische Plateau-Wert bei ~97 % L / 91 % R.

## Nächste Schritte (Empfehlung, nicht umgesetzt)

1. `#[coverage(off)]` auf `derive(Serialize,Deserialize)`-Bodies in
   `type_object/{minimal,complete}/*.rs` evaluieren.
2. Tool-Stubs hinter `#[cfg(feature="bin")]` oder via
   `cargo llvm-cov --ignore-filename-regex 'tools/'` aus TOTAL ziehen.
3. Quick-Wins 1–3 (subscription_data, flags, lint/runner) als naechste
   gezielte Push-Runde — liefert geschaetzt +1.2 pp Lines-TOTAL in < 2 h.

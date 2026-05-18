# Coverage-Baseline

Stand: **2026-04-23**, Messung via `cargo llvm-cov --workspace` gegen
den ganzen Rust-Workspace.

## Ist-Zustand

| Metrik   | Workspace (alle Crates) | Runtime-Kern (siehe Ausschluss-Liste) |
|----------|------------------------:|--------------------------------------:|
| Lines    | 89.1 %                  | 89.7 %                                |
| Regions  | 79.0 %                  | 79.5 %                                |
| Functions| 88.5 %                  | 89.3 %                                |

**Ziel gemaess Memory / Phase-0-Plan**: 99 % Branch-Coverage. Das Ziel
ist bewusst aspirational; es gilt pro Crate nach individueller
Stabilisierung. Der Workspace-Schnitt bleibt realistisch zwischen
88-92 %, solange neue Crates hinzukommen und noch nicht durchtestet
sind.

## Coverage-Floor-Hard-Fail (CI-Welle 2026-05-01)

Der `coverage`-Job in `.gitlab-ci.yml` exit-codet non-zero, wenn
Coverage unter konfigurierte Thresholds faellt. Defaults gewaehlt
~4-5 % unter dem Ist-Stand, damit normale Fluktuation passt aber
echte Regressionen (>5 % drop) gefangen werden:

| Metrik    | Floor (default) | Ist (2026-04-23) | Buffer |
|-----------|----------------:|-----------------:|-------:|
| Lines     | 85 %            | 89.1 %           | 4.1 pp |
| Regions   | 75 %            | 79.0 %           | 4.0 pp |
| Functions | 85 %            | 88.5 %           | 3.5 pp |

Override per Pipeline-Variable:
* `COVERAGE_FLOOR_LINES`
* `COVERAGE_FLOOR_REGIONS`
* `COVERAGE_FLOOR_FUNCTIONS`

Bei Anhebung des Ist-Standes: Floors entsprechend nachziehen, damit
der Buffer konstant bleibt. CI-Image-Update + lokale `cargo llvm-cov
--workspace --summary-only` als Referenz-Messung.

## Ausschluss-Liste (CI-Default)

Im `coverage`-Job der `.gitlab-ci.yml` werden folgende Pfade per
`--ignore-filename-regex` ausgeklammert:

| Pfad                 | Begruendung                                              |
|----------------------|----------------------------------------------------------|
| `tools/`             | Binary-Crates. `main()` wird nicht als Test aufgerufen. |
| `crates/sys/`        | Platzhalter fuer Low-Level-FFI (Bootstrap leer).         |
| `crates/xml/`        | Platzhalter — Implementierung folgt mit WP 4.2-c.        |
| `crates/xrce-*/`     | XRCE-DDS (eXtremely Resource Constrained) — Phase-2.     |
| `crates/recorder/`   | Recorder-Daemon — Phase-2.                               |
| `crates/monitor/`    | Monitor-Daemon — Phase-2.                                |
| `crates/cpp/`        | C++-Binding-Stubs — WP 4.10.                             |
| `crates/cs/`         | C#-Binding-Stubs — v1.5.                                 |
| `crates/java/`       | Java-Binding-Stubs — v1.5.                               |
| `crates/rpc/`        | DDS-RPC-Platzhalter — v1.5.                              |
| `crates/transport-tcp/` | TCP-Transport — Phase-2, noch nicht in Produktionspfad. |

Diese Crates enthalten entweder nur einen Platzhalter-`lib.rs` oder
stehen voll in einer spaeteren Release-Stufe. Wenn sie produktiv
werden, werden sie aus der Ausschluss-Liste entfernt.

## Schwach-Coverage-Crates (Runtime)

Diese Crates haben aktuell < 90 % Lines-Coverage und brauchen
gezielte Test-Nachbesserung (nicht in v1.3/v1.4-Kritisch-Pfad):

| Crate / Datei                        | Lines | Region | Priorität |
|--------------------------------------|------:|-------:|-----------|
| `crates/sql-filter/src/evaluator.rs` | 88 %  | 80 %   | M (WP 3.7d) |
| `crates/sql-filter/src/parser.rs`    | 87 %  | 82 %   | M           |
| `crates/sql-filter/src/lexer.rs`     | 91 %  | 86 %   | L           |
| `crates/transport-udp/`              | 87 %  | 75 %   | H (hot path)|
| `crates/transport-shm/posix.rs`      | 90 %  | 85 %   | M           |
| `crates/types/type_object/*`         | 92-100 % | 60-77 % | M       |
| `crates/types/resolve.rs`            | 91 %  | 73 %   | M           |

## Python-Coverage

Seit **WP 3.10b**-Pipeline-Update (2026-04-23) wird die Python-Seite
via `pytest --cov=zerodds` im `python-tests`-Job gemessen; Cobertura-
XML wird als Coverage-Report an GitLab uebergeben, XML-Artefakt fuer
30 Tage aufbewahrt.

Scope der Python-Messung:
* `crates/py/python/zerodds/__init__.py`
* `crates/py/python/zerodds/cdr.py`
* `crates/py/python/zerodds/idl.py`

**Nicht** enthalten: das Rust-Extension-Module `zerodds._core` —
dessen Pfade werden im Rust-`coverage`-Job gemessen.

## Wie man Coverage lokal reproduziert

```bash
# Volle Workspace-Messung (bin.-crates inklusive):
cargo llvm-cov --workspace --summary-only

# CI-aequivalent (mit Ausschluss-Liste):
cargo llvm-cov --workspace \
  --ignore-filename-regex 'tools/|crates/sys/|crates/xml/|crates/xrce-|crates/recorder/|crates/monitor/|crates/cpp/|crates/cs/|crates/java/|crates/rpc/|crates/transport-tcp/' \
  --summary-only

# HTML-Report pro Crate:
cargo llvm-cov --html --open
```

## Roadmap

* **v1.4** — Runtime-Crates auf ≥ 95 % Lines bringen (gezielt
  `transport-udp`, `types/type_object/*`, `sql-filter`).
* **v1.5** — Branch-Coverage als Enforced-Gate (zusaetzlich zu
  Lines). `cargo llvm-cov` unterstuetzt `--show-missing-lines`.
* **v2.0** — Safety-Crates (`dcps`, `rtps`, `discovery`,
  `security-*`) auf 99 % heben fuer Zertifizierungs-Vorbereitung.

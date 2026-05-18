# Changelog

Format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer 2.0](https://semver.org/).

## [Unreleased]

Vorbereitung auf `1.0.0-rc.3`. Wesentliche Aenderung gegenueber dem
rc.1-Eintrag unten: das CLI hat **flag-style** statt
**subcommand-style** API. Begruendung: bei einem Compiler mit genau
einem Input und genau einem Output bringt die `generate --lang <x>`-
Indirection keinen Mehrwert, und der `clap`-Build-Zeit-Overhead war
fuer ~10 Flags nicht zu rechtfertigen. Der hand-geschriebene Parser
spart die Dependency komplett.

### Added — alle sieben Sprach-Backends im CLI

| Flag | Output | Backend-Library |
| --- | --- | --- |
| `--c` | `<dir>/<base>.h` | `zerodds-idl-cpp::c_mode` (re-exportiert) |
| `--cpp` | `<dir>/<base>.hpp` | `zerodds-idl-cpp` |
| `--rust` | `<dir>/<base>.rs` | `zerodds-idl-rust` |
| `--ts` | `<dir>/<base>.ts` | `zerodds-idl-ts` |
| `--csharp` | `<dir>/<base>.cs` | `zerodds-idl-csharp` |
| `--java` | `<dir>/<pkg/path>/<Class>.java` | `zerodds-idl-java` (multi-file) |
| `--python` | `<dir>/<base>.py` | `zerodds-idl-python` (neu — Phase 1 + 2) |

Mehrfach-Backend-Aufruf wird mit klarer Usage-Message zurueckgewiesen
(`multiple backends selected; pick one of …`). `--parse-only` und
Backend-Flags sind mutually exclusive.

### Changed

- CLI-API ist Flag-driven, nicht Subcommand-driven. Die in rc.1
  geplanten `generate`/`check`/`dump-ast`-Subcommands sind durch
  `--rust`/`--cpp`/`--parse-only`/etc. ersetzt — funktional identisch,
  weniger Argument-Verschachtelung.
- Eigener CLI-Parser statt `clap`. Reduziert die Dep-Liste auf
  `zerodds-idl` + die sieben `zerodds-idl-*`-Backend-Crates.
- `-o <dir>` ist Pflicht fuer alle Backend-Modi; `--parse-only` schreibt
  weiter nach stdout.

### Exit-Codes (unveraendert gegenueber rc.1-Plan)

- `0` Erfolg
- `1` Parse-Fehler (Lex / Recognize / Build)
- `2` CLI- / IO-Fehler (Args, Datei nicht lesbar, fehlendes `-o`)
- `3` Backend-Fehler oder Feature nicht implementiert

### Tests

18 Unit-Tests in `src/main.rs`: ein Erfolgsfall pro Backend mit
Symbol-Verifikation (z.B. `class Greeting` im C++-Output,
`pub struct` im Rust-Output, `@idl_struct` im Python-Output) plus
CLI-Edge-Cases (mutually-exclusive, missing-output, unknown flag,
multiple-backends).

## [1.0.0-rc.1] — 2026-05-07

Initial Release Candidate for the `zerodds-idlc` IDL compiler.

### Subcommands

- `zerodds-idlc generate --lang <lang> <input.idl>` — generate code for
  Rust, C, C++, C#, Java, Python, or TypeScript from an OMG IDL 4.2 input.
- `zerodds-idlc check <input.idl>` — parse and validate an IDL file
  without emitting code.
- `zerodds-idlc dump-ast <input.idl>` — emit the parsed AST as JSON for
  external tooling.

### Spec References

- OMG IDL 4.2 (`formal/2018-01-05`) §7 — IDL syntax and semantics
- DDS-XTypes 1.3 (`formal/2024-04-01`) §7.4 — language-mapping rules
- Per-language PSM specs: DDS-PSM-Cxx 1.0, DDS-Java-PSM 1.0,
  ZeroDDS-IDL-Rust 1.0, ZeroDDS-XCDR2-{C,Csharp,Java,Rust,TS} 1.0

### Implementation

Multi-pass compiler: lexer → parser (LALR) → AST → resolver →
type-checker → backend dispatch. Each backend is a separate
`crates/idl-<lang>/` crate; `idlc` orchestrates them.

### Architecture

- Layer: Tools
- Dependencies: `zerodds-idl`, `zerodds-idl-{cpp,csharp,java,rust,ts}`,
  `clap` (CLI)

### Stability

CLI surface is stable for `1.0.x`. New language backends are additive
minor bumps. Generated-code surface follows each backend crate's
own SemVer commitment.

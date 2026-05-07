# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1]

Initiale Release-Materialisierung der `zerodds-lint`-Crate (internal tooling).

### Spec-Referenzen

- `docs/architecture/04_safety_by_architecture.md §3.4` — Spezifikation aller 7 Projekt-Lints.

### Lint-Inventar

7 aktiv registrierte Lints:

**File-Lints (6):**

- `RequireSafetyComment` — erzwingt Safety-Kommentar bei jeder `unsafe`-Verwendung.
- `NoDynInSafe` — verbietet `dyn Trait` in Safe-Class-Crates.
- `NoPanicInSafe` — verbietet `panic!`/`unreachable!`/`todo!`/`unimplemented!` in Safe-Class.
- `NoAllocInHotPath` — verbietet Heap-Alloc in `// zerodds-lint: hot-path`-Funktionen.
- `NoReallocInHotPath` — verbietet `Vec`-Reallocs in Hot-Path.
- `BoundedRecursion` — erzwingt `// zerodds-lint: recursion-depth N`-Annotation an rekursiven Funktionen.

**Crate-Lint (1):**

- `SafetyClassificationPresent` — erzwingt `Safety classification:`-Block im `lib.rs`-Crate-Doc.

### Public-API

**Aufruf-Pfad** (`runner`-Modul):

- `RunConfig` / `RunReport` — Config/Report-Container.
- `run(&RunConfig) -> Result<RunReport>` — Hauptentry.
- `locate_workspace_root(&Path) -> Result<PathBuf>` — Workspace-Discovery.

**Klassifikation** (`classification`-Modul):

- `SafetyClass::{Safe, Standard, Qualified, Tooling, Unclassified}`.
- `parse_from_lib_rs(&str) -> Option<SafetyClass>` / `read_from_file(&Path)`.

**Workspace-Scan** (`scanner`-Modul):

- `CrateInfo { name, root, lib_rs, src_files, safety_class }`.
- `scan_workspace(&Path) -> Result<Vec<CrateInfo>>` — `cargo_metadata` + `walkdir`-basiert.

**Diagnostik** (`diagnostic`-Modul):

- `Severity::{Error, Warning}`.
- `Diagnostic { file, line, column, lint, severity, message }` mit `Display`-Impl.
- `Diagnostic::error(...)` / `Diagnostic::warning(...)` Constructor-Methoden.

**Lint-Trait-Familie** (`lints`-Modul):

- `FileLint`-Trait + `FileLintContext<'a>` (file-scoped Lints).
- `CrateLint`-Trait (Crate-scoped Lints).
- `default_file_lints()` / `default_crate_lints()` — Default-Listen.

### Implementierung

- `forbid(unsafe_code)`.
- AST-Parsing via `syn` (stable Rust, kein Nightly).
- Workspace-Walk via `cargo_metadata` + `walkdir`.
- 67 Unit-Tests grün.

### CI-Integration

- GitLab CI: `zerodds-lint` job in `.gitlab-ci.yml`.
- Pre-commit hook: `scripts/pre-commit.sh`.

### Public-Strategy

🏠 **internal-only** — wird nicht nach crates.io gepusht. `publish = false` in `Cargo.toml`.

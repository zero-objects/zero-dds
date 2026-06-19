# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1]

Initial release materialization der `zerodds-lint`-Crate (internal tooling).

### Spec-Referenzen

- `docs/architecture/04_safety_by_architecture.md §3.4` — specification of all 7 project lints.

### Lint-Inventar

7 aktiv registrierte Lints:

**File-Lints (6):**

- `RequireSafetyComment` — enforces a safety comment on every `unsafe` use.
- `NoDynInSafe` — forbids `dyn Trait` in safe-class crates.
- `NoPanicInSafe` — forbids `panic!`/`unreachable!`/`todo!`/`unimplemented!` in safe-class.
- `NoAllocInHotPath` — forbids heap alloc in `// zerodds-lint: hot-path` functions.
- `NoReallocInHotPath` — forbids `Vec` reallocs in the hot path.
- `BoundedRecursion` — enforces a `// zerodds-lint: recursion-depth N` annotation on recursive functions.

**Crate-Lint (1):**

- `SafetyClassificationPresent` — enforces a `Safety classification:` block in the `lib.rs` crate doc.

### Public-API

**Invocation path** (`runner` module):

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
- `Diagnostic::error(...)` / `Diagnostic::warning(...)` constructor methods.

**Lint trait family** (`lints` module):

- `FileLint` trait + `FileLintContext<'a>` (file-scoped lints).
- `CrateLint` trait (crate-scoped lints).
- `default_file_lints()` / `default_crate_lints()` — default lists.

### Implementation

- `forbid(unsafe_code)`.
- AST parsing via `syn` (stable Rust, no Nightly).
- Workspace-Walk via `cargo_metadata` + `walkdir`.
- 67 unit tests green.

### CI-Integration

- GitLab CI: `zerodds-lint` job in `.gitlab-ci.yml`.
- Pre-commit hook: `scripts/pre-commit.sh`.

### Public-Strategy

🏠 **internal-only** — not pushed to crates.io. `publish = false` in `Cargo.toml`.

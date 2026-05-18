# RC1 Review — `zerodds-lint`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md` (DoD + Forbidden-Tokens + Public-Strategy).
> **Layer:** 1 (Primitives — Tooling)
> **Reviewer:** Claude
> **Public-Strategy:** 🏠 internal-only

---

## 1 Purpose

Custom-Lint-Runner für ZeroDDS, AST-basiert auf stable Rust (kein Nightly, kein dylint). Erzwingt 7 Projekt-Lints workspace-weit; läuft in GitLab CI und im pre-commit-Hook. Safety classification: TOOLING.

## 2 Public-Strategy

- **Marker:** 🏠 internal-only
- **Begründung:** Tooling-Crate für ZeroDDS-internen Lint-Enforcement. Nicht für externe Konsumenten gedacht (bis Phase-2, falls überhaupt). Kein crates.io-Push, kein github/-Mirror, kein website/-Doc-Page.

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs                          # Crate-Entry, Public-API-Aggregator
├── classification.rs               # SafetyClass-Parser für lib.rs-Doc
├── diagnostic.rs                   # Diagnostic + Severity + Display-Impl
├── runner.rs                       # Orchestrator (run / RunConfig / RunReport)
├── scanner.rs                      # Workspace-Walk (cargo_metadata + walkdir)
├── lints/
│   ├── mod.rs                      # FileLint + CrateLint Trait + Default-Listen
│   ├── bounded_recursion.rs        # dds_bounded_recursion
│   ├── no_alloc_in_hot_path.rs     # dds_no_alloc_in_hot_path
│   ├── no_dyn_in_safe.rs           # dds_no_dyn_in_safe
│   ├── no_panic_in_safe.rs         # dds_no_panic_in_safe
│   ├── no_realloc_in_hot_path.rs   # dds_no_realloc_in_hot_path
│   ├── require_safety_comment.rs   # dds_require_safety_comment
│   └── safety_classification_present.rs  # dds_safety_classification_present
└── bin/
    └── zerodds-lint.rs                 # CLI-Binary (clap + ExitCode)
```

### 3.2 Public-API-Surface

```rust
// classification
pub enum SafetyClass { Safe, Standard, Qualified, Tooling, Unclassified }
pub fn parse_from_lib_rs(content: &str) -> Option<SafetyClass>;
pub fn read_from_file(path: &Path) -> std::io::Result<Option<SafetyClass>>;

// scanner
pub struct CrateInfo { name, root, lib_rs, src_files, safety_class };
pub fn scan_workspace(root: &Path) -> Result<Vec<CrateInfo>>;

// diagnostic
pub enum Severity { Error, Warning }
pub struct Diagnostic { file, line, column, lint, severity, message };
impl Diagnostic {
    pub fn error(file, line, column, lint, message) -> Self;
    pub fn warning(file, line, column, lint, message) -> Self;
}

// lints
pub trait FileLint { fn name() -> &'static str; fn run(&self, ctx: &mut FileLintContext); }
pub trait CrateLint { fn name() -> &'static str; fn run(&self, info: &CrateInfo) -> Vec<Diagnostic>; }
pub struct FileLintContext<'a> { ... };
pub fn default_file_lints() -> Vec<Box<dyn FileLint>>;     // 6 Lints
pub fn default_crate_lints() -> Vec<Box<dyn CrateLint>>;   // 1 Lint

// runner
pub struct RunConfig { root, fail_on_warning };
pub struct RunReport { errors, warnings, scanned_crates, scanned_files };
pub fn run(cfg: &RunConfig) -> Result<RunReport>;
pub fn locate_workspace_root(start: &Path) -> Result<PathBuf>;
```

### 3.3 Tests

- `cargo test -p zerodds-lint`: ✅ 67 unit-tests grün.
- E2E via Production-Self-Test: `cargo run --bin zerodds-lint -- check` ergibt **0 errors / 0 warnings** auf 105 Crates / 1014 Files.

### 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

| Public-Item | Spec-Anker | External Production-Refs | Klassifikation | Decision |
|---|---|---|---|---|
| `SafetyClass` + `parse_from_lib_rs` + `read_from_file` | `04_safety_by_architecture.md §3` | 0 (Tooling-internal) | INTERNAL | — (intern wired in 6 Files) |
| `CrateInfo` + `scan_workspace` | (interner Workspace-Walk) | 0 (Tooling-internal) | INTERNAL | — |
| `Diagnostic` + `Severity` + `error`/`warning` | (interner Output-Pfad) | 0 (Tooling-internal) | INTERNAL | — (10 Lint-Sites pro Diagnostic) |
| `FileLint` / `CrateLint` / `FileLintContext` | `04_safety_by_architecture.md §3.4` | 0 (Tooling-internal) | INTERNAL | — |
| `default_file_lints` + `default_crate_lints` | (Lint-Registry) | 0 (Tooling-internal, vom CLI gerufen) | INTERNAL | — |
| `RunConfig` / `RunReport` / `run` / `locate_workspace_root` | (Top-Level-Entry) | 0 (CLI-Binary `zerodds-lint.rs` ruft `run()`) | INTERNAL | — |
| 7 Lint-Structs (`RequireSafetyComment`, `NoDynInSafe`, `NoPanicInSafe`, `NoAllocInHotPath`, `NoReallocInHotPath`, `BoundedRecursion`, `SafetyClassificationPresent`) | `04_safety_by_architecture.md §3.4` | 0 (alle in `lints/mod.rs::default_*_lints` registriert) | CONNECTED via registry | — |

**Befund:** keine losen Enden. Alle pub-Items sind via Registry oder Trait-Hierarchie wired. Tooling-internal — die "0 external Production-Refs" sind erwartet (kein Library-Konsument, nur CLI-Binary).

### 3.4.1 Sweep-Verifikation (§1.5b Pass 2)

`/tmp/zerodds-audit/lint.tsv` enthält 23 Public-Items: SafetyClass +
parse_from_lib_rs + read_from_file + CrateInfo + scan_workspace +
Diagnostic + Severity + error + warning + FileLint + CrateLint +
FileLintContext + default_file_lints + default_crate_lints +
RunConfig + RunReport + run + locate_workspace_root + 7 Lint-Structs.
Alle in der Tabelle oben durch Family-Rows abgedeckt. **0 DEAD.**

Internal-only Crate (`🏠`) — Coherence-Audit-Erwartung ist "0 ext refs",
da kein Public-API für End-User. Audit-Pass ist trivial-grün.

## 4 Wiring

### 4.1 Dependencies

```toml
[dependencies]
syn = { version = "2", features = ["full", "visit", "extra-traits"] }
proc-macro2 = { version = "1", features = ["span-locations"] }
cargo_metadata = "0.18"
walkdir = "2"
anyhow = "1"
clap = { version = "4", features = ["derive"] }
```

### 4.2 Konsumenten

- `.gitlab-ci.yml::zerodds-lint` job (`cargo run -p zerodds-lint -- check`).
- `scripts/pre-commit.sh` (lokaler Hook vor jedem `git commit`).
- 163 Annotation-Lines (`// zerodds-lint: ...`) in Production-Code für Allow-/Hot-Path-/Recursion-Markierungen.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| (keine) | — | Tooling-Crate ohne Feature-Flags |

## 5 Spec-Relevanz

- **Spec(s):** intern — `docs/architecture/04_safety_by_architecture.md §3.4`.
- **Coverage-Doc:** keine (Tooling, keine OMG-Spec).

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

**Treffer:** keine.

### 6.2 Soft-Review (TODO/FIXME)

**Treffer:** keine echten — die `todo!`-Matches in `no_panic_in_safe.rs` + `no_dyn_in_safe.rs` sind Test-Fixtures, die das Lint-Verhalten gegen das `todo!()`-Makro verifizieren.

### 6.3 Tech-Debt + Loose Ends

- **Keine.** Alle 7 Lint-Structs sind via `default_file_lints` / `default_crate_lints` registriert; Production-Self-Test (`cargo run --bin zerodds-lint -- check`) läuft sauber durch 105 Crates ohne Fehler.

### 6.4 Public-API-Leaks

- Keine Glob-Reexports.
- Keine ungewollt `pub`-markierten Helper.

## 7 Cleanup-Actions

1. SPDX-License-Header in alle 14 `src/*.rs`-Files (incl. `lints/*.rs` + `bin/*.rs`) eingefügt.
2. `src/lib.rs` Crate-Header erweitert: explizite Public-API-Aufzählung mit Lint-Inventar (7 Lints), Schichten-Position, CI-Integration-Notiz.
3. `README.md` neu geschrieben: Lint-Tabelle (7 Lints + Scope + Zweck), CLI-Beispiele, Annotations-Beispiele, CI-Integration.
4. `CHANGELOG.md` neu angelegt mit `[1.0.0-rc.1]`-Initial-Release-Entry.

Cargo.toml bleibt unverändert: `publish = false` ist korrekt (internal-only); `homepage`/`documentation`/`keywords`/`categories` entfallen für internal-only Crates.

## 8 Spec-Doc-Updates

Keine — `zerodds-lint` referenziert nur die interne `04_safety_by_architecture.md §3.4` und nicht eine OMG-Spec.

## 9 Doc-Artefacts

- [x] `Cargo.toml`-Metadata vollständig (für internal-only Crate)
- [x] `lib.rs`-Crate-Header mit Safety-Class + Layer + Public-API-Aufzählung
- [x] `README.md`
- [x] `CHANGELOG.md` mit `[1.0.0-rc.1]`
- [n/a] doc-tested Code-Example (Tooling — CLI-Output statt API-Beispiel)

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-lint                                  # ✅ 67 Tests grün
cargo clippy -p zerodds-lint --all-targets -- -D warnings   # ✅ clean
cargo fmt -p zerodds-lint -- --check                        # ✅ clean
cargo doc -p zerodds-lint --no-deps                         # ✅ clean
cargo run --bin zerodds-lint -- check                       # ✅ 105 Crates / 1014 Files / 0 errors / 0 warnings
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata (für internal-only)
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md
- [x] §1.4 CHANGELOG.md
- [x] §1.5 Public-API-Audit
- [x] §1.5b Coherence-Audit (siehe §3.4 — keine Loose-Ends)
- [n/a] §1.6 Spec-Coverage-Update (kein OMG-Spec-Bezug)
- [x] §1.7 Forbidden-Token-Sweep (keine Treffer)
- [x] §1.8 License-Header pro File
- [x] §1.9 Tests + Lints + Doc-Build grün
- [x] §1.10 Review-Doc ausgefüllt
- [x] §1.11 Tracker auf ✅
- [n/a] §1.12 Public-Mirror-Artifacts (internal-only, kein github/website)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer-Sign-off:** Claude
- **Tracker-Eintrag aktualisiert:** ✅

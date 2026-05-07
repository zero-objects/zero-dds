# `zerodds-lint`

> **Internal Tooling — nicht auf crates.io.**

Custom-Lints und Projekt-Regeln für ZeroDDS, AST-basiert auf stable Rust (kein Nightly, kein dylint). Wird in CI und im pre-commit-Hook ausgeführt; erzwingt **0 errors / 0 warnings workspace-weit** als RC1-DoD-Gate.

Safety classification: **TOOLING**. Part of [**ZeroDDS**](../../README.md).

## Lints (7 aktiv)

| Lint | Scope | Zweck |
|------|-------|-------|
| `dds_require_safety_comment` | File | Erzwingt Safety-Kommentar bei jeder `unsafe`-Verwendung. |
| `dds_no_dyn_in_safe` | File | Verbietet `dyn Trait` in Safe-Class-Crates (Vtable-Indirektion). |
| `dds_no_panic_in_safe` | File | Verbietet `panic!`/`unreachable!`/`todo!`/`unimplemented!` in Safe-Class. |
| `dds_no_alloc_in_hot_path` | File | Verbietet `Vec::new`/`Box::new`/`String::from` in `// zerodds-lint: hot-path`-Funktionen. |
| `dds_no_realloc_in_hot_path` | File | Verbietet `vec.push`/`vec.extend` in Hot-Path (Reallocations). |
| `dds_bounded_recursion` | File | Erzwingt `// zerodds-lint: recursion-depth N` Annotation an rekursiven Funktionen. |
| `dds_safety_classification_present` | Crate | Erzwingt `Safety classification: <KLASSE>` im `lib.rs`-Crate-Doc. |

Vollständige Spec der Lints: `docs/architecture/04_safety_by_architecture.md §3.4`.

## CLI

```bash
cargo run -p zerodds-lint -- check                 # Workspace-weit
cargo run -p zerodds-lint -- check --root <path>   # Custom-Root
cargo run -p zerodds-lint -- check --fail-on-warning   # Strict-Mode
```

## Annotations (in production code)

```rust
// zerodds-lint: allow no_dyn_in_safe
fn callback(handler: Box<dyn Fn()>) { ... }

// zerodds-lint: hot-path
fn write_user_sample(...) { ... }

// zerodds-lint: recursion-depth 16
fn parse_module(module: &Module) { ... }
```

## CI-Integration

- GitLab CI: `.gitlab-ci.yml::zerodds-lint` job läuft `cargo run -p zerodds-lint -- check`.
- Pre-commit hook: `scripts/pre-commit.sh` ruft denselben Command vor jedem `git commit`.

## Tests

```bash
cargo test -p zerodds-lint    # 67 Unit-Tests
```

## See also

- [`docs/architecture/04_safety_by_architecture.md §3.4`](../../docs/architecture/04_safety_by_architecture.md) — Lint-Spezifikationen.
- [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) — Schichten-Architektur.

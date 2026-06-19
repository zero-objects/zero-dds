# `zerodds-lint`

> **Internal tooling — not on crates.io.**

Custom lints and project rules for ZeroDDS, AST-based on stable Rust (no Nightly, no dylint). Runs in CI and the pre-commit hook; enforces **0 errors / 0 warnings workspace-wide** as the RC1 DoD gate.

Safety classification: **TOOLING**. Part of [**ZeroDDS**](../../README.md).

## Lints (7 active)

| Lint | Scope | Purpose |
|------|-------|-------|
| `dds_require_safety_comment` | File | Enforces a safety comment on every `unsafe` use. |
| `dds_no_dyn_in_safe` | File | Forbids `dyn Trait` in safe-class crates (vtable indirection). |
| `dds_no_panic_in_safe` | File | Forbids `panic!`/`unreachable!`/`todo!`/`unimplemented!` in safe-class. |
| `dds_no_alloc_in_hot_path` | File | Forbids `Vec::new`/`Box::new`/`String::from` in `// zerodds-lint: hot-path` functions. |
| `dds_no_realloc_in_hot_path` | File | Forbids `vec.push`/`vec.extend` in the hot path (reallocations). |
| `dds_bounded_recursion` | File | Enforces a `// zerodds-lint: recursion-depth N` annotation on recursive functions. |
| `dds_safety_classification_present` | Crate | Enforces `Safety classification: <CLASS>` in the `lib.rs` crate doc. |

Full spec of the lints: `docs/architecture/04_safety_by_architecture.md §3.4`.

## CLI

```bash
cargo run -p zerodds-lint -- check                 # workspace-wide
cargo run -p zerodds-lint -- check --root <path>   # custom root
cargo run -p zerodds-lint -- check --fail-on-warning   # strict mode
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

- GitLab CI: the `.gitlab-ci.yml::zerodds-lint` job runs `cargo run -p zerodds-lint -- check`.
- Pre-commit hook: `scripts/pre-commit.sh` invokes the same command before every `git commit`.

## Tests

```bash
cargo test -p zerodds-lint    # 67 unit tests
```

## See also

- [`docs/architecture/04_safety_by_architecture.md §3.4`](../../docs/architecture/04_safety_by_architecture.md) — lint specifications.
- [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) — layered architecture.

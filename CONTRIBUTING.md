# Contributing to ZeroDDS

Thank you for your interest in ZeroDDS. This guide describes how to
file issues, submit pull requests, and align with the project's
quality bar.

ZeroDDS is licensed Apache-2.0 and welcomes external contributions
beginning with the `1.0.0-rc.1` release.

## Code of Conduct

This project follows the [Contributor Covenant 2.1](CODE_OF_CONDUCT.md).
By participating you agree to uphold its standards. Report unacceptable
behavior to `conduct@zerodds.org`.

## Developer Certificate of Origin (DCO)

Every commit must be signed off with a DCO line:

```
Signed-off-by: Your Name <you@example.org>
```

Add automatically with `git commit -s`. By signing off, you certify the
DCO 1.1 terms (<https://developercertificate.org/>) — that you have the
right to submit the contribution under the project's Apache-2.0 license.

## Filing Issues

- **Bugs** — include reproduction steps, expected vs. actual, ZeroDDS
  version, OS + architecture, and a minimal `Cargo.toml` if applicable.
- **Feature requests** — describe the use case, the problem you're
  solving, and (if known) the relevant OMG spec section.
- **Security vulnerabilities** — see [SECURITY.md](SECURITY.md) and do
  **not** open a public issue.

## Pull Request Workflow

1. Fork the repository and create a topic branch:
   `feat/<area>-<short-description>` or `fix/<area>-<short-description>`.
2. Make focused commits with [Conventional Commits](https://www.conventionalcommits.org/)
   syntax:

   ```
   <type>(<scope>): <description>

   [optional body]
   [optional footer]
   ```

   Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`,
   `build`, `ci`, `chore`. Scopes are crate names or layer references.

3. Add tests for the change. New features require unit tests; bug fixes
   require regression tests.
4. Run the local pre-flight checks (see "Pre-Push Checks" below).
5. Open a PR against `main`. Fill in the PR template.

## Review Expectations

- **One required reviewer** for non-safety changes.
- **Two required reviewers** for changes touching `dds-security`,
  `bridge-security`, `crypto`, or any Safety classification > `STANDARD`.
- **Spec changes** (anything under `docs/specs/`) require sign-off from
  the spec maintainer.
- All CI jobs must be green; failing jobs are not bypassable.
- Pull requests are squashed into a single commit on merge.

## Layer Discipline

Each crate carries a layer assignment (0–8) in its `lib.rs` header.
Cross-layer dependencies must follow the layering rules:

- A layer-N crate may depend on layers 0 through N-1.
- Reverse or peer dependencies require an ADR documenting the rationale.
- The dependency DAG is enforced by `cargo run -p dds-lint -- check`.

## Build Prerequisites

A workspace build links the **system** `libduckdb` (the `zerodds-durability-store-lakehouse`
cold adapter, ADR 0009). The `bundled` feature — which compiles the full DuckDB C++
amalgamation from source — was dropped because it OOM-killed CI and added ~1h per build,
so `cargo build --workspace` now requires `libduckdb` to be installed locally.

Use the version that matches the `duckdb` crate's ABI (currently **v1.5.3**); a mismatch
fails to link. Releases: <https://github.com/duckdb/duckdb/releases>.

**Linux (x86_64):**

```bash
curl -fsSL -o /tmp/libduckdb.zip \
  https://github.com/duckdb/duckdb/releases/download/v1.5.3/libduckdb-linux-amd64.zip
unzip -o /tmp/libduckdb.zip -d /tmp/duckdb
sudo install -m644 /tmp/duckdb/libduckdb.so /usr/local/lib/
sudo install -m644 /tmp/duckdb/duckdb.h /usr/local/include/
sudo ldconfig
```

**macOS (Apple Silicon):**

```bash
curl -fsSL -o /tmp/libduckdb.zip \
  https://github.com/duckdb/duckdb/releases/download/v1.5.3/libduckdb-osx-universal.zip
unzip -o /tmp/libduckdb.zip -d /tmp/duckdb
install -m644 /tmp/duckdb/libduckdb.dylib /opt/homebrew/lib/
install -m644 /tmp/duckdb/duckdb.h /opt/homebrew/include/
```

Then point the `duckdb` crate at the install via Cargo's global `~/.cargo/config.toml`
(shell-independent — applies to every `cargo` invocation):

```toml
[env]
# Linux:
DUCKDB_LIB_DIR = "/usr/local/lib"
DUCKDB_INCLUDE_DIR = "/usr/local/include"
# macOS (Apple Silicon): "/opt/homebrew/lib" and "/opt/homebrew/include"
```

CI installs `libduckdb` the same way in `ci/Dockerfile.rust`.

## Pre-Push Checks

Run locally before opening a PR:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p dds-lint -- check
```

These four commands run in roughly two minutes and catch ~95% of CI
failures.

## Coding Standards

- **MSRV** — Rust 1.88.0, Edition 2024 (`rust-toolchain.toml` pins it).
- **`unsafe`** — every `unsafe` block needs a `// SAFETY:` comment that
  states the invariants you rely on. `dds-lint` enforces this.
- **Panics** — production code should not panic. Use `Result` everywhere
  in the hot path. Test code may panic.
- **Allocations** — hot paths should be allocation-free; lint-enforced
  via `dds_no_realloc_in_hot_path`.
- **Doc comments** — every `pub` item needs a doc comment.
- **Tests** — prefer integration-style tests in `tests/` over heavy
  mocking. Unit tests live in `#[cfg(test)] mod tests` next to source.

## Spec Annotations

Protocol crates use spec annotations to make traceability explicit:

```rust
#[spec(rtps = "2.5", section = "8.3.7.3")]
pub struct Heartbeat { ... }
```

The annotation references the OMG spec name + section. The
`spec-coverage`-Doc for the corresponding spec links back to the
annotated item.

## Per-Crate READMEs

Each crate in `crates/` and `tools/` carries its own `README.md`.
Most are auto-generated from `Cargo.toml` metadata via
`scripts/gen-crate-readmes.sh` (idempotent — only touches missing
files). Hand-written READMEs (e.g. `crates/cdr/`, `crates/rtps/`,
`crates/idl/`) are recognized by the absence of the auto-generation
marker and left untouched.

## Architecture Decision Records (ADRs)

Non-trivial design decisions go under `docs/adr/` as numbered ADRs
following the [Nygard format](https://www.cognitect.com/blog/2011/11/15/documenting-architecture-decisions).
Larger technical proposals live as RFCs under `internal/rfcs/`.

## Documentation Build

```bash
make -C documentation pdfs       # all stations + vendor specs as PDF
make -C documentation api        # API reference per language
```

Prerequisites: `tectonic` + `pandoc` (PDFs); `doxygen`, `javadoc`, etc.
(API reference) — see `documentation/api/README.md`.

## Project Layout

| Path | Contents |
|---|---|
| `crates/` | Library crates — Rust source organized by layer |
| `tools/` | Binary crates — `idlc`, `admin`, `xmlc`, `dashboard`, etc. |
| `docs/` | Internal docs (German + English): architecture, ADRs, spec coverage |
| `documentation/` | External user-facing docs (English): user/dev/operator guides |
| `examples/` | Tutorials and demos |
| `packaging/` | Linux/macOS/Windows native packages |
| `tests/` | Workspace-level cross-crate and interop tests |
| `xtests/` | Workspace-level integration harnesses |
| `.github/workflows/` | CI pipelines |

## Getting Help

- **GitHub Discussions** — for questions, ideas, and general help.
- **Issues** — for bug reports and feature requests.
- **`security@zerodds.org`** — for vulnerabilities (private).
- **`conduct@zerodds.org`** — for Code of Conduct concerns (private).

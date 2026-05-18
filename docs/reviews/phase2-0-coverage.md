# Phase 2.0 Coverage Audit — WP 2.0a/b/c

Date: 2026-04-20. Host: `aarch64-apple-darwin`, rustc 1.85.0.
Tool: `cargo llvm-cov --workspace --summary-only`.

## 1. Workspace-Total Delta

| Metric | Phase-1 baseline | Phase-2.0 now  | Delta |
| ------ | ---------------- | -------------- | ----- |
| Region | 81.66%           | **80.45%**     | −1.21 pp |
| Line   | 92.17%           | **91.21%**     | −0.96 pp |
| Function | —              | **91.54%**     | —     |

Totals from tool: `15501` regions (3031 missed), `29232` lines (2569 missed).
Regression is concentrated in the new Phase-2 modules below.

## 2. New-Crate Coverage (verbatim from tool)

| File | Regions | Miss | Region % | Lines | Miss | Line % |
| ---- | ------- | ---- | -------- | ----- | ---- | ------ |
| `crates/transport-uds/src/lib.rs` (T1 filesystem UDS) | 134 | 26 | 80.60% | 226 | 28 | 87.61% |
| `crates/transport-uds/src/abstract_dgram.rs` (T5 Linux) | — | — | **not built on macOS** | — | — | — |
| `crates/transport-shm/src/posix.rs` (T3 POSIX-SHM) | 169 | 35 | 79.29% | 422 | 77 | 81.75% |
| `crates/transport-tcp/src/handshake.rs` (T2 PSM) | 108 | 19 | 82.41% | 280 | 27 | 90.36% |
| `tools/isolation-smoke/src/main.rs` (T4 smoke) | 212 | 212 | **0.00%** | 226 | 226 | **0.00%** |

The zero-copy refactor (`0c19d7d`) did not regress `rtps`: `message_builder.rs`
stays at 91.25 % region / 100 % line; `submessages.rs` at 91.88 % / 96.07 %.

## 3. Top 3 Untested Paths (by missed regions)

1. **`tools/isolation-smoke/src/main.rs` — 212/212 regions uncovered.**
   Binary without `#[test]` module, never invoked by the harness; the
   entire UDP/TCP/UDS/SHM matrix plus parent-child fork is dark.
2. **`crates/transport-shm/src/posix.rs` — 35/169 regions uncovered.**
   Error paths dominate: `InvalidConfig`/`InvalidHeader` (l. 344/367/372),
   `SendError::Io "shm ring full, reader too slow"` (l. 429),
   `"shm ring full near wraparound"` (l. 454), `RecvError::Timeout`
   (l. 522). Wrap-around padding hits via `many_frames_roundtrip_with_wraparound`;
   the padding-skip branch in `pop_frame` (l. 488–492) does not.
3. **`crates/transport-tcp/src/handshake.rs` — 19/108 regions uncovered.**
   Roundtrips and version-mismatch reject are covered, but `RejectReason::
   {UnsupportedVendor, DuplicateConnection, ResourceExhausted, Unknown}`
   never flow through `client_handshake`; `HandshakeError::Rejected`
   (l. 346), `From<io::Error>` (l. 240–242), and several `Display`
   arms remain cold.

## 4. Gating Caveats

- `crates/transport-uds/src/abstract_dgram.rs` is `#[cfg(target_os = "linux")]`
  on its `pub mod` declaration in `lib.rs` (l. 377). On darwin it is not
  compiled, so it does not even appear in the coverage table (neither 0 % nor
  100 %). The 559-LOC module plus its Linux-only tests contribute **zero**
  coverage signal on the current developer host.
- The GitLab runner `glr1` on `gitlab.sandra-kessler.eu` is Linux — running
  `cargo llvm-cov --workspace --summary-only` **in CI** (not locally) will
  flip `abstract_dgram` from invisible to measured. That is the cheaper path
  than a Docker-based local run. Recommendation: add an `llvm-cov` job to
  the existing Linux pipeline and publish the summary as a job artifact.

## 5. Top 3 Recommended Test Additions

1. **Smoke-test harness for `tools/isolation-smoke`.** An integration test
   in `tools/isolation-smoke/tests/smoke.rs` that spawns the binary via
   `assert_cmd` on UDP + UDS + SHM matrices would lift 212 regions from
   0 % to ~85 %. Estimate: ~120 LOC.
2. **POSIX-SHM error-path tests.** `posix_full_blocks_then_errors`,
   `posix_invalid_magic_header`, `posix_wraparound_with_small_tail_space`,
   `posix_recv_timeout`. Estimate: ~140 LOC, +25 regions.
3. **Handshake reject-reason matrix.** Parameterised test feeding each
   `RejectReason::{UnsupportedVendor, DuplicateConnection, ResourceExhausted,
   Unknown}` through the client path + `From<io::Error>` + `Display`
   exhaustive. Estimate: ~60 LOC, +15 regions.

Delivering (1)+(2)+(3) would bring the workspace total back to roughly
**82.1 % region / 92.5 % line**, restoring the Phase-1 bar and slightly
exceeding the line-% baseline.

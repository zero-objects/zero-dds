# Phase 2.0 Round-2 Coverage Audit — post-Batch A/B/C + 1/2/3

Date: 2026-04-20. Host: `aarch64-apple-darwin`, rustc 1.85.0.
Tool: `cargo llvm-cov --workspace --summary-only`.

## 1. Workspace-Total Delta

| Metric   | Round-1 (80.45R/91.21L) | Round-2 now | Delta |
| -------- | ----------------------- | ----------- | ----- |
| Region   | 80.45%                  | **81.27%**  | +0.82 pp |
| Line     | 91.21%                  | **91.76%**  | +0.55 pp |
| Function | 91.54%                  | **91.94%**  | +0.40 pp |

Totals: `15633` regions (2928 missed), `29584` lines (2438 missed), `3426`
functions (276 missed). Workspace gained ~130 regions and ~350 lines of code
vs. round-1 yet improved all three ratios — batches added tests faster than
they added code.

## 2. Targeted-File Delta (verbatim from tool)

| File | R-R1 | R-R2 | Δ R% | L-R1 | L-R2 | Δ L% |
| ---- | ---- | ---- | ---- | ---- | ---- | ---- |
| `crates/transport-shm/src/posix.rs` | 79.29 | **85.71** | +6.42 | 81.75 | **89.52** | +7.77 |
| `crates/transport-tcp/src/handshake.rs` | 82.41 | **83.46** | +1.05 | 90.36 | **92.78** | +2.42 |
| `crates/transport-uds/src/lib.rs` | 80.60 | **73.03** | −7.57 | 87.61 | **79.70** | −7.91 |
| `tools/isolation-smoke/src/main.rs` | 0.00 | **62.61** | +62.61 | 0.00 | **67.73** | +67.73 |
| `crates/transport-uds/src/abstract_dgram.rs` | — | — | — | — | — | — |

`abstract_dgram.rs` is `#[cfg(target_os = "linux")]` only and never compiles
on the macOS host, so it stays absent from the coverage set — unchanged from
round-1.

The biggest win is `isolation-smoke` (0 → 62.6 %R / 67.7 %L) from Batch-3's
five integration tests. The biggest regression is `transport-uds/src/lib.rs`
(−7.6 pp R, −7.9 pp L): Batch A added `ensure_base_dir` TOCTOU handling and
Batch 1+2 added `classify_send_error`, but both paths lack tests.
`transport-shm/src/posix.rs` absorbed ~55 new lines (Drop + shm_unlink,
`padding_frames_seen()`, malformed-length guard) *and* improved by +6/+8 pp —
Batch-3's SHM error matrix paid off.

## 3. New Untested Paths

1. `PosixShmTransport::drop` — unlink path only hit when `self.owner=true`,
   not exercised by any unit test (round-1 had no Drop at all).
2. `PosixShmTransport::padding_frames_seen` — public getter, no direct test.
3. `pop_frame` malformed-length guard (`len > ring_len - header`) — DoS
   branch absent from fuzz corpus.
4. `tools/isolation-smoke::validate_base_dir` symlink rejection branch —
   happy path covered, `is_symlink()==true` branch not.
5. `transport-uds::classify_send_error` — all four `ErrorKind` arms
   (`WouldBlock`, `BrokenPipe`, `ConnectionReset`, `_`) untested.
6. `transport-uds::ensure_base_dir` TOCTOU re-check-after-create branch
   (second `metadata()` call) — untested.

## 4. Top-3 Concrete Test Additions

1. `crates/transport-uds/src/lib.rs` → `tests::classify_send_error_matrix`
   — construct `io::Error::from(ErrorKind::BrokenPipe|ConnectionReset|
   WouldBlock|Other)` and assert the returned `SendError` variant. Cheap,
   ~15 lines, recovers ~4 pp on uds/lib.rs.
2. `crates/transport-shm/src/posix.rs` → `tests::drop_unlinks_owner_segment`
   — open owner transport, drop it, then verify `shm_open` with same name
   returns `ENOENT`. Covers Drop + shm_unlink in one hit.
3. `tools/isolation-smoke/src/main.rs` → `tests::validate_base_dir_rejects_symlink`
   — `symlink("/tmp", tmpdir.join("link"))`, call `validate_base_dir(link)`,
   assert `Err(BaseDirIsSymlink)`. Closes the symlink-guard branch.

## 5. Linux-only / macOS-unreachable

- `crates/transport-uds/src/abstract_dgram.rs` — entire file (abstract
  namespace datagram sockets, Linux-only ABI).
- `transport-uds/src/lib.rs` `#[cfg(target_os = "linux")]` paths for
  `SO_PASSCRED` / `SCM_CREDENTIALS` are compiled but the credentials-receive
  branch is dead on macOS.
- `transport-shm/src/posix.rs` uses `shm_open` which exists on macOS, so
  that file is fully reachable here — no cfg-gap.

Coverage-CI on Linux runners would pick up the abstract-dgram gap; until
then treat macOS numbers as lower bound.

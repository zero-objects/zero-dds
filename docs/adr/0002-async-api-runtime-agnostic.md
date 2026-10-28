# 0002 — async DDS API runtime-agnostic, with Tokio glue as an option

- **Status:** accepted
- **Date:** 2026-05-04
- **Authors:** @sandra
- **Context:** crates/dcps-async, docs/specs/zerodds-async-1.0.md

## Context

The Tokio ecosystem dominates (~85 % of the Rust async market). A
naive solution would be to put `tokio` into the async crate as a hard dependency.
That, however, excludes all callers that use `async-std`, `smol`,
`embassy` (no_std) or their own reactor loops.

On the other hand: without Tokio-specific optimizations
(`tokio::sync::Notify`, `tokio::time::sleep` with reactor wakeup instead of
a detached thread) the wakeup path is measurably slower.

## Decision

**`crates/dcps-async` is runtime-agnostic by default.**

- The public API returns `impl Future`/`impl Stream` without a Tokio pin.
- The wakeup path uses a sleep based on a detached thread that
  calls `cx.waker().clone()`. This works under every
  poll-based reactor.
- The optional feature `tokio-glue` switches to wakeups based on
  `tokio::time::sleep` (no detached-thread overhead).
- No hard `tokio` dependency in the default Cargo.toml — workspace CI stays
  lean.

## Alternatives

1. **Tokio as hard dependency** — simplest implementation, but excludes
   ~15 % of the async market. Rejected.
2. **Feature set for several runtimes** (tokio + smol + async-std) —
   3 times the maintenance + 3 times the test surface. Rejected.
3. **Pure futures-core + detached threads** (chosen) — works
   everywhere, with a small performance tax on every wakeup; Tokio glue
   switches that off.

## Consequences

**Positive**:
- `dcps-async` runs with every async reactor without adaptation.
- The caller keeps full control over its runtime.
- By default the workspace stays free of transitive tokio dependencies.

**Negative**:
- Performance tax (detached-thread spawn) on every wakeup. In the
  polling path (5-20 ms tick) negligible (~µs); in the hot path
  (1000+ writes/s) visible.
- A caller that wants maximum throughput must enable `--features tokio-glue`
  explicitly.

**Follow-up tasks**:
- Phase-2-A: `tokio-glue` feature active for `yield_for` (already live).
- Phase-2-B: `spawn_in_tokio` for the DCPS tick loop.
- Bench: `criterion` comparison with/without Tokio glue.

## References

- `docs/specs/zerodds-async-1.0.md` D-1, D-2
- `crates/dcps-async/src/lib.rs::yield_for`
- Tokio documentation: <https://tokio.rs/tokio/topics/bridging>

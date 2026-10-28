# 0001 — Vendor spec strategy for gaps in the OMG ecosystem

- **Status:** accepted
- **Date:** 2026-05-04
- **Authors:** @sandra
- **Context:** docs/specs/, docs/spec-coverage/

## Context

While completing the audit (phase 2 spec coverage), two
topics came up that **have no OMG spec**, but are relevant for a
modern DDS stack:

- **async DDS API** — Tokio/async-std have become established as the
  standard pattern since ~2019. Existing DDS implementations (RTI,
  Cyclone, Fast-DDS) offer C++ `std::future` wrappers, but no
  spec-conformant `async fn` API. dust-dds has a Tokio-only path.
- **Zero-Copy / FlatData** — RTI has "FlatData" as a vendor extension,
  Eclipse Iceoryx has its own standard, ROS-2 has REP-2007
  loaning. No OMG consensus.

We need a **consistent strategy** for how we close such gaps
without (a) compromising OMG compliance and (b) giving the
impression that we bypass standards.

## Decision

**We define our own vendor specs under `docs/specs/zerodds-<topic>-N.M.md`.**

These specs:
- get the same spec coverage treatment as OMG specs
  (`docs/spec-coverage/zerodds-<topic>-N.M.md` with items + repo +
  tests + status, plus `.open.md`).
- have their own `## Decisions` section (D-1..D-N), documented in
  the PROCESS.md style.
- start as `Draft`, and become `Final` only when all items are done +
  tooling is finished + proof of function has been provided.
- define their own wire PIDs only in the vendor range (>= 0x8000), with
  `VENDOR_SPECIFIC_BIT` set and without `MUST_UNDERSTAND`.

## Alternatives

1. **Implement only OMG specs, leave gaps open** — loses the async
   and zero-copy markets completely.
2. **Initiate an OMG spec submission** — 1-3 years lead time, does not
   block us.
3. **Adopt a de facto standard** (e.g. RTI FlatData) — lock-in to
   a competing vendor.
4. **Own vendor spec with full PROCESS.md discipline** (chosen) —
   transparent, auditable, can later be converted into an OMG submission.

## Consequences

**Positive**:
- Spec gaps are closed without us entering a vendor
  lock-in.
- PROCESS.md discipline (items + repo + tests) applies uniformly.
- Vendor PIDs are documented, cross-vendor compatibility is preserved.

**Negative**:
- A caller that uses two DDS vendors in parallel sees two FlatData
  patterns (RTI vs ZeroDDS).
- We alone carry the maintenance effort for the specs.

**Follow-up tasks**:
- ADR-0002 (async runtime-agnostic), ADR-0003 (flatdata backend trait),
  ADR-0004 (Iceoryx2 optional), ADR-0005 (dual stack opt-in),
  ADR-0006 (PID_SHM_LOCATOR).

## References

- `docs/specs/zerodds-async-1.0.md`
- `docs/specs/zerodds-flatdata-1.0.md`
- `docs/spec-coverage/PROCESS.md`
- OMG DDS-XTypes 1.3 §7.3.4 (vendor-specific extensions, allowed bits)

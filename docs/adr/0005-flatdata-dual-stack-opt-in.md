# 0005 — Flatdata dual stack: DCPS integration as an opt-in feature

- **Status:** accepted
- **Date:** 2026-05-04
- **Authors:** @sandra
- **Context:** crates/dcps, crates/flatdata
- **Supersedes:** D-9 in zerodds-flatdata-1.0.md (was: "FlatWriter strictly separate")

## Context

D-9 in the flatdata spec originally said: FlatWriter and
DataWriter remain **strictly separate** types. Reason: do not let
layout constraints from FlatStruct leak into the DCPS layer.

In the phase-2 decision round the question was asked again: is
this ergonomic from the caller's point of view?

- **Caller reality**: a robotics caller has 80 % "normal" topics
  (with strings, Vecs) and 20 % high-frequency sensor topics (Pose,
  IMU). The caller wants ONE pub/sub stack, not two.
- **Type inference**: `pub.create_datawriter::<Pose>(&topic, qos)` →
  and then `writer.write_flat(&pose)?` is more intuitive than
  `flat_pub.create_flat_writer::<Pose>(&topic).write(&pose)?`.
- **Migration**: existing callers that have their CDR DataWriter and
  want to migrate one topic to zero-copy would otherwise have to replace
  their whole pub/sub code.

Risks (see decision round Q3):
- API complexity: multi-pattern DataWriter
- Layout constraints leak into DCPS
- Two wire paths in one class
- Reader-side type inference pain

These risks are real, but they can be mitigated with **opt-in via a build flag**:
a caller that does not want to migrate does not see the methods
at all.

## Decision

**Dual-stack mode: `DataWriter::write_flat` and
`DataReader::read_flat` become available as methods with a `T: DdsType + FlatStruct`
bound on the DCPS classes — behind the build flag
`--features flatdata-integration`.**

Default build (without the feature):
- DataWriter/Reader have **only** the classic CDR paths (`write`,
  `take`).
- FlatWriter/Reader are separate types in `crates/flatdata`.
- No cross-coupling.

With `--features flatdata-integration`:
- DataWriter gets `write_flat`/`loan_flat_slot` as methods with a
  `where T: FlatStruct` bound.
- DataReader gets a `read_flat` method.
- One `Option<Arc<dyn SlotBackend>>` field per DataWriter; on `None`
  fallback to CDR.
- The caller config decides: SHM for same-host, UDP for cross-host
  (see ADR-0003 backend trait).

The derive(FlatStruct) macro (`crates/dcps-derive`) becomes **active with this
feature** — the caller needs it to satisfy the `T: FlatStruct` bound
ergonomically.

## Alternatives

1. **Keep D-9 (FlatWriter strictly separate)** — caller
   migration more expensive; two stacks. Rejected.
2. **DCPS integration always active** — layout constraints leak;
   caller API complex. Rejected.
3. **Opt-in via feature flag** (chosen) — the caller chooses explicitly.

## Consequences

**Positive**:
- A caller that wants to migrate keeps the same pub/sub code.
- Layout constraints stay clearly encapsulated (feature gate).
- derive(FlatStruct) has a clear use-case justification.

**Negative**:
- Test surface doubles (default + flatdata-integration).
- Caller documentation needs examples for two code paths.
- DCPS compile time goes up when the feature is active (macro expansion).

**Follow-up tasks**:
- F11: derive(FlatStruct) macro.
- F-Dual: DataWriter::write_flat/read_flat behind the feature.
- The ADR-0003 SlotBackend trait must be extensible to DataWriter.
- CI: second job with `--features flatdata-integration`.

## References

- `docs/specs/zerodds-flatdata-1.0.md` D-9 (superseded)
- ADR-0003 (backend trait)
- ADR-0004 (Iceoryx2 optional)

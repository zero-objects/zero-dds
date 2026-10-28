# 0006 — PID_SHM_LOCATOR as vendor PID 0x8001

- **Status:** accepted
- **Date:** 2026-05-04
- **Authors:** @sandra
- **Context:** crates/rtps, crates/flatdata, docs/specs/zerodds-flatdata-1.0.md

## Context

Same-host zero-copy needs a discovery component that signals to a
reader: "there is a SHM segment for this topic, here
is the path". Options:

1. Invent our own SEDP submessage — breaks cross-vendor compatibility.
2. Create our own builtin topic — compatibility risk + intervention in the DCPS layer.
3. Vendor PID in PublicationBuiltinTopicData — minimally invasive,
   silently ignored by other vendors.

Spec DDSI-RTPS 2.5 §9.6.2.3 allows vendor-specific PIDs in the range
[0x8000, 0xFFFF) with `VENDOR_SPECIFIC_BIT` set. Cyclone and
Fast-DDS skip unknown vendor PIDs without an error message.

## Decision

**We define `PID_SHM_LOCATOR = 0x8001` as a ZeroDDS vendor PID
without the MUST_UNDERSTAND bit.**

- **Value layout** (little-endian):
  - `u32 hostname_hash` (FNV-1a over the local hostname string)
  - `u32 uid` (POSIX `uid_t`)
  - `u32 slot_count`
  - `u32 slot_size` (total slot size, header + payload + padding)
  - `CDR-String segment_path` (typically `/zddspub_<entity_id>`)
- **MUST_UNDERSTAND**: not set — Cyclone/Fast-DDS readers
  silently ignore it and keep matching via UDP DATA.
- **VENDOR_SPECIFIC_BIT**: set (PID 0x8001 with bit 15 high).
- **Same-host match condition**: local `hostname_hash` ==
  `locator.hostname_hash` AND local `uid` == `locator.uid`.
  Container-friendly (same kernel = same hostname).

## Alternatives

1. **Own SEDP submessage** — wire break with Cyclone/Fast-DDS.
   Rejected.
2. **Builtin topic `ZeroDdsShmLocator`** — intervention in the DCPS layer;
   a Cyclone reader sees the topic but cannot parse it. Rejected.
3. **Vendor PID in PublicationBuiltinTopicData** (chosen) —
   minimally invasive, transparent for other vendors.

## Consequences

**Positive**:
- Cross-vendor domains keep working: ZeroDDS writer +
  Cyclone reader get UDP DATA as always; a ZeroDDS reader additionally
  sees PID_SHM_LOCATOR and attaches to SHM.
- The discovery path does not change — same SEDP endpoint, only
  one additional PID on the wire.
- hostname_hash + uid as the match tuple is container-friendly.

**Negative**:
- 0x8001 is not IANA-registered — if another vendor happens to use
  the same PID, there is a wire collision (silent, no crash).
  Mitigation: a collision is detected by layout validation
  (header length + hostname hash are effectively a 64-bit sentinel).
- A caller that uses cross-user containers (uid 1000 ↔ uid 0) gets
  no SHM — this is intentional (security), but must be documented.

**Follow-up tasks**:
- F2b: PosixSlotAllocator publishes PID_SHM_LOCATOR in the SEDP sample.
- F-Dual: a DataWriter with feature `flatdata-integration` sets the PID
  automatically.
- Documentation: `internal/integration/flatdata-cross-vendor.md` explains the
  behaviour towards Cyclone/Fast-DDS.

## References

- `docs/specs/zerodds-flatdata-1.0.md` §3, D-3, D-4
- DDSI-RTPS 2.5 §9.6.2.3 (Parameter ID Encoding, Vendor-Specific Bit)
- `crates/rtps/src/parameter_list.rs::pid::SHM_LOCATOR`
- `crates/flatdata/src/locator.rs::ShmLocator`

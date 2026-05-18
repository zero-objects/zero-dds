# WP 1.7 — `zerodds-qos` Pre-Merge Review

**Date:** 2026-04-20. **Scope:** `crates/qos/**`, `crates/rtps/src/qos_bridge.rs`.
**Verdict:** *Needs Work* — 2 Critical + 5 High must land before Cyclone/Fast-DDS interop.

## Findings

### A. Spec / Interop

| # | Sev | Where | Finding / Fix |
|---|---|---|---|
| 1 | **Crit** | `qos_set.rs:25-65` | `WriterQos::default()` yields `Reliability::BestEffort` via derive. §2.2.3.14.3: Writer-default is **Reliable**. Silent downgrade → no match with default Reliable readers. *Fix:* hand-written `impl Default` with `reliability.kind=Reliable, max_blocking_time=100ms`. |
| 2 | **Crit** | `compatibility.rs:159-165` + `partition.rs` | Exact-string containment only; §2.2.3.13.6 mandates **fnmatch glob** (`*`,`?`,`[…]`). Cyclone/Fast-DDS glob-match → `"sensor_*"` peer falsely incompatible. L.164 is dead code (intersection symmetric). *Fix:* add glob matcher; single `any_pair_matches`. |
| 3 | **High** | `liveliness.rs:40-46`, `durability.rs:30-38`, `reliability.rs:44-50`, `presentation.rs:37-44`, `destination_order.rs:33-39`, `ownership.rs:32-39`, `history.rs:30-36` | `decode_from` uses `from_u32`, silently coercing unknowns to the *weakest* variant. `check_compatibility` then spuriously passes when a peer sends a future kind. *Fix:* `try_from_u32().ok_or(DecodeError::InvalidData)`; unexport `from_u32`. |
| 4 | **High** | `pid.rs:58-61` | `PID_READER_DATA_LIFECYCLE=0x0046` / `PID_WRITER_DATA_LIFECYCLE=0x0045` are **vendor-reserved** in the OMG PID space (doc admits). Cyclone may drop them. *Fix:* set vendor-flag bit (`0x8000|…`) per RTPS §9.6.3.2.1, or gate behind feature; add PID-byte test (#25). |
| 5 | **High** | `duration.rs:43-49` | `from_millis(-1500)` → `{-1, 2^31}` ≈ −0.5 s (`unsigned_abs()` on remainder). Silent sign bug. *Fix:* `div_euclid/rem_euclid` or `Option<Self>`; tests for `-1`, `-1500`, `i32::MIN`. |

### B. Matrix

| # | Sev | Where | Finding / Fix |
|---|---|---|---|
| 6 | Med | `compatibility.rs:163-164` | Dead symmetric disjunct. *Fix:* single glob-aware helper. |
| 7 | Med | `qos_set.rs:120-164` | Missing intra-QoS **consistency** check (`History.depth>0`, `TimeBasedFilter ≤ Deadline`, `ResourceLimits`). *Fix:* `is_consistent()` with `Inconsistent*` reasons. |
| 8 | Med | `reliability.rs:59`, `qos_set.rs:81` | `max_blocking_time` is Writer-only but also serialized on Reader → byte divergence vs. Cyclone. *Fix:* doc as Reader-ignored; skip in Reader PID emitter. |

### C. Safety / DoS

| # | Sev | Where | Finding / Fix |
|---|---|---|---|
| 9 | **High** | `partition.rs:42-50` | `cap = len.min(remaining/4)` only gates `Vec::with_capacity`; loop runs `0..len` with attacker-u32. Crafted payload → billions of iterations. *Fix:* cap the **loop**: `let len = len.min(remaining/4).min(MAX_PARTITIONS);` return `InvalidData` if exceeded. |
| 10 | **High** | `generic_data.rs:19-27` | `decode_opaque` accepts up to `u32::MAX`; no per-PID allocation cap. `with_capacity(cap)` is moot (`extend_from_slice` re-allocates). SEDP DoS vector. *Fix:* `MAX_OPAQUE_LEN` (~64 KiB); collapse to `Ok(bytes.to_vec())`. |
| 11 | Med | `compatibility.rs:46-70` | `Incompatible(Vec<…>)` allows duplicates/unstable order. *Fix:* dedup+sort (BTreeSet). |

### D. API / Smells

| # | Sev | Where | Finding / Fix |
|---|---|---|---|
| 12 | **High** | `qos_bridge.rs:107-136` | `as_writer_qos`/`as_reader_qos` wires only `durability`+`reliability`; UserData/Partition/Ownership silently `default()`. Bridge **always** reports compatible on real mismatches → defeats WP 1.7. *Fix:* build QoS from full decoded ParameterList, or `debug_assert!` + doc warning. |
| 13 | Med | `qos_bridge.rs:16-103` | Six `From` impls replicate `zerodds-qos` mappings. *Fix:* delete RTPS duplicates (#14) or macro. |
| 14 | Low | `rtps/publication_data.rs` (pre-existing) | Duplicate RTPS enums/`Duration`. Per MEMORY *"Spec-Treue > Diff-Groesse"* — remove now. *Fix:* WP 1.8 ticket; delete bridge. |
| 15 | Low | `generic_data.rs:29-107` | Three byte-identical structs. *Fix:* shared `OpaqueData` newtype/macro. |
| 16 | Low | `data_lifecycle.rs:31-50`, `entity_factory.rs:29-48` | Duplicate bool-with-padding codec. *Fix:* `write_bool_padded`/`read_bool_padded`. |
| 17 | Low | `policies/mod.rs:6` | Blanket `#![allow(missing_docs)]` hides crate lint. *Fix:* remove; doc each `pub mod`. |
| 18 | Low | `qos_set.rs:53-54,92-93` | `Presentation` on Writer+Reader "for convenience"; spec = Publisher/Subscriber. *Fix:* doc TODO for WP 2.x. |

### E. Tests

| # | Sev | Where | Finding / Fix |
|---|---|---|---|
| 19 | Med | all `mod tests` | Only `Little` endianness tested — BE interop blind. *Fix:* parametrize `[Little, Big]`. |
| 20 | Med | `qos_set.rs` tests | No Partition-mismatch aggregate; `contains`-style asserts hide #11. *Fix:* add `partition_mismatch_in_aggregate`, `reasons_deduplicated`, `reasons_stable_order`. |
| 21 | Med | `defaults.rs:108-114` | Writer default not pinned → #1 slips. *Fix:* `writer_qos_default_is_reliable`. |
| 22 | Low | `duration.rs:132-138` | Only positive `from_millis`. *Fix:* negative inputs. |
| 23 | Low | policies | `try_from_u32` OOB tested at one value. *Fix:* add `u32::MAX`, `i32::MIN as u32`. |
| 24 | Low | `qos_bridge.rs:138-225` | Happy-path only. *Fix:* bridged negative-compat test. |
| 25 | Low | `pid.rs` | No numeric PID-table test. *Fix:* `pid_values_match_spec()`. |

## Merge Gate

**Block:** #1, #2, #3, #5, #9, #10, #12. **Defer w/ ticket:** #4, #6–8, #11, #13, #19–21. Rest: cleanup.

## Positives

- Pervasive spec-section refs (aligns MEMORY *"Spec-Treue"*).
- Strict + forward-compat decoder pair is sound (just misapplied in #3).
- `Duration` consts, `#[forbid(unsafe_code)]`, `#[non_exhaustive]` on `Pid`/`IncompatibleReason` — correct hygiene.
- `ResourceLimitsQosPolicy::is_consistent` is the right pattern (extend per #7).
- Bridge module small, scoped, documented — clean deletion path (#14).

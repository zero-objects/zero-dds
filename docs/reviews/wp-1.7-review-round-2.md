# WP 1.7 — `zerodds-qos` Review Round 2

**Date:** 2026-04-20. **Verdict:** *Merge ready.* 23/25 resolved, 1 partial, 1 not addressed. No new criticals.

## Finding Status

| # | Original (short) | Status | Evidence / Note |
|---|---|---|---|
| 1 | Writer default BestEffort | Resolved | `qos_set.rs:80-82` hand `Default` → `Reliable,100ms`; pinned `review_tests.rs:17`. |
| 2 | Partition exact-match, no glob | Resolved | `partition.rs:105` `fnmatch` `*/?/[…]/[!…]/ranges`; `compatibility.rs:180-184`. |
| 3 | Decoders silent downgrade | Resolved | 7 policies: `try_from_u32(v).ok_or(InvalidEnum{kind,value})` — e.g. `durability.rs:77`, `reliability.rs:92`. |
| 4 | DataLifecycle PIDs in OMG space | Resolved | `pid.rs:62,64` → `0x8046`/`0x8045`; MSB pinned `review_tests.rs:240`. |
| 5 | `from_millis(-1500)` sign bug | Resolved | `duration.rs:54-57` `div_euclid`/`rem_euclid`; tests l.121-147. |
| 6 | Dead symmetric disjunct | Resolved | `compatibility.rs:180-185` single helper. |
| 7 | Missing consistency check | Resolved | `qos_set.rs:164,182` + `InconsistentReason{HistoryDepth,ResourceLimits,FilterVsDeadline}`. |
| 8 | `max_blocking_time` Writer-only | Resolved (doc) | `reliability.rs:58-63` "Reader no-op"; still serialized per spec. |
| 9 | Partition decoder DoS | Resolved | `partition.rs:25,62-76` `MAX_PARTITIONS=1024` + name-len 256. |
| 10 | `decode_opaque` unbounded | Resolved | `generic_data.rs:14,28-34` `MAX_OPAQUE_LEN=64KiB`. |
| 11 | Unstable reasons list | Resolved | `compatibility.rs:24` `Ord,Hash`; `from_reasons:70-78` sort+dedup. |
| 12 | Bridge false-Compatible | Resolved (documented) | `qos_bridge.rs:106-123` warning + `with_writer_qos`/`with_reader_qos`. |
| 13 | Six duplicate `From` impls | Deferred | Per original disposition. |
| 14 | Duplicate RTPS enums | Deferred | WP-1.8 ticket. |
| 15 | 3 byte-identical opaque structs | **Partial** | `generic_data.rs` still 3 structs; only `encode/decode_opaque` shared — no newtype/macro. |
| 16 | Duplicate bool-padded codec | Resolved | `wire_helpers.rs:9,20`; used in data_lifecycle + entity_factory. |
| 17 | Blanket `allow(missing_docs)` | Resolved | `policies/mod.rs` clean; 20 submodules documented. |
| 18 | Presentation on Writer+Reader | Resolved (doc) | `qos_set.rs:53-55,126-127` WP-2.x TODO. |
| 19 | BE-endianness untested | Resolved | `review_tests.rs:37-67` Durability BE+LE + History BE. |
| 20 | Partition-mismatch aggregate | Resolved | `review_tests.rs:72,98`. |
| 21 | Writer-default not pinned | Resolved | `review_tests.rs:17`. |
| 22 | `from_millis` negative untested | Resolved | `review_tests.rs:121,132,142`. |
| 23 | `try_from_u32` OOB one-value | Resolved | `review_tests.rs:153` `u32::MAX`/`0x8000_0000` × 7 enums. |
| 24 | Bridge negative-compat test | **Not addressed** | `qos_bridge.rs` has only happy-path `writer_reader_qos_match_by_defaults`. |
| 25 | No numeric PID-table test | Resolved | `review_tests.rs:216` all PIDs + vendor-flag. |

## Regressions (new)

| R# | Sev | Where | Note |
|---|---|---|---|
| R1 | Low | `partition.rs:115-137` | Recursive `'*'` expansion is O(n·m) on `*a*a*a*` patterns. Cap (256) prevents DoS but ReDoS-shape. Iterative backtracking preferable. |
| R2 | Low | `duration.rs:53` | `from_millis(i32::MIN)` panics (`div_euclid(1000)` overflow). #22 only tests small negatives. `Option<Self>` would be safer. |
| R3 | Low | `compatibility.rs:180-185` | Symmetric `fnmatch(o,rq) \|\| fnmatch(rq,o)` — Cyclone treats both sides as patterns. Practical cases agree; malformed classes can diverge. |
| R4 | Low | `partition.rs:151-158` | Malformed `[` without `]` treated as literal — POSIX fnmatch error-semantics differ. |
| R5 | Info | `qos_bridge.rs:125-153` | `as_writer_qos` returns `WriterQos::default()` with peer reliability overlay — other policies are fictional "offered" values, masking real mismatches in `check_compatibility`. Documented, but WP-2.1 trap. |

## Positives

- `InvalidEnum{kind,value}` preserves discriminator for interop debug.
- `from_reasons` sort+dedup → stable logs, robust tests.
- Vendor-flag PID fix (#4) + PID-table test (#25) spec-exact.
- `Duration::from_millis` euclid-split is textbook.
- Bridge limitation *documented*, not silently wrong.

## Merge Gate

Merge ready. WP-1.8 cleanup ticket for #15, #24, R1, R2.

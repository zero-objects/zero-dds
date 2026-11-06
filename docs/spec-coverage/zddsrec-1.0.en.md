# `.zddsrec` v1.0 — Recording format — Spec coverage

**Source:** `docs/specs/zddsrec-1.0.md` (ZeroDDS vendor spec, recording format
for deterministic record-and-replay).

**Context:** `.zddsrec` is the ZeroDDS binary recording format — a header plus a
frame stream with stable participant/topic indexes and a SampleKind encoding.
Implemented in `crates/recorder` (`forbid(unsafe_code)`); its tests check the
octets the spec's layout gives.

Implementation:

- `crates/recorder/` — format codec (`format.rs`), writer (`writer.rs`),
  reader (`reader.rs`), thread-safe session (`session.rs`).

---

## §1 File structure — header + frames

**Spec:** `docs/specs/zddsrec-1.0.md` "File structure" — the file starts with a
header (magic bytes + format version) followed by a stream of typed frames; the
header appears exactly once and before all frames.

**Repo:** `crates/recorder/src/format.rs` (`Header`, `MAGIC`, `Frame`,
`FrameView`), `crates/recorder/src/writer.rs` (`RecordWriter`),
`crates/recorder/src/reader.rs` (`RecordReader`).

**Tests:** `crates/recorder/tests/zddsrec_layout.rs::zddsrec_1_a_header_and_its_frames_are_the_octets_of_the_layout`,
`crates/recorder/tests/zddsrec_layout.rs::zddsrec_1_the_header_comes_once_and_first_and_a_frame_indexes_into_its_lists`,
`crates/recorder/tests/zddsrec_layout.rs::zddsrec_1_a_file_that_breaks_the_layout_is_an_error`,
`crates/recorder/src/session.rs::tests::zddsrec_1_a_capture_without_samples_is_its_header`.

**Status:** done

## §2 Indexes — participant/topic tables

**Spec:** `docs/specs/zddsrec-1.0.md` "Indizes" — participant and topic entries
are addressed via stable index references; frames carry a topic index instead of
repeated strings.

**Repo:** `crates/recorder/src/format.rs` (`ParticipantEntry`, `TopicEntry`,
`TopicKey`), `crates/recorder/src/session.rs` (index assignment + lookup).

**Tests:** `crates/recorder/tests/zddsrec_layout.rs::zddsrec_2_a_topic_add_continues_the_header_list`,
`crates/recorder/src/session.rs::tests::zddsrec_2_a_topic_added_after_the_header_takes_the_next_index`,
`crates/recorder/src/session.rs::tests::an_unknown_topic_is_dropped_and_an_unknown_participant_is_index_zero`,
`crates/recorder/tests/zddsrec_layout.rs::zddsrec_1_the_header_comes_once_and_first_and_a_frame_indexes_into_its_lists`.

**Status:** done

## §3 SampleKind encoding

**Spec:** `docs/specs/zddsrec-1.0.md` "SampleKind-Codierung" — every sample
carries a SampleKind (Data / Dispose / Unregister …) as a compact wire code;
unknown codes are rejected on read.

**Repo:** `crates/recorder/src/format.rs` (`enum SampleKind` + encode/decode).

**Tests:** `crates/recorder/tests/zddsrec_layout.rs::zddsrec_3_the_sample_kinds_are_0_1_and_2`.

**Status:** done

## §4 Versioning

**Spec:** `docs/specs/zddsrec-1.0.md` "Versionierung" — the format version is in
the header; a reader explicitly rejects unsupported major versions (additive 2.0
hooks without breaking the 1.0 wire form).

**Repo:** `crates/recorder/src/format.rs` (`Header` version field + read-time
check).

**Tests:** `crates/recorder/tests/zddsrec_layout.rs::zddsrec_4_a_version_2_frame_reads_without_identifier_and_a_later_version_is_refused`.

**Status:** done

## §5 Recording session — thread-safe, lazy header

**Spec:** `docs/specs/zddsrec-1.0.md` "File structure"/"Use cases" — a session
records samples concurrently; the header is written on the first sample; several
threads may record at the same time.

**Repo:** `crates/recorder/src/session.rs` (`RecordingSession`,
`SessionOptions`, `SessionStats`).

**Tests:** `crates/recorder/src/session.rs::tests::zddsrec_5_the_header_goes_out_with_the_first_sample_and_frames_hold_time_deltas`,
`crates/recorder/src/session.rs::tests::samples_of_four_threads_are_all_frames_of_the_file`,
`crates/recorder/src/session.rs::tests::the_byte_count_is_the_size_of_the_capture`.

**Status:** done

## §6 Goals, use cases, stability and roadmap

**Spec:** `docs/specs/zddsrec-1.0.md` "Goals", "Use cases", "Stability and roadmap" — motivation, use cases (crash reproduction, forensic diff, soak replay) and the extensions planned for 2.0.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — purpose, use cases and outlook.

## §6 Tooling

**Spec:** `docs/specs/zddsrec-1.0.md` "Tooling" — `crates/recorder` as the API for applications; `tools/replay` (`zerodds-replay`) to inspect, dump and replay (`--time-scale`); `tools/recorder-bridge` to record from a running domain.

**Repo:** `tools/replay/src/main.rs` (commands `inspect`, `dump`, `replay`, option `--time-scale`), `tools/record/src/main.rs`, `tools/recorder-bridge/src/main.rs`.

**Tests:** `tools/replay/tests/inject_under_the_recorded_identifier.rs::rtps_10_5_the_injected_samples_keep_their_representation_identifiers`,
`tools/replay/tests/inject_under_the_recorded_identifier.rs::a_capture_without_identifiers_is_not_injected_unless_one_is_named`,
`tools/record/tests/cli_smoke.rs::a_run_with_duration_ends_by_itself_and_leaves_a_capture`,
`tools/record/tests/cli_smoke.rs::info_prints_the_header_of_a_capture`.

**Status:** partial — `zerodds-replay` injects a capture under the identifiers it recorded, `zerodds-record` ends a run by `--duration` and `info` reads a capture; `inspect`, `dump` and `--time-scale` of `zerodds-replay` and `tools/recorder-bridge` have no test. The library `crates/recorder` is listed in §1 to §5.

---

## Audit status

5 done / 1 partial / 0 open / 0 blocked / 1 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zddsrec-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-record --all-targets` (job `merge-rust-record`) — 2 tests green.
* `cargo nextest run --profile merge -p zerodds-recorder --all-targets` (job `merge-rust-recorder`) — 12 tests green.
* `cargo nextest run --profile merge -p zerodds-replay --all-targets` (job `merge-rust-replay`) — 2 tests green.

16 cited tests green, 0 not green.

Open items: `zddsrec-1.0.open.en.md`.

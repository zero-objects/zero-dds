# `.zddsrec` v1.0 — Recording format — Spec coverage

**Source:** `docs/specs/zddsrec-1.0.md` (ZeroDDS vendor spec, recording format
for deterministic record-and-replay).

**Context:** `.zddsrec` is the ZeroDDS binary recording format — a header plus a
frame stream with stable participant/topic indexes and a SampleKind encoding.
Implemented in `crates/recorder` (`forbid(unsafe_code)`), 17 tests green.

Implementation:

- `crates/recorder/` — format codec (`format.rs`), writer (`writer.rs`),
  reader (`reader.rs`), thread-safe session (`session.rs`); 17 tests green.

---

## §1 File structure — header + frames

**Spec:** `docs/specs/zddsrec-1.0.md` "Datei-Struktur" — the file starts with a
header (magic bytes + format version) followed by a stream of typed frames; the
header appears exactly once and before all frames.

**Repo:** `crates/recorder/src/format.rs` (`Header`, `MAGIC`, `Frame`,
`FrameView`), `crates/recorder/src/writer.rs` (`RecordWriter`),
`crates/recorder/src/reader.rs` (`RecordReader`).

**Tests:** `crates/recorder/src/format.rs::tests::header_roundtrip`,
`header_writes_magic_and_version`, `frame_roundtrip`,
`header_must_come_before_frames`, `header_only_once`, `bad_magic_rejected`,
`truncated_header_detected`, `truncated_frame_detected`, `write_all_helper`.

**Status:** done

## §2 Indexes — participant/topic tables

**Spec:** `docs/specs/zddsrec-1.0.md` "Indizes" — participant and topic entries
are addressed via stable index references; frames carry a topic index instead of
repeated strings.

**Repo:** `crates/recorder/src/format.rs` (`ParticipantEntry`, `TopicEntry`,
`TopicKey`), `crates/recorder/src/session.rs` (index assignment + lookup).

**Tests:** `crates/recorder/src/session.rs::tests::session_drops_unknown_topic`,
`session_unknown_participant_falls_back_to_idx_zero`,
`crates/recorder/src/format.rs::tests::frame_idx_must_be_in_range`.

**Status:** done

## §3 SampleKind encoding

**Spec:** `docs/specs/zddsrec-1.0.md` "SampleKind-Codierung" — every sample
carries a SampleKind (Data / Dispose / Unregister …) as a compact wire code;
unknown codes are rejected on read.

**Repo:** `crates/recorder/src/format.rs` (`enum SampleKind` + encode/decode).

**Tests:** `crates/recorder/src/format.rs::tests::sample_kind_roundtrip`,
`bad_sample_kind_rejected`.

**Status:** done

## §4 Versioning

**Spec:** `docs/specs/zddsrec-1.0.md` "Versionierung" — the format version is in
the header; a reader explicitly rejects unsupported major versions (additive 2.0
hooks without breaking the 1.0 wire form).

**Repo:** `crates/recorder/src/format.rs` (`Header` version field + read-time
check).

**Tests:** `crates/recorder/src/format.rs::tests::unsupported_version_rejected`,
`header_writes_magic_and_version`.

**Status:** done

## §5 Recording session — thread-safe, lazy header

**Spec:** `docs/specs/zddsrec-1.0.md` "Datei-Struktur"/"Use-Cases" — a session
records samples concurrently; the header is written on the first sample; several
threads may record at the same time.

**Repo:** `crates/recorder/src/session.rs` (`RecordingSession`,
`SessionOptions`, `SessionStats`).

**Tests:** `crates/recorder/src/session.rs::tests::session_thread_safe_record`,
`session_writes_header_lazy_on_first_sample`.

**Status:** done

## §6 Goals / use cases / tooling / stability

**Spec:** `docs/specs/zddsrec-1.0.md` "Ziele", "Use-Cases", "Tooling",
"Stabilität und Roadmap" — motivation, use cases (CI fixtures, regression
captures, post-mortems), CLI binding (`zerodds-record` / `zerodds-replay`) and
the version roadmap.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — descriptive/roadmap sections, no normative wire
requirement beyond §1–§5.

---

## Audit status

5 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-recorder` — 17 tests green, 0 failed.

Open items: none. Decision records: none.

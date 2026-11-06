# `.zddsrec` v1.0 — Recording-Format — Spec-Coverage

**Quelle:** `docs/specs/zddsrec-1.0.md` (ZeroDDS-Vendor-Spec, Recording-Format
für deterministisches Record-and-Replay).

**Kontext:** `.zddsrec` ist das binäre Aufzeichnungsformat von ZeroDDS —
Header + Frame-Stream mit stabilen Indizes für Participant/Topic, plus
SampleKind-Codierung. Implementiert in `crates/recorder` (`forbid(unsafe_code)`); seine Tests prüfen
die Oktette, die das Layout der Spec gibt.

Implementation:

- `crates/recorder/` — Format-Codec (`format.rs`), Writer (`writer.rs`),
  Reader (`reader.rs`), thread-safe Session (`session.rs`).

---

## §1 Datei-Struktur — Header + Frames

**Spec:** `docs/specs/zddsrec-1.0.md` „Datei-Struktur" — Datei beginnt mit einem
Header (Magic-Bytes + Format-Version), gefolgt von einem Stream typisierter
Frames; Header genau einmal und vor allen Frames.

**Repo:** `crates/recorder/src/format.rs` (`Header`, `MAGIC`, `Frame`,
`FrameView`), `crates/recorder/src/writer.rs` (`RecordWriter`),
`crates/recorder/src/reader.rs` (`RecordReader`).

**Tests:** `crates/recorder/tests/zddsrec_layout.rs::zddsrec_1_a_header_and_its_frames_are_the_octets_of_the_layout`,
`crates/recorder/tests/zddsrec_layout.rs::zddsrec_1_the_header_comes_once_and_first_and_a_frame_indexes_into_its_lists`,
`crates/recorder/tests/zddsrec_layout.rs::zddsrec_1_a_file_that_breaks_the_layout_is_an_error`,
`crates/recorder/src/session.rs::tests::zddsrec_1_a_capture_without_samples_is_its_header`.

**Status:** done

## §2 Indizes — Participant/Topic-Tabellen

**Spec:** `docs/specs/zddsrec-1.0.md` „Indizes" — Participant- und Topic-Einträge
werden über stabile Index-Referenzen adressiert; Frames tragen einen
Topic-Index statt wiederholter Strings.

**Repo:** `crates/recorder/src/format.rs` (`ParticipantEntry`, `TopicEntry`,
`TopicKey`), `crates/recorder/src/session.rs` (Index-Vergabe + Lookup).

**Tests:** `crates/recorder/tests/zddsrec_layout.rs::zddsrec_2_a_topic_add_continues_the_header_list`,
`crates/recorder/src/session.rs::tests::zddsrec_2_a_topic_added_after_the_header_takes_the_next_index`,
`crates/recorder/src/session.rs::tests::an_unknown_topic_is_dropped_and_an_unknown_participant_is_index_zero`,
`crates/recorder/tests/zddsrec_layout.rs::zddsrec_1_the_header_comes_once_and_first_and_a_frame_indexes_into_its_lists`.

**Status:** done

## §3 SampleKind-Codierung

**Spec:** `docs/specs/zddsrec-1.0.md` „SampleKind-Codierung" — jedes Sample trägt
einen SampleKind (Data / Dispose / Unregister …) als kompakten Wire-Code;
unbekannte Codes werden beim Lesen abgewiesen.

**Repo:** `crates/recorder/src/format.rs` (`enum SampleKind` + Encode/Decode).

**Tests:** `crates/recorder/tests/zddsrec_layout.rs::zddsrec_3_the_sample_kinds_are_0_1_and_2`.

**Status:** done

## §4 Versionierung

**Spec:** `docs/specs/zddsrec-1.0.md` „Versionierung" — die Format-Version steht
im Header; ein Reader weist nicht unterstützte Major-Versionen explizit ab
(additive 2.0-Hooks ohne Bruch der 1.0-Wire-Form).

**Repo:** `crates/recorder/src/format.rs` (`Header`-Version-Feld + Check beim
Read).

**Tests:** `crates/recorder/tests/zddsrec_layout.rs::zddsrec_4_a_version_2_frame_reads_without_identifier_and_a_later_version_is_refused`.

**Status:** done

## §5 Aufzeichnungs-Session — thread-safe, lazy Header

**Spec:** `docs/specs/zddsrec-1.0.md` „Datei-Struktur"/„Use-Cases" — eine Session
nimmt nebenläufig Samples auf; der Header wird beim ersten Sample geschrieben;
mehrere Threads dürfen gleichzeitig aufzeichnen.

**Repo:** `crates/recorder/src/session.rs` (`RecordingSession`,
`SessionOptions`, `SessionStats`).

**Tests:** `crates/recorder/src/session.rs::tests::zddsrec_5_the_header_goes_out_with_the_first_sample_and_frames_hold_time_deltas`,
`crates/recorder/src/session.rs::tests::samples_of_four_threads_are_all_frames_of_the_file`,
`crates/recorder/src/session.rs::tests::the_byte_count_is_the_size_of_the_capture`.

**Status:** done

## §6 Ziele, Use-Cases, Stabilität und Roadmap

**Spec:** `docs/specs/zddsrec-1.0.md` „Goals", „Use cases", „Stability and roadmap" — Motivation, Anwendungsfälle (Crash-Reproduktion, forensischer Vergleich, Soak-Replay) und die für 2.0 geplanten Erweiterungen.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Zweck, Anwendungsfälle und Ausblick.

## §6 Tooling

**Spec:** `docs/specs/zddsrec-1.0.md` „Tooling" — `crates/recorder` als API für Anwendungen; `tools/replay` (`zerodds-replay`) zum Ansehen, Ausgeben und Wiedereinspielen (`--time-scale`); `tools/recorder-bridge` zum Aufzeichnen aus einer laufenden Domain.

**Repo:** `tools/replay/src/main.rs` (Befehle `inspect`, `dump`, `replay`, Option `--time-scale`), `tools/record/src/main.rs`, `tools/recorder-bridge/src/main.rs`.

**Tests:** `tools/replay/tests/inject_under_the_recorded_identifier.rs::rtps_10_5_the_injected_samples_keep_their_representation_identifiers`,
`tools/replay/tests/inject_under_the_recorded_identifier.rs::a_capture_without_identifiers_is_not_injected_unless_one_is_named`,
`tools/record/tests/cli_smoke.rs::a_run_with_duration_ends_by_itself_and_leaves_a_capture`,
`tools/record/tests/cli_smoke.rs::info_prints_the_header_of_a_capture`.

**Status:** partial — `zerodds-replay` spielt eine Aufzeichnung unter den aufgezeichneten Kennungen ein, `zerodds-record` beendet einen Lauf mit `--duration`, `info` liest eine Aufzeichnung; `inspect`, `dump` und `--time-scale` von `zerodds-replay` und `tools/recorder-bridge` haben keinen Test. Die Bibliothek `crates/recorder` ist in §1 bis §5 geführt.

---

## Audit-Status

5 done / 1 partial / 0 open / 0 blocked / 1 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zddsrec-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-record --all-targets` (Job `merge-rust-record`) — 2 Tests grün.
* `cargo nextest run --profile merge -p zerodds-recorder --all-targets` (Job `merge-rust-recorder`) — 12 Tests grün.
* `cargo nextest run --profile merge -p zerodds-replay --all-targets` (Job `merge-rust-replay`) — 2 Tests grün.

16 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `zddsrec-1.0.open.md`.

# `.zddsrec` v1.0 — Recording-Format — Spec-Coverage

**Quelle:** `docs/specs/zddsrec-1.0.md` (ZeroDDS-Vendor-Spec, Recording-Format
für deterministisches Record-and-Replay).

**Kontext:** `.zddsrec` ist das binäre Aufzeichnungsformat von ZeroDDS —
Header + Frame-Stream mit stabilen Indizes für Participant/Topic, plus
SampleKind-Codierung. Implementiert in `crates/recorder` (`forbid(unsafe_code)`),
17 Tests grün.

Implementation:

- `crates/recorder/` — Format-Codec (`format.rs`), Writer (`writer.rs`),
  Reader (`reader.rs`), thread-safe Session (`session.rs`); 17 Tests grün.

---

## §1 Datei-Struktur — Header + Frames

**Spec:** `docs/specs/zddsrec-1.0.md` „Datei-Struktur" — Datei beginnt mit einem
Header (Magic-Bytes + Format-Version), gefolgt von einem Stream typisierter
Frames; Header genau einmal und vor allen Frames.

**Repo:** `crates/recorder/src/format.rs` (`Header`, `MAGIC`, `Frame`,
`FrameView`), `crates/recorder/src/writer.rs` (`RecordWriter`),
`crates/recorder/src/reader.rs` (`RecordReader`).

**Tests:** `crates/recorder/src/format.rs::tests::header_roundtrip`,
`header_writes_magic_and_version`, `frame_roundtrip`,
`header_must_come_before_frames`, `header_only_once`, `bad_magic_rejected`,
`truncated_header_detected`, `truncated_frame_detected`, `write_all_helper`.

**Status:** done

## §2 Indizes — Participant/Topic-Tabellen

**Spec:** `docs/specs/zddsrec-1.0.md` „Indizes" — Participant- und Topic-Einträge
werden über stabile Index-Referenzen adressiert; Frames tragen einen
Topic-Index statt wiederholter Strings.

**Repo:** `crates/recorder/src/format.rs` (`ParticipantEntry`, `TopicEntry`,
`TopicKey`), `crates/recorder/src/session.rs` (Index-Vergabe + Lookup).

**Tests:** `crates/recorder/src/session.rs::tests::session_drops_unknown_topic`,
`session_unknown_participant_falls_back_to_idx_zero`,
`crates/recorder/src/format.rs::tests::frame_idx_must_be_in_range`.

**Status:** done

## §3 SampleKind-Codierung

**Spec:** `docs/specs/zddsrec-1.0.md` „SampleKind-Codierung" — jedes Sample trägt
einen SampleKind (Data / Dispose / Unregister …) als kompakten Wire-Code;
unbekannte Codes werden beim Lesen abgewiesen.

**Repo:** `crates/recorder/src/format.rs` (`enum SampleKind` + Encode/Decode).

**Tests:** `crates/recorder/src/format.rs::tests::sample_kind_roundtrip`,
`bad_sample_kind_rejected`.

**Status:** done

## §4 Versionierung

**Spec:** `docs/specs/zddsrec-1.0.md` „Versionierung" — die Format-Version steht
im Header; ein Reader weist nicht unterstützte Major-Versionen explizit ab
(additive 2.0-Hooks ohne Bruch der 1.0-Wire-Form).

**Repo:** `crates/recorder/src/format.rs` (`Header`-Version-Feld + Check beim
Read).

**Tests:** `crates/recorder/src/format.rs::tests::unsupported_version_rejected`,
`header_writes_magic_and_version`.

**Status:** done

## §5 Aufzeichnungs-Session — thread-safe, lazy Header

**Spec:** `docs/specs/zddsrec-1.0.md` „Datei-Struktur"/„Use-Cases" — eine Session
nimmt nebenläufig Samples auf; der Header wird beim ersten Sample geschrieben;
mehrere Threads dürfen gleichzeitig aufzeichnen.

**Repo:** `crates/recorder/src/session.rs` (`RecordingSession`,
`SessionOptions`, `SessionStats`).

**Tests:** `crates/recorder/src/session.rs::tests::session_thread_safe_record`,
`session_writes_header_lazy_on_first_sample`.

**Status:** done

## §6 Ziele / Use-Cases / Tooling / Stabilität

**Spec:** `docs/specs/zddsrec-1.0.md` „Ziele", „Use-Cases", „Tooling",
„Stabilität und Roadmap" — Motivation, Anwendungsfälle (CI-Fixtures,
Regression-Captures, Post-Mortems), CLI-Anbindung (`zerodds-record` /
`zerodds-replay`) und Versions-Roadmap.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — beschreibende/Roadmap-Abschnitte, keine
eigenständige normative Wire-Anforderung über §1–§5 hinaus.

---

## Audit-Status

5 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-recorder` — 17 Tests grün, 0 failed.

Offene Punkte: keine. Decision-Records: keine.

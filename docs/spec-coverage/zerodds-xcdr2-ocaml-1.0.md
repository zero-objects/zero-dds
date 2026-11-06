# `zerodds-xcdr2-ocaml` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) und `internal/standards/cache/omg/idl-4.2.pdf` (§7) — was die OCaml-Spalte beantwortet.

Der Mechanismus, den diese Spalte mit den zwanzig anderen teilt — die beiden Walks, die Rahmung, Table 9, der KeyHash und der Treiber —, steht in `zerodds-xcdr2-bindings-conformance-1.0.md`. Dieses Dokument führt nur, was die OCaml-Spalte selbst entscheidet, und ihre eigenen Nachweise.

## §7.5.1 Die Teile der Spalte

### §7.5.1 Der konkrete Syntaxbaum und seine Textform

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Deklarationen dieser Spalte werden als Baum gebaut und erst danach als Text geschrieben; kein Emitter setzt Quelltext zusammen.

**Repo:** `crates/lang-cst/src/ocaml/`, `crates/lang-render/src/ocaml.rs`

**Tests:** `crates/lang-render/src/ocaml.rs::tests` (12 Tests), darunter `xtypes_7_5_1_a_record_writes_one_field_per_line_with_a_trailing_semicolon`, `xtypes_7_5_1_a_group_of_two_joins_them_with_and`, `xtypes_7_5_1_a_postfix_type_needs_no_brackets_at_any_depth`, `xtypes_7_5_1_a_tuple_is_always_bracketed_because_star_binds_loosest`, `xtypes_7_5_1_a_variant_writes_a_leading_bar_per_constructor`;
querschnittlich `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 Der Regelkatalog

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Was aus welchem Modellknoten wird, steht als Regelmenge und nicht als Sonderfall im Walk.

**Repo:** `crates/lang-rules/src/rule/ocaml.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 Der Materialisierer

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Der Materialisierer trägt die Eigenheiten der Sprache: Namens- und Schlüsselwortbehandlung, Kollisionen und das, was die Sprache gar nicht ausdrücken kann.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 Das Codec-Backend

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Das Backend liefert die Schreibweise, in der die geteilte Kodierungsentscheidung in dieser Sprache steht — Empfänger, Methodenname, Argumentform.

**Repo:** `crates/lang-rules/src/emit/ocaml.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Der Table-9-Defaulter

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Ein Leser ohne Tab.9-Antwort hat an fünf Stellen keinen definierten Wert; diese Datei ist die Antwort dieser Spalte.

**Repo:** `crates/lang-rules/src/default_ocaml.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Die Wire-Laufzeit

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Laufzeit liest §7.4 eigenständig aus dem Klauseltext und ruft keine andere Spalte; dort liegt die Unabhängigkeit dieser Spalte.

**Repo:** `runtime/ocaml`, Endpoint-SDK `endpoints/ocaml`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 Was diese Spalte selbst entscheidet

### §7.2.2.2 Jedes IDL-Primitiv erreicht einen OCaml-Typ; keine Breite, die ein Member erreicht, ist plattformabhängig

**Spec:** §7.2.2.2 — Primitive Types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; jede Breite ist festgelegt.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_2_2_2_every_idl_primitive_reaches_an_ocaml_type`,
`crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_2_2_2_no_generated_type_is_platform_dependent_in_a_width_a_member_can_reach`

**Status:** done

### §7.2.2.2 `long double` wird ein String und nie ein Float

**Spec:** §7.2.2.2 — FLOAT128 ist ein IEEE-754-binary128-Wert von 16 Oktetten.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_2_2_2_a_long_double_is_a_string_and_never_a_float`

**Status:** done

### §7.2.2.4.3 Eine Sequenz wird ein Array und nie eine Liste; ein Array verliert seine Schranke an den Kommentar

**Spec:** §7.2.2.4.3 — Sequence — eindimensional, Schranke optional.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_2_2_4_3_a_sequence_becomes_an_array_and_never_a_list`,
`crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_2_2_4_3_an_array_loses_its_bound_to_the_comment_because_ocaml_has_no_fixed_size_array`

**Status:** done

### §7.2.2.4.3 Eine `map` wird ein Array von Entry-Records und nie eine `Hashtbl`

**Spec:** §7.2.2.4.3 — Map — Key/Value-Paare; §7.4.3.5.3 Regeln 11-16 legen Reihenfolge und Rahmung fest.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_2_2_4_3_a_map_becomes_an_array_of_entry_records_and_never_a_hashtbl`

**Status:** done

### §7.2.2.4.4.3 Eine Union wird ein Record und nie eine Variante

**Spec:** §7.2.2.4.4.3 — Union — der Diskriminator ist immer das erste Member und trägt `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_2_2_4_4_3_a_union_becomes_a_record_and_never_a_variant`

**Status:** done

### §7.2.2.4.1.1 Eine Enumeration wird eine Variante mit einem Paar Konversionen

**Spec:** §7.2.2.4.1.1 — Enumeration — Int32-Literale, geordnet, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_2_2_4_1_1_an_enum_becomes_a_variant_and_a_pair_of_conversions`,
`crates/lang-rules/tests/ocaml_codec.rs::xtypes_7_2_2_4_1_1_an_enumeration_crosses_through_its_conversions`

**Status:** done

### IDL 4.2 §7.2.3 Ein OCaml-Keyword und ein vordefinierter Typ werden entschärft; die Case-Faltung fügt keine neue Kollision hinzu

**Spec:** IDL 4.2 §7.2.3 — Identifier sind ASCII und beim Vergleich case-insensitiv; eine Kollision muss gemeldet werden.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::idl_7_2_3_a_member_named_after_an_ocaml_keyword_is_escaped_and_keeps_its_idl_name`,
`crates/lang-rules/tests/ocaml_end_to_end.rs::idl_7_2_3_a_member_named_after_a_predefined_type_is_escaped_too`,
`crates/lang-rules/tests/ocaml_end_to_end.rs::idl_7_2_3_the_case_fold_adds_no_collision_the_frontend_does_not_already_report`

**Status:** done

### IDL 4.2 §7.5.2 `module` flacht auf den Namen ab, weil eine rekursive Gruppe keine Modulgrenze überschreitet

**Spec:** IDL 4.2 §7.5.2 — `module` ist ein Naming-Scope; ein Identifier ist je Scope einmalig.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::idl_7_5_2_a_module_flattens_onto_the_name_because_a_recursive_group_cannot_cross_one`

**Status:** done

### §7.2.2.4.9 Zwei wechselseitig rekursive Typen kommen als eine `and`-verbundene Gruppe heraus

**Spec:** §7.2.2.4.9 — Ein selbstbezüglicher Typ braucht eine Stelle, an der der Abstieg endet.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_2_2_4_9_a_self_recursive_type_is_one_declaration_that_names_itself`,
`crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_2_2_4_9_two_mutually_recursive_types_come_out_as_one_and_joined_group`,
`crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_2_2_4_9_a_recursive_group_compiles`

**Status:** done

### §7.4.3 Ein Member-Store ist ein funktionales Record-Update; ein mutable Aggregat übersetzt mit seinen Seen-Flags

**Spec:** §7.4.3 — XCDR Version 2 — Plain CDR2, Delimited CDR2 und PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_codec.rs::xtypes_7_4_3_a_member_store_is_a_functional_record_update`,
`crates/lang-rules/tests/ocaml_codec.rs::xtypes_7_4_3_a_mutable_aggregate_compiles_with_its_seen_flags`,
`crates/lang-rules/tests/ocaml_codec.rs::xtypes_7_4_3_the_shapes_the_corpus_has_reach_the_end_of_both_walks`

**Status:** done

## §7.3.1 Woran die Spalte gehalten wird

### §7.3.1 Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat

**Spec:** §7.3.1 — Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat. Ein Fall, der zu rendern aufhört, würde sonst erst von der nächsten Messung gefunden — ein Build-Verzeichnis vom Konstrukt entfernt, das ihn verursacht hat.

**Repo:** `crates/lang-rules/src/emit/ocaml.rs`, `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_3_1_every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache

**Spec:** §7.3.1 — Die erzeugte Quelle übersetzt unter dem Übersetzer der Sprache. Ein Snapshot sagt, dass der Emitter den Text erzeugt hat, den er letztes Mal erzeugt hat; er sagt nicht, dass der Text die Sprache ist. Fehlt der Übersetzer, sagt der Test das auf Standardfehler.

**Repo:** `crates/lang-rules/src/emit/ocaml.rs`, `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_3_1_the_generated_ocaml_compiles`

**Status:** done

### §7.3.1 Zwei Läufe über eine Quelle erzeugen denselben Text

**Spec:** §7.3.1 — Zwei Läufe über eine Quelle erzeugen denselben Text. Ohne diese Eigenschaft ist jeder Diff eines erzeugten Baums unlesbar.

**Repo:** `crates/lang-rules/src/emit/ocaml.rs`, `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_3_1_two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe

**Spec:** §7.2.2.4 — Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe. Die Tabelle in `crates/lang-rules/src/coverage.rs` ist die Behauptung; dieser Test ist ihre Prüfung.

**Repo:** `crates/lang-rules/src/emit/ocaml.rs`, `crates/lang-rules/src/adapter/ocaml.rs`

**Tests:** `crates/lang-rules/tests/ocaml_end_to_end.rs::xtypes_7_2_2_4_every_construct_the_table_claims_for_ocaml_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt

**Spec:** §7.2.2.4.8 — `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt.

**Repo:** `crates/lang-rules/src/adapter/ocaml.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_1_a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Treiber und Messung

### §7.3.1 `idlc generate --lang ocaml`

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe der Erzeugung; die Ausgabe ist die Quelle dieser Spalte.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::deployment_1_3_every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::xtypes_7_3_1_language_names_lists_every_language_once`

**Status:** done

### §7.4 Der Messaufbau dieser Spalte

**Spec:** §7.4 — die Oktette einer Spalte werden gegen aufgezeichnete Oktette verglichen und nicht in ihren eigenen Tests behauptet.

**Repo:** `tools/wireprobe/wireprobe-zerodds-ocaml-gen/`, `tools/wireprobe/wireprobe-zerodds-ocaml`, `tools/wireprobe/wireprobe-ocaml-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_4_every_language_has_a_cell_generator`

**Status:** done

---

## Audit-Status

23 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-xcdr2-ocaml-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (Job `merge-rust-idlc`) — 2 Tests grün.
* `cargo nextest run --profile merge -p zerodds-lang-render --all-targets` (Job `merge-rust-lang-render`) — 7 Tests grün.
* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (Job `merge-rust-lang-rules`) — 11 Tests grün.
* `python3 ci/run_integration.py merge-codegen-ocaml` (Job `merge-codegen-ocaml`) — 23 Tests grün.

43 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.

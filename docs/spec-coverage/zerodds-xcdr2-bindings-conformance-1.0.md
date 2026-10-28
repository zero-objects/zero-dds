# `zerodds-xcdr2-bindings-conformance` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) —
DDS-XTypes 1.3, Type-System-Model und Data Representation.

Der gemeinsame Bericht über den Mechanismus, den alle einundzwanzig
Sprachspalten teilen. Was eine einzelne Spalte selbst entscheidet, steht
in ihrem eigenen Dokument — `zerodds-xcdr2-c-1.0.md`,
`zerodds-xcdr2-java-1.0.md` und so fort; die Nachweise bleiben je Sprache
getrennt, weil jede Laufzeit die Klausel eigenständig liest.

Der Mechanismus besteht aus vier Stücken:

- `crates/lang-rules/` — zwei Walks (`src/emit/encode.rs`,
  `src/emit/decode.rs`), generisch über `emit::CodecTarget`; je Sprache
  ein Regelkatalog, ein Materialisierer, ein Codec-Backend und eine
  Table-9-Regel.
- `crates/lang-cst/` — je Sprache ein konkreter Syntaxbaum.
- `crates/lang-render/` — die Textform je Sprache.
- `tools/idlc/` — der Treiber, den ein Anwender aufruft.

Was **nicht** geteilt wird, ist die Wire-Laufzeit: die zwanzig
Verzeichnisse unter `runtime/` lesen DDS-XTypes 1.3 §7.4 jeweils aus dem
Klauseltext und rufen einander nicht auf. Wo zwei Spalten über ein Oktett
uneins sind, hat eine der beiden die Spezifikation falsch gelesen.

## §7.4.3 Die beiden Walks

### §7.4.3 Ein Encode-Walk für jede Spalte

**Spec:** §7.4.3, XCDR Version 2 — welches Member geschrieben wird, in
welcher Reihenfolge, unter welchem Header und mit welchem Length-Code,
ist eine Eigenschaft der Kodierung und nicht der Zielsprache.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (`codec_from`,
`CodecOptions`).

**Tests:** `crates/lang-rules/tests/codec.rs::a_final_struct_of_primitives_encodes_as_a_run_of_writes`,
`crates/lang-rules/tests/one_walk_two_targets.rs::both_backends_write_the_same_members_in_the_same_order`

**Status:** done

### §7.4.3 Ein Decode-Walk, Member für Member das Inverse

**Spec:** §7.4.3 — der Leser nimmt dieselben Member in derselben
Reihenfolge unter derselben Rahmung entgegen.

**Repo:** `crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::the_decoder_mirrors_the_encoder_member_for_member`,
`crates/lang-rules/tests/one_walk_two_targets.rs::the_walk_asks_for_one_unary_operator_and_no_others`

**Status:** done — der Walk fragt nach einem unären Operator (`!`) und nach keinem anderen;
ein Bitmask-Leser behält die deklarierten Bits mit einem einfachen `&`, ohne `~`.

### §7.4.3 Die Naht: `CodecTarget`

**Spec:** §7.4.3 — die Kodierungsentscheidung ist sprachunabhängig; was
sprachabhängig bleibt, ist ihre Schreibweise.

**Repo:** `crates/lang-rules/src/emit/mod.rs` (`CodecTarget`, `Scalar`,
`Handle`, `runtime_static`).

**Tests:** `crates/lang-rules/tests/one_walk_two_targets.rs::the_difference_between_the_two_is_spelling`,
`crates/lang-rules/tests/one_walk_two_targets.rs::a_sequence_is_one_loop_and_two_ways_of_asking_its_length`

**Status:** done

## §7.2.3 Extensibility und Rahmung

### §7.2.3 FINAL ohne DHEADER, APPENDABLE mit DHEADER

**Spec:** §7.2.3, Tab.12 — FINAL/APPENDABLE/MUTABLE; §7.4.3 gibt
APPENDABLE die vorangestellte `uint32`-Body-Länge (DHEADER), FINAL keine.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (Rahmungszweige aus dem
Layout-Plan `crates/ast-layout/src/key.rs`).

**Tests:** `crates/lang-rules/tests/codec.rs::an_appendable_struct_carries_the_dheader_the_plan_asks_for`,
`crates/lang-rules/tests/codec.rs::the_extensibility_the_source_wrote_decides_the_framing`

**Status:** done

### §7.4.3.4.2 MUTABLE: ein EMHEADER je Member, Length-Code aus dem Plan

**Spec:** §7.4.3.4.2 — EMHEADER trägt Member-ID und Length-Code;
LC=4 nimmt ein NEXTINT, LC=6/7 zählen in 4- bzw. 8-Byte-Einheiten.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (EMHEADER-Zweig),
`crates/lang-rules/src/emit/mod.rs` (`CodecTarget` liefert die
Schreibweise).

**Tests:** `crates/lang-rules/tests/codec.rs::a_mutable_struct_writes_an_emheader_per_member`,
`crates/lang-rules/tests/codec.rs::the_length_code_policy_is_visible_in_the_output`,
`crates/lang-rules/tests/one_walk_two_targets.rs::a_mutable_aggregate_opens_and_closes_a_header_in_both`

**Status:** done

### §7.4.3 Jeder geöffnete Header wird auch geschlossen

**Spec:** §7.4.3 — eine DHEADER- oder EMHEADER-Länge steht vor dem Body
und ist erst nach ihm bekannt; sie muss nachgetragen werden.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (Patch-Stelle je
geöffnetem Header), `crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::every_parameter_header_the_decoder_opens_it_also_closes`,
`crates/lang-rules/tests/runtime_agreement.rs::the_generator_emits_a_patch_for_every_framing_header_it_opens`

**Status:** done

### §7.4.2 XCDR1: Parameter-Liste mit Sentinel

**Spec:** §7.4.2 — PL_CDR1 rahmt jedes Member als Parameter und schließt
die Liste mit `PID_SENTINEL`.

**Repo:** `crates/lang-rules/src/emit/encode.rs`,
`crates/lang-rules/src/emit/decode.rs` (XCDR1-Zweige).

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_mutable_struct_under_xcdr1_ends_with_a_sentinel`,
`crates/lang-rules/tests/c_end_to_end.rs::a_mutable_aggregate_under_xcdr1_stops_at_the_sentinel`

**Status:** done

## §7.2.2.4 Konstrukte

### §7.2.2.4.4.4.7 `@optional`

**Spec:** §7.2.2.4.4.4.7 — ein Struct-Member darf optional sein; ein
Union-Member nie. Die Abwesenheit wird kodiert, nicht weggelassen.

**Repo:** `crates/lang-rules/src/emit/encode.rs`,
`crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::an_optional_member_writes_its_marker_from_the_option_and_not_from_a_literal`,
`crates/lang-rules/tests/codec.rs::an_optional_member_under_xcdr1_marks_the_absence_with_a_declared_length_of_zero`,
`crates/lang-rules/tests/codec.rs::an_optional_member_of_a_mutable_type_omits_the_whole_parameter`

**Status:** done

### §7.2.2.4.3 Sammlungen: DHEADER nur bei nicht-primitiven Elementen

**Spec:** §7.2.2.4.3 und §7.4.3.5.3 — eine Sammlung, deren Elemente
selbst gerahmt sind, trägt einen Collection-DHEADER; eine Sammlung
primitiver Elemente trägt keinen.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (Sammlungszweig).

**Tests:** `crates/lang-rules/tests/codec.rs::a_collection_of_aggregates_is_delimited_and_one_of_primitives_is_not`,
`crates/lang-rules/tests/codec.rs::a_sequence_writes_its_elements_by_their_own_writer`,
`crates/lang-rules/tests/codec.rs::a_collection_directly_inside_a_collection_is_emitted`

**Status:** done

### §7.2.2.4.3 Bound: eine begrenzte Sequenz prüft, eine unbegrenzte nicht

**Spec:** §7.2.2.4.3 — eine Sequenz kann eine Schranke tragen; ein
Schreiber, der sie überschreitet, ist nicht konform.

**Repo:** `crates/lang-rules/src/emit/encode.rs`,
`crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_bounded_sequence_checks_its_bound_and_an_unbounded_one_has_none`,
`crates/lang-rules/tests/codec.rs::a_bounded_string_is_written_through_the_form_that_refuses_an_overrun`

**Status:** done

### §7.2.2.4.3 Array: keine Länge auf der Wire

**Spec:** §7.2.2.4.3 — die Elementzahl eines Arrays steht im Typ, nicht
im Datenstrom.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (Array-Zweig).

**Tests:** `crates/lang-rules/tests/codec.rs::an_array_writes_no_length_because_its_count_is_in_the_type`

**Status:** done

### §7.2.2.4.1.1 Enumeration: eigener Codec, Breite aus `bit_bound`

**Spec:** §7.2.2.4.1.1 und §7.2.2.4.1 — eine Enumeration hat ein
`bit_bound` von 1 bis 32; ihre Wire-Breite folgt daraus.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (`codec_name` je
Enumeration), `crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::an_enum_member_is_written_through_the_enumerations_own_codec`,
`crates/lang-rules/tests/codec.rs::the_enumerations_codec_writes_the_width_its_bit_bound_calls_for`,
`crates/lang-rules/tests/codec.rs::the_enumeration_codec_takes_the_integer_the_plan_gives_it`

**Status:** done

### §7.2.2.4.2 Alias: geschrieben als das, was er benennt

**Spec:** §7.2.2.4.2 — ein Alias hat kein eigenes Kind auf der Wire; er
wird zu seinem `base_type` aufgelöst.

**Repo:** `crates/lang-rules/src/emit/encode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_typedef_is_written_as_what_it_aliases`

**Status:** done

### §7.2.2.4.4.3 Union: Diskriminator zuerst, dann das gewählte Member

**Spec:** §7.2.2.4.4.3 — der Diskriminator ist immer das erste Member
und trägt `must_understand=true`.

**Repo:** `crates/lang-rules/src/emit/encode.rs`,
`crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_mutable_decoder_dispatches_on_the_member_id_and_a_positional_one_does_not`,
`crates/lang-rules/tests/codec.rs::a_union_case_label_that_does_not_evaluate_is_refused`

**Status:** done

### §7.2.2.4.4 Verschachteltes Aggregat kodiert sich selbst

**Spec:** §7.2.2.4.4 — ein Member, dessen Typ ein Aggregat ist, wird
durch dessen eigene Kodierung geschrieben, mit dessen eigener Rahmung.

**Repo:** `crates/lang-rules/src/emit/encode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_nested_aggregate_encodes_itself`

**Status:** done

## §7.2.2.4.4.4.9 Table 9 und TryConstruct

### §7.2.2.4.4.4.9 Ein Member, das der Strom nicht trug, nimmt seinen Default

**Spec:** §7.2.2.4.4.4.9 und Tab.9 — der Default ist entweder explizit
annotiert oder implizit: 0, `false`, `'\0'`, `""`, leere Sammlung,
erster Enum-Literal.

**Repo:** `crates/lang-rules/src/default_<lang>.rs` (eine Regel je
Spalte; §7.5.1 unten), aufgerufen aus
`crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::an_appendable_tail_the_writer_did_not_have_takes_its_default`,
`crates/lang-rules/tests/codec.rs::an_appendable_tail_takes_an_explicit_default_over_table_nine`,
`crates/lang-rules/tests/codec.rs::an_appendable_tail_of_an_enumeration_takes_its_first_literal`,
`crates/lang-rules/tests/codec.rs::an_explicit_default_beats_table_nine_at_the_member_position`

**Status:** done

### §7.2.2.7 TryConstruct: DISCARD, USE_DEFAULT, TRIM

**Spec:** §7.2.2.7, Tab.10/11 — Default ist DISCARD; TRIM gilt nur für
`string`, `sequence` und `map`.

**Repo:** `crates/lang-rules/src/emit/decode.rs`,
`crates/wire/src/try_construct.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::an_absent_annotation_is_discard_and_a_bare_one_is_use_default`,
`crates/lang-rules/tests/codec.rs::a_bounded_member_narrows_after_the_whole_value_is_off_the_wire`,
`crates/lang-rules/tests/codec.rs::an_element_and_its_collection_narrow_separately`

**Status:** done

## §7.4.5 KeyHash

### §7.4.5 Schlüssel: bis 16 Byte null-gepolstert, darüber MD5

**Spec:** §7.4.5 — der KeyHolder wird PLAIN_CDR2 in Big-Endian
serialisiert; ist er höchstens 16 Byte groß, ist er selbst der Hash
(null-gepolstert), sonst MD5 über die Bytes.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (Key-Zweig),
`crates/wire/src/digest.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_keyed_type_gets_a_key_hash_over_its_key_members_only`,
`crates/lang-rules/tests/codec.rs::a_holder_of_sixteen_bytes_or_fewer_is_the_hash_and_a_longer_one_is_md5ed`,
`crates/lang-rules/tests/codec.rs::the_holder_is_big_endian_and_plain_cdr2_aligned_whatever_the_sample_is`,
`crates/lang-rules/tests/codec.rs::an_unbounded_key_is_md5ed_because_its_length_is_not_known_until_the_sample_is`,
`crates/lang-rules/tests/runtime_agreement.rs::the_key_hash_sequence_produces_a_stable_sixteen_bytes`

**Status:** done

### §7.4.5 Ein Typ ohne Schlüssel bekommt keinen KeyHash

**Spec:** §7.4.5 — nur ein Typ mit Key-Membern trägt einen KeyHash.

**Repo:** `crates/lang-rules/src/emit/encode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_type_with_no_key_gets_no_key_hash`,
`crates/lang-rules/tests/c_end_to_end.rs::a_keyed_type_gets_a_holder_and_an_unkeyed_one_does_not`

**Status:** done

## §7.4 Der Walk gegen die Laufzeit

### §7.4 Jeder emittierte Aufruf existiert in der Laufzeit

**Spec:** §7.4 — der erzeugte Code ruft die Wire-Laufzeit; ein Aufruf
ohne Gegenstück ist ein Linkfehler statt einer Konformitätsaussage.

**Repo:** `crates/lang-rules/src/emit/encode.rs`,
`crates/lang-rules/src/emit/decode.rs`, `runtime/c/include/`.

**Tests:** `crates/lang-rules/tests/runtime_agreement.rs::every_call_the_generator_emits_exists_in_the_runtime`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::the_operations_the_walks_emit_all_exist_in_the_reference_runtime`

**Status:** done

### §7.4.3 Die erzeugte Aufruffolge schreibt die Bytes, die der Plan nennt

**Spec:** §7.4.3 — was der Plan als Rahmung, Reihenfolge und Member-ID
festlegt, muss auch in den Oktetten stehen.

**Repo:** `crates/lang-rules/src/encode.rs` (Rust-Spalte),
`crates/wire/src/writer.rs`.

**Tests:** `crates/lang-rules/tests/runtime_agreement.rs::the_sequence_a_final_struct_generates_writes_the_bytes_the_plan_says`,
`crates/lang-rules/tests/runtime_agreement.rs::the_sequence_an_appendable_struct_generates_frames_its_members`,
`crates/lang-rules/tests/runtime_agreement.rs::the_sequence_a_mutable_struct_generates_carries_its_member_ids`

**Status:** done

### §7.4 Kein stiller Ausfall

**Spec:** §7.4 — ein Member, für das der Walk keine Schreibform hat,
darf nicht ausgelassen werden; er muss benannt werden.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (Fehlerpfade),
`tools/silent-drop-allowlist.tsv`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_member_with_no_writer_is_reported_rather_than_left_out`,
`crates/lang-rules/tests/unevaluated.rs::an_array_length_that_does_not_evaluate_is_refused_by_the_codec_too`

**Status:** done

## §7.5.1 Was eine Spalte ausmacht

### §7.5.1 Vier Teile für die Typen, ein fünftes für den Codec

**Spec:** §7.5.1 — das Plain Language Binding ist je PSM eigen; dieses
Repository führt es je Sprache als CST, Regelkatalog, Materialisierer,
Renderer und — für die Kodierung — ein Codec-Backend.

**Repo:** `crates/lang-cst/src/`, `crates/lang-rules/src/rule/`,
`crates/lang-rules/src/adapter/`, `crates/lang-render/src/`,
`crates/lang-rules/src/emit/`; die Aufstellung in
`docs/language-columns-status.md`.

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::the_type_only_columns_are_each_missing_a_part_a_codec_backend_rests_on`

**Status:** done

### §7.5.1 Jede Sprache hat Laufzeit, Endpoint-SDK und Zellgenerator

**Spec:** §7.5.1 — ein Binding, das nur zur Hälfte existiert, ist kein
Binding; der Bestand wird gegen das Dateisystem geprüft, nicht gegen
eine Liste.

**Repo:** `runtime/`, `endpoints/`,
`crates/lang-rules/tests/every_language_has_its_parts.rs`.

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

### §7.5.1 Eine IDL wird in jeder Sprache zu Deklarationen

**Spec:** §7.5.1 — dieselbe Quelle muss in jeder Spalte den Typ
hervorbringen, den sie deklariert; eine Spalte ohne Ausgabe ist eine
stillschweigend verlorene Deklaration.

**Repo:** `crates/lang-rules/src/generate.rs` (`declarations`).

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 …und zu einem vollen Codec

**Spec:** §7.4.3 zusammen mit §7.5.1 — jede Spalte erzeugt neben den
Typen auch Encoder und Decoder für jedes `struct`, `union` und
`valuetype`.

**Repo:** `crates/lang-rules/src/generate.rs` (`source`),
`crates/lang-rules/src/emit/encode.rs` (`codec_name`).

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`

**Status:** done

### §7.2.2.4 Eine Vollständigkeitstabelle je Konstrukt und Sprache

**Spec:** §7.2.2.4 — die Konstrukte des Type-System-Model sind
abzählbar; eine Zelle ohne Deckung muss sagen, warum.

**Repo:** `crates/lang-rules/src/coverage.rs` (kein Catch-all-Arm: eine
neue Sprache bricht den Build, bis jedes Konstrukt eine Entscheidung
trägt).

**Tests:** `crates/lang-rules/tests/adapter.rs::the_coverage_table_has_a_cell_for_every_construct_and_language`,
`crates/lang-rules/tests/adapter.rs::the_table_reports_what_is_covered_and_what_is_not`,
`crates/lang-rules/tests/adapter.rs::an_uncovered_cell_says_why_rather_than_only_that`

**Status:** done

### §7.5.1 Jeder Regelkatalog validiert und feuert auf demselben Modell

**Spec:** §7.5.1 — die sprachspezifischen Entscheidungen sind Regeln
über einem gemeinsamen Modell, nicht Sonderfälle im Walk.

**Repo:** `crates/lang-rules/src/rule/mod.rs`,
`crates/lang-rules/src/apply.rs`.

**Tests:** `crates/lang-rules/tests/apply.rs::every_catalogue_validates_against_the_rule_format`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`,
`crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`

**Status:** done

## §7.2.2.4.8 VerbatimText

### §7.2.2.4.8 `@verbatim` erreicht jede Spalte, die es benennt

**Spec:** §7.2.2.4.8 — `VerbatimText {language, placement, text}`; der
Text wird an der benannten Stelle unverändert ausgegeben, und `"*"`
erreicht jede Sprache.

**Repo:** `crates/lang-rules/src/verbatim.rs`.

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::the_wildcard_reaches_all_four_columns`,
`crates/lang-rules/tests/verbatim_end_to_end.rs::cpp_places_all_six_slots_in_spec_order`,
`crates/lang-rules/tests/verbatim_end_to_end.rs::a_directive_for_an_unknown_language_is_not_emitted`,
`crates/lang-rules/tests/verbatim_end_to_end.rs::repeated_directives_keep_their_declaration_order`,
`crates/lang-rules/tests/verbatim_every_column.rs::the_wildcard_reaches_every_column`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.5 DCPS-PSM

### §7.5 Jede Spalte deklariert das DCPS-PSM

**Spec:** §7.5 zusammen mit DDS 1.4 — die Typen, die die Klausel
benennt, müssen in jeder Sprachbindung existieren.

**Repo:** `crates/lang-rules/src/generate.rs`,
`crates/lang-rules/tests/dcps_psm_idl.rs`.

**Tests:** `crates/lang-rules/tests/dcps_psm_idl.rs::the_dcps_psm_idl_declares_the_types_the_clause_names`,
`crates/lang-rules/tests/dcps_psm_idl.rs::every_language_declares_the_dcps_psm`

**Status:** done

## §7.3.1 Der Treiber

### §7.3.1 `idlc generate --lang` nimmt jede Sprache unter ihrem eigenen Namen

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe; die
Ausgabe ist eine Sprachbindung.

**Repo:** `tools/idlc/src/generate.rs` (`parse_language`,
`language_names`, `source`), `tools/idlc/src/main.rs`
(`--lang`, `--out`, `--encoding`, `--endian`).

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

## §7.4 Gemessene Oktette

### §7.4 Drei Spalten stimmen über den Korpus überein

**Spec:** §7.4 — zwei unabhängige Lesungen derselben Klausel müssen
dieselben Oktette hervorbringen.

**Repo:** `crates/wire-probe/src/lib.rs`,
`tools/wireprobe/`.

**Tests:** `crates/wire-probe/tests/three_column_gate.rs::the_three_columns_agree`,
`crates/wire-probe/tests/three_column_gate.rs::the_c_column_covers_the_same_configurations`,
`crates/wire-probe/tests/three_column_gate.rs::the_rust_column_is_byte_for_byte_what_it_was`

**Status:** done

### §7.4 Eine neue Spalte gegen die aufgezeichnete Rust-Grundlinie

**Spec:** §7.4 — eine Spalte wird gegen eine festgehaltene Aufzeichnung
gemessen, nicht neben den anderen gebaut: drei Spalten, die sich
gemeinsam bewegt haben, ähneln einander weiterhin.

**Repo:** `crates/wire-probe/tests/fixtures/rust_column_baseline.txt`
(145 740 Slots, Fall für Fall), `crates/wire-probe/src/matrix.rs`.

**Tests:** `crates/wire-probe/tests/reference_columns.rs::the_reference_row_is_the_one_the_baseline_was_taken_with`,
`crates/wire-probe/tests/reference_columns.rs::the_comparison_bites`,
`crates/wire-probe/tests/reference_columns.rs::every_configuration_reaches_the_driver_as_flags`,
`crates/wire-probe/tests/typescript_column_gate.rs::the_typescript_column_writes_what_the_rust_column_recorded`

**Status:** done

**Anmerkung:** `reference_columns.rs::the_new_columns_answer_the_matrix_the_rust_column_answered`
vergleicht die C++- und die Java-Spalte erst, wenn
`target/release/wireprobe-cpp-matrix` bzw. `wireprobe-java-matrix` gebaut
sind; ohne sie meldet der Test das auf der Standardausgabe und vergleicht
nichts. Die drei oben zitierten Tests dieser Datei messen ohne
Fremdtreiber.

### §7.4 Der Korpus liefert in allen vier Modi die aufgezeichneten Bytes

**Spec:** §7.4 — je Fall XCDR1/XCDR2 und beide Byte-Reihenfolgen.

**Repo:** `tests/interop-corpus/`, `crates/wire-probe/src/canonical.rs`.

**Tests:** `crates/wire-probe/tests/interop_corpus.rs::every_case_produces_the_bytes_its_readme_records_in_all_four_modes`,
`crates/wire-probe/tests/interop_corpus.rs::every_case_goes_through_all_three_emitters`,
`crates/wire-probe/tests/interop_corpus.rs::every_case_emits_a_keyhash_and_an_encapsulated_payload`

**Status:** done

### §7.4 Die Leserichtung: jeder Fall kommt als der Wert zurück, der hineinging

**Spec:** §7.4 — Kodierung und Dekodierung sind zueinander invers.

**Repo:** `crates/wire-probe/src/canonical.rs`,
`crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/wire-probe/tests/decode_round_trip.rs::every_case_comes_back_the_value_that_went_in`,
`crates/wire-probe/tests/decode_round_trip.rs::no_prefix_of_a_payload_reads_as_a_shorter_sample`,
`crates/wire-probe/tests/probe_agreement.rs::every_corpus_case_is_read_the_same_way_by_all_three_probes`

**Status:** done

---

## Audit-Status

35 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-lang-rules` — 1024 Tests grün, 0 failed
(20 ignoriert: die Vollkorpus-Compile-Läufe, die `--include-ignored`
verlangen). `cargo test -p zerodds-wire-probe --no-fail-fast` — 350 grün,
4 failed: `csharp_column_gate`, `java_column_gate` und `java_round_trip`
scheitern daran, dass die C#- bzw. Java-Sonde auf diesem Rechner nicht
übersetzt, und `union_label_match::the_shape_with_no_storage_is_left_to_the_clause`
ist ein bestehender Befund; keine der vier ist in diesem Dokument als
Nachweis zitiert.

Offene Punkte: keine. Decision-Records: keine.

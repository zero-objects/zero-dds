# `zerodds-xcdr2-c` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) und `internal/standards/cache/omg/idl-4.2.pdf` (§7) — was die C-Spalte beantwortet.

Der Mechanismus, den diese Spalte mit den zwanzig anderen teilt — die beiden Walks, die Rahmung, Table 9, der KeyHash und der Treiber —, steht in `zerodds-xcdr2-bindings-conformance-1.0.md`. Dieses Dokument führt nur, was die C-Spalte selbst entscheidet, und ihre eigenen Nachweise.

## §7.5.1 Die Teile der Spalte

### §7.5.1 Der konkrete Syntaxbaum und seine Textform

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Deklarationen dieser Spalte werden als Baum gebaut und erst danach als Text geschrieben; kein Emitter setzt Quelltext zusammen.

**Repo:** `crates/lang-cst/src/c/`, `crates/lang-render/src/c.rs`

**Tests:** `crates/lang-render/tests/rendering.rs`: `xtypes_7_5_1_a_c_else_holding_one_if_renders_as_else_if`, `xtypes_7_5_1_a_c_else_holding_anything_else_keeps_its_block`, `xtypes_7_5_1_a_c_struct_renders_with_its_members`, `xtypes_7_5_1_a_c_prototype_with_no_parameters_writes_void`, `xtypes_7_5_1_a_c_pointer_type_binds_to_the_declarator`, `xtypes_7_5_1_a_c_file_writes_its_include_guard_at_both_ends`;
querschnittlich `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 Der Regelkatalog

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Was aus welchem Modellknoten wird, steht als Regelmenge und nicht als Sonderfall im Walk.

**Repo:** `crates/lang-rules/src/rule/c.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 Der Materialisierer

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Der Materialisierer trägt die Eigenheiten der Sprache: Namens- und Schlüsselwortbehandlung, Kollisionen und das, was die Sprache gar nicht ausdrücken kann.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 Das Codec-Backend

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Das Backend liefert die Schreibweise, in der die geteilte Kodierungsentscheidung in dieser Sprache steht — Empfänger, Methodenname, Argumentform.

**Repo:** `crates/lang-rules/src/emit/c.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Der Table-9-Defaulter

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Ein Leser ohne Tab.9-Antwort hat an fünf Stellen keinen definierten Wert; diese Datei ist die Antwort dieser Spalte.

**Repo:** `crates/lang-rules/src/default_c.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Die Wire-Laufzeit

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Laufzeit liest §7.4 eigenständig aus dem Klauseltext und ruft keine andere Spalte; dort liegt die Unabhängigkeit dieser Spalte.

**Repo:** `runtime/c`, Endpoint-SDK `endpoints/c`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 Was diese Spalte selbst entscheidet

### §7.2.2.2 Jedes IDL-Primitiv erreicht einen C-Typ fester Breite

**Spec:** §7.2.2.2 — Primitive Types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; jede Breite ist festgelegt.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_5_1_1_1_every_idl_primitive_reaches_a_c_type`,
`crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_2_2_2_a_struct_of_primitives_becomes_a_c_struct`

**Status:** done

### §7.2.2.2 `long double` ist ein C-Typ in der Deklaration und in beiden Codecs

**Spec:** §7.2.2.2 — FLOAT128 ist ein IEEE-754-binary128-Wert von 16 Oktetten.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_2_2_2_a_long_double_is_one_c_type_in_the_declaration_and_in_both_codecs`

**Status:** done

### §7.2.2.4.3 Eine Sequenz wird eine Länge und ein Puffer

**Spec:** §7.2.2.4.3 — Sequence — eindimensional, Schranke optional.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_5_1_a_sequence_becomes_a_length_and_a_buffer`,
`crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_2_2_4_3_two_collections_of_the_same_shape_are_one_declaration`

**Status:** done

### §7.2.2.4.4.4.7 Ein optionales Member ist ein Zeiger, und NULL ist das abwesende


**Spec:** §7.2.2.4.4.4.7 — Ein Struct-Member darf optional sein; ein Union-Member nie.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_5_1_2_4_1_an_optional_member_is_a_pointer_and_null_is_the_absent_one`,
`crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_2_2_4_4_4_7_an_optional_member_is_written_only_when_it_is_there`

**Status:** done

### §7.2.2.4.1.1 Eine Enumeration wird ein Integer mit Konstanten und kein C-`enum`

**Spec:** §7.2.2.4.1.1 — Enumeration — Int32-Literale, geordnet, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_5_1_2_1_1_an_enumeration_becomes_an_integer_and_constants_not_a_c_enum`,
`crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_2_2_4_1_1_an_unknown_enumerator_is_refused_rather_than_handed_back_as_a_number`

**Status:** done

### IDL 4.2 §7.4.13 Ein `bitset` wird getrennte Integer und keine C-Bitfelder

**Spec:** IDL 4.2 §7.4.13 — `bitset` mit `bitfield`-Membern deklarierter Breite.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::idl_7_4_13_a_bitset_becomes_separate_integers_not_c_bitfields`,
`crates/lang-rules/tests/c_end_to_end.rs::idl_7_4_13_a_bitset_that_inherits_packs_the_base_at_the_low_positions`

**Status:** done

### §7.2.2.4.4.3 Eine Union wird ein Diskriminator neben einer C-`union`

**Spec:** §7.2.2.4.4.3 — Union — der Diskriminator ist immer das erste Member und trägt `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_5_1_4_a_union_becomes_a_discriminator_beside_a_union`,
`crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_2_2_4_4_3_a_union_selects_its_branch_by_the_discriminator`

**Status:** done

### IDL 4.2 §7.5.2 `module` wird ein Präfix, weil C keine Namensräume hat

**Spec:** IDL 4.2 §7.5.2 — `module` ist ein Naming-Scope; ein Identifier ist je Scope einmalig.

**Repo:** `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::idl_7_5_2_a_module_becomes_a_prefix_because_c_has_no_namespaces`

**Status:** done

### §7.2.2.4.4.4.9 Jede Zeile von Tab.9 hat einen C-Default, und jeder gerufene Defaulter ist definiert

**Spec:** §7.2.2.4.4.4.9 — Ein Member, das der Strom nicht trug, nimmt seinen Default nach Tab.9.

**Repo:** `crates/lang-rules/src/default_c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_2_2_4_4_4_7_every_table_nine_row_has_a_c_default`,
`crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_2_2_4_4_4_9_every_defaulter_the_decoder_calls_is_also_defined`,
`crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_2_2_4_4_4_9_an_explicit_default_beats_table_nine`

**Status:** done

### §7.4.3 Encoder und Decoder tragen verschiedene Namen, ebenso Daten- und Key-Encoder

**Spec:** §7.4.3 — XCDR Version 2 — Plain CDR2, Delimited CDR2 und PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_4_3_the_encoder_and_the_decoder_do_not_share_a_name`,
`crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_4_3_the_data_and_key_encoders_do_not_share_a_name`,
`crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_4_3_a_decoder_reads_a_primitive_through_the_runtime_and_not_by_hand`

**Status:** done

## §7.3.1 Woran die Spalte gehalten wird

### §7.3.1 Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat

**Spec:** §7.3.1 — Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat. Ein Fall, der zu rendern aufhört, würde sonst erst von der nächsten Messung gefunden — ein Build-Verzeichnis vom Konstrukt entfernt, das ihn verursacht hat.

**Repo:** `crates/lang-rules/src/emit/c.rs`, `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_3_1_a_member_of_type_any_is_refused_by_name_rather_than_emitted`

**Status:** done

### §7.5.2 Ein Typ ist deklariert, bevor er benutzt wird

**Spec:** §7.5.2 — Ein Typ ist deklariert, bevor er benutzt wird. Die Reihenfolge der Ausgabe ist eine Eigenschaft der Zielsprache und nicht der Quelle.

**Repo:** `crates/lang-rules/src/emit/c.rs`, `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_5_2_a_type_is_declared_before_it_is_used`

**Status:** done

### §7.3.1 Zwei Läufe über eine Quelle erzeugen denselben Text

**Spec:** §7.3.1 — Zwei Läufe über eine Quelle erzeugen denselben Text. Ohne diese Eigenschaft ist jeder Diff eines erzeugten Baums unlesbar.

**Repo:** `crates/lang-rules/src/emit/c.rs`, `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::xtypes_7_3_1_two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.4 Die beiden C-Generatoren lesen den Korpus identisch

**Spec:** §7.4 — Die beiden C-Generatoren lesen den Korpus identisch.

**Repo:** `crates/lang-rules/src/emit/c.rs`, `crates/lang-rules/src/adapter/c.rs`

**Tests:** `crates/wire-probe/tests/probe_agreement.rs::the_two_c_generators_read_the_corpus_identically`

**Status:** done

### §7.4 Die Oktette der Spalte werden gegen eine feste Aufzeichnung gehalten

**Spec:** §7.4 — Die Oktette der Spalte werden gegen eine feste Aufzeichnung gehalten: `tests/interop-corpus/expected-bytes.tsv`, die Oktette der Hersteller, nicht die anderen Spalten: drei Spalten, die sich gemeinsam bewegt haben, ähneln einander weiterhin.

**Repo:** `crates/lang-rules/src/emit/c.rs`, `crates/lang-rules/src/adapter/c.rs`

**Tests:** `tools/wireprobe/wireprobe-zerodds-c-gen/tests/expected_bytes.rs::specification`, `tools/wireprobe/wireprobe-zerodds-c-gen/tests/expected_bytes.rs::cyclone`, `tools/wireprobe/wireprobe-zerodds-c-gen/tests/expected_bytes.rs::rti`, `tools/wireprobe/wireprobe-zerodds-c-gen/tests/expected_bytes.rs::fastdds`, `tools/wireprobe/wireprobe-zerodds-c-gen/tests/expected_bytes.rs::omniorb`, `tools/wireprobe/wireprobe-zerodds-c-gen/tests/expected_bytes.rs::tao`, `tools/wireprobe/wireprobe-zerodds-c-gen/tests/expected_bytes.rs::jacorb`, `tools/wireprobe/wireprobe-zerodds-c-gen/tests/expected_bytes.rs::opendds`, `tools/wireprobe/wireprobe-zerodds-c-gen/tests/expected_bytes.rs::ros_fastrtps`, `tools/wireprobe/wireprobe-zerodds-c-gen/tests/expected_bytes.rs::ros_cyclone`

**Status:** done

### §7.2.2.4.8 `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt

**Spec:** §7.2.2.4.8 — `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt.

**Repo:** `crates/lang-rules/src/adapter/c.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_1_a_directive_reaches_its_own_column_and_no_other`,
`crates/lang-rules/tests/verbatim_end_to_end.rs::xtypes_7_2_2_4_8_c_places_the_file_slots_and_the_declaration_text_inside_the_struct`

**Status:** done

## §7.3.1 Treiber und Messung

### §7.3.1 `idlc generate --lang c`

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe der Erzeugung; die Ausgabe ist die Quelle dieser Spalte.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::deployment_1_3_every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::xtypes_7_3_1_language_names_lists_every_language_once`

**Status:** done

### §7.4 Der Messaufbau dieser Spalte

**Spec:** §7.4 — die Oktette einer Spalte werden gegen aufgezeichnete Oktette verglichen und nicht in ihren eigenen Tests behauptet.

**Repo:** `tools/wireprobe/wireprobe-zerodds-c-gen/`, `tools/wireprobe/wireprobe-zerodds-c`, `tools/wireprobe/wireprobe-zerodds-c-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_4_every_language_has_a_cell_generator`

**Status:** done

---

## Audit-Status

24 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-xcdr2-c-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (Job `merge-rust-idlc`) — 2 Tests grün.
* `cargo nextest run --profile merge -p zerodds-lang-render --all-targets` (Job `merge-rust-lang-render`) — 28 Tests grün.
* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (Job `merge-rust-lang-rules`) — 12 Tests grün.
* `cargo nextest run --profile merge -p zerodds-wire-probe --all-targets` (Job `merge-rust-wire-probe`) — 1 Test grün.
* `cargo nextest run --profile merge -p zerodds-wireprobe-c-gen --all-targets` (Job `merge-rust-wireprobe-c-gen`) — 10 Tests grün.
* `python3 ci/run_integration.py merge-codegen-c` (Job `merge-codegen-c`) — 23 Tests grün.

76 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.

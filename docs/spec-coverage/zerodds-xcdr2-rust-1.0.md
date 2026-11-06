# `zerodds-xcdr2-rust` 1.0 — Spec-Coverage

**Quelle:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) und `internal/standards/cache/omg/idl-4.2.pdf` (§7) — was die Rust-Spalte beantwortet.

Der Mechanismus, den diese Spalte mit den zwanzig anderen teilt — die beiden Walks, die Rahmung, Table 9, der KeyHash und der Treiber —, steht in `zerodds-xcdr2-bindings-conformance-1.0.md`. Dieses Dokument führt nur, was die Rust-Spalte selbst entscheidet, und ihre eigenen Nachweise.

Die Rust-Spalte ist die älteste und die Referenz, gegen die jede andere gemessen wird. Zwei Dinge sind an ihr eigen: ihr Materialisierer heißt `crates/lang-rules/src/adapter/cst.rs` — er war der erste und wurde nach der Aufgabe benannt, nicht nach der Sprache —, und ihr Codec wird von `crates/lang-rules/src/encode.rs` direkt gegen den Rust-CST gebaut statt über `emit::CodecTarget`. Ihre Wire-Laufzeit ist die Crate `crates/wire` und kein Verzeichnis unter `runtime/`.

## §7.5.1 Die Teile der Spalte

### §7.5.1 Der konkrete Syntaxbaum und seine Textform

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Deklarationen dieser Spalte werden als Baum gebaut und erst danach als Text geschrieben; kein Emitter setzt Quelltext zusammen.

**Repo:** `crates/lang-cst/src/rust/`, `crates/lang-render/src/rust.rs`

**Tests:** `crates/lang-render/tests/rendering.rs`: `xtypes_7_5_1_a_rust_struct_renders_with_its_derives_on_one_line`, `xtypes_7_5_1_a_rust_enum_renders_its_variants_and_discriminants`, `xtypes_7_5_1_a_rust_impl_renders_its_method_body`, `xtypes_7_5_1_a_rust_generic_type_renders_its_arguments`, `xtypes_7_5_1_a_rust_one_element_tuple_keeps_its_comma`, `xtypes_7_5_1_a_rust_array_length_renders_as_the_expression_it_is`;
querschnittlich `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 Der Regelkatalog

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Was aus welchem Modellknoten wird, steht als Regelmenge und nicht als Sonderfall im Walk.

**Repo:** `crates/lang-rules/src/rule/rust.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 Der Materialisierer

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Der Materialisierer trägt die Eigenheiten der Sprache: Namens- und Schlüsselwortbehandlung, Kollisionen und das, was die Sprache gar nicht ausdrücken kann.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 Das Codec-Backend

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Das Backend liefert die Schreibweise, in der die geteilte Kodierungsentscheidung in dieser Sprache steht — Empfänger, Methodenname, Argumentform.

**Repo:** `crates/lang-rules/src/encode.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Der Table-9-Defaulter

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Ein Leser ohne Tab.9-Antwort hat an fünf Stellen keinen definierten Wert; diese Datei ist die Antwort dieser Spalte.

**Repo:** `crates/lang-rules/src/default_rust.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 Die Wire-Laufzeit

**Spec:** §7.5.1 — Plain Language Binding — die Abbildung auf die Zielsprache ist PSM-eigen. Die Laufzeit liest §7.4 eigenständig aus dem Klauseltext und ruft keine andere Spalte; dort liegt die Unabhängigkeit dieser Spalte.

**Repo:** `crates/wire`, Endpoint-SDK `endpoints/rust`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 Was diese Spalte selbst entscheidet

### §7.2.2.2 Jedes IDL-Primitiv erreicht einen Rust-Typ

**Spec:** §7.2.2.2 — Primitive Types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; jede Breite ist festgelegt.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_2_every_idl_primitive_reaches_a_rust_type`,
`crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_2_a_struct_of_primitives_becomes_a_rust_struct`

**Status:** done

### §7.2.2.4.4.4.7 Ein optionales Member wird eine `Option`, die anderen nicht

**Spec:** §7.2.2.4.4.4.7 — Ein Struct-Member darf optional sein; ein Union-Member nie.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_4_4_7_an_optional_member_becomes_an_option_and_the_others_do_not`

**Status:** done

### §7.2.2.4.3 Eine Sequenz wird ein `Vec`, ein `string` ein `String`; ein Array verschachtelt äußerste Dimension zuerst

**Spec:** §7.2.2.4.3 — Sequence — eindimensional, Schranke optional.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_3_a_sequence_becomes_a_vec_and_a_string_a_string`,
`crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_3_an_array_nests_outermost_dimension_first`

**Status:** done

### §7.2.2.4.2 Ein `typedef` wird ein Type-Alias und löst zugleich im Member auf

**Spec:** §7.2.2.4.2 — Alias — `typedef` auf `base_type`, ohne eigenes Kind beim Typvergleich.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_2_a_typedef_becomes_a_type_alias_as_well_as_resolving_in_a_member`,
`crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_2_a_typedef_resolves_to_what_it_aliases`

**Status:** done

### §7.2.2.4.2 Ein `typedef`, ein Member oder ein Union-Zweig hält einen eigenen Rust-Typ (`rust_type`)

**Spec:** §7.2.2.4.2 — Alias — `typedef` auf `base_type`, ohne eigenes Kind beim Typvergleich; §7.5.1 — die Abbildung auf die Zielsprache ist die des PSM. `zerodds_build::Config::rust_type` lässt einen Rust-Typ mit `zerodds_wire::WireRepr` für ein Typedef, einen Struct-Member oder einen Union-Zweig stehen (GitHub #33, `internal/release/rc8-rust-newtype.md`); Oktette, TypeObject, TypeIdentifier und Key-Hash bleiben die der IDL, ein abgelehnter Wert weist das Sample mit seinem Grund zurück.

**Repo:** `crates/zerodds-build/src/rust_type.rs`, `crates/lang-rules/src/adapter/cst.rs` (`RustTypeScope`), `crates/lang-rules/src/encode.rs` (`MemberWrite::Mapped`), `crates/lang-rules/src/default_rust.rs`, `crates/wire/src/repr.rs`

**Tests:** `crates/zerodds-build/tests/dds_type.rs::xtypes_7_4_issue_33_a_rust_type_writes_and_reads_the_octets_of_its_idl_type`,
`crates/zerodds-build/tests/dds_type.rs::xtypes_7_4_issue_33_a_c_peer_reads_what_the_rust_type_wrote`,
`crates/zerodds-build/tests/dds_type.rs::issue_33_a_rust_type_of_another_idl_type_fails_the_build`,
`crates/zerodds-build/tests/rust_type.rs::xtypes_7_3_4_issue_33_the_type_objects_and_identifiers_are_the_idls`,
`crates/zerodds-build/tests/rust_type.rs::issue_33_a_path_no_rust_type_can_stand_for_fails_the_build`,
`crates/wire/tests/repr.rs::xtypes_7_4_1_1_2_a_displayed_string_is_the_octets_of_its_text`,
`crates/wire/tests/repr.rs::xtypes_7_4_1_1_2_a_displayed_string_over_its_bound_or_with_a_nul_is_refused_whole`,
`crates/wire/tests/repr.rs::xtypes_7_4_1_1_2_read_str_lends_the_text_out_of_the_sample`,
`crates/wire/tests/repr.rs::issue_33_a_zero_for_a_non_zero_type_is_refused_with_its_reason`,
`crates/wire/tests/repr.rs::issue_33_display_from_str_writes_the_text_and_refuses_what_from_str_refuses`

**Status:** done

### §7.5.1 Ein in Rust definierter Topic-Typ ist der Typ seiner IDL (`derive(DdsType)`)

**Spec:** §7.5.1 — die Abbildung auf die Zielsprache ist die des PSM; §7.3.4 — TypeObject und TypeIdentifier; §7.4 — die Oktette unter XCDR1 und XCDR2; §7.6.8 — der Key-Holder. `derive(DdsType)` aus `zerodds-codec` (Crate `zerodds-derive`) definiert den Typ in Rust (GitHub #33, `internal/release/rc8-rust-newtype.md`, „Derive route“, „Codec API“): Seine Beschreibung wird zu IDL, die `zerodds-idl-compose`, `zerodds-types` und `zerodds-ast-layout` wie eine `.idl`-Datei durchläuft, und die Pläne treiben den Serializer von `zerodds-wire`. Ein Enum mit Daten ist eine Union, unter `key` mit ihrem Diskriminator als Key (`switch (@key D)`, §7.2.2.4.4.4.8), jede Instanziierung eines generischen Typs ein eigener IDL-Typ, und ein geliehenes Feld wird aus dem Sample geliehen. Jedes Aggregat im Key-Holder ist sein eigener Holder, `@final`, mit nur seinen Key-Membern, falls es welche hat (§7.6.8 Schritt 3); ein Inhaltsfilter liest die Zweige einer Union und ihren `#discriminator` (DDS-XTypes 1.4 Beta 1 §7.6.7.1). Je Konstrukt haben der abgeleitete Typ und der aus der gleichwertigen IDL erzeugte Typ denselben TypeIdentifier, dasselbe TypeObject, dieselben Abhängigkeiten, dieselben Oktette in allen vier Kodierungen und denselben Key-Holder; ein aus der exportierten IDL erzeugtes C-Gegenüber liest die Oktette; ein von Hand geschriebener Codec wird beim Übersetzen an der Beschreibung gemessen.

**Repo:** `crates/derive/src/lib.rs`, `crates/codec/src/idl.rs`, `crates/codec/src/model.rs`, `crates/codec/src/lib.rs`, `crates/wire/src/codec/`, `crates/lang-rules/src/encode.rs` (der Key-Holder-Leser eines Key-Members vom Struct-Typ)

**Tests:** `crates/zerodds-build/tests/dds_type.rs::xtypes_7_3_4_7_4_issue_33_a_derived_type_and_its_idl_are_one_type`,
`crates/zerodds-build/tests/dds_type.rs::xtypes_7_4_issue_33_a_c_peer_reads_what_a_derived_type_wrote`,
`crates/zerodds-build/tests/dds_type.rs::issue_33_a_hand_written_codec_missing_or_adding_a_member_does_not_compile`,
`crates/zerodds-build/tests/dds_type.rs::issue_33_a_hand_written_codec_of_another_type_or_description_does_not_compile`,
`crates/codec/tests/derive_behaviour.rs::xtypes_7_4_3_5_a_value_over_its_bound_is_refused_and_writes_nothing`,
`crates/codec/tests/derive_behaviour.rs::issue_33_a_value_the_rust_type_refuses_rejects_the_sample_with_its_reason`,
`crates/codec/tests/derive_behaviour.rs::xtypes_7_4_3_4_a_mutable_type_reads_its_members_by_id_in_any_order`,
`crates/codec/tests/derive_behaviour.rs::xtypes_7_2_2_4_4_4_2_an_appendable_type_reads_a_newer_and_an_older_writer`,
`crates/codec/tests/derive_behaviour.rs::issue_33_a_derived_type_exports_the_idl_it_stands_for`,
`crates/codec/tests/derive_behaviour.rs::dds_1_4_2_3_3_duration_infinite_and_the_values_a_time_t_cannot_hold`,
`crates/codec/tests/derive_behaviour.rs::dds_1_4_b_2_1_a_derived_type_answers_the_content_filter`,
`crates/codec/tests/derive_behaviour.rs::xtypes_7_6_3_1_2_the_encapsulation_identifiers`,
`crates/codec/tests/derive_behaviour.rs::xtypes_7_6_8_the_key_hash_is_the_holder_or_its_md5`,
`crates/zerodds-build/tests/dds_type.rs::xtypes_7_4_issue_33_a_c_peer_reads_a_derived_union_and_generic_type`,
`crates/zerodds-build/tests/dds_type.rs::issue_33_a_hand_written_union_codec_of_another_branch_type_does_not_compile`,
`crates/codec/tests/derive_union_generic_borrowed.rs::idl_4_2_7_4_1_4_4_4_a_data_carrying_enum_exports_its_union`,
`crates/codec/tests/derive_union_generic_borrowed.rs::xtypes_7_4_3_5_3_rule_10_the_discriminator_a_variant_carries_goes_out`,
`crates/codec/tests/derive_union_generic_borrowed.rs::idl_4_2_7_4_1_4_4_4_a_discriminator_of_another_case_is_refused`,
`crates/codec/tests/derive_union_generic_borrowed.rs::issue_33_each_instantiation_of_a_generic_type_has_its_own_name`,
`crates/codec/tests/derive_union_generic_borrowed.rs::issue_33_a_borrowed_field_is_lent_out_of_the_sample`,
`crates/codec/tests/derive_union_generic_borrowed.rs::xtypes_7_6_8_the_key_holder_of_a_union_keyed_by_its_discriminator`,
`crates/codec/tests/derive_union_generic_borrowed.rs::xtypes_1_4_7_6_7_1_a_filter_reads_the_selected_branch_and_the_discriminator`,
`crates/zerodds-build/tests/dds_type.rs::xtypes_7_6_8_a_nested_key_member_is_its_own_final_key_holder`,
`crates/codec/tests/contract_types.rs::xtypes_7_3_4_5_the_object_of_a_derived_type_is_the_vendors`

**Status:** done

### §7.2.2.4.1.1 Eine Enumeration wird ein Rust-`enum` mit ausgeschriebener Numerierung

**Spec:** §7.2.2.4.1.1 — Enumeration — Int32-Literale, geordnet, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_1_1_an_enum_becomes_a_rust_enum_with_its_numbering_written_out`

**Status:** done

### §7.2.2.4.4.3 Eine Union wird ein `enum` mit einer Variante je Case

**Spec:** §7.2.2.4.4.3 — Union — der Diskriminator ist immer das erste Member und trägt `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`, die Form in
`crates/lang-rules/src/rust_union.rs`: ein Case mit einem Label hält das
Member, ein Case mit mehreren Labels und der `default:`-Case halten den
Diskriminator davor, und `_NoMember(<Diskriminator>)` hält einen
Diskriminator, der kein Member wählt (IDL 4.2 §7.4.1.4.4.4). Der Codec
schreibt einen getragenen Diskriminator nach der Prüfung gegen die Labels
des Case, einen `boolean`- oder `char`-Diskriminator als `true`/`false`
und als Zeichen.

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_4_3_a_union_becomes_an_enum_with_one_variant_per_case`,
`crates/lang-rules/tests/rust_corpus_compiles.rs::xtypes_7_2_2_4_4_3_a_union_keeps_its_discriminator_through_rustc_and_back`

**Status:** done

### §7.2.2.4.1.2 Eine Bitmask wird ein Integer mit einer Konstanten je Bit; ein Bitset ein Struct holdertypisierter Felder

**Spec:** §7.2.2.4.1.2 — Bitmask — Schranke 1..64, benannte Bits.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_1_2_a_bitmask_becomes_an_integer_and_one_constant_per_bit`,
`crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_1_2_a_bitset_becomes_a_struct_of_holder_typed_fields`

**Status:** done

### IDL 4.2 §7.2.3 Ein Member, das ein Rust-Keyword benennt, wird entschärft

**Spec:** IDL 4.2 §7.2.3 — Identifier sind ASCII und beim Vergleich case-insensitiv; eine Kollision muss gemeldet werden.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::idl_7_2_3_a_member_named_after_a_rust_keyword_is_escaped`

**Status:** done

### IDL 4.2 §7.4.3 `interface` wird ein Trait von Signaturen; `native` ein opaker Alias

**Spec:** IDL 4.2 §7.4.3 — `interface` — Operationen mit `in`/`out`/`inout`-Parametern.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::idl_7_4_3_an_interface_becomes_a_trait_of_signatures`,
`crates/lang-rules/tests/end_to_end.rs::idl_7_4_3_a_native_declaration_becomes_an_opaque_alias`

**Status:** done

### §7.4 Die erzeugte Aufruffolge schreibt die Bytes, die der Plan nennt

**Spec:** §7.4 — Data Representation — die erzeugte Quelle ruft eine Laufzeit, die §7.4 aus dem Klauseltext liest.

**Repo:** `crates/lang-rules/src/encode.rs`

**Tests:** `crates/lang-rules/tests/runtime_agreement.rs::xtypes_7_4_every_call_the_generator_emits_exists_in_the_runtime`,
`crates/lang-rules/tests/runtime_agreement.rs::xtypes_7_4_3_the_sequence_a_final_struct_generates_writes_the_bytes_the_plan_says`,
`crates/lang-rules/tests/runtime_agreement.rs::xtypes_7_4_3_the_sequence_a_mutable_struct_generates_carries_its_member_ids`,
`crates/lang-rules/tests/runtime_agreement.rs::xtypes_7_4_5_the_key_hash_sequence_produces_a_stable_sixteen_bytes`

**Status:** done

## §7.3.1 Woran die Spalte gehalten wird

### §7.3.1 Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat

**Spec:** §7.3.1 — Jeder Korpusfall rendert oder benennt die Schicht, die ihn abgelehnt hat. Ein Fall, der zu rendern aufhört, würde sonst erst von der nächsten Messung gefunden — ein Build-Verzeichnis vom Konstrukt entfernt, das ihn verursacht hat.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_3_1_every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.4 Ein Konstrukt ohne Datenform in dieser Sprache wird benannt und nicht ausgelassen

**Spec:** §7.4 — Ein Konstrukt ohne Datenform in dieser Sprache wird benannt und nicht ausgelassen. Ein stillschweigend ausgelassenes Konstrukt ist die schlimmste Fehlerklasse: der Empfänger sieht keinen Fehler, sondern falsche Daten.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_4_a_construct_with_no_rule_produces_nothing_rather_than_something_wrong`

**Status:** done

### §7.3.1 Zwei Läufe über eine Quelle erzeugen denselben Text

**Spec:** §7.3.1 — Zwei Läufe über eine Quelle erzeugen denselben Text. Ohne diese Eigenschaft ist jeder Diff eines erzeugten Baums unlesbar.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_3_1_two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe

**Spec:** §7.2.2.4 — Jedes Konstrukt, das die Vollständigkeitstabelle für diese Spalte behauptet, erzeugt auch Ausgabe. Die Tabelle in `crates/lang-rules/src/coverage.rs` ist die Behauptung; dieser Test ist ihre Prüfung.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_every_construct_the_table_claims_for_rust_produces_output`

**Status:** done

### §7.4 Die Oktette der Spalte werden gegen eine feste Aufzeichnung gehalten

**Spec:** §7.4 — Die Oktette der Spalte werden gegen eine feste Aufzeichnung gehalten. Gemessen gegen die Rust-Spalte, die `crates/wire-probe/tests/expected_bytes.rs` gegen die Oktette der Hersteller hält, nicht neben den anderen Spalten: drei Spalten, die sich gemeinsam bewegt haben, ähneln einander weiterhin.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/wire-probe/tests/expected_bytes.rs::specification`

**Status:** done

### §7.2.2.4.8 `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt

**Spec:** §7.2.2.4.8 — `@verbatim` landet an den Stellen, die die Klausel für diese Sprache nennt.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_1_a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Treiber und Messung

### §7.3.1 `idlc generate --lang rust`

**Spec:** §7.3.1 — die IDL-Type-Representation ist die Eingabe der Erzeugung; die Ausgabe ist die Quelle dieser Spalte.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::deployment_1_3_every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::xtypes_7_3_1_language_names_lists_every_language_once`

**Status:** done

### §7.4 Der Messaufbau dieser Spalte

**Spec:** §7.4 — die Oktette einer Spalte werden gegen aufgezeichnete Oktette verglichen und nicht in ihren eigenen Tests behauptet.

**Repo:** `tools/wireprobe/wireprobe-zerodds-gen/`, `crates/wire-probe/src/main.rs`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_4_every_language_has_a_cell_generator`

**Status:** done

---

## Audit-Status

26 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9301 von `main` (`python3 ci/coverage_results.py --document zerodds-xcdr2-rust-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-build --all-targets` (Job `merge-rust-build`) — 2 Tests grün.
* `cargo nextest run --profile merge -p zerodds-codec --all-targets` (Job `merge-rust-codec`) — 15 Tests grün.
* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (Job `merge-rust-idlc`) — 2 Tests grün.
* `cargo nextest run --profile merge -p zerodds-lang-render --all-targets` (Job `merge-rust-lang-render`) — 28 Tests grün.
* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (Job `merge-rust-lang-rules`) — 33 Tests grün.
* `cargo nextest run --profile merge -p zerodds-wire --all-targets` (Job `merge-rust-wire`) — 5 Tests grün.
* `cargo nextest run --profile merge -p zerodds-wire-probe --all-targets` (Job `merge-rust-wire-probe`) — 1 Test grün.
* `python3 ci/run_integration.py merge-codegen-rust` (Job `merge-codegen-rust`) — 1 Test grün.
* `python3 ci/run_integration.py integration-codegen-build` (Job `integration-codegen-build`) — 9 Tests grün.

96 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.

# `zerodds-xcdr2-rust` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the Rust column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the Rust column decides for itself, and its own evidence.

The Rust column is the oldest and the reference every other one is measured against. Two things are its own: its materialiser is `crates/lang-rules/src/adapter/cst.rs` — it was the first and the file was named for the job rather than for the language — and its codec is built by `crates/lang-rules/src/encode.rs` directly against the Rust CST rather than through `emit::CodecTarget`. Its wire runtime is the `crates/wire` crate and not a directory under `runtime/`.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/rust/`, `crates/lang-render/src/rust.rs`

**Tests:** `crates/lang-render/tests/rendering.rs`: `xtypes_7_5_1_a_rust_struct_renders_with_its_derives_on_one_line`, `xtypes_7_5_1_a_rust_enum_renders_its_variants_and_discriminants`, `xtypes_7_5_1_a_rust_impl_renders_its_method_body`, `xtypes_7_5_1_a_rust_generic_type_renders_its_arguments`, `xtypes_7_5_1_a_rust_one_element_tuple_keeps_its_comma`, `xtypes_7_5_1_a_rust_array_length_renders_as_the_expression_it_is`;
across all renderers `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/rust.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/encode.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_rust.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `crates/wire`, endpoint SDK `endpoints/rust`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches a Rust type

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_2_every_idl_primitive_reaches_a_rust_type`,
`crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_2_a_struct_of_primitives_becomes_a_rust_struct`

**Status:** done

### §7.2.2.4.4.4.7 An optional member becomes an `Option` and the others do not

**Spec:** §7.2.2.4.4.4.7 — A structure member may be optional; a union member never is.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_4_4_7_an_optional_member_becomes_an_option_and_the_others_do_not`

**Status:** done

### §7.2.2.4.3 A sequence becomes a `Vec` and a `string` a `String`; an array nests outermost dimension first

**Spec:** §7.2.2.4.3 — Sequence — one-dimensional, bound optional.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_3_a_sequence_becomes_a_vec_and_a_string_a_string`,
`crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_3_an_array_nests_outermost_dimension_first`

**Status:** done

### §7.2.2.4.2 A `typedef` becomes a type alias as well as resolving in a member

**Spec:** §7.2.2.4.2 — Alias — `typedef` onto `base_type`, with no kind of its own when types are compared.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_2_a_typedef_becomes_a_type_alias_as_well_as_resolving_in_a_member`,
`crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_2_a_typedef_resolves_to_what_it_aliases`

**Status:** done

### §7.2.2.4.2 A `typedef`, a member or a union branch holds a Rust type of its own (`rust_type`)

**Spec:** §7.2.2.4.2 — Alias — `typedef` onto `base_type`, with no kind of its own when types are compared; §7.5.1 — the mapping onto the target language is the PSM's own. `zerodds_build::Config::rust_type` lets a Rust type with `zerodds_wire::WireRepr` stand for a typedef, a struct member or a union branch (GitHub #33, `internal/release/rc8-rust-newtype.md`); the octets, the TypeObject, the TypeIdentifier and the key hash stay the IDL's, and a refused value rejects the sample with its reason.

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

### §7.5.1 A topic type defined in Rust is the type of its IDL (`derive(DdsType)`)

**Spec:** §7.5.1 — the mapping onto the target language is the PSM's own; §7.3.4 — the TypeObject and the TypeIdentifier; §7.4 — the octets under XCDR1 and XCDR2; §7.6.8 — the key holder. `derive(DdsType)` of `zerodds-codec` (crate `zerodds-derive`) defines the type in Rust (GitHub #33, `internal/release/rc8-rust-newtype.md`, "Derive route", "Codec API"): its description becomes IDL, which goes through `zerodds-idl-compose`, `zerodds-types` and `zerodds-ast-layout` as a `.idl` file does, and the plans drive the serializer of `zerodds-wire`. A data-carrying enum is a union, keyed by its discriminator under `key` (`switch (@key D)`, §7.2.2.4.4.4.8), each instantiation of a generic type its own IDL type, and a borrowed field is lent out of the sample. Each aggregate inside a key holder is its own holder, `@final`, with only its key members when it has any (§7.6.8 step 3); a content filter reads a union's branches and its `#discriminator` (DDS-XTypes 1.4 Beta 1 §7.6.7.1). Per construct the derived type and the type generated from the equivalent IDL have the same TypeIdentifier, TypeObject, dependencies, octets in all four encodings and key holder; a C peer generated from the exported IDL reads the octets; a hand-written codec is held to the description at compile time.

**Repo:** `crates/derive/src/lib.rs`, `crates/codec/src/idl.rs`, `crates/codec/src/model.rs`, `crates/codec/src/lib.rs`, `crates/wire/src/codec/`, `crates/lang-rules/src/encode.rs` (the key-holder reader of a struct-typed key member)

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

### §7.2.2.4.1.1 An enum becomes a Rust enum with its numbering written out

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_1_1_an_enum_becomes_a_rust_enum_with_its_numbering_written_out`

**Status:** done

### §7.2.2.4.4.3 A union becomes an enum with one variant per case

**Spec:** §7.2.2.4.4.3 — Union — the discriminator is always the first member and carries `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`, the shape in
`crates/lang-rules/src/rust_union.rs`: a case with one label holds the
member, a case with several labels and the `default:` case hold the
discriminator ahead of it, and `_NoMember(<discriminator>)` holds a
discriminator that selects no member (IDL 4.2 §7.4.1.4.4.4). The codec
writes a carried discriminator after checking it against the case's
labels, and a `boolean` or `char` discriminator as `true`/`false` and a
character.

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_4_3_a_union_becomes_an_enum_with_one_variant_per_case`,
`crates/lang-rules/tests/rust_corpus_compiles.rs::xtypes_7_2_2_4_4_3_a_union_keeps_its_discriminator_through_rustc_and_back`

**Status:** done

### §7.2.2.4.1.2 A bitmask becomes an integer with one constant per bit; a bitset a struct of holder-typed fields

**Spec:** §7.2.2.4.1.2 — Bitmask — bound 1..64, named bits.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_1_2_a_bitmask_becomes_an_integer_and_one_constant_per_bit`,
`crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_1_2_a_bitset_becomes_a_struct_of_holder_typed_fields`

**Status:** done

### IDL 4.2 §7.2.3 A member named after a Rust keyword is escaped

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::idl_7_2_3_a_member_named_after_a_rust_keyword_is_escaped`

**Status:** done

### IDL 4.2 §7.4.3 `interface` becomes a trait of signatures; `native` an opaque alias

**Spec:** IDL 4.2 §7.4.3 — `interface` — operations with `in`/`out`/`inout` parameters.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::idl_7_4_3_an_interface_becomes_a_trait_of_signatures`,
`crates/lang-rules/tests/end_to_end.rs::idl_7_4_3_a_native_declaration_becomes_an_opaque_alias`

**Status:** done

### §7.4 The generated call sequence writes the bytes the plan names

**Spec:** §7.4 — Data representation — the generated source calls a runtime that reads §7.4 from the clause text.

**Repo:** `crates/lang-rules/src/encode.rs`

**Tests:** `crates/lang-rules/tests/runtime_agreement.rs::xtypes_7_4_every_call_the_generator_emits_exists_in_the_runtime`,
`crates/lang-rules/tests/runtime_agreement.rs::xtypes_7_4_3_the_sequence_a_final_struct_generates_writes_the_bytes_the_plan_says`,
`crates/lang-rules/tests/runtime_agreement.rs::xtypes_7_4_3_the_sequence_a_mutable_struct_generates_carries_its_member_ids`,
`crates/lang-rules/tests/runtime_agreement.rs::xtypes_7_4_5_the_key_hash_sequence_produces_a_stable_sixteen_bytes`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_3_1_every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.4 A construct with no data form in this language is named and not left out

**Spec:** §7.4 — A construct with no data form in this language is named and not left out. A silently dropped construct is the worst class of fault: the receiver sees no error, it sees wrong data.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_4_a_construct_with_no_rule_produces_nothing_rather_than_something_wrong`

**Status:** done

### §7.3.1 Two runs over one source produce the same text

**Spec:** §7.3.1 — Two runs over one source produce the same text. Without it every diff of a generated tree is unreadable.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_3_1_two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Every construct the completeness table claims for this column also produces output

**Spec:** §7.2.2.4 — Every construct the completeness table claims for this column also produces output. The table in `crates/lang-rules/src/coverage.rs` is the claim; this test is what checks it.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/lang-rules/tests/end_to_end.rs::xtypes_7_2_2_4_every_construct_the_table_claims_for_rust_produces_output`

**Status:** done

### §7.4 The column's octets are held against a fixed recording

**Spec:** §7.4 — The column's octets are held against a fixed recording. Measured against the Rust column, which `crates/wire-probe/tests/expected_bytes.rs` holds against the vendors' octets, not beside the other columns: three columns that moved together still resemble each other.

**Repo:** `crates/lang-rules/src/encode.rs`, `crates/lang-rules/src/adapter/cst.rs`

**Tests:** `crates/wire-probe/tests/expected_bytes.rs::specification`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/cst.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_1_a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang rust`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::deployment_1_3_every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::xtypes_7_3_1_language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-gen/`, `crates/wire-probe/src/main.rs`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_4_every_language_has_a_cell_generator`

**Status:** done

---

## Audit Status

26 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9301 of `main` (`python3 ci/coverage_results.py --document zerodds-xcdr2-rust-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-build --all-targets` (job `merge-rust-build`) — 2 tests green.
* `cargo nextest run --profile merge -p zerodds-codec --all-targets` (job `merge-rust-codec`) — 15 tests green.
* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (job `merge-rust-idlc`) — 2 tests green.
* `cargo nextest run --profile merge -p zerodds-lang-render --all-targets` (job `merge-rust-lang-render`) — 28 tests green.
* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (job `merge-rust-lang-rules`) — 33 tests green.
* `cargo nextest run --profile merge -p zerodds-wire --all-targets` (job `merge-rust-wire`) — 5 tests green.
* `cargo nextest run --profile merge -p zerodds-wire-probe --all-targets` (job `merge-rust-wire-probe`) — 1 test green.
* `python3 ci/run_integration.py merge-codegen-rust` (job `merge-codegen-rust`) — 1 test green.
* `python3 ci/run_integration.py integration-codegen-build` (job `integration-codegen-build`) — 9 tests green.

96 cited tests green, 0 not green.

Open items: none.

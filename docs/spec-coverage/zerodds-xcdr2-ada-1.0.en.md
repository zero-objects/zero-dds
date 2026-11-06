# `zerodds-xcdr2-ada` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the Ada column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the Ada column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/ada/`, `crates/lang-render/src/ada.rs`

**Tests:** `crates/lang-render/src/ada.rs::tests` (26 tests), among them `xtypes_7_5_1_a_record_writes_one_component_per_line`, `xtypes_7_5_1_a_record_with_no_components_is_a_null_record`, `xtypes_7_5_1_a_discriminated_record_writes_its_variant_part`, `xtypes_7_5_1_an_enumeration_and_its_representation_clause_are_two_declarations`, `xtypes_7_5_1_an_unconstrained_array_and_the_access_type_that_reaches_it`;
across all renderers `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/ada.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/ada.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_ada.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/ada`, endpoint SDK `endpoints/ada`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches an `Interfaces` type

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_2_every_idl_primitive_reaches_an_interfaces_type`,
`crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_2_no_generated_type_has_an_implementation_defined_width`

**Status:** done

### §7.2.2.2 `long double` becomes text and never an Ada float

**Spec:** §7.2.2.2 — FLOAT128 is an IEEE 754 binary128 value of sixteen octets.

**Repo:** `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_2_a_long_double_is_text_and_never_an_ada_float`

**Status:** done

### §7.2.2.3 `string` becomes `Unbounded_String` and never `String`

**Spec:** §7.2.2.3 — STRING8 over Char8 and STRING16 over Char16, bounded or unbounded.

**Repo:** `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_3_a_string_is_an_unbounded_string_and_never_an_ada_string`

**Status:** done

### §7.2.2.4.4.4.7 An optional member becomes a discriminated record

**Spec:** §7.2.2.4.4.4.7 — A structure member may be optional; a union member never is.

**Repo:** `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_4_4_4_7_an_optional_member_becomes_a_discriminated_record`

**Status:** done

### §7.2.2.4.3 `map` becomes a sequence of entries and never a hashed container

**Spec:** §7.2.2.4.3 — Map — key/value pairs; §7.4.3.5.3 rules 11-16 fix the order and the framing.

**Repo:** `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_4_3_a_map_becomes_a_sequence_of_entries_and_never_a_hashed_container`

**Status:** done

### §7.2.2.4.1.1 An enumeration carries its numbering in a representation clause

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_4_1_1_an_enum_carries_its_numbering_in_a_representation_clause`,
`crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_4_1_1_an_enum_whose_values_do_not_ascend_is_refused_by_name`

**Status:** done

### §7.2.2.4.4.3 A union becomes a record with a variant part

**Spec:** §7.2.2.4.4.3 — Union — the discriminator is always the first member and carries `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_4_4_3_a_union_becomes_a_discriminated_record_with_a_variant_part`,
`crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_4_4_3_a_union_default_branch_becomes_the_others_arm`

**Status:** done

### IDL 4.2 §7.2.3 A reserved word is escaped with a letter in front, folding case

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::idl_7_2_3_a_member_named_after_a_reserved_word_escapes_with_a_letter`,
`crates/lang-rules/tests/ada_end_to_end.rs::idl_7_2_3_the_reserved_word_check_folds_case_because_ada_does`,
`crates/lang-rules/tests/ada_end_to_end.rs::idl_7_2_3_two_members_that_fold_onto_one_ada_identifier_are_refused_by_name`

**Status:** done

### IDL 4.2 §7.5.2 `module` flattens onto the name because a declarative part is contiguous

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::idl_7_5_2_a_module_flattens_onto_the_name_because_a_declarative_part_is_contiguous`

**Status:** done

### §7.2.2.4.9 Recursion is cut with an incomplete type declaration

**Spec:** §7.2.2.4.9 — A self-referential type needs a place where the descent terminates.

**Repo:** `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_4_9_a_recursive_type_is_cut_with_an_incomplete_type_declaration`,
`crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_4_9_mutual_recursion_needs_only_one_incomplete_declaration`,
`crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_4_9_a_recursion_with_no_collection_and_no_reference_is_refused_by_name`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/ada.rs`, `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_3_1_every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 The generated source compiles under the language's own compiler

**Spec:** §7.3.1 — The generated source compiles under the language's own compiler. A snapshot says the emitter produced the text it produced last time; it does not say the text is the language. Where the compiler is absent the test says so on standard error.

**Repo:** `crates/lang-rules/src/emit/ada.rs`, `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_3_1_the_generated_ada_compiles`

**Status:** done

### §7.3.1 Two runs over one source produce the same text

**Spec:** §7.3.1 — Two runs over one source produce the same text. Without it every diff of a generated tree is unreadable.

**Repo:** `crates/lang-rules/src/emit/ada.rs`, `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_3_1_two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Every construct the completeness table claims for this column also produces output

**Spec:** §7.2.2.4 — Every construct the completeness table claims for this column also produces output. The table in `crates/lang-rules/src/coverage.rs` is the claim; this test is what checks it.

**Repo:** `crates/lang-rules/src/emit/ada.rs`, `crates/lang-rules/src/adapter/ada.rs`

**Tests:** `crates/lang-rules/tests/ada_end_to_end.rs::xtypes_7_2_2_4_every_construct_the_table_claims_for_ada_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/ada.rs`, `crates/lang-rules/src/adapter/ada83.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_1_a_directive_reaches_its_own_column_and_no_other`,
`crates/lang-rules/tests/verbatim_every_column.rs::xtypes_7_2_2_4_8_ada_writes_its_file_text_outside_the_package`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang ada`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::deployment_1_3_every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::xtypes_7_3_1_language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-ada-gen/`, `tools/wireprobe/wireprobe-zerodds-ada`, `tools/wireprobe/wireprobe-ada-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_4_every_language_has_a_cell_generator`

**Status:** done

---

## Audit Status

23 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-xcdr2-ada-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (job `merge-rust-idlc`) — 2 tests green.
* `cargo nextest run --profile merge -p zerodds-lang-render --all-targets` (job `merge-rust-lang-render`) — 7 tests green.
* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (job `merge-rust-lang-rules`) — 12 tests green.
* `python3 ci/run_integration.py merge-codegen-ada` (job `merge-codegen-ada`) — 21 tests green.

42 cited tests green, 0 not green.

Open items: none.

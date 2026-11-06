# `zerodds-xcdr2-elixir` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the Elixir column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the Elixir column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/elixir/`, `crates/lang-render/src/elixir.rs`

**Tests:** `crates/lang-render/src/elixir.rs::tests` (20 tests), among them `xtypes_7_5_1_a_module_is_a_do_end_block_indented_by_two_spaces`, `xtypes_7_5_1_a_module_name_joins_its_segments_with_dots`, `xtypes_7_5_1_a_moduledoc_is_a_heredoc_at_the_top_of_the_body`, `xtypes_7_5_1_a_struct_with_no_fields_is_defstruct_with_an_empty_list`, `xtypes_7_5_1_a_struct_of_several_short_fields_stays_on_one_line`;
across all renderers `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/elixir.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/elixir.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_elixir.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/elixir`, endpoint SDK `endpoints/elixir`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every width and signedness is written into the attribute, and every attribute has a function that reads it

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_2_2_2_every_width_and_signedness_is_written_into_the_attribute`,
`crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_2_2_2_every_attribute_the_output_writes_has_a_function_that_reads_it`

**Status:** done

### §7.2.2.2 `long double` becomes a binary and never a float

**Spec:** §7.2.2.2 — FLOAT128 is an IEEE 754 binary128 value of sixteen octets.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_2_2_2_a_long_double_is_a_binary_and_never_a_float`

**Status:** done

### §7.2.2.3 `string`, `wstring` and `fixed` are all binaries with their facts beside them

**Spec:** §7.2.2.3 — STRING8 over Char8 and STRING16 over Char16, bounded or unbounded.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_2_2_3_a_string_a_wstring_and_a_fixed_are_all_binaries_with_their_facts_beside_them`

**Status:** done

### §7.2.2.4.3 A sequence is a list and an array a tuple, and nothing is truncated

**Spec:** §7.2.2.4.3 — Sequence — one-dimensional, bound optional.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_2_2_4_3_a_sequence_is_a_list_and_an_array_is_a_tuple`,
`crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_2_2_4_3_an_array_of_a_thousand_is_written_out_and_nothing_truncates_it`

**Status:** done

### §7.2.2.4.3 A `map` is a list of pairs and never an Elixir map

**Spec:** §7.2.2.4.3 — Map — key/value pairs; §7.4.3.5.3 rules 11-16 fix the order and the framing.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_2_2_4_3_a_map_is_a_list_of_pairs_and_never_an_elixir_map`

**Status:** done

### §7.2.2.4.1.1 An enum becomes a module of atoms with its numbering written out

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_2_2_4_1_1_an_enum_becomes_a_module_of_atoms_with_its_numbering_written_out`,
`crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_2_2_4_1_1_a_member_of_an_enumeration_is_an_integer_and_not_the_enumeration`

**Status:** done

### §7.2.2.4.4.3 A union is a struct with a discriminator and not a tagged tuple

**Spec:** §7.2.2.4.4.3 — Union — the discriminator is always the first member and carries `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_2_2_4_4_3_a_union_is_a_struct_with_a_discriminator_and_not_a_tagged_tuple`,
`crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_2_2_4_4_3_a_default_case_is_recorded_as_the_default_and_not_as_a_number`

**Status:** done

### IDL 4.2 §7.2.3 A reserved word is escaped, and a capital becomes a lower-case atom that records its spelling

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::idl_7_2_3_a_member_named_after_an_elixir_reserved_word_is_escaped_and_keeps_its_idl_name`,
`crates/lang-rules/tests/elixir_end_to_end.rs::idl_7_2_3_an_idl_member_with_a_capital_becomes_a_lower_case_atom_that_records_its_spelling`,
`crates/lang-rules/tests/elixir_end_to_end.rs::idl_7_2_3_two_members_that_would_reach_one_atom_are_refused_by_name`

**Status:** done

### IDL 4.2 §7.5.2 `module` nests because Elixir module names nest; `Elixir` itself is escaped

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::idl_7_5_2_a_module_nests_because_elixir_module_names_nest`,
`crates/lang-rules/tests/elixir_end_to_end.rs::idl_7_5_2_a_module_named_elixir_is_escaped_because_it_is_the_root_namespace`

**Status:** done

### §7.4.3 Every shape encodes and decodes, and the generated Elixir compiles without warnings

**Spec:** §7.4.3 — XCDR version 2 — plain CDR2, delimited CDR2 and PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_codec.rs::xtypes_7_4_3_every_shape_encodes_and_the_generated_elixir_compiles_without_warnings`,
`crates/lang-rules/tests/elixir_codec.rs::xtypes_7_4_3_every_shape_decodes_and_the_generated_elixir_compiles_without_warnings`,
`crates/lang-rules/tests/elixir_codec.rs::xtypes_7_4_3_the_compiler_check_catches_a_deliberate_error`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/elixir.rs`, `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_3_1_every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 The generated source compiles under the language's own compiler

**Spec:** §7.3.1 — The generated source compiles under the language's own compiler. A snapshot says the emitter produced the text it produced last time; it does not say the text is the language. Where the compiler is absent the test says so on standard error.

**Repo:** `crates/lang-rules/src/emit/elixir.rs`, `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_3_1_the_generated_elixir_compiles`

**Status:** done

### §7.3.1 Two runs over one source produce the same text

**Spec:** §7.3.1 — Two runs over one source produce the same text. Without it every diff of a generated tree is unreadable.

**Repo:** `crates/lang-rules/src/emit/elixir.rs`, `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_3_1_two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Every construct the completeness table claims for this column also produces output

**Spec:** §7.2.2.4 — Every construct the completeness table claims for this column also produces output. The table in `crates/lang-rules/src/coverage.rs` is the claim; this test is what checks it.

**Repo:** `crates/lang-rules/src/emit/elixir.rs`, `crates/lang-rules/src/adapter/elixir.rs`

**Tests:** `crates/lang-rules/tests/elixir_end_to_end.rs::xtypes_7_2_2_4_every_construct_the_column_claims_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/elixir.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_1_a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang elixir`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::deployment_1_3_every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::xtypes_7_3_1_language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-elixir-gen/`, `tools/wireprobe/wireprobe-zerodds-elixir`, `tools/wireprobe/wireprobe-elixir-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_4_every_language_has_a_cell_generator`

**Status:** done

---

## Audit Status

23 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-xcdr2-elixir-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (job `merge-rust-idlc`) — 2 tests green.
* `cargo nextest run --profile merge -p zerodds-lang-render --all-targets` (job `merge-rust-lang-render`) — 7 tests green.
* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (job `merge-rust-lang-rules`) — 11 tests green.
* `python3 ci/run_integration.py merge-codegen-elixir` (job `merge-codegen-elixir`) — 32 tests green.

52 cited tests green, 0 not green.

Open items: none.

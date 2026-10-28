# `zerodds-xcdr2-nim` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the Nim column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the Nim column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/nim/`, `crates/lang-render/src/nim.rs`

**Tests:** `crates/lang-render/src/nim.rs::tests` (17 tests), among them `an_object_indents_its_fields_by_two_under_the_type_section`, `an_object_with_no_fields_is_a_bare_object`, `an_enumeration_writes_one_field_per_line_with_no_export_marker`, `an_empty_section_writes_nothing_because_the_keyword_alone_is_a_syntax_error`, `a_const_carries_its_declared_type_before_the_value`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/nim.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/nim.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/nim.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_nim.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/nim`, endpoint SDK `endpoints/nim`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches a Nim type and none has an implementation-defined width

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/nim.rs`

**Tests:** `crates/lang-rules/tests/nim_end_to_end.rs::every_idl_primitive_reaches_a_nim_type`,
`crates/lang-rules/tests/nim_end_to_end.rs::no_generated_type_has_an_implementation_defined_width`

**Status:** done

### §7.2.2.2.1.2 An IDL `char` becomes a `uint8` and never a Nim `char`

**Spec:** §7.2.2.2.1.2 — Char8 is unspecified (UTF-8 by default), Char16 is UTF-16 over the BMP.

**Repo:** `crates/lang-rules/src/adapter/nim.rs`

**Tests:** `crates/lang-rules/tests/nim_end_to_end.rs::an_idl_char_is_a_uint8_and_never_a_nim_char`,
`crates/lang-rules/tests/nim_end_to_end.rs::a_long_double_is_a_string_and_never_a_float64`

**Status:** done

### §7.2.2.4.4.4.7 An optional member becomes an `Option`; the `options` import appears exactly then

**Spec:** §7.2.2.4.4.4.7 — A structure member may be optional; a union member never is.

**Repo:** `crates/lang-rules/src/adapter/nim.rs`

**Tests:** `crates/lang-rules/tests/nim_end_to_end.rs::an_optional_member_becomes_an_option_and_the_others_do_not`,
`crates/lang-rules/tests/nim_end_to_end.rs::the_options_import_appears_exactly_when_an_option_does`

**Status:** done

### §7.2.2.4.9 An `@external` member is held by reference whether or not it recurses

**Spec:** §7.2.2.4.9 — `@external` — the member is held by reference; self-reference is admitted.

**Repo:** `crates/lang-rules/src/adapter/nim.rs`

**Tests:** `crates/lang-rules/tests/nim_end_to_end.rs::an_external_member_is_held_by_reference_whether_or_not_it_recurses`,
`crates/lang-rules/tests/nim_end_to_end.rs::a_recursive_optional_external_member_is_finite`,
`crates/lang-rules/tests/nim_codec.rs::an_external_member_reads_through_zd_ref_and_writes_the_dereference`

**Status:** done

### IDL 4.2 §7.2.3 A Nim keyword is stropped; so is a name that only normalises onto one

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/nim.rs`

**Tests:** `crates/lang-rules/tests/nim_end_to_end.rs::a_member_named_after_a_nim_keyword_is_stropped_and_keeps_its_spelling`,
`crates/lang-rules/tests/nim_end_to_end.rs::a_name_that_only_normalises_onto_a_keyword_is_stropped_too`,
`crates/lang-rules/tests/nim_end_to_end.rs::two_members_nim_cannot_tell_apart_are_refused_by_name`

**Status:** done

### §7.2.2.4.3 A `map` becomes a `seq` of entries and never a `Table`

**Spec:** §7.2.2.4.3 — Map — key/value pairs; §7.4.3.5.3 rules 11-16 fix the order and the framing.

**Repo:** `crates/lang-rules/src/adapter/nim.rs`

**Tests:** `crates/lang-rules/tests/nim_end_to_end.rs::a_map_becomes_a_seq_of_entries_and_never_a_table`,
`crates/lang-rules/tests/nim_end_to_end.rs::a_sequence_becomes_a_seq_and_a_string_a_string`

**Status:** done

### §7.2.2.4.1.1 An enum becomes one enum with every ordinal written out; a non-ascending one is refused

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/nim.rs`

**Tests:** `crates/lang-rules/tests/nim_end_to_end.rs::an_enum_becomes_one_enum_with_every_ordinal_written_out`,
`crates/lang-rules/tests/nim_end_to_end.rs::an_enumerator_that_does_not_ascend_is_refused_by_name`

**Status:** done

### IDL 4.2 §7.5.2 `module` flattens onto the name because a Nim module is a file; one type section holds everything

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/nim.rs`

**Tests:** `crates/lang-rules/tests/nim_end_to_end.rs::a_module_flattens_onto_the_name_because_a_nim_module_is_a_file`,
`crates/lang-rules/tests/nim_end_to_end.rs::one_type_section_holds_every_declaration`,
`crates/lang-rules/tests/nim_end_to_end.rs::a_type_may_name_itself_inside_the_one_section`

**Status:** done

### §7.4.3 A forward declaration precedes every body; the operators are Nim's and not C's

**Spec:** §7.4.3 — XCDR version 2 — plain CDR2, delimited CDR2 and PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/nim.rs`

**Tests:** `crates/lang-rules/tests/nim_codec.rs::a_forward_declaration_precedes_every_body`,
`crates/lang-rules/tests/nim_codec.rs::the_operators_are_nims_and_not_cs`,
`crates/lang-rules/tests/nim_codec.rs::the_shapes_the_corpus_has_reach_the_end_of_both_walks`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/nim.rs`, `crates/lang-rules/src/adapter/nim.rs`

**Tests:** `crates/lang-rules/tests/nim_end_to_end.rs::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 The generated source compiles under the language's own compiler

**Spec:** §7.3.1 — The generated source compiles under the language's own compiler. A snapshot says the emitter produced the text it produced last time; it does not say the text is the language. Where the compiler is absent the test says so on standard error.

**Repo:** `crates/lang-rules/src/emit/nim.rs`, `crates/lang-rules/src/adapter/nim.rs`

**Tests:** `crates/lang-rules/tests/nim_end_to_end.rs::the_generated_nim_compiles`

**Status:** done

### §7.3.1 Two runs over one source produce the same text

**Spec:** §7.3.1 — Two runs over one source produce the same text. Without it every diff of a generated tree is unreadable.

**Repo:** `crates/lang-rules/src/emit/nim.rs`, `crates/lang-rules/src/adapter/nim.rs`

**Tests:** `crates/lang-rules/tests/nim_end_to_end.rs::two_runs_over_one_source_produce_the_same_text`

**Status:** done

### §7.2.2.4 Every construct the completeness table claims for this column also produces output

**Spec:** §7.2.2.4 — Every construct the completeness table claims for this column also produces output. The table in `crates/lang-rules/src/coverage.rs` is the claim; this test is what checks it.

**Repo:** `crates/lang-rules/src/emit/nim.rs`, `crates/lang-rules/src/adapter/nim.rs`

**Tests:** `crates/lang-rules/tests/nim_end_to_end.rs::every_construct_the_table_claims_for_nim_produces_output`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/nim.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang nim`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-nim-gen/`, `tools/wireprobe/wireprobe-zerodds-nim`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

22 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.

# `zerodds-xcdr2-ts` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the TypeScript column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the TypeScript column decides for itself, and its own evidence.

The page is named `ts` for historical reasons; the column itself is `typescript` — that is how `idlc generate --lang` takes it, how it is spelled in `crates/lang-cst/src/common.rs`, and what its directory under `runtime/` is called.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/typescript/`, `crates/lang-render/src/typescript.rs`

**Tests:** `crates/lang-cst/src/typescript/mod.rs::tests` (9 tests):
`every_type_keyword_writes_its_own_spelling`,
`a_reserved_binding_escapes_with_a_trailing_underscore`,
`a_contextual_keyword_is_a_legal_binding_and_keeps_its_spelling`,
`a_predefined_type_name_is_reserved_although_javascript_does_not_reserve_it`,
`a_property_name_keeps_its_text_whichever_form_it_takes`,
`a_bigint_literal_type_carries_its_suffix_in_the_written_form`;
`crates/lang-render/tests/dispatch_chains.rs::a_node_dispatch_chain_of_ten_thousand_arms_renders_flat`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/typescript.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/typescript.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_typescript.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/typescript`, endpoint SDK `endpoints/typescript`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 A sixty-four-bit integer is a `bigint` and everything narrower a `number`

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_sixty_four_bit_integer_is_a_bigint_and_everything_narrower_is_a_number`,
`crates/lang-rules/src/adapter/typescript.rs::tests::a_bigint_field_starts_from_a_bigint_literal`

**Status:** done

### §7.5.1 A struct becomes an interface and a factory and never a class

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_struct_is_an_interface_and_a_factory_and_never_a_class`

**Status:** done

### §7.2.2.4.3 A `map` member becomes a readonly map over a staged entry type

**Spec:** §7.2.2.4.3 — Map — key/value pairs; §7.4.3.5.3 rules 11-16 fix the order and the framing.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_map_member_is_a_readonly_map_over_a_staged_entry_type`,
`crates/lang-rules/src/adapter/typescript.rs::tests::two_maps_of_one_typescript_shape_and_two_idl_shapes_are_two_entry_types`

**Status:** done

### §7.2.2.4.4.4.7 An optional member is a question mark and not a null

**Spec:** §7.2.2.4.4.4.7 — A structure member may be optional; a union member never is.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::an_optional_member_is_a_question_mark_and_not_a_null`,
`crates/lang-rules/tests/typescript_reader_generates.rs::an_optional_member_gets_a_reader`

**Status:** done

### §7.2.2.4.4.3 A union becomes a discriminated union narrowed on a literal

**Spec:** §7.2.2.4.4.3 — Union — the discriminator is always the first member and carries `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_union_is_a_discriminated_union_narrowed_on_a_literal`,
`crates/lang-rules/src/adapter/typescript.rs::tests::a_union_discriminant_takes_the_discriminators_own_literal_form`

**Status:** done

### §7.2.2.4.1.1 An enumeration becomes an `as const` object beside a string literal union

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::an_enumeration_is_an_as_const_object_beside_a_string_literal_union`,
`crates/lang-rules/tests/typescript_reader_generates.rs::a_signed_enumerator_pattern_gets_a_reader`

**Status:** done

### IDL 4.2 §7.4.1.4 A constant past the safe integer range is refused rather than rounded

**Spec:** IDL 4.2 §7.4.1.4 — `const` — a value of its declared type, evaluated at translation time.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_constant_past_the_safe_integer_range_is_refused_rather_than_rounded`,
`crates/lang-rules/src/adapter/typescript.rs::tests::an_unevaluated_array_length_is_named_and_not_substituted`

**Status:** done

### IDL 4.2 §7.5.2 `module` becomes a namespace; a reopened module stays one

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_module_is_a_namespace_and_two_modules_do_not_collide`,
`crates/lang-rules/src/adapter/typescript.rs::tests::a_module_reopened_beside_a_nested_one_is_one_namespace`,
`crates/lang-rules/src/adapter/typescript.rs::tests::a_nested_name_that_would_capture_a_root_reference_is_refused`

**Status:** done

### §7.4 The generated module names nothing the runtime does not have

**Spec:** §7.4 — Data representation — the generated source calls a runtime that reads §7.4 from the clause text.

**Repo:** `runtime/typescript`

**Tests:** `crates/lang-rules/tests/typescript_names_its_runtime.rs::the_backend_names_nothing_the_runtime_does_not_export`,
`crates/lang-rules/tests/typescript_names_its_runtime.rs::what_the_runtime_carries_and_the_backend_has_not_reached`

**Status:** done

### §7.4.3 Every framing shape gets a reader, and the generated reader type-checks

**Spec:** §7.4.3 — XCDR version 2 — plain CDR2, delimited CDR2 and PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/typescript.rs`

**Tests:** `crates/lang-rules/tests/typescript_reader_generates.rs::a_mutable_struct_gets_a_reader`,
`crates/lang-rules/tests/typescript_reader_generates.rs::an_appendable_struct_gets_a_reader`,
`crates/lang-rules/tests/typescript_reader_generates.rs::the_generated_reader_type_checks`,
`crates/lang-rules/tests/typescript_codec.rs::a_structure_of_primitives_becomes_a_typescript_encoder`

**Status:** done

### IDL 4.2 §7.4.3 `interface` becomes a client, a handler and a service descriptor

**Spec:** IDL 4.2 §7.4.3 — `interface` — operations with `in`/`out`/`inout` parameters.

**Repo:** `crates/lang-rules/src/adapter/interface.rs`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::typescript_maps_an_interface_to_a_client_a_handler_and_a_descriptor`,
`crates/lang-rules/tests/interface_bindings.rs::typescript_aggregates_out_parameters_into_the_resolved_value`,
`crates/lang-rules/tests/interface_bindings.rs::typescript_writes_the_service_descriptor_of_eleven_five`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/typescript.rs`, `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.3.1 The generated source compiles under the language's own compiler

**Spec:** §7.3.1 — The generated source compiles under the language's own compiler. A snapshot says the emitter produced the text it produced last time; it does not say the text is the language. Where the compiler is absent the test says so on standard error.

**Repo:** `crates/lang-rules/src/emit/typescript.rs`, `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/tests/typescript_descriptors.rs::the_generated_types_type_check`

**Status:** done

### §7.4 A construct with no data form in this language is named and not left out

**Spec:** §7.4 — A construct with no data form in this language is named and not left out. A silently dropped construct is the worst class of fault: the receiver sees no error, it sees wrong data.

**Repo:** `crates/lang-rules/src/emit/typescript.rs`, `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/lang-rules/src/adapter/typescript.rs::tests::a_construct_with_no_typescript_data_form_is_named_and_not_left_out`

**Status:** done

### §7.4 The column's octets are held against a fixed recording

**Spec:** §7.4 — The column's octets are held against a fixed recording. Measured against `crates/wire-probe/tests/fixtures/rust_column_baseline.txt`, not beside the other columns: three columns that moved together still resemble each other.

**Repo:** `crates/lang-rules/src/emit/typescript.rs`, `crates/lang-rules/src/adapter/typescript.rs`

**Tests:** `crates/wire-probe/tests/typescript_column_gate.rs::the_typescript_column_writes_what_the_rust_column_recorded`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/typescript.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang typescript`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-typescript-gen/`, `tools/wireprobe/wireprobe-zerodds-typescript`, `tools/wireprobe/wireprobe-typescript-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

24 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.

# `zerodds-xcdr2-cpp` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the C++ column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the C++ column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/cpp/`, `crates/lang-render/src/cpp.rs`

**Tests:** `crates/lang-render/tests/rendering.rs`: `a_cpp_class_renders_its_access_sections_in_order`, `a_cpp_accessor_with_one_statement_stays_on_one_line`, `a_cpp_constructor_with_initialisers_puts_them_on_their_own_line`, `a_cpp_expression_brackets_only_where_the_tree_asked_for_it`, `a_cpp_namespace_body_is_not_indented`, `a_cpp_template_specialisation_renders_its_header_above_the_declaration`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/cpp.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/cpp.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_cpp.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/cpp`, endpoint SDK `endpoints/cpp`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every member takes the runtime type and never the compiler's

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::a_long_double_is_the_runtime_type_and_never_the_compiler_s`

**Status:** done

### §7.2.2.4.4.4.7 An optional member is a pointer and not a second field


**Spec:** §7.2.2.4.4.4.7 — A structure member may be optional; a union member never is.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::an_optional_member_is_a_plain_pointer_and_not_a_second_field`,
`crates/lang-rules/tests/cpp_reader_generates.rs::an_optional_member_gets_a_reader`

**Status:** done

### §7.2.2.4.3 A sequence becomes a `vector` and a `map` the class `zerodds::Map` over a `vector` of pairs


**Spec:** §7.2.2.4.3 — Sequence — one-dimensional, bound optional.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::a_sequence_is_a_vector_and_a_map_is_a_class_over_a_vector_of_pairs`

**Status:** done

### §7.2.2.4.3 An array nests with the outer dimension outermost

**Spec:** §7.2.2.4.3 — Array — multi-dimensional, fixed element count, row-major.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::an_array_nests_with_the_outer_dimension_outermost`

**Status:** done

### §7.2.2.4.1.1 An enumeration takes an underlying type from its bit bound

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::an_enumeration_takes_an_underlying_type_from_its_bit_bound`,
`crates/lang-rules/tests/cpp_reader_generates.rs::an_enumeration_whose_first_literal_is_not_zero`

**Status:** done

### §7.2.2.4.4.3 A union becomes a class with a discriminator and one field per case

**Spec:** §7.2.2.4.4.3 — Union — the discriminator is always the first member and carries `must_understand=true`.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::a_union_is_a_class_with_a_discriminator_and_one_field_per_case`,
`crates/lang-rules/tests/cpp_reader_generates.rs::a_union_gets_a_reader`

**Status:** done

### IDL 4.2 §7.5.2 `module` becomes a namespace and not a prefix; references are written from the root

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::a_module_is_a_namespace_and_not_a_prefix`,
`crates/lang-rules/src/adapter/cpp.rs::tests::a_reference_across_modules_is_written_from_the_root`

**Status:** done

### IDL 4.2 §7.2.3 The tag keeps apart what the C++ spelling collapses

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::the_tag_keeps_apart_what_the_cpp_spelling_collapses`,
`crates/lang-rules/src/adapter/cpp.rs::tests::an_any_member_is_refused_by_name_at_any_depth`

**Status:** done

### §7.2.2.4.4.4.9 Every kind Table 9 answers for gets a defaulter

**Spec:** §7.2.2.4.4.4.9 — A member the stream did not carry takes its Table 9 default.

**Repo:** `crates/lang-rules/src/default_cpp.rs`

**Tests:** `crates/lang-rules/tests/cpp_reader_generates.rs::every_kind_table_9_answers_for_gets_a_defaulter`

**Status:** done

### §7.4.3 Framing: an appendable and a mutable aggregate each get a reader

**Spec:** §7.4.3 — XCDR version 2 — plain CDR2, delimited CDR2 and PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/cpp.rs`

**Tests:** `crates/lang-rules/tests/cpp_reader_generates.rs::an_appendable_struct_gets_a_reader`,
`crates/lang-rules/tests/cpp_reader_generates.rs::a_mutable_struct_gets_a_reader`,
`crates/lang-rules/tests/cpp_reader_generates.rs::a_bounded_string_gets_a_reader`

**Status:** done

### IDL 4.2 §7.4.3 `interface` binds the DDS-RPC entities in C++

**Spec:** IDL 4.2 §7.4.3 — `interface` — operations with `in`/`out`/`inout` parameters.

**Repo:** `crates/lang-rules/src/adapter/interface.rs`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_two_abstract_classes_of_rule_one`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_synchronous_operations_of_rule_six`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_declares_the_asynchronous_operations_of_rule_seven`,
`crates/lang-rules/tests/interface_bindings.rs::cpp_prefixes_a_reserved_word_with_cxx`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/cpp.rs`, `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/src/adapter/cpp.rs::tests::every_corpus_case_renders_or_names_its_refusal`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/emit/cpp.rs`, `crates/lang-rules/src/adapter/cpp.rs`

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::cpp_places_all_six_slots_in_spec_order`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang c++`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-cpp-gen/`, `tools/wireprobe/wireprobe-zerodds-cpp`, `tools/wireprobe/wireprobe-cpp-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

21 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.

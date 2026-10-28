# `zerodds-xcdr2-java` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the Java column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the Java column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/java/`, `crates/lang-render/src/java.rs`

**Tests:** `crates/lang-render/tests/rendering.rs`: `a_java_class_renders_its_modifiers_annotations_and_members`, `a_java_enum_ends_its_constants_with_a_semicolon_only_when_members_follow`, `a_java_generic_type_renders_its_arguments_with_dots_in_the_path`, `a_java_file_writes_its_package_and_imports`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/java.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/java.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_java.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/java`, endpoint SDK `endpoints/java`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches the Java type that holds it

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_idl_primitive_reaches_the_java_type_that_holds_it`

**Status:** done

### IDL 4.2 §7.4.13 `fixed` becomes a `BigDecimal`


**Spec:** IDL 4.2 §7.4.13 — `fixed<P,S>` — a decimal with a declared digit count and scale.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_fixed_is_a_bigdecimal`, `crates/lang-rules/tests/java_runtime_selftest.rs`

**Status:** done

### §7.2.2.4.3 A sequence and an array are both Java arrays

**Spec:** §7.2.2.4.3 — Sequence — one-dimensional, bound optional.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_sequence_and_an_array_are_both_java_arrays`

**Status:** done

### §7.2.2.4.3 A `map` becomes a class over `java.util.LinkedHashMap`; two IDL shapes stay two classes


**Spec:** §7.2.2.4.3 — Map — key/value pairs; §7.4.3.5.3 rules 11-16 fix the order and the framing.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_map_is_a_java_util_map_over_a_generated_entry_class`,
`crates/lang-rules/src/adapter/java.rs::tests::two_maps_of_one_shape_are_one_entry_class`,
`crates/lang-rules/src/adapter/java.rs::tests::two_maps_of_one_java_shape_and_two_idl_shapes_are_two_entry_classes`

**Status:** done

### §7.2.2.4.4.4.7 An optional member is boxed, and a primitive one too

**Spec:** §7.2.2.4.4.4.7 — A structure member may be optional; a union member never is.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_optional_member_is_boxed_and_a_primitive_one_too`

**Status:** done

### §7.2.2.4.1.1 An enumeration becomes a class with int constants; a member of it is an instance


**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_enumeration_is_a_class_of_instances_and_a_member_of_it_is_one`,
`crates/lang-rules/src/adapter/java.rs::tests::an_explicit_enumerator_value_is_written_out`

**Status:** done

### IDL 4.2 §7.5.2 `module` becomes a nested class; a reopened module stays one class

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_module_is_a_nested_class_and_two_modules_do_not_collide`,
`crates/lang-rules/src/adapter/java.rs::tests::a_module_reopened_beside_a_nested_one_is_one_class`,
`crates/lang-rules/src/adapter/java.rs::tests::a_declaration_named_after_the_outer_class_is_refused`

**Status:** done

### IDL 4.2 §7.2.3 A Java keyword is escaped; a name that would capture the qualified references is refused

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::an_identifier_that_is_a_java_keyword_is_escaped`,
`crates/lang-rules/src/adapter/java.rs::tests::a_name_that_would_capture_the_qualified_references_is_refused`

**Status:** done

### §7.4.3 The Java backend spells the shapes the materialiser declares; an unsigned local is wide enough

**Spec:** §7.4.3 — XCDR version 2 — plain CDR2, delimited CDR2 and PL_CDR2.

**Repo:** `crates/lang-rules/src/emit/java.rs`

**Tests:** `crates/lang-rules/tests/one_walk_two_targets.rs::the_java_backend_spells_the_shapes_the_materialiser_declares`,
`crates/lang-rules/tests/one_walk_two_targets.rs::an_unsigned_local_is_wide_enough_in_java`

**Status:** done

### IDL 4.2 §7.4.3 `interface` binds the same entities as C++, with a future and out types

**Spec:** IDL 4.2 §7.4.3 — `interface` — operations with `in`/`out`/`inout` parameters.

**Repo:** `crates/lang-rules/src/adapter/interface.rs`

**Tests:** `crates/lang-rules/tests/interface_bindings.rs::java_declares_the_same_entities_as_cpp`,
`crates/lang-rules/tests/interface_bindings.rs::java_returns_a_future_of_the_boxed_result`,
`crates/lang-rules/tests/interface_bindings.rs::java_returns_the_out_type_where_cpp_takes_a_reference`,
`crates/lang-rules/tests/interface_bindings.rs::java_carries_the_topic_names_as_constants`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/java.rs`, `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.4 A construct with no data form in this language is named and not left out

**Spec:** §7.4 — A construct with no data form in this language is named and not left out. A silently dropped construct is the worst class of fault: the receiver sees no error, it sees wrong data.

**Repo:** `crates/lang-rules/src/emit/java.rs`, `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/src/adapter/java.rs::tests::a_construct_with_no_java_data_form_is_named_and_not_left_out`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/emit/java.rs`, `crates/lang-rules/src/adapter/java.rs`

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::java_wraps_the_file_and_fills_the_class_body`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang java`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-java-gen/`, `tools/wireprobe/wireprobe-zerodds-java`, `tools/wireprobe/wireprobe-java-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

21 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.

# `zerodds-xcdr2-csharp` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the C# column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the C# column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/csharp/`, `crates/lang-render/src/csharp.rs`

**Tests:** `crates/lang-render/src/csharp.rs::tests` (13 tests), among them `a_class_opens_its_brace_on_a_line_of_its_own`, `a_rooted_path_renders_with_global`, `an_array_renders_one_pair_of_brackets_per_level`, `a_nullable_value_type_renders_with_a_question_mark`, `a_namespace_nests_and_its_members_are_indented`;
across all renderers `crates/lang-render/tests/rendering.rs::every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/csharp.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/csharp.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_csharp.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/csharp`, endpoint SDK `endpoints/csharp`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches the C# type that holds it

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_idl_primitive_reaches_the_csharp_type_that_holds_it`

**Status:** done

### IDL 4.2 §7.4.13 `fixed` becomes a `string` and not a `decimal`

**Spec:** IDL 4.2 §7.4.13 — `fixed<P,S>` — a decimal with a declared digit count and scale.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_fixed_is_a_string_and_not_a_decimal`

**Status:** done

### §7.2.2.4.3 A sequence and an array are both C# arrays

**Spec:** §7.2.2.4.3 — Sequence — one-dimensional, bound optional.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_sequence_and_an_array_are_both_csharp_arrays`

**Status:** done

### §7.2.2.4.3 A `map` becomes an array of a generated entry class; one shape means one class

**Spec:** §7.2.2.4.3 — Map — key/value pairs; §7.4.3.5.3 rules 11-16 fix the order and the framing.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_map_is_an_array_of_a_generated_entry_class`,
`crates/lang-rules/src/adapter/csharp.rs::tests::two_maps_of_one_shape_are_one_entry_class`,
`crates/lang-rules/src/adapter/csharp.rs::tests::two_maps_whose_csharp_spelling_coincides_are_still_two_entry_classes`

**Status:** done

### §7.2.2.4.4.4.7 An optional value type is nullable and a reference one is not

**Spec:** §7.2.2.4.4.4.7 — A structure member may be optional; a union member never is.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_optional_value_type_is_nullable_and_a_reference_one_is_not`,
`crates/lang-rules/src/adapter/csharp.rs::tests::an_optional_enum_is_nullable_because_an_enum_is_a_value_type`

**Status:** done

### §7.2.2.4.1.1 An enumeration becomes a real `enum`, and a member of it has that type

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_enumeration_is_a_real_enum_and_a_member_of_it_is_that_type`,
`crates/lang-rules/src/adapter/csharp.rs::tests::an_explicit_enumerator_value_is_written_out`

**Status:** done

### §7.2.2.4.1.2 A bitmask becomes a `[Flags]` enum over the holder its bound calls for

**Spec:** §7.2.2.4.1.2 — Bitmask — bound 1..64, named bits.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_bitmask_is_a_flags_enum_over_the_holder_its_bound_calls_for`,
`crates/lang-rules/src/adapter/csharp.rs::tests::a_bitset_is_a_class_of_one_field_per_bitfield`

**Status:** done

### IDL 4.2 §7.5.2 `module` becomes a namespace; a reopened module stays one block

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_module_is_a_namespace_and_two_modules_do_not_collide`,
`crates/lang-rules/src/adapter/csharp.rs::tests::a_module_reopened_beside_a_nested_one_is_one_namespace_block`

**Status:** done

### IDL 4.2 §7.2.3 A C# keyword is escaped with `@`; the IDL name stays on the field

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_identifier_that_is_a_csharp_keyword_is_escaped_with_an_at_sign`,
`crates/lang-rules/src/adapter/csharp.rs::tests::every_field_carries_the_idl_name_it_was_folded_from`,
`crates/lang-rules/src/adapter/csharp.rs::tests::two_members_that_differ_only_after_the_fold_stay_two_members`

**Status:** done

### IDL 4.2 §7.4.3 `interface` becomes an interface of signatures with the IDL passing modes

**Spec:** IDL 4.2 §7.4.3 — `interface` — operations with `in`/`out`/`inout` parameters.

**Repo:** `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::an_interface_is_an_interface_of_signatures_with_the_idl_passing_modes`,
`crates/lang-rules/src/adapter/csharp.rs::tests::an_exception_is_a_data_class_and_not_a_system_exception`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/csharp.rs`, `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.4 A construct with no data form in this language is named and not left out

**Spec:** §7.4 — A construct with no data form in this language is named and not left out. A silently dropped construct is the worst class of fault: the receiver sees no error, it sees wrong data.

**Repo:** `crates/lang-rules/src/emit/csharp.rs`, `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/src/adapter/csharp.rs::tests::a_construct_with_no_csharp_data_form_is_named_and_not_left_out`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/emit/csharp.rs`, `crates/lang-rules/src/adapter/csharp.rs`

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::csharp_puts_begin_and_end_inside_the_type_body`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang c#`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-csharp-gen/`, `tools/wireprobe/wireprobe-zerodds-csharp`, `tools/wireprobe/wireprobe-csharp-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

---

## Audit Status

21 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed (20 ignored: the whole-corpus compile runs, which need `--include-ignored`).

Open items: none. Decision records: none.

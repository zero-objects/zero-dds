# `zerodds-xcdr2-python` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) and `internal/standards/cache/omg/idl-4.2.pdf` (§7) — what the Python column answers.

The mechanism this column shares with the other twenty — the two walks, the framing, Table 9, the key hash and the driver — is in `zerodds-xcdr2-bindings-conformance-1.0.md`. This document carries only what the Python column decides for itself, and its own evidence.

## §7.5.1 The parts of the column

### §7.5.1 The concrete syntax tree and its text form

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. This column's declarations are built as a tree and only then written as text; no emitter concatenates source.

**Repo:** `crates/lang-cst/src/python/`, `crates/lang-render/src/python.rs`

**Tests:** `crates/lang-render/src/python.rs::tests` (24 tests), among them `xtypes_7_5_1_a_class_body_is_indented_by_four_and_closed_by_the_dedent_alone`, `xtypes_7_5_1_a_class_with_a_docstring_and_no_members_is_still_legal_python`, `xtypes_7_5_1_an_empty_body_takes_the_pass_the_tree_carries`, `xtypes_7_5_1_an_import_carries_its_alias_and_its_from_form`, `xtypes_7_5_1_a_call_takes_positional_and_keyword_arguments_in_written_order`;
across all renderers `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_every_renderer_ends_its_output_with_exactly_one_newline`, `crates/lang-render/tests/rendering.rs::xtypes_7_5_1_no_rendered_line_carries_trailing_whitespace`.

**Status:** done

### §7.5.1 The rule catalogue

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. What becomes of which model node is a rule set and not a special case inside the walk.

**Repo:** `crates/lang-rules/src/rule/python.rs`

**Tests:** `crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_language_catalogue_fires_on_the_same_model`,
`crates/lang-rules/tests/apply.rs::xtypes_7_5_1_every_catalogue_serialises_and_reads_back_the_same`

**Status:** done

### §7.5.1 The materialiser

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The materialiser carries the language's own rules: names and keywords, collisions, and what the language cannot express at all.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 The codec backend

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The backend supplies the spelling the shared encoding decision takes in this language — receiver, method name, argument form.

**Repo:** `crates/lang-rules/src/emit/python.rs`

**Tests:** `crates/lang-rules/tests/generate_contract.rs::xtypes_7_5_1_one_idl_becomes_a_full_codec_in_every_wired_language`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The Table 9 defaulter

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. A reader with no Table 9 answer has no defined value at five positions; this file is this column's answer.

**Repo:** `crates/lang-rules/src/default_python.rs`

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::xtypes_7_5_1_a_codec_backend_never_exists_without_the_three_parts_it_needs`

**Status:** done

### §7.5.1 The wire runtime

**Spec:** §7.5.1 — Plain language binding — the mapping onto the target language is the PSM's own. The runtime reads §7.4 from the clause text on its own and calls no other column; that is where this column's independence lives.

**Repo:** `runtime/python`, endpoint SDK `endpoints/python`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_5_1_every_language_has_an_endpoint_sdk`

**Status:** done

## §7.2.2 What this column decides for itself

### §7.2.2.2 Every IDL primitive reaches an annotation that states its width and signedness

**Spec:** §7.2.2.2 — Primitive types — BOOLEAN, BYTE, INT8..UINT64, FLOAT32, FLOAT64, FLOAT128, CHAR8, CHAR16; every width is fixed.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::xtypes_7_2_2_2_every_idl_primitive_reaches_an_annotation_that_states_its_width`,
`crates/lang-rules/src/adapter/python.rs::tests::xtypes_7_2_2_2_every_width_alias_states_its_bits_and_its_signedness`

**Status:** done

### §7.2.2.2 `long double` becomes the decimal text and never a Python float

**Spec:** §7.2.2.2 — FLOAT128 is an IEEE 754 binary128 value of sixteen octets.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::xtypes_7_2_2_2_a_long_double_is_the_decimal_text_and_never_a_python_float`,
`crates/lang-rules/src/default_python.rs::tests::xtypes_7_2_2_4_4_4_7_a_long_double_is_a_float128_and_never_a_float`

**Status:** done

### §7.2.2.4.3 A sequence is a list and an array a tuple of its length; the bound reaches the annotation

**Spec:** §7.2.2.4.3 — Sequence — one-dimensional, bound optional.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::xtypes_7_2_2_4_3_a_sequence_is_a_list_and_an_array_is_a_tuple_of_its_length`,
`crates/lang-rules/src/adapter/python.rs::tests::xtypes_7_2_2_4_3_a_bound_reaches_the_annotation_and_an_unbounded_one_leaves_it_alone`

**Status:** done

### §7.2.2.4.3 A `map` is a list of pairs and never a `dict`

**Spec:** §7.2.2.4.3 — Map — key/value pairs; §7.4.3.5.3 rules 11-16 fix the order and the framing.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::xtypes_7_2_2_4_3_a_map_is_a_list_of_pairs_and_never_a_dict`,
`crates/lang-rules/tests/python_codec.rs::xtypes_7_2_2_4_3_a_map_entry_is_written_into_as_a_list_and_answered_as_a_tuple`

**Status:** done

### §7.2.2.4.4.4.7 An optional member is `Optional`, defaults to `None` and is tested by identity

**Spec:** §7.2.2.4.4.4.7 — A structure member may be optional; a union member never is.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::xtypes_7_2_2_4_4_4_7_an_optional_member_is_optional_and_defaults_to_none`,
`crates/lang-rules/tests/python_codec.rs::xtypes_7_2_2_4_4_4_7_an_optional_member_is_tested_against_none_by_identity`

**Status:** done

### §7.2.2.4.1.1 An enumeration becomes an `IntEnum`; a member of it is an `int`

**Spec:** §7.2.2.4.1.1 — Enumeration — int32 literals, ordered, `bit_bound` 1..32.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::xtypes_7_2_2_4_1_1_an_enumeration_is_an_intenum_and_a_member_of_it_is_an_int`,
`crates/lang-rules/src/adapter/python.rs::tests::xtypes_7_2_2_4_1_1_a_bitmask_is_an_intflag_and_a_wide_shift_needs_no_suffix`

**Status:** done

### IDL 4.2 §7.4.1.4 A constant outside its IDL range is refused although Python would hold it

**Spec:** IDL 4.2 §7.4.1.4 — `const` — a value of its declared type, evaluated at translation time.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::idl_7_4_1_4_a_constant_outside_its_idl_range_is_refused_although_python_would_hold_it`,
`crates/lang-rules/src/adapter/python.rs::tests::idl_7_4_1_4_a_constant_is_final_and_carries_its_idl_type`

**Status:** done

### IDL 4.2 §7.2.3 A Python keyword and a builtin the annotations use are both escaped

**Spec:** IDL 4.2 §7.2.3 — Identifiers are ASCII and case-insensitive on comparison; a collision has to be reported.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::idl_7_2_3_an_identifier_that_is_a_python_keyword_is_escaped_and_the_idl_name_is_kept`,
`crates/lang-rules/src/adapter/python.rs::tests::idl_7_2_3_a_member_named_after_a_builtin_the_annotations_use_is_escaped_too`,
`crates/lang-rules/src/adapter/python.rs::tests::idl_7_2_3_two_names_that_would_escape_onto_one_are_refused`

**Status:** done

### IDL 4.2 §7.5.2 `module` becomes a nested class; a reference is written from the root

**Spec:** IDL 4.2 §7.5.2 — `module` is a naming scope; an identifier is unique per scope.

**Repo:** `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::idl_7_5_2_a_module_is_a_nested_class_and_a_reference_is_written_from_the_root`,
`crates/lang-rules/src/adapter/python.rs::tests::idl_7_5_2_a_module_reopened_beside_a_nested_one_is_one_class`

**Status:** done

### §7.4 The generated module names nothing the runtime does not have

**Spec:** §7.4 — Data representation — the generated source calls a runtime that reads §7.4 from the clause text.

**Repo:** `runtime/python`

**Tests:** `crates/lang-rules/tests/python_names_its_runtime.rs::xtypes_7_4_the_generated_module_names_nothing_the_runtime_does_not_have`,
`crates/lang-rules/tests/python_names_its_runtime.rs::xtypes_7_4_every_policy_the_backend_writes_is_one_the_runtime_accepts`,
`crates/lang-rules/tests/python_names_its_runtime.rs::xtypes_7_4_every_operation_the_walks_ask_for_is_one_the_runtime_answers_to`

**Status:** done

### §7.2.2.4.4.4.9 Table 9: every required field is named, and the closure reaches every defaulter a batch calls

**Spec:** §7.2.2.4.4.4.9 — A member the stream did not carry takes its Table 9 default.

**Repo:** `crates/lang-rules/src/default_python.rs`

**Tests:** `crates/lang-rules/src/default_python.rs::tests::xtypes_7_2_2_4_4_4_7_a_structure_of_primitives_names_every_required_field`,
`crates/lang-rules/src/default_python.rs::tests::xtypes_7_2_2_4_4_4_9_the_closure_reaches_every_defaulter_a_batch_calls`,
`crates/lang-rules/src/default_python.rs::tests::xtypes_7_2_2_4_4_4_7_a_typedef_and_its_target_are_one_function_and_it_does_not_call_itself`

**Status:** done

## §7.3.1 How the column is held to it

### §7.3.1 Every corpus case renders or names the layer that refused it

**Spec:** §7.3.1 — Every corpus case renders or names the layer that refused it. A case that stopped rendering would otherwise be found by whichever probe ran next, a build directory away from the construct that caused it.

**Repo:** `crates/lang-rules/src/emit/python.rs`, `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::xtypes_7_3_1_every_corpus_case_renders_or_names_its_failure`

**Status:** done

### §7.4 A construct with no data form in this language is named and not left out

**Spec:** §7.4 — A construct with no data form in this language is named and not left out. A silently dropped construct is the worst class of fault: the receiver sees no error, it sees wrong data.

**Repo:** `crates/lang-rules/src/emit/python.rs`, `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/src/adapter/python.rs::tests::xtypes_7_4_a_construct_with_no_python_data_form_is_named_and_not_left_out`

**Status:** done

### §7.4.3 The shapes the corpus has reach the end of both walks

**Spec:** §7.4.3 — The shapes the corpus has reach the end of both walks.

**Repo:** `crates/lang-rules/src/emit/python.rs`, `crates/lang-rules/src/adapter/python.rs`

**Tests:** `crates/lang-rules/tests/python_codec.rs::xtypes_7_4_3_the_shapes_the_corpus_has_reach_the_end_of_both_walks`

**Status:** done

### §7.4 The column's octets are held against a fixed recording

**Spec:** §7.4 — The column's octets are held against a fixed recording: `tests/interop-corpus/expected-bytes.tsv`, the vendors' octets, not the other columns: three columns that moved together still resemble each other.

**Repo:** `crates/lang-rules/src/emit/python.rs`, `crates/lang-rules/src/adapter/python.rs`

**Tests:** `tools/wireprobe/wireprobe-zerodds-python-gen/tests/expected_bytes.rs::specification`, `tools/wireprobe/wireprobe-zerodds-python-gen/tests/expected_bytes.rs::cyclone`, `tools/wireprobe/wireprobe-zerodds-python-gen/tests/expected_bytes.rs::rti`, `tools/wireprobe/wireprobe-zerodds-python-gen/tests/expected_bytes.rs::fastdds`, `tools/wireprobe/wireprobe-zerodds-python-gen/tests/expected_bytes.rs::omniorb`, `tools/wireprobe/wireprobe-zerodds-python-gen/tests/expected_bytes.rs::tao`, `tools/wireprobe/wireprobe-zerodds-python-gen/tests/expected_bytes.rs::jacorb`, `tools/wireprobe/wireprobe-zerodds-python-gen/tests/expected_bytes.rs::opendds`, `tools/wireprobe/wireprobe-zerodds-python-gen/tests/expected_bytes.rs::ros_fastrtps`, `tools/wireprobe/wireprobe-zerodds-python-gen/tests/expected_bytes.rs::ros_cyclone`

**Status:** done

### §7.2.2.4.8 `@verbatim` lands at the placements the clause names for this language

**Spec:** §7.2.2.4.8 — `@verbatim` lands at the placements the clause names for this language.

**Repo:** `crates/lang-rules/src/adapter/python.rs`, `crates/lang-rules/src/verbatim.rs`

**Tests:** `crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_2_every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::ddsts_9_1_a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.3.1 Driver and measurement

### §7.3.1 `idlc generate --lang python`

**Spec:** §7.3.1 — the IDL type representation is the input of generation; the output is this column's source.

**Repo:** `tools/idlc/src/generate.rs`, `tools/idlc/src/main.rs`

**Tests:** `tools/idlc/src/generate.rs::tests::deployment_1_3_every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::xtypes_7_3_1_language_names_lists_every_language_once`

**Status:** done

### §7.4 The measurement harness of this column

**Spec:** §7.4 — a column's octets are compared against recorded octets rather than asserted in its own tests.

**Repo:** `tools/wireprobe/wireprobe-zerodds-python-gen/`, `tools/wireprobe/wireprobe-zerodds-python-walk`, `tools/wireprobe/wireprobe-python-walk-corpus`

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::xtypes_7_4_every_language_has_a_cell_generator`

**Status:** done

---

## Audit Status

24 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-xcdr2-python-1.0.md`):

* `cargo nextest run --profile merge -p zerodds-idlc --all-targets` (job `merge-rust-idlc`) — 2 tests green.
* `cargo nextest run --profile merge -p zerodds-lang-render --all-targets` (job `merge-rust-lang-render`) — 7 tests green.
* `cargo nextest run --profile merge -p zerodds-lang-rules --all-targets` (job `merge-rust-lang-rules`) — 33 tests green.
* `cargo nextest run --profile merge -p zerodds-wireprobe-python-gen --all-targets` (job `merge-rust-wireprobe-python-gen`) — 10 tests green.
* `python3 ci/run_integration.py merge-codegen-python` (job `merge-codegen-python`) — 6 tests green.

58 cited tests green, 0 not green.

Open items: none.

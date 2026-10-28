# `zerodds-xcdr2-bindings-conformance` 1.0 — Spec Coverage

**Source:** `internal/standards/cache/omg/dds-xtypes-1.3.pdf` (§7.2, §7.4) —
DDS-XTypes 1.3, Type System Model and Data Representation.

The common report on the mechanism all twenty-one language columns share.
What a single column decides for itself is in its own document —
`zerodds-xcdr2-c-1.0.md`, `zerodds-xcdr2-java-1.0.md` and so on; the
evidence stays separated per language, because each runtime reads the
clause on its own.

The mechanism is four pieces:

- `crates/lang-rules/` — two walks (`src/emit/encode.rs`,
  `src/emit/decode.rs`), generic over `emit::CodecTarget`; per language a
  rule catalogue, a materialiser, a codec backend and a Table 9 rule.
- `crates/lang-cst/` — one concrete syntax tree per language.
- `crates/lang-render/` — the text form per language.
- `tools/idlc/` — the driver a user invokes.

What is **not** shared is the wire runtime: the twenty directories under
`runtime/` each read DDS-XTypes 1.3 §7.4 from the clause text and none
calls another. Where two columns disagree on an octet, one of the two has
misread the specification.

## §7.4.3 The two walks

### §7.4.3 One encode walk for every column

**Spec:** §7.4.3, XCDR Version 2 — which member is written, in what
order, under which header and with which length code, is a property of
the encoding and not of the target language.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (`codec_from`,
`CodecOptions`).

**Tests:** `crates/lang-rules/tests/codec.rs::a_final_struct_of_primitives_encodes_as_a_run_of_writes`,
`crates/lang-rules/tests/one_walk_two_targets.rs::both_backends_write_the_same_members_in_the_same_order`

**Status:** done

### §7.4.3 One decode walk, member for member the inverse

**Spec:** §7.4.3 — the reader takes the same members in the same order
under the same framing.

**Repo:** `crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::the_decoder_mirrors_the_encoder_member_for_member`,
`crates/lang-rules/tests/one_walk_two_targets.rs::the_walk_asks_for_one_unary_operator_and_no_others`

**Status:** done — the walk asks for one unary operator (`!`) and for no other; a bitmask
reader keeps the declared bits with a plain `&`, without `~`.

### §7.4.3 The seam: `CodecTarget`

**Spec:** §7.4.3 — the encoding decision is language-independent; what
stays language-dependent is its spelling.

**Repo:** `crates/lang-rules/src/emit/mod.rs` (`CodecTarget`, `Scalar`,
`Handle`, `runtime_static`).

**Tests:** `crates/lang-rules/tests/one_walk_two_targets.rs::the_difference_between_the_two_is_spelling`,
`crates/lang-rules/tests/one_walk_two_targets.rs::a_sequence_is_one_loop_and_two_ways_of_asking_its_length`

**Status:** done

## §7.2.3 Extensibility and framing

### §7.2.3 FINAL without a DHEADER, APPENDABLE with one

**Spec:** §7.2.3, Tab.12 — FINAL/APPENDABLE/MUTABLE; §7.4.3 gives
APPENDABLE the leading `uint32` body length (DHEADER) and FINAL none.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (framing branches taken
from the layout plan in `crates/ast-layout/src/key.rs`).

**Tests:** `crates/lang-rules/tests/codec.rs::an_appendable_struct_carries_the_dheader_the_plan_asks_for`,
`crates/lang-rules/tests/codec.rs::the_extensibility_the_source_wrote_decides_the_framing`

**Status:** done

### §7.4.3.4.2 MUTABLE: one EMHEADER per member, length code from the plan

**Spec:** §7.4.3.4.2 — the EMHEADER carries the member ID and a length
code; LC=4 takes a NEXTINT, LC=6/7 count in 4- and 8-byte units.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (EMHEADER branch),
`crates/lang-rules/src/emit/mod.rs` (`CodecTarget` supplies the
spelling).

**Tests:** `crates/lang-rules/tests/codec.rs::a_mutable_struct_writes_an_emheader_per_member`,
`crates/lang-rules/tests/codec.rs::the_length_code_policy_is_visible_in_the_output`,
`crates/lang-rules/tests/one_walk_two_targets.rs::a_mutable_aggregate_opens_and_closes_a_header_in_both`

**Status:** done

### §7.4.3 Every header that opens is also closed

**Spec:** §7.4.3 — a DHEADER or EMHEADER length precedes its body and is
only known after it; it has to be patched back in.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (one patch site per
opened header), `crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::every_parameter_header_the_decoder_opens_it_also_closes`,
`crates/lang-rules/tests/runtime_agreement.rs::the_generator_emits_a_patch_for_every_framing_header_it_opens`

**Status:** done

### §7.4.2 XCDR1: a parameter list closed by a sentinel

**Spec:** §7.4.2 — PL_CDR1 frames every member as a parameter and closes
the list with `PID_SENTINEL`.

**Repo:** `crates/lang-rules/src/emit/encode.rs`,
`crates/lang-rules/src/emit/decode.rs` (XCDR1 branches).

**Tests:** `crates/lang-rules/tests/c_end_to_end.rs::a_mutable_struct_under_xcdr1_ends_with_a_sentinel`,
`crates/lang-rules/tests/c_end_to_end.rs::a_mutable_aggregate_under_xcdr1_stops_at_the_sentinel`

**Status:** done

## §7.2.2.4 Constructs

### §7.2.2.4.4.4.7 `@optional`

**Spec:** §7.2.2.4.4.4.7 — a structure member may be optional; a union
member never is. Absence is encoded, not left out.

**Repo:** `crates/lang-rules/src/emit/encode.rs`,
`crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::an_optional_member_writes_its_marker_from_the_option_and_not_from_a_literal`,
`crates/lang-rules/tests/codec.rs::an_optional_member_under_xcdr1_marks_the_absence_with_a_declared_length_of_zero`,
`crates/lang-rules/tests/codec.rs::an_optional_member_of_a_mutable_type_omits_the_whole_parameter`

**Status:** done

### §7.2.2.4.3 Collections: a DHEADER only where the elements are not primitive

**Spec:** §7.2.2.4.3 and §7.4.3.5.3 — a collection whose elements are
themselves framed carries a collection DHEADER; one of primitive elements
carries none.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (collection branch).

**Tests:** `crates/lang-rules/tests/codec.rs::a_collection_of_aggregates_is_delimited_and_one_of_primitives_is_not`,
`crates/lang-rules/tests/codec.rs::a_sequence_writes_its_elements_by_their_own_writer`,
`crates/lang-rules/tests/codec.rs::a_collection_directly_inside_a_collection_is_emitted`

**Status:** done

### §7.2.2.4.3 Bounds: a bounded sequence checks, an unbounded one does not

**Spec:** §7.2.2.4.3 — a sequence may carry a bound; a writer that
exceeds it is not conformant.

**Repo:** `crates/lang-rules/src/emit/encode.rs`,
`crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_bounded_sequence_checks_its_bound_and_an_unbounded_one_has_none`,
`crates/lang-rules/tests/codec.rs::a_bounded_string_is_written_through_the_form_that_refuses_an_overrun`

**Status:** done

### §7.2.2.4.3 Array: no length on the wire

**Spec:** §7.2.2.4.3 — an array's element count is in the type, not in
the stream.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (array branch).

**Tests:** `crates/lang-rules/tests/codec.rs::an_array_writes_no_length_because_its_count_is_in_the_type`

**Status:** done

### §7.2.2.4.1.1 Enumeration: its own codec, width from `bit_bound`

**Spec:** §7.2.2.4.1.1 and §7.2.2.4.1 — an enumeration has a `bit_bound`
from 1 to 32; its wire width follows from it.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (`codec_name` per
enumeration), `crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::an_enum_member_is_written_through_the_enumerations_own_codec`,
`crates/lang-rules/tests/codec.rs::the_enumerations_codec_writes_the_width_its_bit_bound_calls_for`,
`crates/lang-rules/tests/codec.rs::the_enumeration_codec_takes_the_integer_the_plan_gives_it`

**Status:** done

### §7.2.2.4.2 Alias: written as what it names

**Spec:** §7.2.2.4.2 — an alias has no kind of its own on the wire; it
resolves to its `base_type`.

**Repo:** `crates/lang-rules/src/emit/encode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_typedef_is_written_as_what_it_aliases`

**Status:** done

### §7.2.2.4.4.3 Union: the discriminator first, then the selected member

**Spec:** §7.2.2.4.4.3 — the discriminator is always the first member and
carries `must_understand=true`.

**Repo:** `crates/lang-rules/src/emit/encode.rs`,
`crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_mutable_decoder_dispatches_on_the_member_id_and_a_positional_one_does_not`,
`crates/lang-rules/tests/codec.rs::a_union_case_label_that_does_not_evaluate_is_refused`

**Status:** done

### §7.2.2.4.4 A nested aggregate encodes itself

**Spec:** §7.2.2.4.4 — a member whose type is an aggregate is written
through that aggregate's own encoding, with its own framing.

**Repo:** `crates/lang-rules/src/emit/encode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_nested_aggregate_encodes_itself`

**Status:** done

## §7.2.2.4.4.4.9 Table 9 and TryConstruct

### §7.2.2.4.4.4.9 A member the stream did not carry takes its default

**Spec:** §7.2.2.4.4.4.9 and Tab.9 — the default is either explicitly
annotated or implicit: 0, `false`, `'\0'`, `""`, the empty collection,
the first enumeration literal.

**Repo:** `crates/lang-rules/src/default_<lang>.rs` (one rule per column;
§7.5.1 below), called from `crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::an_appendable_tail_the_writer_did_not_have_takes_its_default`,
`crates/lang-rules/tests/codec.rs::an_appendable_tail_takes_an_explicit_default_over_table_nine`,
`crates/lang-rules/tests/codec.rs::an_appendable_tail_of_an_enumeration_takes_its_first_literal`,
`crates/lang-rules/tests/codec.rs::an_explicit_default_beats_table_nine_at_the_member_position`

**Status:** done

### §7.2.2.7 TryConstruct: DISCARD, USE_DEFAULT, TRIM

**Spec:** §7.2.2.7, Tab.10/11 — the default is DISCARD; TRIM applies only
to `string`, `sequence` and `map`.

**Repo:** `crates/lang-rules/src/emit/decode.rs`,
`crates/wire/src/try_construct.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::an_absent_annotation_is_discard_and_a_bare_one_is_use_default`,
`crates/lang-rules/tests/codec.rs::a_bounded_member_narrows_after_the_whole_value_is_off_the_wire`,
`crates/lang-rules/tests/codec.rs::an_element_and_its_collection_narrow_separately`

**Status:** done

## §7.4.5 KeyHash

### §7.4.5 The key: zero-padded up to sixteen octets, MD5 beyond

**Spec:** §7.4.5 — the key holder is serialised PLAIN_CDR2 big-endian; at
sixteen octets or fewer it is the hash itself (zero-padded), beyond that
MD5 over the octets.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (key branch),
`crates/wire/src/digest.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_keyed_type_gets_a_key_hash_over_its_key_members_only`,
`crates/lang-rules/tests/codec.rs::a_holder_of_sixteen_bytes_or_fewer_is_the_hash_and_a_longer_one_is_md5ed`,
`crates/lang-rules/tests/codec.rs::the_holder_is_big_endian_and_plain_cdr2_aligned_whatever_the_sample_is`,
`crates/lang-rules/tests/codec.rs::an_unbounded_key_is_md5ed_because_its_length_is_not_known_until_the_sample_is`,
`crates/lang-rules/tests/runtime_agreement.rs::the_key_hash_sequence_produces_a_stable_sixteen_bytes`

**Status:** done

### §7.4.5 A type with no key gets no KeyHash

**Spec:** §7.4.5 — only a type with key members carries a KeyHash.

**Repo:** `crates/lang-rules/src/emit/encode.rs`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_type_with_no_key_gets_no_key_hash`,
`crates/lang-rules/tests/c_end_to_end.rs::a_keyed_type_gets_a_holder_and_an_unkeyed_one_does_not`

**Status:** done

## §7.4 The walk against the runtime

### §7.4 Every call the generator emits exists in the runtime

**Spec:** §7.4 — generated code calls the wire runtime; a call with no
counterpart is a link error rather than a conformance statement.

**Repo:** `crates/lang-rules/src/emit/encode.rs`,
`crates/lang-rules/src/emit/decode.rs`, `runtime/c/include/`.

**Tests:** `crates/lang-rules/tests/runtime_agreement.rs::every_call_the_generator_emits_exists_in_the_runtime`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::the_operations_the_walks_emit_all_exist_in_the_reference_runtime`

**Status:** done

### §7.4.3 The generated call sequence writes the bytes the plan names

**Spec:** §7.4.3 — what the plan fixes as framing, order and member ID
has to be in the octets too.

**Repo:** `crates/lang-rules/src/encode.rs` (the Rust column),
`crates/wire/src/writer.rs`.

**Tests:** `crates/lang-rules/tests/runtime_agreement.rs::the_sequence_a_final_struct_generates_writes_the_bytes_the_plan_says`,
`crates/lang-rules/tests/runtime_agreement.rs::the_sequence_an_appendable_struct_generates_frames_its_members`,
`crates/lang-rules/tests/runtime_agreement.rs::the_sequence_a_mutable_struct_generates_carries_its_member_ids`

**Status:** done

### §7.4 No silent drop

**Spec:** §7.4 — a member the walk has no write form for must not be left
out; it has to be named.

**Repo:** `crates/lang-rules/src/emit/encode.rs` (error paths),
`tools/silent-drop-allowlist.tsv`.

**Tests:** `crates/lang-rules/tests/codec.rs::a_member_with_no_writer_is_reported_rather_than_left_out`,
`crates/lang-rules/tests/unevaluated.rs::an_array_length_that_does_not_evaluate_is_refused_by_the_codec_too`

**Status:** done

## §7.5.1 What a column is made of

### §7.5.1 Four parts for the types, a fifth for the codec

**Spec:** §7.5.1 — the plain language binding is each PSM's own; this
repository carries it per language as a CST, a rule catalogue, a
materialiser, a renderer and — for the encoding — a codec backend.

**Repo:** `crates/lang-cst/src/`, `crates/lang-rules/src/rule/`,
`crates/lang-rules/src/adapter/`, `crates/lang-render/src/`,
`crates/lang-rules/src/emit/`; the ledger in
`docs/language-columns-status.md`.

**Tests:** `crates/lang-rules/tests/codec_backend_prerequisites.rs::a_codec_backend_never_exists_without_the_three_parts_it_needs`,
`crates/lang-rules/tests/codec_backend_prerequisites.rs::the_type_only_columns_are_each_missing_a_part_a_codec_backend_rests_on`

**Status:** done

### §7.5.1 Every language has a runtime, an endpoint SDK and a cell generator

**Spec:** §7.5.1 — a binding that exists by halves is not a binding; the
inventory is checked against the filesystem, not against a list.

**Repo:** `runtime/`, `endpoints/`,
`crates/lang-rules/tests/every_language_has_its_parts.rs`.

**Tests:** `crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_runtime`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_an_endpoint_sdk`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_language_has_a_cell_generator`,
`crates/lang-rules/tests/every_language_has_its_parts.rs::every_cell_generator_guards_against_a_missing_toolchain`

**Status:** done

### §7.5.1 One IDL becomes declarations in every language

**Spec:** §7.5.1 — the same source has to produce, in every column, the
type it declares; a column with no output is a silently dropped
declaration.

**Repo:** `crates/lang-rules/src/generate.rs` (`declarations`).

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_every_wired_language`

**Status:** done

### §7.5.1 …and a full codec

**Spec:** §7.4.3 with §7.5.1 — beside the types, every column generates
an encoder and a decoder for every `struct`, `union` and `valuetype`.

**Repo:** `crates/lang-rules/src/generate.rs` (`source`),
`crates/lang-rules/src/emit/encode.rs` (`codec_name`).

**Tests:** `crates/lang-rules/tests/generate_contract.rs::one_idl_becomes_a_full_codec_in_every_wired_language`

**Status:** done

### §7.2.2.4 A completeness table per construct and language

**Spec:** §7.2.2.4 — the constructs of the type system model are
countable; a cell with no coverage has to say why.

**Repo:** `crates/lang-rules/src/coverage.rs` (no catch-all arm: a new
language breaks the build until every construct carries a decision).

**Tests:** `crates/lang-rules/tests/adapter.rs::the_coverage_table_has_a_cell_for_every_construct_and_language`,
`crates/lang-rules/tests/adapter.rs::the_table_reports_what_is_covered_and_what_is_not`,
`crates/lang-rules/tests/adapter.rs::an_uncovered_cell_says_why_rather_than_only_that`

**Status:** done

### §7.5.1 Every rule catalogue validates and fires on the same model

**Spec:** §7.5.1 — the language-specific decisions are rules over a
common model, not special cases inside the walk.

**Repo:** `crates/lang-rules/src/rule/mod.rs`,
`crates/lang-rules/src/apply.rs`.

**Tests:** `crates/lang-rules/tests/apply.rs::every_catalogue_validates_against_the_rule_format`,
`crates/lang-rules/tests/apply.rs::every_catalogue_serialises_and_reads_back_the_same`,
`crates/lang-rules/tests/apply.rs::every_language_catalogue_fires_on_the_same_model`

**Status:** done

## §7.2.2.4.8 VerbatimText

### §7.2.2.4.8 `@verbatim` reaches every column it names

**Spec:** §7.2.2.4.8 — `VerbatimText {language, placement, text}`; the
text is emitted unchanged at the named placement, and `"*"` reaches every
language.

**Repo:** `crates/lang-rules/src/verbatim.rs`.

**Tests:** `crates/lang-rules/tests/verbatim_end_to_end.rs::the_wildcard_reaches_all_four_columns`,
`crates/lang-rules/tests/verbatim_end_to_end.rs::cpp_places_all_six_slots_in_spec_order`,
`crates/lang-rules/tests/verbatim_end_to_end.rs::a_directive_for_an_unknown_language_is_not_emitted`,
`crates/lang-rules/tests/verbatim_end_to_end.rs::repeated_directives_keep_their_declaration_order`,
`crates/lang-rules/tests/verbatim_every_column.rs::the_wildcard_reaches_every_column`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_places_the_six_slots_in_its_declarations`,
`crates/lang-rules/tests/verbatim_every_column.rs::every_column_ends_its_source_file_with_the_end_file_text`,
`crates/lang-rules/tests/verbatim_every_column.rs::a_directive_reaches_its_own_column_and_no_other`

**Status:** done

## §7.5 The DCPS PSM

### §7.5 Every column declares the DCPS PSM

**Spec:** §7.5 with DDS 1.4 — the types the clause names have to exist in
every language binding.

**Repo:** `crates/lang-rules/src/generate.rs`,
`crates/lang-rules/tests/dcps_psm_idl.rs`.

**Tests:** `crates/lang-rules/tests/dcps_psm_idl.rs::the_dcps_psm_idl_declares_the_types_the_clause_names`,
`crates/lang-rules/tests/dcps_psm_idl.rs::every_language_declares_the_dcps_psm`

**Status:** done

## §7.3.1 The driver

### §7.3.1 `idlc generate --lang` takes every language under its own name

**Spec:** §7.3.1 — the IDL type representation is the input; the output
is a language binding.

**Repo:** `tools/idlc/src/generate.rs` (`parse_language`,
`language_names`, `source`), `tools/idlc/src/main.rs`
(`--lang`, `--out`, `--encoding`, `--endian`).

**Tests:** `tools/idlc/src/generate.rs::tests::every_language_round_trips_through_its_own_name`,
`tools/idlc/src/generate.rs::tests::language_names_lists_every_language_once`

**Status:** done

## §7.4 Measured octets

### §7.4 Three columns agree over the corpus

**Spec:** §7.4 — two independent readings of the same clause have to
produce the same octets.

**Repo:** `crates/wire-probe/src/lib.rs`, `tools/wireprobe/`.

**Tests:** `crates/wire-probe/tests/three_column_gate.rs::the_three_columns_agree`,
`crates/wire-probe/tests/three_column_gate.rs::the_c_column_covers_the_same_configurations`,
`crates/wire-probe/tests/three_column_gate.rs::the_rust_column_is_byte_for_byte_what_it_was`

**Status:** done

### §7.4 A new column against the recorded Rust baseline

**Spec:** §7.4 — a column is measured against a fixed recording rather
than built beside the others: three columns that moved together still
resemble each other.

**Repo:** `crates/wire-probe/tests/fixtures/rust_column_baseline.txt`
(145,740 slots, pinned case by case), `crates/wire-probe/src/matrix.rs`.

**Tests:** `crates/wire-probe/tests/reference_columns.rs::the_reference_row_is_the_one_the_baseline_was_taken_with`,
`crates/wire-probe/tests/reference_columns.rs::the_comparison_bites`,
`crates/wire-probe/tests/reference_columns.rs::every_configuration_reaches_the_driver_as_flags`,
`crates/wire-probe/tests/typescript_column_gate.rs::the_typescript_column_writes_what_the_rust_column_recorded`

**Status:** done

**Note:** `reference_columns.rs::the_new_columns_answer_the_matrix_the_rust_column_answered`
compares the C++ and Java columns only once
`target/release/wireprobe-cpp-matrix` and `wireprobe-java-matrix` are
built; without them the test says so on standard output and compares
nothing. The three tests of that file cited above measure with no foreign
driver.

### §7.4 The corpus produces the recorded bytes in all four modes

**Spec:** §7.4 — per case XCDR1/XCDR2 and both byte orders.

**Repo:** `tests/interop-corpus/`, `crates/wire-probe/src/canonical.rs`.

**Tests:** `crates/wire-probe/tests/interop_corpus.rs::every_case_produces_the_bytes_its_readme_records_in_all_four_modes`,
`crates/wire-probe/tests/interop_corpus.rs::every_case_goes_through_all_three_emitters`,
`crates/wire-probe/tests/interop_corpus.rs::every_case_emits_a_keyhash_and_an_encapsulated_payload`

**Status:** done

### §7.4 The read direction: every case comes back the value that went in

**Spec:** §7.4 — encoding and decoding are inverse to each other.

**Repo:** `crates/wire-probe/src/canonical.rs`,
`crates/lang-rules/src/emit/decode.rs`.

**Tests:** `crates/wire-probe/tests/decode_round_trip.rs::every_case_comes_back_the_value_that_went_in`,
`crates/wire-probe/tests/decode_round_trip.rs::no_prefix_of_a_payload_reads_as_a_shorter_sample`,
`crates/wire-probe/tests/probe_agreement.rs::every_corpus_case_is_read_the_same_way_by_all_three_probes`

**Status:** done

---

## Audit Status

35 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-lang-rules` — 1024 tests green, 0 failed
(20 ignored: the whole-corpus compile runs, which need
`--include-ignored`). `cargo test -p zerodds-wire-probe --no-fail-fast` —
350 green, 4 failed: `csharp_column_gate`, `java_column_gate` and
`java_round_trip` fail because the C# and Java probes do not compile on
this machine, and `union_label_match::the_shape_with_no_storage_is_left_to_the_clause`
is a standing finding; none of the four is cited as evidence in this
document.

Open items: none. Decision records: none.

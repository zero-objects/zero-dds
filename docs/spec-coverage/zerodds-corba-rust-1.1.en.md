# zerodds-corba-rust 1.1 — Spec Coverage

**Source:** `docs/specs/zerodds-corba-rust-1.1.md` (ZeroDDS vendor spec)

Implementation:

- `crates/corba-rust/` — CORBA-specific IDL→Rust mappings (valuetype/interface/component/home).

## §3 Interface mapping

### §3 Trait + stub + skeleton

**Spec:** §3 — one trait per `interface I`, `IStub` (client, GIOP marshalling), `dispatch_i` (server skeleton).

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_interface` (`emit_interface_trait`, `emit_interface_stub`, `emit_interface_skeleton`).

**Tests:** `crates/corba-rust/tests/codegen_mapping.rs::corbarust_3_an_interface_is_a_trait_a_stub_and_a_dispatch_function`; `compile_check.rs::corbarust_3_the_generated_code_compiles_with_the_data_column`.

**Status:** done

### §3.1 Operation mapping

**Spec:** §3.1 — void/T/in/out/inout/oneway; `raises` → typed `IError` enum (`System(CorbaException)` + variants).

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_op_stub_impl`, `emit_interface_exceptions_enum`.

**Tests:** `codegen_mapping.rs::corbarust_3_1_raises_returns_the_error_enum_of_the_interface`, `corbarust_3_1_out_and_inout_parameters_are_mutable_references`, `corbarust_3_1_a_oneway_operation_is_sent_without_waiting_for_a_reply`; `compile_check.rs::corbarust_3_the_generated_code_compiles_with_the_data_column`.

**Status:** done

### §3.2 Attribute mapping

**Spec:** §3.2 — `readonly`/writable attributes → getter/setter.

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_attr_stub_impl`.

**Tests:** `codegen_mapping.rs::corbarust_3_2_a_readonly_attribute_has_a_getter_a_writable_one_also_a_setter`.

**Status:** done

## §4 Valuetype mapping (§15.3.4)

### §4.1 State carrier + factory + flattening

**Spec:** §4.1 — `<Name>Value` struct + `ValueBase`/`ValueMarshal` + factory; inheritance state flattening (base first); truncatable/custom → `<NAME>_BASE_IDS`.

**Repo:** `crates/corba-rust/src/valuetype_emit.rs::emit_value_struct`, `flatten_fields`, `collect_base_repo_ids`.

**Tests:** `codegen_mapping.rs::corbarust_4_1_a_valuetype_is_a_trait_a_state_carrier_and_a_factory`, `corbarust_4_1_the_state_of_the_base_comes_first`, `corbarust_4_1_a_truncatable_valuetype_lists_its_base_ids`; `compile_check.rs::corbarust_3_the_generated_code_compiles_with_the_data_column`.

**Status:** done

### §4.2 Value wire engine (sharing)

**Spec:** §4.2 — value_tag (null/indirection/single/list), value sharing (Rc identity → indirection, DAG preserved).

**Repo:** `crates/corba-rust/src/value_wire.rs::ValueWriter`, `ValueReader`, `ValueRegistry`.

**Tests:** `value_wire.rs::tests`: `corbarust_4_2_a_value_and_a_null_are_the_octets_of_15_3_4_1`, `corbarust_4_2_a_shared_value_is_an_indirection_back_to_its_tag`, `corbarust_4_2_two_instances_are_two_values`, `corbarust_4_2_a_forward_indirection_is_refused`, `corbarust_4_2_a_value_is_the_octets_jacorb_writes`.

**Status:** done

### §4.3 Chunked encoding + truncation + custom

**Spec:** §4.2 — `write_chunked` (0x7fffff0e + chunk + end tag); truncation decode (most-derived unknown → read base, skip the rest); custom via the chunked path; byte-conformance to JacORB.

**Repo:** `crates/corba-rust/src/value_wire.rs::ValueWriter::write_chunked`, `read_chunked_state`.

**Tests:** `value_wire.rs::tests`: `corbarust_4_3_a_chunked_value_is_the_octets_jacorb_writes`, `corbarust_4_3_a_known_most_derived_type_reads_the_whole_value`, `corbarust_4_3_an_unknown_most_derived_type_truncates_to_its_base`.

**Status:** done

### §4.4 Nested chunked values + codebase URL

**Spec:** §15.3.4.3 — nested chunked values + multi-level end tags; §15.3.4.1 codebase URL (value_tag bit 0).

**Repo:** `crates/corba-rust/src/value_wire.rs::read_chunked_state` (recursive via `skip_value_from_tag`/`skip_chunked_body`, multi-level end tags, indirection in the chunk stream); codebase via `ValueRegistry::ctor_for` + `CodebaseResolver`, `ValueWriter::write_with_codebase`.

**Tests:** `value_wire.rs::tests`: `corbarust_4_4_a_nested_value_in_the_skipped_tail_is_consumed`, `corbarust_4_4_one_end_tag_closes_the_nested_and_the_outer_value`, `corbarust_4_4_the_codebase_url_comes_ahead_of_the_repository_id`, `corbarust_4_4_the_codebase_resolver_supplies_a_missing_factory`.

**Status:** done

## §5 Asynchronous Method Invocation (§22)

### §5 AMI codegen

**Spec:** §5 — `@ami` → `<Iface>AmiHandler` + `sendc_<op>` (callback) + `sendp_<op>`/poller (polling) via `AsyncCorbaChannel`.

**Repo:** `crates/corba-rust/src/ami_emit.rs::emit_ami`; `crates/corba-rust/src/runtime.rs::AsyncCorbaChannel`.

**Tests:** `crates/corba-rust/tests/codegen_mapping.rs::{corbarust_5_ami_emits_the_handler_sendc_sendp_and_the_poller, corbarust_5_no_ami_without_the_annotation}`; `crates/corba-rust/tests/compile_check.rs::corbarust_3_the_generated_code_compiles_with_the_data_column`; `crates/corba-interop/tests/ami.rs::{corbarust_5_ami_callback_multiple_outstanding, corbarust_5_ami_polling_get_reply, corbarust_5_ami_mixed_callback_and_polling, corbarust_5_ami_exception_path, corbarust_5_ami_get_reply_unknown_id_errors}`, `crates/corba-interop/tests/ami_generated.rs`; against omniORB's Bench server `crates/corba-interop/tests/omniorb_cross_orb.rs::corbarust_5_the_ami_client_calls_an_omniorb_server_by_callback_and_by_polling` (`#[ignore]`, job `integration-bridges-vendor-corba`).

**Status:** done

## §6 Asynchronous Method Handling (§22.9)

### §6 AmhEndpoint

**Spec:** §6 — `AmhEndpoint::accept_request` + `AmhResponseHandler` (deferred reply, out-of-order).

**Repo:** `crates/corba-interop/src/runtime.rs::AmhEndpoint`, `AmhResponseHandler`.

**Tests:** `crates/corba-interop/tests/amh.rs::{corbarust_6_amh_deferred_out_of_order_replies, corbarust_6_amh_deferred_exception}`.

**Status:** done

## §7 Bidirectional GIOP (§15.8)

### §7 BiDirEndpoint

**Spec:** §7 — connection-reuse peer, request_id parity, BiDir SC (tag 5), reentrant collect_reply, object key from KeyAddr/ProfileAddr/ReferenceAddr.

**Repo:** `crates/corba-interop/src/runtime.rs::BiDirEndpoint`, `target_object_key`.

**Tests:** `crates/corba-interop/tests/bidir_giop.rs::{corbarust_7_acceptor_callbacks_originator_over_shared_connection, corbarust_7_out_of_order_reply_demux, corbarust_7_sync_invoke_convenience}`; against JacORB `crates/corba-interop/tests/bidir_cross_orb.rs::corbarust_7_a_jacorb_server_calls_back_over_the_connection_zerodds_opened` (`#[ignore]`, job `integration-bridges-vendor-corba`).

**Status:** done

## §8 Portable Interceptors (§16)

### §8 RequestInfo + registry

**Spec:** §8 — `RequestInfo` (SC add/get, forward_reference), named points, `ServiceContextInjector`.

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::RequestInfo`, `InterceptorRegistry`, `ServiceContextInjector`.

**Tests:** `crates/corba-ccm/tests/pi_service_contexts.rs::{ots_and_csiv2_service_contexts_route_through_pi, forward_reference_via_receive_other, reply_service_context_injection}`.

**Status:** done

## §9 Runtime API

### §9 Connection + channel + exception

**Spec:** §9 — `CorbaConnection`, `AsyncCorbaChannel`, `CorbaException`, `SkeletonResult` (without NotYetWired), `ValueBase`/`ValueMarshal`.

**Repo:** `crates/corba-rust/src/runtime.rs`; transport `crates/corba-interop/src/runtime.rs`.

**Tests:** `crates/corba-rust/src/runtime.rs::{corbarust_9_a_list_count_the_stream_cannot_hold_is_refused, corbarust_9_a_chunked_list_count_the_stream_cannot_hold_is_refused, corbarust_9_a_list_count_within_the_stream_is_read}` (the `ValueBase`/`ValueMarshal` stream through `ValueStreamReader`/`ValueStreamWriter`), `crates/corba-rust/tests/wire_giop.rs::{corbarust_9_a_value_tag_is_the_tag_and_the_repository_id, corbarust_9_the_null_tag_reads_as_no_value, corbarust_9_a_tag_with_a_bit_outside_15_3_4_1_is_refused, corbarust_9_a_list_of_repository_ids_is_the_count_and_the_strings, corbarust_9_chunks_carry_their_size_on_a_long_boundary_and_end_with_minus_1}`; `CorbaConnection`/`AsyncCorbaChannel`/`SkeletonResult` through the generated code: `crates/corba-rust/tests/compile_check.rs::corbarust_3_the_generated_code_compiles_with_the_data_column`.

**Status:** done

## §10 Repository ID format

### §10 Scope-correct RepoIds

**Spec:** §10 — `*_REPOSITORY_ID` constant per interface/valuetype, scope-correct (module path + typeprefix).

**Repo:** `crates/corba-rust/src/emitter.rs` (scope threading),
`crates/corba-rust/src/valuetype_emit.rs`,
`crates/corba-rust/src/interface_emit.rs::emit_interface_exceptions_enum`
(the `*_REPOSITORY_ID` constant comes from
`zerodds_corba_codegen::repository_id_of`, i.e. from the model that
carries `#pragma prefix`/`version`/`ID`).

**Tests:** `crates/corba-rust/tests/codegen_mapping.rs::corbarust_10_an_exception_repository_id_has_its_scope_and_typeprefix`.

**Status:** done

## §11/§13 Conformance + tests

### §13 Cross-ORB live + byte-conformance

**Spec:** §13 — GIOP e2e + cross-ORB live against JacORB + byte-conformant against omniORB.

**Repo:** `crates/corba-interop/competitors/` (the JacORB, omniORB and TAO programs, `build_peers.sh`).

**Tests:** `crates/corba-interop/tests/valuetype_cross_orb.rs::corbarust_13_a_jacorb_server_echoes_a_valuetype_parameter`,
`crates/corba-interop/tests/csiv2_cross_orb.rs::corbarust_13_a_jacorb_target_accepts_a_gssup_credential_and_refuses_a_wrong_one`,
`crates/corba-interop/tests/bidir_cross_orb.rs::corbarust_7_a_jacorb_server_calls_back_over_the_connection_zerodds_opened`
(all `#[ignore]`; each test builds and starts its JacORB server, job
`integration-bridges-vendor-corba`); `crates/corba-iiop/src/bidir.rs::bidir_sc_byte_identical_to_omniorb`;
`crates/corba-interop/tests/omniorb_cross_orb.rs::{corbarust_13_zerodds_calls_echo_and_bench_on_an_omniorb_server, corbarust_13_an_omniorb_client_calls_echo_and_bench_on_a_zerodds_server, corbarust_13_an_omniorb_client_drives_a_zerodds_naming_context, corbarust_13_zerodds_binds_and_resolves_in_omninames, corbarust_13_an_omniorb_client_calls_echo_over_ssliop_on_a_zerodds_server, corbarust_13_zerodds_calls_echo_over_ssliop_on_an_omniorb_server, corbarust_13_the_generated_stubs_call_every_operation_of_features_and_shapes_on_an_omniorb_server, corbarust_13_an_omniorb_client_calls_every_operation_of_the_generated_features_and_shapes_skeletons}`,
`crates/corba-interop/tests/jacorb_cross_orb.rs::{corbarust_13_zerodds_calls_echo_and_bench_on_a_jacorb_server, corbarust_13_a_jacorb_client_calls_echo_and_bench_on_a_zerodds_server, corbarust_13_a_jacorb_client_drives_a_zerodds_naming_context, corbarust_13_zerodds_binds_and_resolves_in_jacorbs_name_server, corbarust_13_a_jacorb_transactional_client_propagates_its_ots_context_to_a_zerodds_server}`
(`#[ignore]`; the programs of `competitors/build_peers.sh`, job
`integration-bridges-vendor-corba`);
`crates/corba-interop/tests/tao_cross_orb.rs::{corbarust_13_zerodds_calls_echo_and_bench_on_a_tao_server, corbarust_13_a_tao_client_calls_echo_and_bench_on_a_zerodds_server, corbarust_13_a_tao_client_drives_a_zerodds_naming_context}`
(`#[ignore]`, job `vendor-live-corba-tao`). Each side's code is generated from
the same IDL by its own compiler, the foreign ORB is the oracle of what
ZeroDDS writes and reads.

**Status:** done

---

## Audit status

14 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9071 of `main` (`python3 ci/coverage_results.py --document zerodds-corba-rust-1.1.md`):

* `cargo nextest run --profile merge -p zerodds-corba-ccm --all-targets` (job `merge-rust-corba-ccm`) — 3 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-iiop --all-targets` (job `merge-rust-corba-iiop`) — 1 test green.
* `cargo nextest run --profile merge -p zerodds-corba-interop --all-targets` (job `merge-rust-corba-interop`) — 12 tests green.
* `cargo nextest run --profile merge -p zerodds-corba-rust --all-targets` (job `merge-rust-corba-rust`) — 32 tests green.
* `python3 ci/run_integration.py integration-bridges-vendor-corba` (job `integration-bridges-vendor-corba`) — 17 tests green.
* job `vendor-live-corba-tao` — 3 tests green.

68 cited tests green, 0 not green.


# zerodds-corba-rust 1.1 — Spec Coverage

**Source:** `docs/specs/zerodds-corba-rust-1.1.md` (ZeroDDS vendor spec)

Implementation:

- `crates/corba-rust/` — CORBA-specific IDL→Rust mappings (valuetype/interface/component/home).

## §3 Interface mapping

### §3 Trait + stub + skeleton

**Spec:** §3 — one trait per `interface I`, `IStub` (client, GIOP marshalling), `dispatch_i` (server skeleton).

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_interface` (`emit_interface_trait`, `emit_interface_stub`, `emit_interface_skeleton`).

**Tests:** `crates/corba-rust/tests/snapshot_codegen.rs::snapshot_simple_interface`; `compile_check.rs::compile_check_simple_interface`.

**Status:** done

### §3.1 Operation mapping

**Spec:** §3.1 — void/T/in/out/inout/oneway; `raises` → typed `IError` enum (`System(CorbaException)` + variants).

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_op_stub_impl`, `emit_interface_exceptions_enum`.

**Tests:** `snapshot_codegen.rs::snapshot_interface_with_raises`, `snapshot_interface_with_inout_param`, `snapshot_interface_with_oneway_op`; `compile_check.rs::compile_check_interface_raises`.

**Status:** done

### §3.2 Attribute mapping

**Spec:** §3.2 — `readonly`/writable attributes → getter/setter.

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_attr_stub_impl`.

**Tests:** `snapshot_codegen.rs::snapshot_interface_with_attribute`.

**Status:** done

## §4 Valuetype mapping (§15.3.4)

### §4.1 State carrier + factory + flattening

**Spec:** §4.1 — `<Name>Value` struct + `ValueBase`/`ValueMarshal` + factory; inheritance state flattening (base first); truncatable/custom → `<NAME>_BASE_IDS`.

**Repo:** `crates/corba-rust/src/valuetype_emit.rs::emit_value_struct`, `flatten_fields`, `collect_base_repo_ids`.

**Tests:** `snapshot_codegen.rs::snapshot_valuetype_with_state_member`, `snapshot_valuetype_with_inheritance`, `truncatable_valuetype_emits_base_ids`; `compile_check.rs::compile_check_valuetype_state`, `compile_check_valuetype_inheritance`.

**Status:** done

### §4.2 Value wire engine (sharing)

**Spec:** §4.2 — value_tag (null/indirection/single/list), value sharing (Rc identity → indirection, DAG preserved).

**Repo:** `crates/corba-rust/src/value_wire.rs::ValueWriter`, `ValueReader`, `ValueRegistry`.

**Tests:** `value_wire.rs::tests`: `single_value_and_null_roundtrip`, `value_sharing_indirection`, `distinct_values_are_not_shared`, `forward_indirection_rejected`, `value_tag_is_single_repo_id_on_wire`, `jacorb_capture_byte_identical`.

**Status:** done

### §4.3 Chunked encoding + truncation + custom

**Spec:** §4.2 — `write_chunked` (0x7fffff0e + chunk + end tag); truncation decode (most-derived unknown → read base, skip the rest); custom via the chunked path; byte-conformance to JacORB.

**Repo:** `crates/corba-rust/src/value_wire.rs::ValueWriter::write_chunked`, `read_chunked_state`.

**Tests:** `value_wire.rs::tests`: `chunked_encode_byte_identical_to_jacorb`, `chunked_decode_full_when_derived_known`, `chunked_decode_truncates_to_base`, `chunked_roundtrip_zerodds_to_zerodds`.

**Status:** done

### §4.4 Nested chunked values + codebase URL

**Spec:** §15.3.4.3 — nested chunked values + multi-level end tags; §15.3.4.1 codebase URL (value_tag bit 0).

**Repo:** `crates/corba-rust/src/value_wire.rs::read_chunked_state` (recursive via `skip_value_from_tag`/`skip_chunked_body`, multi-level end tags, indirection in the chunk stream); codebase via `ValueRegistry::ctor_for` + `CodebaseResolver`, `ValueWriter::write_with_codebase`.

**Tests:** `value_wire.rs::tests`: `nested_chunked_value_in_tail_is_consumed_on_truncation`, `shared_end_tag_closes_nested_and_outer`, `codebase_value_tag_and_roundtrip`, `codebase_resolver_supplies_missing_factory`.

**Status:** done

## §5 Asynchronous Method Invocation (§22)

### §5 AMI codegen

**Spec:** §5 — `@ami` → `<Iface>AmiHandler` + `sendc_<op>` (callback) + `sendp_<op>`/poller (polling) via `AsyncCorbaChannel`.

**Repo:** `crates/corba-rust/src/ami_emit.rs::emit_ami`; `crates/corba-rust/src/runtime.rs::AsyncCorbaChannel`.

**Tests:** `crates/corba-rust/tests/snapshot_codegen.rs::{ami_codegen_emits_handler_sendc_sendp_poller, no_ami_codegen_without_annotation}`; `crates/corba-rust/tests/compile_check.rs::compile_check_ami`; `crates/corba-interop/tests/ami.rs::{ami_callback_multiple_outstanding, ami_polling_get_reply, ami_mixed_callback_and_polling, ami_exception_path, ami_get_reply_unknown_id_errors}`, `crates/corba-interop/tests/ami_generated.rs`.

**Status:** done

## §6 Asynchronous Method Handling (§22.9)

### §6 AmhEndpoint

**Spec:** §6 — `AmhEndpoint::accept_request` + `AmhResponseHandler` (deferred reply, out-of-order).

**Repo:** `crates/corba-interop/src/runtime.rs::AmhEndpoint`, `AmhResponseHandler`.

**Tests:** `crates/corba-interop/tests/amh.rs::{amh_deferred_out_of_order_replies, amh_deferred_exception}`.

**Status:** done

## §7 Bidirectional GIOP (§15.8)

### §7 BiDirEndpoint

**Spec:** §7 — connection-reuse peer, request_id parity, BiDir SC (tag 5), reentrant collect_reply, object key from KeyAddr/ProfileAddr/ReferenceAddr.

**Repo:** `crates/corba-interop/src/runtime.rs::BiDirEndpoint`, `target_object_key`.

**Tests:** `crates/corba-interop/tests/bidir_giop.rs::{acceptor_callbacks_originator_over_shared_connection, out_of_order_reply_demux, sync_invoke_convenience}`; cross-ORB live `crates/corba-interop/tests/bidir_cross_orb.rs` (JacORB, `#[ignore]`).

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

**Tests:** `crates/corba-rust/src/runtime.rs::{value_tag_list_rejects_oversized_count, value_tag_chunked_list_rejects_oversized_count, value_tag_list_within_bound_still_round_trips}` (the `ValueBase`/`ValueMarshal` stream through `ValueStreamReader`/`ValueStreamWriter`), `crates/corba-rust/tests/wire_giop.rs::{wire_value_tag_roundtrip, wire_value_tag_null_reference, wire_value_tag_truly_unsupported_returns_error, wire_value_tag_multi_repo_id_list_round_trip, wire_value_tag_chunked_with_list_round_trip}`; `CorbaConnection`/`AsyncCorbaChannel`/`SkeletonResult` through the generated code: `crates/corba-rust/tests/compile_check.rs::{compile_check_simple_interface, compile_check_interface_raises}`.

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

**Tests:** `crates/corba-rust/tests/snapshot_codegen.rs::scope_aware_exception_repo_id_with_typeprefix`.

**Status:** done

## §11/§13 Conformance + tests

### §13 Cross-ORB live + byte-conformance

**Spec:** §13 — GIOP e2e + cross-ORB live against JacORB + byte-conformant against omniORB.

**Repo:** `crates/corba-interop/competitors/` (JacORB/omniORB/TAO harness).

**Tests:** `valuetype_cross_orb.rs`, `ami_cross_orb.rs`, `csiv2_cross_orb.rs`, `bidir_cross_orb.rs` (ignored, Linux bench host); `bidir.rs::bidir_sc_byte_identical_to_omniorb`.

**Status:** done

---

## Audit status

14 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-corba-rust --lib --test wire_giop --test snapshot_codegen`
  — 20 + 5 + 14 tests green, 0 failed.
* `cargo test -p zerodds-corba-interop --test amh --test ami --test ami_generated --test bidir_giop`
  — 2 + 5 + 2 + 3 tests green, 0 failed.
* `cargo test -p zerodds-corba-ccm --test pi_service_contexts` — 3 tests
  green, 0 failed.

The `*_cross_orb.rs` suites of §13 carry `#[ignore]` and run only on the
Linux bench host with JacORB/omniORB/TAO.

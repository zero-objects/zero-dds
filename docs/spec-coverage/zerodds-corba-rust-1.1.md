# zerodds-corba-rust 1.1 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-corba-rust-1.1.md` (ZeroDDS Vendor-Spec)

Implementation:

- `crates/corba-rust/` — CORBA-spezifische IDL→Rust-Mappings (valuetype/interface/component/home).

## §3 Interface-Mapping

### §3 Trait + Stub + Skeleton

**Spec:** §3 — pro `interface I` ein Trait, `IStub` (Client, GIOP-Marshalling), `dispatch_i` (Server-Skeleton).

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_interface` (`emit_interface_trait`, `emit_interface_stub`, `emit_interface_skeleton`).

**Tests:** `crates/corba-rust/tests/snapshot_codegen.rs::snapshot_simple_interface`; `compile_check.rs::compile_check_simple_interface`.

**Status:** done

### §3.1 Operation-Mapping

**Spec:** §3.1 — void/T/in/out/inout/oneway; `raises` → typisierter `IError`-Enum (`System(CorbaException)` + Varianten).

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_op_stub_impl`, `emit_interface_exceptions_enum`.

**Tests:** `snapshot_codegen.rs::snapshot_interface_with_raises`, `snapshot_interface_with_inout_param`, `snapshot_interface_with_oneway_op`; `compile_check.rs::compile_check_interface_raises`.

**Status:** done

### §3.2 Attribute-Mapping

**Spec:** §3.2 — `readonly`/writable Attribute → getter/setter.

**Repo:** `crates/corba-rust/src/interface_emit.rs::emit_attr_stub_impl`.

**Tests:** `snapshot_codegen.rs::snapshot_interface_with_attribute`.

**Status:** done

## §4 Valuetype-Mapping (§15.3.4)

### §4.1 State-Carrier + Factory + Flattening

**Spec:** §4.1 — `<Name>Value`-Struct + `ValueBase`/`ValueMarshal` + Factory; Inheritance-State-Flattening (Basis zuerst); truncatable/custom → `<NAME>_BASE_IDS`.

**Repo:** `crates/corba-rust/src/valuetype_emit.rs::emit_value_struct`, `flatten_fields`, `collect_base_repo_ids`.

**Tests:** `snapshot_codegen.rs::snapshot_valuetype_with_state_member`, `snapshot_valuetype_with_inheritance`, `truncatable_valuetype_emits_base_ids`; `compile_check.rs::compile_check_valuetype_state`, `compile_check_valuetype_inheritance`.

**Status:** done

### §4.2 Value-Wire-Engine (Sharing)

**Spec:** §4.2 — value_tag (null/Indirection/single/list), Value-Sharing (Rc-Identität → Indirection, DAG erhalten).

**Repo:** `crates/corba-rust/src/value_wire.rs::ValueWriter`, `ValueReader`, `ValueRegistry`.

**Tests:** `value_wire.rs::tests`: `single_value_and_null_roundtrip`, `value_sharing_indirection`, `distinct_values_are_not_shared`, `forward_indirection_rejected`, `value_tag_is_single_repo_id_on_wire`, `jacorb_capture_byte_identical`.

**Status:** done

### §4.3 Chunked encoding + Truncation + Custom

**Spec:** §4.2 — `write_chunked` (0x7fffff0e + Chunk + End-Tag); Truncation-Decode (most-derived unknown → Basis lesen, Rest skippen); Custom über chunked-Pfad; Byte-Konformität zu JacORB.

**Repo:** `crates/corba-rust/src/value_wire.rs::ValueWriter::write_chunked`, `read_chunked_state`.

**Tests:** `value_wire.rs::tests`: `chunked_encode_byte_identical_to_jacorb`, `chunked_decode_full_when_derived_known`, `chunked_decode_truncates_to_base`, `chunked_roundtrip_zerodds_to_zerodds`.

**Status:** done

### §4.4 Nested chunked Values + codebase-URL

**Spec:** §15.3.4.3 — verschachtelte chunked Values + mehrstufige End-Tags; §15.3.4.1 codebase-URL (value_tag-Bit 0).

**Repo:** `crates/corba-rust/src/value_wire.rs::read_chunked_state` (rekursiv via `skip_value_from_tag`/`skip_chunked_body`, mehrstufige End-Tags, Indirection im Chunk-Strom); codebase über `ValueRegistry::ctor_for` + `CodebaseResolver`, `ValueWriter::write_with_codebase`.

**Tests:** `value_wire.rs::tests`: `nested_chunked_value_in_tail_is_consumed_on_truncation`, `shared_end_tag_closes_nested_and_outer`, `codebase_value_tag_and_roundtrip`, `codebase_resolver_supplies_missing_factory`.

**Status:** done

## §5 Asynchronous Method Invocation (§22)

### §5 AMI-Codegen

**Spec:** §5 — `@ami` → `<Iface>AmiHandler` + `sendc_<op>` (Callback) + `sendp_<op>`/Poller (Polling) über `AsyncCorbaChannel`.

**Repo:** `crates/corba-rust/src/ami_emit.rs::emit_ami`; `crates/corba-rust/src/runtime.rs::AsyncCorbaChannel`.

**Tests:** `snapshot_codegen.rs::ami_codegen_emits_handler_sendc_sendp_poller`, `no_ami_codegen_without_annotation`; `compile_check.rs::compile_check_ami`; `corba-interop/tests/ami.rs` (5), `ami_generated.rs` (2).

**Status:** done

## §6 Asynchronous Method Handling (§22.9)

### §6 AmhEndpoint

**Spec:** §6 — `AmhEndpoint::accept_request` + `AmhResponseHandler` (verzögerter Reply, out-of-order).

**Repo:** `crates/corba-interop/src/runtime.rs::AmhEndpoint`, `AmhResponseHandler`.

**Tests:** `corba-interop/tests/amh.rs::amh_deferred_out_of_order_replies`, `amh_deferred_exception`.

**Status:** done

## §7 Bidirectional GIOP (§15.8)

### §7 BiDirEndpoint

**Spec:** §7 — Connection-Reuse-Peer, request_id-Parität, BiDir-SC (Tag 5), reentrantes collect_reply, Object-Key aus KeyAddr/ProfileAddr/ReferenceAddr.

**Repo:** `crates/corba-interop/src/runtime.rs::BiDirEndpoint`, `target_object_key`.

**Tests:** `corba-interop/tests/bidir_giop.rs` (3); cross-ORB live `bidir_cross_orb.rs` (JacORB, ignored).

**Status:** done

## §8 Portable Interceptors (§16)

### §8 RequestInfo + Registry

**Spec:** §8 — `RequestInfo` (SC add/get, forward_reference), benannte Points, `ServiceContextInjector`.

**Repo:** `crates/corba-ccm/src/orb_extensions.rs::RequestInfo`, `InterceptorRegistry`, `ServiceContextInjector`.

**Tests:** `corba-ccm/tests/pi_service_contexts.rs` (3): OTS/CSIv2-SC durch PI, forward_reference, Reply-SC.

**Status:** done

## §9 Runtime-API

### §9 Connection + Channel + Exception

**Spec:** §9 — `CorbaConnection`, `AsyncCorbaChannel`, `CorbaException`, `SkeletonResult` (ohne NotYetWired), `ValueBase`/`ValueMarshal`.

**Repo:** `crates/corba-rust/src/runtime.rs`; Transport `crates/corba-interop/src/runtime.rs`.

**Tests:** `crates/corba-rust/src/runtime.rs::tests` (object-reference roundtrip); Transport via interop-e2e.

**Status:** done

## §10 Repository-ID-Format

### §10 Scope-korrekte RepoIds

**Spec:** §10 — `*_REPOSITORY_ID`-Konstante pro Interface/Valuetype, scope-korrekt (Modul-Pfad + typeprefix).

**Repo:** `crates/corba-rust/src/emitter.rs` (scope-Threading), `valuetype_emit.rs`, `interface_emit.rs::exc_repo_id`.

**Tests:** `snapshot_codegen.rs::scope_aware_exception_repo_id_with_typeprefix`.

**Status:** done

## §11/§13 Konformität + Tests

### §13 Cross-ORB-Live + Byte-Konformität

**Spec:** §13 — GIOP-e2e + cross-ORB live gegen JacORB + byte-konform gegen omniORB.

**Repo:** `crates/corba-interop/competitors/` (JacORB/omniORB/TAO-Harness).

**Tests:** `valuetype_cross_orb.rs`, `ami_cross_orb.rs`, `csiv2_cross_orb.rs`, `bidir_cross_orb.rs` (ignored, codepit); `bidir.rs::bidir_sc_byte_identical_to_omniorb`.

**Status:** done

---

## Audit-Status

14 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-corba-rust -p zerodds-corba-interop -p zerodds-corba-ccm` — alle Suites grün, 0 failed.

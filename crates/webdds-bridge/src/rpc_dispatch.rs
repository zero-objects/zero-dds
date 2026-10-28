// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Dispatches a SOAP operation of a config-named IDL `interface` as a
//! DDS-RPC request, and returns the reply — DDS-RPC 1.0 §7.5.1.1 Basic
//! Service Mapping: **one request/reply topic pair per interface**
//! (§7.4.3), a `Call`/`Return` union multiplexing every operation onto
//! it (§7.5.1.1.6/§7.5.1.1.7), each operation's reply case
//! itself a `Result` union over `dds::RETCODE_OK` (carrying the `Out`
//! struct) and its declared `raises` exceptions (§7.5.1.1.5). Reuses
//! `zerodds_lang_rules::adapter::interface` for the operation model —
//! the same one `crate::wsdl_bridge` builds the WSDL from ("WSDL and
//! runtime from the same model") — and that crate's own
//! `operation_hash`/`exception_hash`/`in_struct_name`/`out_struct_name`
//! for the exact names/discriminators DDS-RPC 1.0 §7.5.1.1.2/§7.5.1.1.6
//! rule 5a fixes: the same functions `emit::interface_rust`'s generated
//! Rust binding calls for the identical purpose, so a Requester built
//! here and a `Replier` `zerodds-idlc generate --lang rust` wrote for
//! the *same* interface compute the *same* topic name, the *same*
//! discriminator per operation, and the *same* member layout —
//! verified end to end in `tests/soap_rpc_e2e.rs` against exactly such
//! a generated `Replier` (`zerodds-rpc-rust-e2e`'s own `Calculator`
//! binding).
//!
//! # The wire types are synthesized here, not code-generated
//!
//! `zerodds_rpc::codegen` describes the same shapes
//! (`RequestType`/`ReplyType`, `CallUnionDef`, `ResultUnionDef`) for a
//! *compiled* binding: real structs/enums, a real `DdsType` impl,
//! written by `idlc` before the binary exists. This gateway has no
//! compiled type for a business interface it only read from a config
//! file at start-up — it builds the equivalent `CompleteTypeObject`s
//! straight from the operation model
//! (`zerodds_types::from_ast::type_identifier` per parameter) and hands
//! them to [`crate::dynamic_rpc::DynamicRpcValue`], the same way
//! `crate::gateway` hands a topic's `CompleteTypeObject` to
//! `SerializedTypeSupport`. Every synthesized type is `@final` — no
//! `DHEADER`, classic-CDR shape — matching `emit::interface_rust`'s own
//! choice for exactly this reason (its module doc: "matching the
//! classic-CDR shape DDS-RPC 1.0 predates XTypes 1.3's extensibility
//! kinds with"); building them `@appendable` here once produced bytes a
//! generated peer's hand-written codec would not decode.
//!
//! # A union's `DynamicData` shape
//!
//! `zerodds_wire_codec`'s encoder/decoder represent a union value the
//! same way a struct is represented — a
//! [`zerodds_types::dynamic::DynamicData::Aggregate`] — with a
//! `"_d"` entry for the discriminator and one more entry named after
//! whichever case it selects (`crates/wire-codec/src/encode.rs`'s own
//! `write_union`: "A union's discriminator and its branch are members,
//! whatever the union itself was reached as" / "for `(name,
//! member_value)` in object { if name == "_d" { continue } ... }").
//! This module builds/reads that shape directly rather than needing a
//! dedicated union variant.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use zerodds_ast::{Ast, ExceptionNode, TypeNode};
use zerodds_ast_core::Id;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_lang_rules::adapter::interface::{
    self, InterfaceError, Operation, Raise, Service, all_operations_flattened,
    exception_member_name, in_struct_name, operation_hash, out_struct_name, services,
};
use zerodds_rpc::codegen::{
    DUMMY_MEMBER, RESULT_MEMBER, RETCODE_OK, RETURN_MEMBER, UNKNOWN_OP_MEMBER,
};
use zerodds_rpc::endpoint_options::RpcEndpointOptions;
use zerodds_rpc::qos_profile::RpcQos;
use zerodds_rpc::requester::Requester;
use zerodds_rpc::topic_naming::{AnnotatedTopics, BindingStyle};
use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::dynamic::DynamicData;
use zerodds_types::{CompleteTypeObject, PrimitiveKind, TypeIdentifier};

use crate::dynamic_rpc::{self, DynamicRpcValue};
use crate::dynxml;
use crate::types_registry::{ResolvedType, TypeRegistry};

/// Dispatch error.
#[derive(Debug)]
pub enum RpcDispatchError {
    /// The config-named IDL file's `interface`s did not build (an
    /// unresolved base, an inheritance cycle).
    Model(String),
    /// No operation of any configured interface has this name.
    UnknownOperation(String),
    /// Building a synthetic `CompleteTypeObject` refused a parameter's
    /// type — XTypes 1.3 §7.3 has no `TypeIdentifier` for it, or it is
    /// recursive.
    UnsupportedParameterType(String),
    /// The request body did not convert to the `In` struct's shape.
    BadRequest(String),
    /// The `Requester`/DCPS layer refused the call.
    Rpc(String),
    /// The remote side reported a foundation-level `RemoteExceptionCode
    /// != Ok` (Spec §7.6.2) — a protocol-level refusal (e.g. the
    /// `Replier` itself does not recognize the discriminator), not a
    /// declared IDL `raises` exception (see
    /// [`Self::OperationException`]).
    RemoteException(u32),
    /// The reply's `Result` union (Spec §7.5.1.1.5) selected a declared
    /// `raises` exception's case, not `dds::RETCODE_OK` — the call
    /// reached the service and it refused *this* request specifically.
    OperationException {
        /// The operation that raised it.
        operation: String,
        /// The exception's simple IDL name.
        exception: String,
        /// Its fields, as the same XML representation a successful
        /// reply's `Out` struct would use.
        fields_xml: String,
    },
    /// The call timed out waiting for a reply.
    Timeout,
}

impl std::fmt::Display for RpcDispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Model(m) => write!(f, "interface model: {m}"),
            Self::UnknownOperation(op) => {
                write!(f, "no configured interface declares operation `{op}`")
            }
            Self::UnsupportedParameterType(m) => write!(f, "unsupported parameter type: {m}"),
            Self::BadRequest(m) => write!(f, "bad request: {m}"),
            Self::Rpc(m) => write!(f, "rpc: {m}"),
            Self::RemoteException(code) => write!(f, "remote exception, code {code}"),
            Self::OperationException {
                operation,
                exception,
                fields_xml,
            } => write!(
                f,
                "operation `{operation}` raised `{exception}`: {fields_xml}"
            ),
            Self::Timeout => f.write_str("timed out waiting for a reply"),
        }
    }
}

impl std::error::Error for RpcDispatchError {}

/// Default bound on one RPC round trip.
const RPC_TIMEOUT: Duration = Duration::from_secs(10);

/// One operation's wire model within its interface's `Call`/`Return`
/// unions.
struct OpModel {
    /// `${I}_${op}_In`'s [`ResolvedType`] — the `Call` union's case
    /// type for this operation.
    in_resolved: ResolvedType,
    /// `${I}_${op}_Result`'s [`ResolvedType`] —
    /// the `Return` union's case type; its own `dds::RETCODE_OK` case
    /// carries [`Self::out_resolved`], one more case per
    /// [`Self::exceptions`]. `dispatch()` never looks this up again by
    /// itself — the type is already reachable through
    /// `InterfaceModel::return_resolved`'s own dependency closure, one
    /// case type among others — so only this module's own tests read
    /// the field back, to assert the model built the right shape.
    #[cfg_attr(not(test), allow(dead_code))]
    result_resolved: ResolvedType,
    /// `${I}_${op}_Out`'s [`ResolvedType`].
    out_resolved: ResolvedType,
    /// One entry per `raises` exception: its discriminator (Spec
    /// §7.5.1.1.5 rule 4b), the `Result` union's member name for it
    /// (rule 5b), the exception's simple name, and its own
    /// [`ResolvedType`].
    exceptions: Vec<(i32, String, String, ResolvedType)>,
}

/// One interface's whole DDS-RPC wire model — built once, cached for
/// the daemon's lifetime (an `interfaces:` file does not change
/// without a restart).
struct InterfaceModel {
    /// `${I}_Call`'s [`ResolvedType`] — the `Requester`/`Replier`'s
    /// `TIn`/`TOut` wire type on the request/reply topic respectively.
    call_resolved: ResolvedType,
    /// `${I}_Return`'s [`ResolvedType`].
    return_resolved: ResolvedType,
    /// Keyed by [`operation_hash`] of the operation's own (unqualified)
    /// name — the same key both the `Call` and the `Return` union
    /// case use (DDS-RPC 1.0 §7.5.1.1.6 rule 5a/§7.5.1.1.7).
    operations: HashMap<i32, OpModel>,
}

/// Dispatches business-interface operations as DDS-RPC calls.
///
/// One instance per daemon, built from every config-named `interfaces:`
/// IDL file, sharing one dedicated client `DomainParticipant`
/// (`daemon::server` creates it on the daemon's configured `domain`).
pub struct RpcDispatcher {
    ast: Arc<Ast>,
    services: Vec<Service>,
    participant: DomainParticipant,
    types: Arc<TypeRegistry>,
    models: Mutex<HashMap<String, Arc<InterfaceModel>>>,
    requesters: Mutex<HashMap<String, Arc<Requester<DynamicRpcValue, DynamicRpcValue>>>>,
}

impl RpcDispatcher {
    /// Parses every interface out of `ast` (already the union of every
    /// config-named `interfaces:` file — `daemon::server` builds it the
    /// same way it builds the WSDL).
    ///
    /// # Errors
    /// [`RpcDispatchError::Model`] on a malformed interface.
    pub fn new(ast: Arc<Ast>, participant: DomainParticipant) -> Result<Self, RpcDispatchError> {
        let services = services(&ast).map_err(interface_error)?;
        Ok(Self {
            ast,
            services,
            participant,
            types: Arc::new(TypeRegistry::new()),
            models: Mutex::new(HashMap::new()),
            requesters: Mutex::new(HashMap::new()),
        })
    }

    /// Finds the `(Service, Operation)` pair a bare operation name
    /// (as a SOAP body's root element names it) resolves to, across
    /// every configured interface's flattened operation list
    /// (declared + inherited + attribute-implied).
    fn find_operation(&self, op_name: &str) -> Result<(&Service, Operation), RpcDispatchError> {
        for service in &self.services {
            let flattened =
                all_operations_flattened(&self.ast, service.id).map_err(interface_error)?;
            if let Some(op) = flattened.into_iter().find(|op| op.name == op_name) {
                return Ok((service, op));
            }
        }
        Err(RpcDispatchError::UnknownOperation(op_name.to_string()))
    }

    /// `true` if `op_name` names an operation of a configured
    /// interface — `soap_service` uses this to decide whether a
    /// non-builtin operation name is an RPC call or genuinely unknown.
    #[must_use]
    pub fn knows_operation(&self, op_name: &str) -> bool {
        self.find_operation(op_name).is_ok()
    }

    /// The cached [`InterfaceModel`] for `service`, building it on
    /// first use.
    fn model_for(&self, service: &Service) -> Result<Arc<InterfaceModel>, RpcDispatchError> {
        {
            let models = self
                .models
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(m) = models.get(&service.qualified_name) {
                return Ok(Arc::clone(m));
            }
        }
        let model = Arc::new(build_interface_model(&self.ast, service, &self.types)?);
        let mut models = self
            .models
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        models.insert(service.qualified_name.clone(), Arc::clone(&model));
        Ok(model)
    }

    /// Dispatches `op_name` with `request_body_xml` (the SOAP body's
    /// operation element, e.g. `<add><a>3</a><b>4</b></add>`) as a
    /// DDS-RPC call, and returns the reply's body XML (e.g.
    /// `<data><return_>7</return_></data>`) — empty for an operation with
    /// no outputs and no return value.
    ///
    /// # Errors
    /// See [`RpcDispatchError`].
    pub fn dispatch(
        &self,
        op_name: &str,
        request_body_xml: &str,
    ) -> Result<String, RpcDispatchError> {
        let (service, operation) = self.find_operation(op_name)?;
        let model = self.model_for(service)?;
        let disc = operation_hash(&operation.name);
        let op_model = model.operations.get(&disc).ok_or_else(|| {
            RpcDispatchError::Model(format!(
                "operation `{op_name}`: not found in its own interface's model"
            ))
        })?;

        let in_data =
            dynxml::from_xml_document(request_body_xml, &op_model.in_resolved.dynamic_type)
                .map_err(|e| RpcDispatchError::BadRequest(e.to_string()))?;
        let call_ast = Arc::clone(model.call_resolved.dynamic_type.ast());
        let call_type_name = model.call_resolved.dynamic_type.name().to_string();
        let call_data = DynamicData::Aggregate(vec![
            (
                "_d".to_string(),
                DynamicData::integer(zerodds_ast::PrimitiveKind::Int32, i128::from(disc)),
            ),
            (operation.name.clone(), in_data),
        ]);
        // The participant's `wire.codec_dialect`, for the call it writes and
        // the reply it reads.
        let dialect = self
            .participant
            .runtime()
            .map(|rt| rt.codec_dialect())
            .unwrap_or_default();
        let call_value =
            DynamicRpcValue::new(call_ast, call_type_name, call_data).in_dialect(dialect);

        let requester = self.requester_for(service)?;
        let return_ast = Arc::clone(model.return_resolved.dynamic_type.ast());
        let return_type_name = model.return_resolved.dynamic_type.name().to_string();
        let reply =
            dynamic_rpc::with_decode_context(&return_ast, &return_type_name, dialect, || {
                requester.send_request_blocking(&call_value, Some(RPC_TIMEOUT))
            })
            .map_err(|e| match e {
                zerodds_rpc::error::RpcError::Timeout => RpcDispatchError::Timeout,
                zerodds_rpc::error::RpcError::RemoteException(code) => {
                    RpcDispatchError::RemoteException(code)
                }
                other => RpcDispatchError::Rpc(other.to_string()),
            })?;

        // Unwrap `${I}_Return`'s case (named after the operation) —
        // the `${I}_${op}_Result` union.
        let result_data = reply.data.member(&operation.name).ok_or_else(|| {
            RpcDispatchError::Rpc(format!(
                "reply named no `{}` case (Return union discriminator mismatch)",
                operation.name
            ))
        })?;
        let result_disc = match result_data.member("_d") {
            Some(DynamicData::Integer { value, .. }) => *value as i32,
            _ => {
                return Err(RpcDispatchError::Rpc(
                    "Result union reply carried no discriminator".to_string(),
                ));
            }
        };
        if result_disc == RETCODE_OK {
            let out_resolved = &op_model.out_resolved;
            let out_data = result_data.member(RESULT_MEMBER).ok_or_else(|| {
                RpcDispatchError::Rpc(format!(
                    "Result union's `{RESULT_MEMBER}` case named no `{RESULT_MEMBER}` member"
                ))
            })?;
            return dynxml::to_xml_document("data", out_data, &out_resolved.dynamic_type)
                .map_err(|e| RpcDispatchError::Rpc(e.to_string()));
        }
        // A declared exception case.
        let (_, member_name, exception_name, exc_resolved) = op_model
            .exceptions
            .iter()
            .find(|(disc, _, _, _)| *disc == result_disc)
            .ok_or_else(|| {
                RpcDispatchError::Rpc(format!(
                    "Result union reply named an unrecognised discriminator {result_disc}"
                ))
            })?;
        let exc_data = result_data.member(member_name).ok_or_else(|| {
            RpcDispatchError::Rpc(format!("Result union named no `{member_name}` member"))
        })?;
        let fields_xml = dynxml::to_xml_document("data", exc_data, &exc_resolved.dynamic_type)
            .map_err(|e| RpcDispatchError::Rpc(e.to_string()))?;
        Err(RpcDispatchError::OperationException {
            operation: operation.name.clone(),
            exception: exception_name.clone(),
            fields_xml,
        })
    }

    /// The `Requester` for `qualified_interface`, created on first use
    /// against this dispatcher's dedicated client participant — one
    /// per *interface* (DDS-RPC 1.0 §7.4.3), shared across all its
    /// operations.
    fn requester_for(
        &self,
        service: &Service,
    ) -> Result<Arc<Requester<DynamicRpcValue, DynamicRpcValue>>, RpcDispatchError> {
        let mut requesters = self
            .requesters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(r) = requesters.get(&service.qualified_name) {
            return Ok(Arc::clone(r));
        }
        // §7.5.1.1.6/7: the topic types are the interface's own, in its
        // module, which is what the service's endpoints announce.
        let scope = interface::module_chain(&self.ast, service.module)
            .map_err(interface_error)?
            .into_iter()
            .map(|(_, name)| name)
            .collect::<Vec<_>>()
            .join("::");
        let options = RpcEndpointOptions::for_interface(&scope, &service.name);
        let requester = Requester::with_options(
            &self.participant,
            &service.qualified_name,
            "",
            &RpcQos::default(),
            BindingStyle::FunctionCall,
            &AnnotatedTopics::none(),
            &options,
        )
        .map_err(|e| RpcDispatchError::Rpc(e.to_string()))?;
        let requester = Arc::new(requester);
        requesters.insert(service.qualified_name.clone(), Arc::clone(&requester));
        Ok(requester)
    }
}

fn interface_error(e: InterfaceError) -> RpcDispatchError {
    RpcDispatchError::Model(format!("{e:?}"))
}

/// Builds `service`'s whole [`InterfaceModel`] — every operation's
/// `In`/`Out`/exception structs, its `Result` union, and the
/// interface-wide `Call`/`Return` unions multiplexing them — and
/// registers every one of them in `types`.
fn build_interface_model(
    ast: &Ast,
    service: &Service,
    types: &TypeRegistry,
) -> Result<InterfaceModel, RpcDispatchError> {
    let qualified = &service.qualified_name;
    let flattened = all_operations_flattened(ast, service.id).map_err(interface_error)?;

    let mut call_cases: Vec<(i32, String, ResolvedType)> = Vec::new();
    let mut return_cases: Vec<(i32, String, ResolvedType)> = Vec::new();
    let mut operations: HashMap<i32, OpModel> = HashMap::new();

    for op in &flattened {
        let disc = operation_hash(&op.name);

        let in_type_name = in_struct_name(qualified, &op.name);
        let in_members: Vec<(String, Id<TypeNode>)> =
            op.inputs().map(|p| (p.name.clone(), p.type_)).collect();
        let in_resolved = resolve_synthetic_struct(types, ast, &in_type_name, &in_members)?;
        call_cases.push((disc, op.name.clone(), in_resolved.clone()));

        let out_type_name = out_struct_name(qualified, &op.name);
        let mut out_members: Vec<(String, Id<TypeNode>)> =
            op.outputs().map(|p| (p.name.clone(), p.type_)).collect();
        if let Some(ret) = op.returns {
            out_members.push((RETURN_MEMBER.to_string(), ret));
        }
        let out_resolved = resolve_synthetic_struct(types, ast, &out_type_name, &out_members)?;

        let mut result_cases: Vec<(i32, String, ResolvedType)> =
            vec![(RETCODE_OK, RESULT_MEMBER.to_string(), out_resolved.clone())];
        let mut exceptions = Vec::new();
        for raise in &op.raises {
            let ex_disc = interface::exception_hash(&raise.scoped_name);
            let ex_member = exception_member_name(&raise.name);
            let ex_type_name = format!("{qualified}_{}_{}_Ex", op.name, raise.name);
            let ex_members = exception_members(ast, raise)
                .map_err(RpcDispatchError::UnsupportedParameterType)?;
            let ex_resolved = resolve_synthetic_struct(types, ast, &ex_type_name, &ex_members)?;
            result_cases.push((ex_disc, ex_member.clone(), ex_resolved.clone()));
            exceptions.push((ex_disc, ex_member, raise.name.clone(), ex_resolved));
        }
        let result_type_name = format!("{qualified}_{}_Result", op.name);
        let result_resolved =
            resolve_synthetic_union(types, &result_type_name, &result_cases, None)?;
        return_cases.push((disc, op.name.clone(), result_resolved.clone()));

        operations.insert(
            disc,
            OpModel {
                in_resolved,
                result_resolved,
                out_resolved,
                exceptions,
            },
        );
    }

    let call_type_name = format!("{qualified}_Call");
    let call_resolved =
        resolve_synthetic_union(types, &call_type_name, &call_cases, Some(UNKNOWN_OP_MEMBER))?;
    let return_type_name = format!("{qualified}_Return");
    let return_resolved = resolve_synthetic_union(
        types,
        &return_type_name,
        &return_cases,
        Some(UNKNOWN_OP_MEMBER),
    )?;

    Ok(InterfaceModel {
        call_resolved,
        return_resolved,
        operations,
    })
}

/// The members of `raise`'s exception declaration — `(name,
/// Id<TypeNode>)` pairs, the same shape [`resolve_synthetic_struct`]
/// builds an `In`/`Out` struct from.
///
/// # Errors
/// A named string if the exception's own AST node (or one of its
/// members) does not resolve — a stale handle, which would mean the
/// model changed under us mid-request.
fn exception_members(ast: &Ast, raise: &Raise) -> Result<Vec<(String, Id<TypeNode>)>, String> {
    let node: &ExceptionNode = ast
        .exception(raise.id)
        .map_err(|e| format!("exception `{}`: {e:?}", raise.name))?;
    node.members
        .iter()
        .map(|member_id| {
            let member = ast
                .member(*member_id)
                .map_err(|e| format!("exception `{}`: {e:?}", raise.name))?;
            let type_id = member.type_.resolved().ok_or_else(|| {
                format!(
                    "exception `{}`, member `{}`: type not resolved",
                    raise.name, member.name
                )
            })?;
            Ok((member.name.clone(), type_id))
        })
        .collect()
}

/// Builds (if not already cached) the `CompleteTypeObject::Struct` for
/// `type_name` with one member per `(name, type node)` pair — DDS-RPC
/// 1.0 rule 3a-c (declaration order; a single `dummy` member when the
/// list is empty) — and registers it.
fn resolve_synthetic_struct(
    types: &TypeRegistry,
    ast: &Ast,
    type_name: &str,
    members: &[(String, Id<TypeNode>)],
) -> Result<ResolvedType, RpcDispatchError> {
    if let Some(r) = types.get(type_name) {
        return Ok(r);
    }
    let (root, dependencies) = build_synthetic_struct(ast, type_name, members)
        .map_err(RpcDispatchError::UnsupportedParameterType)?;
    types
        .resolve_synthetic(type_name, root, dependencies)
        .map_err(|e| RpcDispatchError::Model(e.to_string()))
}

/// Builds a `CompleteTypeObject::Struct` for `type_name` with one
/// member per `(name, type node)` pair, plus the dependency closure
/// every named member type needs (its own `CompleteTypeObject`, and
/// everything *it* reaches — `zerodds_types::from_ast::dependency_type_objects`).
/// `@final` — no `DHEADER` — matching the classic-CDR shape every
/// DDS-RPC synthesized type uses (see this module's doc).
///
/// # Errors
/// A named string describing the refusal — a parameter type XTypes 1.3
/// §7.3 has no `TypeIdentifier` for, or one that is recursive
/// (`zerodds_types::from_ast::FromAstError`).
fn build_synthetic_struct(
    ast: &Ast,
    type_name: &str,
    members: &[(String, Id<TypeNode>)],
) -> Result<(CompleteTypeObject, Vec<CompleteTypeObject>), String> {
    use zerodds_types::from_ast::{
        Form, FromAstError, complete_type_object, dependency_type_objects, type_identifier,
    };

    let mut builder = TypeObjectBuilder::struct_type(type_name).extensibility(Extensibility::Final);
    let mut dependencies = Vec::new();

    if members.is_empty() {
        builder = builder.member(
            DUMMY_MEMBER,
            TypeIdentifier::Primitive(PrimitiveKind::Byte),
            |mb| mb,
        );
    } else {
        for (name, type_id) in members {
            let ti = type_identifier(ast, *type_id, Form::Complete)
                .map_err(|e| format!("member `{name}`: {e:?}"))?;
            builder = builder.member(name.clone(), ti, |mb| mb);

            match complete_type_object(ast, *type_id) {
                Ok(object) => dependencies.push(object),
                Err(FromAstError::Unrepresentable { .. }) => {
                    // A plain (fully descriptive) type — primitive,
                    // string, sequence, array, map — has no
                    // `TypeObject` behind its `TypeIdentifier`, per
                    // XTypes 1.3 §7.3.4.6. Nothing to add.
                }
                Err(e) => return Err(format!("member `{name}`: {e:?}")),
            }
            for dep in dependency_type_objects(ast, *type_id)
                .map_err(|e| format!("member `{name}`: {e:?}"))?
            {
                dependencies.push(dep);
            }
        }
    }

    Ok((
        CompleteTypeObject::Struct(builder.build_complete()),
        dependencies,
    ))
}

/// Builds (if not already cached) a `CompleteTypeObject::Union` for
/// `type_name` — discriminator `long` (DDS-RPC 1.0 §7.5.1.1.6 rule 1:
/// `switch (long _d)`), one case per `(label, member_name,
/// ResolvedType)` in `cases`, plus `default_member` (named
/// `UNKNOWN_OP_MEMBER` for `Call`/`Return`, `None` for a `Result`
/// union, which DDS-RPC 1.0 §7.5.1.1.5 gives no default case at all) —
/// of type `octet` (`UnknownOperation`/`UnusedMember`, §7.5.1.1.1).
/// `@final`, same reason as [`build_synthetic_struct`]. The
/// dependencies are every case's own `CompleteTypeObject` plus its
/// transitive closure — each case's [`ResolvedType`] already carries
/// what it needs, recovered here from `types` by name (a synthetic
/// type is registered under its own name as soon as it is built, so
/// this is always a cache hit, never a rebuild).
fn resolve_synthetic_union(
    types: &TypeRegistry,
    type_name: &str,
    cases: &[(i32, String, ResolvedType)],
    default_member: Option<&str>,
) -> Result<ResolvedType, RpcDispatchError> {
    use zerodds_types::from_ast::{
        Form, complete_type_object, dependency_type_objects, type_identifier,
    };

    if let Some(r) = types.get(type_name) {
        return Ok(r);
    }
    let mut builder =
        TypeObjectBuilder::union_type(type_name, TypeIdentifier::Primitive(PrimitiveKind::Int32))
            .extensibility(Extensibility::Final);
    let mut dependencies = Vec::new();
    for (label, member_name, case_resolved) in cases {
        let case_ast = case_resolved.dynamic_type.ast();
        let case_id = case_resolved.dynamic_type.id();
        let case_type_id = type_identifier(case_ast, case_id, Form::Complete)
            .map_err(|e| RpcDispatchError::Model(format!("{member_name}: {e:?}")))?;
        builder = builder.case(member_name.clone(), case_type_id, vec![*label]);
        dependencies.push(
            complete_type_object(case_ast, case_id)
                .map_err(|e| RpcDispatchError::Model(format!("{member_name}: {e:?}")))?,
        );
        for dep in dependency_type_objects(case_ast, case_id)
            .map_err(|e| RpcDispatchError::Model(format!("{member_name}: {e:?}")))?
        {
            dependencies.push(dep);
        }
    }
    if let Some(default_member) = default_member {
        builder = builder.default_case(
            default_member,
            TypeIdentifier::Primitive(PrimitiveKind::Byte),
        );
    }
    let root = CompleteTypeObject::Union(builder.build_complete());
    types
        .resolve_synthetic(type_name, root, dependencies)
        .map_err(|e| RpcDispatchError::Model(e.to_string()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn idl_ast(idl: &str) -> Arc<Ast> {
        Arc::new(zerodds_wire_probe::encode::model_from_idl(idl, "t.idl").unwrap())
    }

    fn leased_participant() -> (zerodds_test_domains::DomainLease, DomainParticipant) {
        let lease = zerodds_test_domains::lease_domain();
        let participant = zerodds_dcps::DomainParticipantFactory::instance()
            .create_participant(lease.id(), zerodds_dcps::DomainParticipantQos::default())
            .expect("participant");
        (lease, participant)
    }

    #[test]
    fn finds_a_declared_operation_across_flattened_services() {
        let ast = idl_ast("interface Calculator { long add(in long a, in long b); };");
        let (_lease, participant) = leased_participant();
        let dispatcher = RpcDispatcher::new(ast, participant).expect("new");
        assert!(dispatcher.knows_operation("add"));
        assert!(!dispatcher.knows_operation("subtract"));
    }

    #[test]
    fn an_operation_with_no_params_gets_a_dummy_member() {
        let ast = idl_ast("interface Fyi { void ping(); };");
        let (root, _deps) = build_synthetic_struct(&ast, "Fyi_ping::In", &[]).expect("build");
        let CompleteTypeObject::Struct(s) = root else {
            panic!("expected struct")
        };
        assert_eq!(s.member_seq.len(), 1);
        assert_eq!(s.member_seq[0].detail.name, DUMMY_MEMBER);
    }

    #[test]
    fn every_synthetic_type_is_final_not_appendable() {
        let ast = idl_ast("interface Calculator { long add(in long a, in long b); };");
        let members = vec![("a".to_string(), {
            let svc = services(&ast).unwrap().into_iter().next().unwrap();
            let op = all_operations_flattened(&ast, svc.id)
                .unwrap()
                .into_iter()
                .next()
                .unwrap();
            op.params[0].type_
        })];
        let (root, _deps) = build_synthetic_struct(&ast, "Calculator_add::In", &members).unwrap();
        let CompleteTypeObject::Struct(s) = root else {
            panic!("expected struct")
        };
        use zerodds_types::type_object::flags::StructTypeFlag;
        assert!(!s.struct_flags.has(StructTypeFlag::IS_APPENDABLE));
        assert!(!s.struct_flags.has(StructTypeFlag::IS_MUTABLE));
    }

    #[test]
    fn builds_one_interface_wide_call_and_return_union() {
        let ast = idl_ast(
            "interface Calculator { long add(in long a, in long b); void log(in string msg); };",
        );
        let (_lease, participant) = leased_participant();
        let dispatcher = RpcDispatcher::new(Arc::clone(&ast), participant).expect("new");
        let service = services(&ast).unwrap().into_iter().next().unwrap();
        let model = dispatcher.model_for(&service).expect("model");

        // Both operations share the SAME Call/Return union type name —
        // one topic pair per interface, not one per operation.
        assert_eq!(model.call_resolved.dynamic_type.name(), "Calculator_Call");
        assert_eq!(model.call_resolved.dynamic_type.name(), "Calculator_Call");
        assert_eq!(model.operations.len(), 2);

        let add_disc = operation_hash("add");
        let log_disc = operation_hash("log");
        assert!(model.operations.contains_key(&add_disc));
        assert!(model.operations.contains_key(&log_disc));
        // Every operation has its Return-union case (§7.5.1.1.7 rule 5).
        assert_eq!(
            model.operations[&log_disc]
                .result_resolved
                .dynamic_type
                .name(),
            "Calculator_log_Result"
        );
        assert_eq!(
            model.operations[&add_disc]
                .result_resolved
                .dynamic_type
                .name(),
            "Calculator_add_Result"
        );
    }
}

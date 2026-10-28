// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Request/reply codegen data model — Spec §7.5.1.
//!
//! This stage (C6.1.B) derives from a [`ServiceDef`] the wire data models
//! of the request and reply topics. The spec distinguishes two layouts:
//!
//! * **Basic service** (Spec §7.5.1.1): _one_ request topic + _one_ reply
//!   topic per service type, both with an untyped discriminated union
//!   over all methods. Wire form:
//!
//!   ```text
//!   struct <Service>_Request {
//!       RequestHeader header;
//!       union <Service>_Call switch (long _d) {
//!           case OP_<m1>: <Service>_<m1>_In   m1_args;
//!           case OP_<m2>: <Service>_<m2>_In   m2_args;
//!           ...
//!       };
//!   };
//!   ```
//!
//!   `<Service>_Reply` analogously with `<Service>_<m>_Out` and `ReplyHeader`.
//!
//! * **Enhanced service** (Spec §7.5.1.2, §7.4.3): _one_ request topic
//!   and _one_ reply topic per interface, typed by `@Choice @Autoid`
//!   structures with one member per operation, the inherited operations
//!   flattened in. No header: the request is identified by its sample
//!   identity, the reply by `PID_RELATED_SAMPLE_IDENTITY` (§7.8.2).
//!
//!   ```text
//!   @Autoid struct <I>_<op>_In  { /* in/inout params */ };
//!   @Autoid struct <I>_<op>_Out { /* out/inout params, return_ */ };
//!   @Choice @Autoid struct <I>_<op>_Result { <I>_<op>_Out result; E e_ex; };
//!   @Choice @Autoid struct <I>_Request { <I>_<op>_In op; ... };
//!   @Choice @Autoid struct <I>_Reply   { <I>_<op>_Result op; ...;
//!                                        RemoteExceptionCode_t remoteEx; };
//!   ```
//!
//!   [`build_enhanced_service`] returns this model.
//!
//! We produce **data structures** here ([`RequestType`], [`ReplyType`],
//! [`EnhancedService`]) with member lists — no language codegen. The
//! language backends consume the model and emit bindings.
//!
//! Oneway methods appear in the basic request union but not in the
//! reply union — the reply topic is nonetheless shared.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::error::{RpcError, RpcResult};
use crate::service_mapping::{MethodDef, ParamDirection, RaisedException, ServiceDef, TypeRef};

/// Layout variant of the service wire model (Spec §7.5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceLayout {
    /// Spec §7.5.1.1 — one request/reply topic pair per service.
    Basic,
    /// Spec §7.5.1.2 — one topic pair per method.
    Enhanced,
}

/// Member of a generated wire structure.
#[derive(Debug, Clone, PartialEq)]
pub struct StructMember {
    /// Member name (e.g. `header`, `request_id`, `result`).
    pub name: String,
    /// Type reference or synthetic token (e.g. `RequestHeader`).
    pub type_ref: MemberType,
}

impl StructMember {
    /// Constructor.
    #[must_use]
    pub fn new(name: impl Into<String>, type_ref: MemberType) -> Self {
        Self {
            name: name.into(),
            type_ref,
        }
    }
}

/// Type of a struct member. Either one of the common types §7.5.1.1.1
/// declares, a synthesized union, or an IDL type referenced by handle
/// into the model.
#[derive(Debug, Clone, PartialEq)]
pub enum MemberType {
    /// `dds::rpc::RequestHeader` from `common_types.rs`.
    RequestHeader,
    /// `dds::rpc::ReplyHeader` from `common_types.rs`.
    ReplyHeader,
    /// `dds::rpc::UnusedMember` — §7.5.1.1.1 `typedef octet
    /// UnusedMember`. The type of the `dummy` member an `In` or an `Out`
    /// structure carries when it would otherwise be empty (§7.5.1.1.4
    /// rule 3c, §7.5.1.1.5 rule 3c). An empty structure is not the same
    /// thing: XCDR gives it no octets, and a reader cannot tell a sample
    /// that arrived from one that did not.
    UnusedMember,
    /// `dds::rpc::UnknownOperation` — §7.5.1.1.1 `typedef octet
    /// UnknownOperation`. The type of the `unknownOp` member on the
    /// `default` case of a `Call` or `Return` union (§7.5.1.1.6 rule 3).
    UnknownOperation,
    /// Synthetic union over all methods of a service — `${I}_Call` on
    /// the request side, `${I}_Return` on the reply side.
    CallUnion(CallUnionDef),
    /// The per-operation `${I}_${op}_Result` union of §7.5.1.1.5.
    ResultUnion(ResultUnionDef),
    /// A structure the mapping synthesizes, by name (Enhanced Service
    /// Mapping: `${I}_${op}_In`, `_Out`, `_Result`).
    Struct(String),
    /// `dds::rpc::RemoteExceptionCode_t`, the type of the `remoteEx`
    /// member of an Enhanced `Reply` (§7.5.1.2.5 rule 5). The clause
    /// spells the type `dds::rpc::RemoteException_t`, which no other
    /// clause defines; the enumeration of §7.5.2 is the only type that
    /// carries a remote exception code.
    RemoteException,
    /// An exception a `Result` structure carries (§7.5.1.2.3 rule 6).
    Exception(RaisedException),
    /// A normal IDL type.
    Idl(TypeRef),
}

/// One case of a `Call` or `Return` union. §7.5.1.1.6 rule 5,
/// §7.5.1.1.7 rule 5.
#[derive(Debug, Clone, PartialEq)]
pub struct CallUnionCase {
    /// Method name, which rule 5b also makes the member name.
    pub method: String,
    /// The case label. §7.5.1.1.6 rule 5a: "The integral value of the
    /// case label shall be the HASH of `${operationName}`" — the
    /// *unqualified* name, through the algorithm of §7.5.1.1.2. A
    /// `long`, so the 32 bits the hash produces are read as signed.
    pub discriminator: i32,
    /// The constant §7.5.1.1.6 rule 4 requires beside the union:
    /// `${interfaceName}_${operationName}_Hash`.
    pub hash_constant: String,
    /// `${I}_${op}_In` on the request side, `${I}_${op}_Result` on the
    /// reply side.
    pub case_type_name: String,
    /// The members of the `In` structure. Empty on the reply side,
    /// where the case type is a union and [`Self::result_union`] carries
    /// it.
    pub members: Vec<StructMember>,
    /// The `${I}_${op}_Result` union this case selects, on the reply
    /// side only.
    pub result_union: Option<ResultUnionDef>,
}

/// One exception case of a `Result` union. §7.5.1.1.5 rules 4 and 5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionCase {
    /// The exception's simple IDL name.
    pub exception: String,
    /// The constant rule 4a requires: `${exceptionType}_Ex_Hash`.
    pub hash_constant: String,
    /// Rule 4b: "the HASH of the fully qualified name of the exception
    /// type where module separator to be used is `::` (2 colons)".
    pub discriminator: i32,
    /// Rule 5b: "the lower-case version of the exception name with the
    /// suffix `_ex` added".
    pub member: String,
}

/// `${I}_${op}_Result` — §7.5.1.1.5.
///
/// A union over the outcome of one operation: the `dds::RETCODE_OK`
/// case carries the `Out` structure, and every exception the operation
/// raises carries its own.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultUnionDef {
    /// `${interfaceName}_${operationName}_Result`.
    pub name: String,
    /// Rule 3a: the member of the `dds::RETCODE_OK` case is named
    /// `result` and has type `${I}_${op}_Out`.
    pub out_type_name: String,
    /// The `Out` structure's members, so a consumer does not have to
    /// look it up separately.
    pub out_members: Vec<StructMember>,
    /// One case per exception in the `raises` clause, in declaration
    /// order.
    pub exceptions: Vec<ExceptionCase>,
}

/// `${I}_Call` (request) or `${I}_Return` (reply) union. §7.5.1.1.6,
/// §7.5.1.1.7.
#[derive(Debug, Clone, PartialEq)]
pub struct CallUnionDef {
    /// Complete union type name (e.g. `Calculator_Call`).
    pub name: String,
    /// §7.5.1.1.6 rule 3: "The Call union shall have a default case
    /// label containing a member named `unknownOp` of type
    /// `dds::rpc::UnknownOperation`." The name is fixed by the clause
    /// and carried rather than assumed, so a consumer reads it instead
    /// of spelling it again.
    pub default_member: String,
    /// Cases. The `Return` union contains only methods that are not
    /// `oneway`.
    pub cases: Vec<CallUnionCase>,
}

/// The member name §7.5.1.1.6 rule 3 gives the `default` case.
pub const UNKNOWN_OP_MEMBER: &str = "unknownOp";

/// The member name §7.5.1.1.4 rule 3c and §7.5.1.1.5 rule 3c give an
/// otherwise-empty `In` or `Out` structure.
pub const DUMMY_MEMBER: &str = "dummy";

/// The member name §7.5.1.1.5 rule 3b gives an operation's return
/// value.
///
/// `return_` and not `_return`: IDL has no `return` keyword to escape,
/// and the clause prints the trailing underscore in its own
/// non-normative example.
pub const RETURN_MEMBER: &str = "return_";

/// The member name §7.5.1.1.5 rule 3a gives the `dds::RETCODE_OK` case
/// of a `Result` union.
pub const RESULT_MEMBER: &str = "result";

/// The label of that case. DDS-DCPS gives `RETCODE_OK` the value 0.
pub const RETCODE_OK: i32 = 0;

/// Request wire data model.
///
/// * Basic layout: one RequestType per service, with `header` +
///   `union` member over all methods.
/// * Enhanced layout: one RequestType per method, with `header` +
///   typed per-method fields.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestType {
    /// Complete struct name (e.g. `Calculator_Request` or
    /// `Calculator_add_Request`).
    pub name: String,
    /// Topic name of this request wire structure.
    pub topic_name: String,
    /// Layout variant from which the struct was produced.
    pub layout: ServiceLayout,
    /// Method name for enhanced; `None` for basic.
    pub method: Option<String>,
    /// Member list (header + body).
    pub members: Vec<StructMember>,
}

/// Reply wire data model. Analogous to [`RequestType`].
#[derive(Debug, Clone, PartialEq)]
pub struct ReplyType {
    /// Complete struct name.
    pub name: String,
    /// Topic name.
    pub topic_name: String,
    /// Layout variant.
    pub layout: ServiceLayout,
    /// Method name for enhanced; `None` for basic.
    pub method: Option<String>,
    /// Member list.
    pub members: Vec<StructMember>,
}

// ---------------------------------------------------------------------
// Basic service mapping (Spec §7.5.1.1)
// ---------------------------------------------------------------------

/// Returns the data models of the basic-layout topics
/// (`<Service>_Request` plus `<Service>_Reply`). An empty `ServiceDef`
/// returns a pair with empty unions — the caller must decide for itself
/// whether to discard it.
///
/// # Errors
/// `RpcError::InvalidServiceName` if the service name is empty (this
/// happens only for manually constructed ServiceDefs — `lower_service`
/// already validates).
pub fn build_basic_pair(svc: &ServiceDef) -> RpcResult<(RequestType, ReplyType)> {
    if svc.name.is_empty() {
        return Err(RpcError::InvalidServiceName(String::new()));
    }
    let topics = svc.topic_names()?;
    let req_union = build_call_union(svc, /*include_oneway=*/ true, /*reply=*/ false)?;
    let rep_union = build_call_union(svc, /*include_oneway=*/ false, /*reply=*/ true)?;

    let request = RequestType {
        name: format!("{}_Request", svc.name),
        topic_name: topics.request.clone(),
        layout: ServiceLayout::Basic,
        method: None,
        // §7.5.1.1.6 rule 2: "The first member of the Request structure
        // shall be named `header` […] The second member […] shall be
        // named `data` and be of type ${interfaceName}_Call."
        members: alloc::vec![
            StructMember::new("header", MemberType::RequestHeader),
            StructMember::new("data", MemberType::CallUnion(req_union)),
        ],
    };

    let reply = ReplyType {
        name: format!("{}_Reply", svc.name),
        topic_name: topics.reply,
        layout: ServiceLayout::Basic,
        method: None,
        // §7.5.1.1.7 rule 3: "The first member of the Reply structure
        // shall be named `header` […] The second member of the Reply
        // type shall be named `data` and be of type
        // ${interfaceName}_Return."
        members: alloc::vec![
            StructMember::new("header", MemberType::ReplyHeader),
            StructMember::new("data", MemberType::CallUnion(rep_union)),
        ],
    };

    Ok((request, reply))
}

fn build_call_union(
    svc: &ServiceDef,
    include_oneway: bool,
    reply: bool,
) -> RpcResult<CallUnionDef> {
    // §7.5.1.1.6 rule 1 and §7.5.1.1.7 rule 1: `${I}_Call` on the
    // request side, `${I}_Return` on the reply side. The reply union is
    // not `${I}_Result` — that name belongs to the *per-operation* union
    // of §7.5.1.1.5, and using it twice put two different shapes under
    // one name.
    let union_name = if reply {
        format!("{}_Return", svc.name)
    } else {
        format!("{}_Call", svc.name)
    };
    let mut cases = Vec::with_capacity(svc.methods.len());
    for m in &svc.all_methods() {
        if m.oneway && !include_oneway {
            continue;
        }
        // §7.5.1.1.6 rule 4: the constant beside the union, and rule 5a:
        // the label is the HASH of the unqualified operation name. The
        // hash is 32 bits and the discriminator is a `long`, so the same
        // bits are read signed.
        let hash = crate::rpc_hash::rpc_member_hash(&m.name) as i32;
        let (case_type, members, result_union) = if reply {
            let out_members = method_out_members(m);
            let result = build_result_union(svc, m, out_members.clone());
            (result.name.clone(), Vec::new(), Some(result))
        } else {
            (
                format!("{}_{}_In", svc.name, m.name),
                method_in_members(m),
                None,
            )
        };
        cases.push(CallUnionCase {
            hash_constant: format!("{}_{}_Hash", svc.name, m.name),
            method: m.name.clone(),
            discriminator: hash,
            case_type_name: case_type,
            members,
            result_union,
        });
    }
    Ok(CallUnionDef {
        name: union_name,
        default_member: UNKNOWN_OP_MEMBER.to_string(),
        cases,
    })
}

/// `${I}_${op}_Result` — §7.5.1.1.5.
fn build_result_union(
    svc: &ServiceDef,
    m: &MethodDef,
    out_members: Vec<StructMember>,
) -> ResultUnionDef {
    let exceptions = m
        .raises
        .iter()
        .map(|raised| ExceptionCase {
            // Rule 4b: the hash is of the *fully qualified* name with
            // `::` separators, unlike the operation hash of §7.5.1.1.6,
            // which is of the unqualified one.
            discriminator: crate::rpc_hash::rpc_member_hash(&raised.scoped_name) as i32,
            hash_constant: format!("{}_Ex_Hash", raised.name),
            // Rule 5b: "the lower-case version of the exception name
            // with the suffix `_ex` added".
            member: format!("{}_ex", raised.name.to_lowercase()),
            exception: raised.name.clone(),
        })
        .collect();
    ResultUnionDef {
        name: format!("{}_{}_Result", svc.name, m.name),
        out_type_name: format!("{}_{}_Out", svc.name, m.name),
        out_members,
        exceptions,
    }
}

// ---------------------------------------------------------------------
// Enhanced Service Mapping (Spec §7.5.1.2)
// ---------------------------------------------------------------------

/// How an operation without parameters, return value or `out` arguments
/// is mapped in the Enhanced Service Mapping.
///
/// §7.5.1.2.1.3 introduces `@Empty` "to capture operations that do not
/// accept any parameters" and lets such a structure be empty, while the
/// rules §7.5.1.2.2 and §7.5.1.2.3 take over §7.5.1.1.4 rule 3c and
/// §7.5.1.1.5 rule 3c, which give the structure a `dummy` member of type
/// `UnusedMember`, and the non-normative example of §7.5.1.2.6 shows a
/// `dummy` member. The default follows the normative rules and the
/// example.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmptyOperationStyle {
    /// The structure carries one `dummy` member (rule 3c). The default.
    #[default]
    DummyMember,
    /// The structure is empty and carries `@Empty` (§7.5.1.2.1.3).
    EmptyAnnotation,
}

/// Settings of [`build_enhanced_service_with`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnhancedOptions {
    /// See [`EmptyOperationStyle`].
    pub empty_operations: EmptyOperationStyle,
}

/// Member of a structure of the Enhanced Service Mapping.
#[derive(Debug, Clone, PartialEq)]
pub struct EnhancedMember {
    /// Member name.
    pub name: String,
    /// Member type.
    pub type_ref: MemberType,
    /// The member id `@Autoid` assigns (§7.5.1.2.1.2): the HASH of the
    /// member name, 28 bits of it because that is what a member id holds
    /// (XTypes 1.3 §7.4.3.4.2 EMHEADER). `None` in a structure that is
    /// not `@Autoid`.
    pub member_id: Option<u32>,
}

/// Structure synthesized by the Enhanced Service Mapping.
#[derive(Debug, Clone, PartialEq)]
pub struct EnhancedStruct {
    /// Structure name.
    pub name: String,
    /// `@Choice` (§7.5.1.2.1.1): the structure is read as `@mutable`
    /// with every member `@optional`, and exactly one member is present.
    pub choice: bool,
    /// `@Autoid` (§7.5.1.2.1.2).
    pub autoid: bool,
    /// `@Empty` (§7.5.1.2.1.3): the structure has no member.
    pub empty: bool,
    /// `@Extensibility(MUTABLE_EXTENSIBILITY)`: the operation carries
    /// `@Mutable` (§7.5.1.2.1.4, §7.5.1.2.2, §7.5.1.2.3).
    pub mutable: bool,
    /// Members.
    pub members: Vec<EnhancedMember>,
}

/// The three structures one operation maps to.
#[derive(Debug, Clone, PartialEq)]
pub struct EnhancedOperation {
    /// The interface that declares the operation: `${interfaceName}` of
    /// its structure names. A derived interface keeps the base's name
    /// for an inherited operation (§7.5.1.2.6).
    pub interface: String,
    /// Operation name: the member name in the Request and Reply
    /// structures.
    pub name: String,
    /// `${I}_${op}_In`, §7.5.1.2.2.
    pub in_struct: EnhancedStruct,
    /// `${I}_${op}_Out`, §7.5.1.2.3 rule 1.
    pub out_struct: EnhancedStruct,
    /// `${I}_${op}_Result`, §7.5.1.2.3 rules 2 to 7.
    pub result_struct: EnhancedStruct,
}

/// The wire model of an interface under the Enhanced Service Mapping.
///
/// §7.4.3: one request topic and one reply topic per interface, each
/// carrying a `@Choice` structure over all the operations, the inherited
/// ones included (§7.5.1.2.6). Neither structure has a header: the
/// request is identified by the sample identity of its DATA, the reply
/// by `PID_RELATED_SAMPLE_IDENTITY` (§7.8.2), and the instance name
/// travels in discovery (§7.6.2), not in the sample.
#[derive(Debug, Clone, PartialEq)]
pub struct EnhancedService {
    /// Request topic name (§7.4.1).
    pub request_topic: String,
    /// Reply topic name (§7.4.1).
    pub reply_topic: String,
    /// One entry per operation: the inherited ones, base by base, and then
    /// the interface's own (the order of the example of §7.5.1.2.6).
    pub operations: Vec<EnhancedOperation>,
    /// `${interfaceName}_Request`, §7.5.1.2.4.
    pub request: EnhancedStruct,
    /// `${interfaceName}_Reply`, §7.5.1.2.5.
    pub reply: EnhancedStruct,
}

/// The member name §7.5.1.2.5 rule 5 gives the remote exception code of
/// a `Reply` structure.
pub const REMOTE_EX_MEMBER: &str = "remoteEx";

/// The mask that turns the 32-bit HASH of §7.5.1.1.2 into a member id.
const MEMBER_ID_MASK: u32 = 0x0FFF_FFFF;

/// Builds the Enhanced Service Mapping of `svc` with the default
/// settings.
///
/// # Errors
/// See [`build_enhanced_service_with`].
pub fn build_enhanced_service(svc: &ServiceDef) -> RpcResult<EnhancedService> {
    build_enhanced_service_with(svc, EnhancedOptions::default())
}

/// Builds the Enhanced Service Mapping of `svc` (§7.5.1.2).
///
/// # Errors
/// * `RpcError::InvalidServiceName` for an empty service name.
/// * `RpcError::InvalidMethodName` for an empty operation name.
/// * `RpcError::DuplicateMethod` when two operations of the flattened
///   interface share a name.
/// * `RpcError::MemberIdCollision` when two member names of an
///   `@Autoid` structure hash to one id (§7.5.1.2.1.2).
pub fn build_enhanced_service_with(
    svc: &ServiceDef,
    options: EnhancedOptions,
) -> RpcResult<EnhancedService> {
    if svc.name.is_empty() {
        return Err(RpcError::InvalidServiceName(String::new()));
    }
    // §7.4.3: "The Enhanced Service Mapping shall map an interface to
    // exactly one request topic and a reply topic."
    let topics = svc.topic_names()?;

    // §7.5.1.2.6: "A derived interface is interpreted as if all the
    // inherited operations are declared in-place in the derived
    // interface" — the order of the example, the bases' operations in
    // declaration order of the bases and then the interface's own.
    let mut sources: Vec<(String, MethodDef)> = Vec::new();
    for base in &svc.inherited {
        for method in &base.methods {
            sources.push((base.interface.clone(), method.clone()));
        }
    }
    for method in svc.all_methods() {
        sources.push((svc.name.clone(), method));
    }

    let mut operations = Vec::with_capacity(sources.len());
    for (interface, method) in &sources {
        if method.name.is_empty() {
            return Err(RpcError::InvalidMethodName(String::new()));
        }
        if operations
            .iter()
            .any(|op: &EnhancedOperation| op.name == method.name)
        {
            return Err(RpcError::DuplicateMethod(method.name.clone()));
        }
        operations.push(build_enhanced_operation(interface, method, options)?);
    }

    // §7.5.1.2.4: one member per operation, typed `…_In`.
    let request_members: Vec<(String, MemberType)> = operations
        .iter()
        .map(|op| {
            (
                op.name.clone(),
                MemberType::Struct(op.in_struct.name.clone()),
            )
        })
        .collect();
    let request = autoid_choice(format!("{}_Request", svc.name), false, request_members)?;

    // §7.5.1.2.5: one member per operation, typed `…_Result`, and
    // `remoteEx`.
    let mut reply_members: Vec<(String, MemberType)> = operations
        .iter()
        .map(|op| {
            (
                op.name.clone(),
                MemberType::Struct(op.result_struct.name.clone()),
            )
        })
        .collect();
    let remote_ex = remote_ex_member_name(operations.iter().map(|op| op.name.as_str()));
    reply_members.push((remote_ex, MemberType::RemoteException));
    let reply = autoid_choice(format!("{}_Reply", svc.name), false, reply_members)?;

    Ok(EnhancedService {
        request_topic: topics.request,
        reply_topic: topics.reply,
        operations,
        request,
        reply,
    })
}

/// `remoteEx`, or `remoteEx_N` with N "the first integer in the range 1
/// to 2^31 that avoids the collision" when an operation has that name
/// (§7.5.1.2.5 rule 5, case-sensitive).
pub fn remote_ex_member_name<'a>(operations: impl Iterator<Item = &'a str> + Clone) -> String {
    let taken = |candidate: &str| operations.clone().any(|name| name == candidate);
    if !taken(REMOTE_EX_MEMBER) {
        return REMOTE_EX_MEMBER.to_string();
    }
    for n in 1..=i32::MAX {
        let candidate = format!("{REMOTE_EX_MEMBER}_{n}");
        if !taken(&candidate) {
            return candidate;
        }
    }
    // As for `return_N`: no model holds 2^31 operations.
    format!("{REMOTE_EX_MEMBER} ")
}

fn build_enhanced_operation(
    interface: &str,
    method: &MethodDef,
    options: EnhancedOptions,
) -> RpcResult<EnhancedOperation> {
    let empty = options.empty_operations == EmptyOperationStyle::EmptyAnnotation;
    // §7.5.1.2.2 / §7.5.1.2.3 rule 1: the rules of §7.5.1.1.4 and
    // §7.5.1.1.5 plus `@Mutable`.
    let in_members = if empty {
        method_in_members_or_none(method)
    } else {
        method_in_members(method)
    };
    let out_members = if empty {
        method_out_members_or_none(method)
    } else {
        method_out_members(method)
    };
    let in_struct = autoid_struct(
        format!("{interface}_{}_In", method.name),
        method.mutable,
        in_members,
    )?;
    let out_struct = autoid_struct(
        format!("{interface}_{}_Out", method.name),
        method.mutable,
        out_members,
    )?;

    // §7.5.1.2.3 rules 2 to 7.
    let mut result_members = alloc::vec![(
        RESULT_MEMBER.to_string(),
        MemberType::Struct(out_struct.name.clone()),
    )];
    for raised in &method.raises {
        result_members.push((
            format!("{}_ex", raised.name.to_lowercase()),
            MemberType::Exception(raised.clone()),
        ));
    }
    let result_struct = autoid_choice(
        format!("{interface}_{}_Result", method.name),
        method.mutable,
        result_members,
    )?;

    Ok(EnhancedOperation {
        interface: interface.to_string(),
        name: method.name.clone(),
        in_struct,
        out_struct,
        result_struct,
    })
}

/// `@Autoid` structure over `members`.
fn autoid_struct(
    name: String,
    mutable: bool,
    members: Vec<StructMember>,
) -> RpcResult<EnhancedStruct> {
    let named: Vec<(String, MemberType)> =
        members.into_iter().map(|m| (m.name, m.type_ref)).collect();
    let empty = named.is_empty();
    let mut built = autoid(name, false, mutable, named)?;
    built.empty = empty;
    Ok(built)
}

/// `@Choice @Autoid` structure over `members`.
fn autoid_choice(
    name: String,
    mutable: bool,
    members: Vec<(String, MemberType)>,
) -> RpcResult<EnhancedStruct> {
    autoid(name, true, mutable, members)
}

fn autoid(
    name: String,
    choice: bool,
    mutable: bool,
    members: Vec<(String, MemberType)>,
) -> RpcResult<EnhancedStruct> {
    let mut built: Vec<EnhancedMember> = Vec::with_capacity(members.len());
    for (member_name, type_ref) in members {
        let id = crate::rpc_hash::rpc_member_hash(&member_name) & MEMBER_ID_MASK;
        if let Some(clash) = built.iter().find(|m| m.member_id == Some(id)) {
            return Err(RpcError::MemberIdCollision {
                structure: name,
                first: clash.name.clone(),
                second: member_name,
                id,
            });
        }
        built.push(EnhancedMember {
            name: member_name,
            type_ref,
            member_id: Some(id),
        });
    }
    Ok(EnhancedStruct {
        name,
        choice,
        autoid: true,
        empty: false,
        mutable,
        members: built,
    })
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

/// The members of `${I}_${op}_In` — §7.5.1.1.4 rule 3.
fn method_in_members(m: &MethodDef) -> Vec<StructMember> {
    let out: Vec<StructMember> = m
        .params
        .iter()
        .filter(|p| p.direction.is_in())
        .map(|p| StructMember::new(p.name.clone(), MemberType::Idl(p.type_ref.clone())))
        .collect();
    // Rule 3c: "If an operation has no in and inout parameters the
    // resulting structure shall contain a single member of type
    // UnusedMember and name `dummy`."
    if out.is_empty() {
        return alloc::vec![StructMember::new(DUMMY_MEMBER, MemberType::UnusedMember)];
    }
    out
}

/// The members of the `In` structure without the `dummy` substitute, for
/// [`EmptyOperationStyle::EmptyAnnotation`].
fn method_in_members_or_none(m: &MethodDef) -> Vec<StructMember> {
    m.params
        .iter()
        .filter(|p| p.direction.is_in())
        .map(|p| StructMember::new(p.name.clone(), MemberType::Idl(p.type_ref.clone())))
        .collect()
}

/// The members of the `Out` structure without the `dummy` substitute.
fn method_out_members_or_none(m: &MethodDef) -> Vec<StructMember> {
    let members = method_out_members(m);
    if members.len() == 1 && members[0].name == DUMMY_MEMBER {
        return Vec::new();
    }
    members
}

/// The members of `${I}_${op}_Out` — §7.5.1.1.5 rule 3.
///
/// The `out` and `inout` parameters first, in declaration order and
/// under their formal names, then the return value *last* — rule 3b:
/// "If the operation defines a non-void return value the Out structure
/// shall have its last member named `return_`."
fn method_out_members(m: &MethodDef) -> Vec<StructMember> {
    let mut out = Vec::new();
    for p in m.params.iter().filter(|p| p.direction.is_out()) {
        // `out` and `inout` both land in the reply.
        let _ = ParamDirection::Out;
        out.push(StructMember::new(
            p.name.clone(),
            MemberType::Idl(p.type_ref.clone()),
        ));
    }
    if let Some(ret) = &m.return_type {
        out.push(StructMember::new(
            return_member_name(&out),
            MemberType::Idl(ret.clone()),
        ));
    }
    // Rule 3c: an operation with no return and no `out`/`inout`
    // parameters carries the `dummy` member instead of nothing.
    if out.is_empty() {
        return alloc::vec![StructMember::new(DUMMY_MEMBER, MemberType::UnusedMember)];
    }
    out
}

/// The name the return value takes in an `Out` structure. §7.5.1.1.5
/// rule 3b.
///
/// "If an operation has an out argument named `return_` and the
/// operation has a regular return value, the regular return value in
/// the Out structure shall be represented as `return_N` member of the
/// appropriate type where N is the first integer in the range 1 to
/// 2^31 that avoids the collision."
fn return_member_name(taken: &[StructMember]) -> String {
    let collides = |name: &str| taken.iter().any(|member| member.name == name);
    if !collides(RETURN_MEMBER) {
        return RETURN_MEMBER.to_string();
    }
    for n in 1..=i32::MAX {
        let candidate = format!("{RETURN_MEMBER}{n}");
        if !collides(&candidate) {
            return candidate;
        }
    }
    // Unreachable with fewer than 2^31 parameters, and IDL 4.2 puts no
    // such operation in any model this tree can hold. Returning the
    // colliding name would produce two members of one name, so the
    // last resort is a name no parameter can have: an IDL identifier
    // cannot contain a space (§7.2.3).
    format!("{RETURN_MEMBER} ")
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::*;
    use crate::service_mapping::lower_service;
    use zerodds_ast::Ast;

    /// Parse, lower, resolve — the frontend the service model comes
    /// through. The hand-built `InterfaceDef` values this module used to
    /// carry agreed with nothing but themselves.
    fn model(idl: &str) -> Ast {
        let parsed = zerodds_idl_parse::parse(idl);
        assert!(
            parsed.diagnostics().iter().all(|d| !d.is_error()),
            "the source does not parse: {:?}",
            parsed.diagnostics()
        );
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "rpc_codegen.idl");
        assert!(
            lowered.diagnostics.iter().all(|d| !d.is_error()),
            "the source does not lower: {:?}",
            lowered.diagnostics
        );
        let mut ast = lowered.ast;
        let report = ast.resolve_all_by_name().expect("a resolve pass");
        assert!(
            report.broken.is_empty(),
            "every name must resolve: {:?}",
            report.broken
        );
        ast
    }

    /// The one interface the source declares, lowered.
    fn service_of(idl: &str) -> ServiceDef {
        let ast = model(idl);
        let (id, _) = ast.interfaces().next().expect("one interface");
        lower_service(&ast, id).expect("a service")
    }

    fn calc_service() -> ServiceDef {
        service_of(
            "@service interface Calculator {
                 long add(in long a, in long b);
                 void log(in string msg);
             };",
        )
    }

    /// A service with one `oneway` method, built through the model API.
    ///
    /// The IDL frontend refuses `oneway` (§7.3.1.1 production (87)
    /// removed it), but the code generator still maps a `oneway` method
    /// for a service that reaches it another way — a Java definition of
    /// §7.3.2, or a model built by hand — so the exclusion of its reply
    /// is still exercised here.
    fn oneway_service() -> ServiceDef {
        ServiceDef::new(
            "Calculator",
            alloc::vec![
                MethodDef {
                    name: "add".to_string(),
                    params: Vec::new(),
                    return_type: None,
                    oneway: false,
                    raises: Vec::new(),
                    mutable: false,
                },
                MethodDef {
                    name: "log".to_string(),
                    params: Vec::new(),
                    return_type: None,
                    oneway: true,
                    raises: Vec::new(),
                    mutable: false,
                },
            ],
        )
    }

    /// §7.4.1: the topic carries `<interface_name>_<service_name>_`,
    /// and `<service_name>` defaults to `"Service"`. The *type* name is
    /// §7.5.1.1.6's `${interfaceName}_Request` and is a different string.
    #[test]
    fn basic_pair_topic_names() {
        let svc = calc_service();
        let (req, rep) = build_basic_pair(&svc).unwrap();
        assert_eq!(req.topic_name, "Calculator_Service_Request");
        assert_eq!(rep.topic_name, "Calculator_Service_Reply");
        assert_eq!(req.name, "Calculator_Request");
        assert_eq!(rep.name, "Calculator_Reply");
    }

    #[test]
    fn basic_pair_layout_marker() {
        let svc = calc_service();
        let (req, rep) = build_basic_pair(&svc).unwrap();
        assert_eq!(req.layout, ServiceLayout::Basic);
        assert_eq!(rep.layout, ServiceLayout::Basic);
        assert_eq!(req.method, None);
        assert_eq!(rep.method, None);
    }

    /// §7.5.1.1.6 rule 2: the Request structure's two members are
    /// `header` and `data`, and `data` is the `${I}_Call` union.
    #[test]
    fn basic_request_has_header_and_call_union() {
        let svc = calc_service();
        let (req, _) = build_basic_pair(&svc).unwrap();
        assert_eq!(req.members.len(), 2);
        assert_eq!(req.members[0].name, "header");
        assert!(matches!(req.members[0].type_ref, MemberType::RequestHeader));
        assert_eq!(req.members[1].name, "data");
        let call_union = match &req.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => panic!("expected CallUnion"),
        };
        assert_eq!(call_union.name, "Calculator_Call");
        // Both methods must be in the request union.
        assert_eq!(call_union.cases.len(), 2);
        assert_eq!(call_union.cases[0].method, "add");
        assert_eq!(call_union.cases[0].case_type_name, "Calculator_add_In");
        assert_eq!(call_union.cases[1].method, "log");
    }

    /// §7.5.1.1.6 rules 4 and 5a: a constant
    /// `${I}_${op}_Hash` beside the union, and the case label is the
    /// HASH of the *unqualified* operation name.
    #[test]
    fn the_case_labels_are_the_operation_hashes() {
        let svc = calc_service();
        let (req, _) = build_basic_pair(&svc).unwrap();
        let call_union = match &req.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        for (case, name) in call_union.cases.iter().zip(["add", "log"]) {
            assert_eq!(case.hash_constant, alloc::format!("Calculator_{name}_Hash"));
            assert_eq!(
                case.discriminator,
                crate::rpc_hash::rpc_member_hash(name) as i32,
                "{name}"
            );
        }
        // Two operations, two different labels — the ordinals the
        // previous version used collided with nothing and matched
        // nothing either.
        assert_ne!(
            call_union.cases[0].discriminator,
            call_union.cases[1].discriminator
        );
    }

    /// §7.5.1.1.6 rule 3: "The Call union shall have a default case
    /// label containing a member named `unknownOp` of type
    /// `dds::rpc::UnknownOperation`." §7.5.1.1.7 rule 3 says the same
    /// for the Return union.
    /// §7.5.1.1.2's pseudo-implementation, run by hand against a
    /// published MD5 digest.
    ///
    /// The label is what a peer switches on, so it is held against an
    /// external value rather than against our own hash function:
    /// `md5("add") = 34ec78fcc91ffb1e54cd85e4a0924332`, and the clause
    /// reads the first four octets as
    /// `md5[0] + 256*md5[1] + 256*256*md5[2] + 256*256*256*md5[3]`
    /// into a `long`.
    #[test]
    fn the_case_label_is_the_published_hash_arithmetic() {
        let digest = [0x34u32, 0xec, 0x78, 0xfc];
        let expected = digest[0]
            .wrapping_add(256 * digest[1])
            .wrapping_add(256 * 256 * digest[2])
            .wrapping_add(256 * 256 * 256 * digest[3]) as i32;
        assert_eq!(expected, -59_184_076);

        let svc = calc_service();
        let (req, _) = build_basic_pair(&svc).unwrap();
        let call_union = match &req.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        let add = call_union
            .cases
            .iter()
            .find(|case| case.method == "add")
            .expect("the `add` case");
        assert_eq!(add.discriminator, expected);
    }

    /// §7.5.1.1.5 rule 4b hashes the *fully qualified* exception name
    /// with `::` separators — `md5("robot::Overflow") =
    /// 826a1e948267ea1e29a034f820c8e6e9`.
    #[test]
    fn the_exception_label_hashes_the_scoped_name() {
        let digest = [0x82u32, 0x6a, 0x1e, 0x94];
        let expected = digest[0]
            .wrapping_add(256 * digest[1])
            .wrapping_add(256 * 256 * digest[2])
            .wrapping_add(256 * 256 * 256 * digest[3]) as i32;

        let svc = service_of(
            "module robot {
               exception Overflow { long code; };
               interface Calculator { long add(in long a) raises (Overflow); };
             };",
        );
        let (_, rep) = build_basic_pair(&svc).unwrap();
        let return_union = match &rep.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        let raised = &return_union.cases[0]
            .result_union
            .as_ref()
            .expect("a Result union")
            .exceptions[0];
        assert_eq!(raised.discriminator, expected);
    }

    #[test]
    fn both_unions_carry_the_default_unknown_operation_case() {
        let svc = calc_service();
        let (req, rep) = build_basic_pair(&svc).unwrap();
        for member in [&req.members[1], &rep.members[1]] {
            let union = match &member.type_ref {
                MemberType::CallUnion(u) => u,
                _ => unreachable!(),
            };
            assert_eq!(union.default_member, "unknownOp");
        }
    }

    /// §7.5.1.1.7 rule 1: the reply union is `${I}_Return`, and rule 5b
    /// makes its case type the per-operation `${I}_${op}_Result` union
    /// of §7.5.1.1.5 — not the `Out` structure directly.
    #[test]
    fn basic_reply_excludes_oneway_methods() {
        let svc = oneway_service();
        let (_, rep) = build_basic_pair(&svc).unwrap();
        let return_union = match &rep.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => panic!("expected CallUnion"),
        };
        assert_eq!(rep.members[1].name, "data");
        assert_eq!(return_union.name, "Calculator_Return");
        // `log` is oneway → not in the reply.
        assert_eq!(return_union.cases.len(), 1);
        assert_eq!(return_union.cases[0].method, "add");
        assert_eq!(
            return_union.cases[0].case_type_name,
            "Calculator_add_Result"
        );
    }

    /// §7.5.1.1.5: the `Result` union carries the `Out` structure under
    /// `result` on the `dds::RETCODE_OK` case, and one case per raised
    /// exception with the label and member name rules 4 and 5 give.
    #[test]
    fn the_result_union_of_seven_five_one_one_five_is_built() {
        let svc = service_of(
            "module robot {
               exception TooFast { long code; };
               interface Calculator {
                 long add(in long a) raises (TooFast);
               };
             };",
        );
        let (_, rep) = build_basic_pair(&svc).unwrap();
        let return_union = match &rep.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        let result = return_union.cases[0]
            .result_union
            .as_ref()
            .expect("the reply case selects a Result union");
        assert_eq!(result.name, "Calculator_add_Result");
        assert_eq!(result.out_type_name, "Calculator_add_Out");
        assert_eq!(RESULT_MEMBER, "result");
        assert_eq!(RETCODE_OK, 0);

        assert_eq!(result.exceptions.len(), 1);
        let raised = &result.exceptions[0];
        assert_eq!(raised.exception, "TooFast");
        assert_eq!(raised.hash_constant, "TooFast_Ex_Hash");
        // Rule 5b: lower-case name plus `_ex`.
        assert_eq!(raised.member, "toofast_ex");
        // Rule 4b: the hash is of the *fully qualified* name with `::`,
        // which is a different value from the unqualified one.
        assert_eq!(
            raised.discriminator,
            crate::rpc_hash::rpc_member_hash("robot::TooFast") as i32
        );
        assert_ne!(
            raised.discriminator,
            crate::rpc_hash::rpc_member_hash("TooFast") as i32
        );
    }

    /// §7.5.1.1.4 rule 3c and §7.5.1.1.5 rule 3c: an `In` or `Out` with
    /// nothing to carry takes a single `dummy` member of type
    /// `dds::rpc::UnusedMember`, because an empty structure is no
    /// octets and a reader cannot tell one that arrived from one that
    /// did not.
    #[test]
    fn an_empty_in_or_out_carries_the_dummy_member() {
        let svc = service_of("interface Ping { void ping(); };");
        let (req, rep) = build_basic_pair(&svc).unwrap();
        let call_union = match &req.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        assert_eq!(call_union.cases[0].members.len(), 1);
        assert_eq!(call_union.cases[0].members[0].name, "dummy");
        assert!(matches!(
            call_union.cases[0].members[0].type_ref,
            MemberType::UnusedMember
        ));

        let return_union = match &rep.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        let out = &return_union.cases[0]
            .result_union
            .as_ref()
            .expect("a Result union")
            .out_members;
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "dummy");
        assert!(matches!(out[0].type_ref, MemberType::UnusedMember));
    }

    /// §7.5.1.1.5 rule 3b's collision rule: an `out` parameter already
    /// named `return_` pushes the regular return value to `return_1`.
    #[test]
    fn a_colliding_return_member_takes_the_first_free_number() {
        let svc = service_of("interface Odd { long f(out long return_); };");
        let (_, rep) = build_basic_pair(&svc).unwrap();
        let return_union = match &rep.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        let out = &return_union.cases[0]
            .result_union
            .as_ref()
            .expect("a Result union")
            .out_members;
        assert_eq!(out[0].name, "return_", "the out parameter keeps its name");
        assert_eq!(out[1].name, "return_1", "the return value moves");
    }

    /// §7.5.1.1.5 rule 3a.i: "The members shall appear in the same order
    /// as the parameters in the operation, starting from the left." The
    /// `in` parameter `a` is not an Out member; `inout b` and `out c` are,
    /// in that order, and a non-void return value follows them.
    #[test]
    fn basic_out_members_keep_the_order_of_the_parameters() {
        let svc = service_of(
            "interface Mixer { void mix(in long a, inout long b, out double c); \
             long mix2(out double c, in long a, inout long b); };",
        );
        let (_, rep) = build_basic_pair(&svc).unwrap();
        let return_union = match &rep.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        let names = |case: usize| -> Vec<String> {
            return_union.cases[case]
                .result_union
                .as_ref()
                .expect("a Result union")
                .out_members
                .iter()
                .map(|m| m.name.clone())
                .collect()
        };
        assert_eq!(names(0), ["b", "c"], "a is `in` only and stays out");
        assert_eq!(names(1), ["c", "b", "return_"]);
    }

    #[test]
    fn basic_request_in_params_become_case_members() {
        let svc = calc_service();
        let (req, _) = build_basic_pair(&svc).unwrap();
        let call_union = match &req.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        let add_case = &call_union.cases[0];
        assert_eq!(add_case.members.len(), 2);
        assert_eq!(add_case.members[0].name, "a");
        assert_eq!(add_case.members[1].name, "b");
    }

    /// §7.5.1.1.5 rule 3b: "If the operation defines a non-void return
    /// value the Out structure shall have its **last** member named
    /// `return_`."
    #[test]
    fn basic_reply_return_value_is_the_last_member() {
        let svc = service_of("interface Cursor { long advance(out long wrapped); };");
        let (_, rep) = build_basic_pair(&svc).unwrap();
        let return_union = match &rep.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        let out = &return_union.cases[0]
            .result_union
            .as_ref()
            .expect("a Result union")
            .out_members;
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "wrapped", "the out parameter comes first");
        assert_eq!(out[1].name, "return_", "the return value comes last");
    }

    /// The hierarchy of §7.5.1.2.6, with an exception and a `@Mutable`
    /// operation added.
    fn calculator_hierarchy() -> ServiceDef {
        let ast = model(
            "module robot {
                 exception TooFast { long limit; };
                 @service interface Adder { long add(in long a, in long b); };
                 @service interface Subtractor { long sub(in long a, in long b); };
                 @service interface Calculator : Adder, Subtractor {
                     void on();
                     void off();
                     @Mutable long scale(in long k) raises (TooFast);
                 };
             };",
        );
        let (id, _) = ast
            .interfaces()
            .find(|(_, node)| node.name == "Calculator")
            .expect("Calculator");
        lower_service(&ast, id).expect("a service")
    }

    fn member<'a>(of: &'a EnhancedStruct, name: &str) -> &'a EnhancedMember {
        of.members
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("{} has no member {name}", of.name))
    }

    /// §7.4.3: "The Enhanced Service Mapping shall map an interface to
    /// exactly one request topic and a reply topic."
    #[test]
    fn enhanced_service_has_one_topic_pair() {
        let service = build_enhanced_service(&calc_service()).unwrap();
        assert_eq!(service.request_topic, "Calculator_Service_Request");
        assert_eq!(service.reply_topic, "Calculator_Service_Reply");
    }

    /// §7.5.1.2.4 rules 1 to 4: `${I}_Request`, `@Choice`, `@Autoid`, a
    /// member per operation typed `${I}_${op}_In`. No header.
    #[test]
    fn enhanced_request_is_a_choice_over_the_operations() {
        let service = build_enhanced_service(&calc_service()).unwrap();
        let request = &service.request;
        assert_eq!(request.name, "Calculator_Request");
        assert!(request.choice && request.autoid);
        let names: Vec<&str> = request.members.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["add", "log"]);
        assert_eq!(
            member(request, "add").type_ref,
            MemberType::Struct("Calculator_add_In".to_string())
        );
    }

    /// §7.5.1.2.5: `${I}_Reply` has a member per operation typed
    /// `${I}_${op}_Result` and `remoteEx`.
    #[test]
    fn enhanced_reply_has_the_results_and_remote_ex() {
        let service = build_enhanced_service(&calc_service()).unwrap();
        let reply = &service.reply;
        assert_eq!(reply.name, "Calculator_Reply");
        assert!(reply.choice && reply.autoid);
        assert_eq!(
            member(reply, "add").type_ref,
            MemberType::Struct("Calculator_add_Result".to_string())
        );
        assert_eq!(
            member(reply, "remoteEx").type_ref,
            MemberType::RemoteException
        );
    }

    /// §7.5.1.2.5 rule 5: an operation named `remoteEx` moves the code
    /// to `remoteEx_1`.
    #[test]
    fn enhanced_remote_ex_avoids_an_operation_of_that_name() {
        let svc = service_of("@service interface S { void remoteEx(); void remoteEx_1x(); };");
        let reply = build_enhanced_service(&svc).unwrap().reply;
        assert_eq!(
            member(&reply, "remoteEx").type_ref,
            MemberType::Struct("S_remoteEx_Result".to_string())
        );
        assert_eq!(
            member(&reply, "remoteEx_1").type_ref,
            MemberType::RemoteException
        );
    }

    /// §7.5.1.2.5 rule 5 with N = 2: operations `remoteEx` and `remoteEx_1`
    /// take the names `remoteEx` and `remoteEx_1` in the Reply structure,
    /// so the `RemoteException` member is `remoteEx_2`.
    #[test]
    fn enhanced_remote_ex_takes_the_first_free_number_past_one() {
        let svc = service_of("@service interface S { void remoteEx(); void remoteEx_1(); };");
        let reply = build_enhanced_service(&svc).unwrap().reply;
        assert_eq!(
            member(&reply, "remoteEx").type_ref,
            MemberType::Struct("S_remoteEx_Result".to_string())
        );
        assert_eq!(
            member(&reply, "remoteEx_1").type_ref,
            MemberType::Struct("S_remoteEx_1_Result".to_string())
        );
        assert_eq!(
            member(&reply, "remoteEx_2").type_ref,
            MemberType::RemoteException
        );
    }

    /// §7.5.1.2.3 rules 2 to 6: the `Result` structure is `@Choice
    /// @Autoid`, its first member `result` has the `Out` type and every
    /// exception adds `<lowercase>_ex`.
    #[test]
    fn enhanced_result_carries_out_and_the_exceptions() {
        let service = build_enhanced_service(&calculator_hierarchy()).unwrap();
        let scale = service
            .operations
            .iter()
            .find(|op| op.name == "scale")
            .unwrap();
        let result = &scale.result_struct;
        assert_eq!(result.name, "Calculator_scale_Result");
        assert!(result.choice && result.autoid);
        assert_eq!(result.members[0].name, "result");
        assert_eq!(
            result.members[0].type_ref,
            MemberType::Struct("Calculator_scale_Out".to_string())
        );
        assert_eq!(result.members[1].name, "toofast_ex");
        assert!(matches!(
            &result.members[1].type_ref,
            MemberType::Exception(raised) if raised.name == "TooFast"
        ));
    }

    /// §7.5.1.2.6: the inherited operations stand in the derived
    /// interface's `Request` and `Reply`, under the base's type names.
    #[test]
    fn enhanced_inheritance_is_flattened() {
        let service = build_enhanced_service(&calculator_hierarchy()).unwrap();
        for op in ["on", "off", "scale", "add", "sub"] {
            assert!(service.request.members.iter().any(|m| m.name == op), "{op}");
            assert!(service.reply.members.iter().any(|m| m.name == op), "{op}");
        }
        assert_eq!(
            member(&service.request, "add").type_ref,
            MemberType::Struct("Adder_add_In".to_string())
        );
        assert_eq!(
            member(&service.reply, "sub").type_ref,
            MemberType::Struct("Subtractor_sub_Result".to_string())
        );
        assert_eq!(service.request.members.len(), 5);
    }

    /// §7.5.1.2.1.4 and §7.5.1.2.2/3: `@Mutable` makes the three
    /// structures of that operation mutable and no other.
    #[test]
    fn enhanced_mutable_operation_has_mutable_structures() {
        let service = build_enhanced_service(&calculator_hierarchy()).unwrap();
        for op in &service.operations {
            let expected = op.name == "scale";
            assert_eq!(op.in_struct.mutable, expected, "{}", op.name);
            assert_eq!(op.out_struct.mutable, expected, "{}", op.name);
            assert_eq!(op.result_struct.mutable, expected, "{}", op.name);
        }
    }

    /// §7.5.1.2.1.2: the member id is the HASH of the member name.
    #[test]
    fn enhanced_autoid_member_ids_are_the_hash_of_the_name() {
        let service = build_enhanced_service(&calc_service()).unwrap();
        let add = member(&service.request, "add");
        assert_eq!(
            add.member_id,
            Some(crate::rpc_hash::rpc_member_hash("add") & 0x0FFF_FFFF)
        );
        let in_struct = &service.operations[0].in_struct;
        assert_eq!(
            member(in_struct, "a").member_id,
            Some(crate::rpc_hash::rpc_member_hash("a") & 0x0FFF_FFFF)
        );
    }

    /// §7.5.1.2.1.2: "the collision shall be reported as an error".
    /// `m9852` and `m12254` hash to the same 28-bit id.
    #[test]
    fn enhanced_member_id_collision_is_an_error() {
        let svc =
            service_of("@service interface Clash { void op(in long m9852, in long m12254); };");
        let err = build_enhanced_service(&svc).unwrap_err();
        assert!(
            matches!(&err, RpcError::MemberIdCollision { structure, .. } if structure == "Clash_op_In"),
            "{err:?}"
        );
        // The Basic mapping has no @Autoid and maps the same service.
        assert!(build_basic_pair(&svc).is_ok());
    }

    /// §7.5.1.2.6 example: an operation without parameters gets the
    /// `dummy` member of §7.5.1.1.4 rule 3c by default.
    #[test]
    fn enhanced_empty_operation_has_a_dummy_member_by_default() {
        let service = build_enhanced_service(&calculator_hierarchy()).unwrap();
        let on = service
            .operations
            .iter()
            .find(|op| op.name == "on")
            .unwrap();
        assert_eq!(on.in_struct.members.len(), 1);
        assert_eq!(on.in_struct.members[0].name, DUMMY_MEMBER);
        assert!(!on.in_struct.empty);
        assert_eq!(on.out_struct.members[0].name, DUMMY_MEMBER);
    }

    /// §7.5.1.2.1.3, the other reading: `@Empty` structures.
    #[test]
    fn enhanced_empty_operation_can_be_an_empty_structure() {
        let service = build_enhanced_service_with(
            &calculator_hierarchy(),
            EnhancedOptions {
                empty_operations: EmptyOperationStyle::EmptyAnnotation,
            },
        )
        .unwrap();
        let on = service
            .operations
            .iter()
            .find(|op| op.name == "on")
            .unwrap();
        assert!(on.in_struct.empty && on.in_struct.members.is_empty());
        assert!(on.out_struct.empty && on.out_struct.members.is_empty());
    }

    /// An `inout` parameter is in both the `In` and the `Out`
    /// structure (§7.5.1.1.4, §7.5.1.1.5).
    #[test]
    fn enhanced_inout_param_is_in_both_structures() {
        let svc = service_of("interface Swap { void swap(inout long v); };");
        let service = build_enhanced_service(&svc).unwrap();
        let op = &service.operations[0];
        assert!(op.in_struct.members.iter().any(|m| m.name == "v"));
        assert!(op.out_struct.members.iter().any(|m| m.name == "v"));
    }

    /// The return value is the last member of `Out`, named `return_`.
    #[test]
    fn enhanced_out_carries_the_return_value() {
        let service = build_enhanced_service(&calc_service()).unwrap();
        let out = &service.operations[0].out_struct;
        assert_eq!(out.name, "Calculator_add_Out");
        assert_eq!(out.members.last().unwrap().name, RETURN_MEMBER);
    }

    #[test]
    fn empty_service_yields_empty_unions_in_basic() {
        let svc = ServiceDef::new("Empty", Vec::new());
        let (req, rep) = build_basic_pair(&svc).unwrap();
        let req_u = match &req.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        let rep_u = match &rep.members[1].type_ref {
            MemberType::CallUnion(u) => u,
            _ => unreachable!(),
        };
        assert_eq!(req_u.cases.len(), 0);
        assert_eq!(rep_u.cases.len(), 0);
    }

    #[test]
    fn invalid_service_name_is_error_in_codegen() {
        let svc = ServiceDef::new(String::new(), Vec::new());
        let err = build_basic_pair(&svc).unwrap_err();
        assert!(matches!(err, RpcError::InvalidServiceName(_)));
    }

    #[test]
    fn enhanced_service_with_empty_name_is_an_error() {
        let svc = ServiceDef::new(String::new(), Vec::new());
        assert!(matches!(
            build_enhanced_service(&svc),
            Err(RpcError::InvalidServiceName(_))
        ));
    }

    #[test]
    fn enhanced_method_with_invalid_name_is_error() {
        let svc = ServiceDef::new(
            "S",
            alloc::vec![MethodDef {
                name: String::new(),
                params: Vec::new(),
                return_type: None,
                oneway: false,
                raises: Vec::new(),
                mutable: false,
            }],
        );
        let err = build_enhanced_service(&svc).unwrap_err();
        assert!(matches!(err, RpcError::InvalidMethodName(_)));
    }
}

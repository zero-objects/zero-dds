// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A DDS-RPC 1.0 service defined by a pair of IDL types (§7.3.1.4), as a
//! Java binding: the request type and the reply type are the topic types
//! of an `org.omg.dds.rpc.Requester` and `org.omg.dds.rpc.Replier`.
//!
//! The structures are in `ZdTypes`, as every structure is. In the Basic
//! Service Mapping their `header` member is of the runtime's
//! `org.omg.dds.rpc.RequestHeader` / `ReplyHeader` (§7.5.1.1.1): the
//! declarations the user's IDL gives `dds::rpc::RequestHeader` and the
//! other common types are not emitted, and the codec writes no function
//! for them; [`adjust_declarations`] does the first, and
//! [`crate::emit::type_pair_cpp::excluded_types`] the second. The header
//! is the first member of the sample, its layout is the one
//! `org.omg.dds.rpc.HeaderCodec` writes, and the members after it are
//! written by the shared codec's functions.
//!
//! What is written in `ZdServices`, beside the services of the interfaces:
//!
//! * `<Request>Support`, with the topic constants (§7.4.1, §7.4.2.2), the
//!   DDS type names (the structures' scoped names), the two
//!   `org.omg.dds.rpc.TypeSupport`s, which implement `HeaderInRequestType`
//!   and `HeaderInReplyType` for the Basic mapping, and
//!   `requesterParams` / `replierParams`, which carry the annotated
//!   service name and topic names. The run-time service name set on the
//!   params takes precedence over them (§7.4.2.3).

use std::fmt::Write as _;

use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::Purpose;
use zerodds_lang_cst::common::Path;
use zerodds_lang_cst::java;

use crate::emit::java::Java;
use crate::emit::type_pair_cpp::{PairNames, body_member_ids, pair_names};
use crate::emit::type_pair_rust::{Pair, collect};

/// The common types of §7.5.1.1.1 whose declaration the generated file
/// leaves out: the runtime's classes take their place. The other ten are
/// declared and encoded like any other type, so a type that refers to one
/// has its codec.
const COMMON: [&str; 2] = ["RequestHeader", "ReplyHeader"];

fn quoted(text: &str) -> String {
    format!("{text:?}")
}

/// Whether `path` names one of the two header types of §7.5.1.1.1 as
/// `ZdTypes` declares them.
fn header_leaf(path: &Path) -> Option<&str> {
    let segments = path.segments();
    let [.., dds, rpc, leaf] = segments else {
        return None;
    };
    (dds.as_str() == "dds"
        && rpc.as_str() == "rpc"
        && matches!(leaf.as_str(), "RequestHeader" | "ReplyHeader"))
    .then(|| leaf.as_str())
}

/// The deepest module nesting the walk follows (`adapter::interface::MAX_MODULE_DEPTH`
/// bounds what the model can hold; a class nested deeper is left as it is,
/// and `javac` refuses a header reference that then stays unresolved).
const MAX_NESTING: usize = 64;

/// zerodds-lint: recursion-depth 64 (nested classes, capped at `MAX_NESTING`)
fn retarget(members: &mut Vec<java::Member>, in_spec_scope: &[&str], depth: usize) {
    if depth > MAX_NESTING {
        return;
    }
    members.retain_mut(|member| match member {
        java::Member::Nested(decl) => match decl.as_mut() {
            java::TypeDecl::Class(class) => {
                let name = class.name.as_str().to_string();
                let mut scope: Vec<&str> = in_spec_scope.to_vec();
                scope.push(&name);
                let spec_module = scope == ["dds"] || scope == ["dds", "rpc"];
                if in_spec_scope == ["dds", "rpc"] && COMMON.contains(&name.as_str()) {
                    return false;
                }
                retarget(
                    &mut class.members,
                    if spec_module || in_spec_scope.is_empty() {
                        &scope
                    } else {
                        &[]
                    },
                    depth + 1,
                );
                !(spec_module && class.members.is_empty())
            }
            java::TypeDecl::Enum(enumeration) => {
                !(in_spec_scope == ["dds", "rpc"] && COMMON.contains(&enumeration.name.as_str()))
            }
            _ => true,
        },
        java::Member::Field(field) => {
            runtime_header(&mut field.type_);
            true
        }
        // Under the DDS-Java-PSM Clause 9 mapping the member is private and
        // its type is spelled again by the accessors.
        java::Member::Method(method) => {
            runtime_header(&mut method.returns);
            for param in &mut method.params {
                runtime_header(&mut param.type_);
            }
            true
        }
        _ => true,
    });
}

/// The runtime's class in place of the declaration of one of the two
/// headers.
fn runtime_header(type_: &mut java::Type) {
    if let java::Type::Named { path, .. } = type_ {
        if let Some(leaf) = header_leaf(path) {
            let runtime = Path::new(&["org", "omg", "dds", "rpc", leaf]);
            *type_ = java::Type::named(runtime);
        }
    }
}

/// The read of the member `field` of `value`, as the mapping in force
/// spells it: a field under IDL-to-Java, the getter under Clause 9.
fn read_member(field: &str) -> String {
    if crate::adapter::java::mapping() == crate::adapter::java::JavaMapping::Psm {
        format!("value.{}()", crate::adapter::java::getter_name(field))
    } else {
        format!("value.{field}")
    }
}

/// The statement that stores `expression` in the member `field` of `value`.
fn write_member(field: &str, expression: &str) -> String {
    if crate::adapter::java::mapping() == crate::adapter::java::JavaMapping::Psm {
        format!(
            "value.{}({expression});",
            crate::adapter::java::setter_name(field)
        )
    } else {
        format!("value.{field} = {expression};")
    }
}

/// Whether the model defines a service by a pair of types.
#[must_use]
pub fn has_pair_types(ast: &Ast) -> bool {
    crate::emit::type_pair_rust::has_pair_types(ast)
}

/// The types the codec walk leaves out when the model defines pairs: the
/// two headers, whose codecs [`header_codecs`] writes over the runtime's
/// classes, and the pair types, whose codecs the binding writes.
#[must_use]
pub fn excluded_types(ast: &Ast) -> Vec<Id<TypeNode>> {
    crate::emit::type_pair_cpp::excluded_types(ast)
        .into_iter()
        .filter(|id| {
            !ast.type_(*id)
                .is_ok_and(|node| node.name == "SampleIdentity")
        })
        .collect()
}

/// The codec members of `RequestHeader` and `ReplyHeader` of the model, in
/// the names the generated codec walk calls them by. They read and write
/// the runtime's classes as §7.5.1.1.1 lays the plain types out; the
/// writer and reader apply the selected encoding's alignment.
#[must_use]
pub fn header_codecs(ast: &Ast) -> Vec<java::Member> {
    let purpose = Purpose::Data;
    let mut out = Vec::new();
    if !has_pair_types(ast) {
        return out;
    }
    for id in crate::emit::type_pair_cpp::runtime_shaped_types(ast) {
        let Ok(node) = ast.type_(id) else { continue };
        let (class, encode_tail, decode_tail) = match node.name.as_str() {
            "RequestHeader" => (
                "RequestHeader",
                "        zdIdentity(w, v.requestId());
        w.writeStringBounded(v.instanceName(), 255);",
                "        org.omg.dds.rpc.SampleIdentity id = zdIdentity(r);
        return new org.omg.dds.rpc.RequestHeader(id, r.readString());",
            ),
            "ReplyHeader" => (
                "ReplyHeader",
                "        zdIdentity(w, v.relatedRequestId());
        w.writeU32(v.remoteEx().wireValue() & 0xFFFFFFFFL);",
                "        org.omg.dds.rpc.SampleIdentity id = zdIdentity(r);
        return new org.omg.dds.rpc.ReplyHeader(
                id, org.omg.dds.rpc.RemoteExceptionCode.fromWire((int) r.readU32()));",
            ),
            _ => continue,
        };
        let enc = crate::emit::encode::codec_name::<Java>(ast, id, purpose);
        let dec = crate::emit::decode::decoder_name::<Java>(ast, id);
        out.push(java::Member::Verbatim(format!(
            "    public static void {enc}(org.zerodds.wire.Writer w, org.omg.dds.rpc.{class} v) {{
{encode_tail}
    }}

    public static org.omg.dds.rpc.{class} {dec}(
            org.zerodds.wire.Reader r, org.zerodds.wire.TryConstruct tc) {{
{decode_tail}
    }}
"
        )));
    }
    if !out.is_empty() {
        out.push(java::Member::Verbatim(
            "    private static void zdIdentity(org.zerodds.wire.Writer w, org.omg.dds.rpc.SampleIdentity v) {
        for (byte octet : v.writerGuid()) {
            w.writeU8(octet & 0xFF);
        }
        w.writeI32((int) (v.sequenceNumber() >>> 32));
        w.writeU32(v.sequenceNumber() & 0xFFFFFFFFL);
    }

    private static org.omg.dds.rpc.SampleIdentity zdIdentity(org.zerodds.wire.Reader r) {
        byte[] guid = new byte[16];
        for (int i = 0; i < 16; ++i) {
            guid[i] = (byte) r.readU8();
        }
        long high = r.readI32() & 0xFFFFFFFFL;
        long low = r.readU32();
        return new org.omg.dds.rpc.SampleIdentity(guid, (high << 32) | low);
    }
"
            .to_string(),
        ));
    }
    out
}

/// Leaves the declarations of the two headers of §7.5.1.1.1 out of the data types and gives the `header` members the runtime's classes. Only
/// called for a model with a pair.
pub fn adjust_declarations(items: &mut [java::TypeDecl]) {
    for item in items.iter_mut() {
        if let java::TypeDecl::Class(class) = item {
            let scope: Vec<&str> = Vec::new();
            retarget(&mut class.members, &scope, 0);
        }
    }
}

fn data_type(pair: &Pair, name: &str) -> String {
    let mut out = String::from("ZdTypes");
    for segment in &pair.scope {
        out.push('.');
        out.push_str(&java::escape_keyword(segment));
    }
    out.push('.');
    out.push_str(&java::escape_keyword(name));
    out
}

fn type_support(
    ast: &Ast,
    pair: &Pair,
    id: Id<TypeNode>,
    request: bool,
    text: &mut String,
) -> Result<(), String> {
    let node = ast.type_(id).map_err(|_| "a stale type".to_string())?;
    let type_name = data_type(pair, &node.name);
    let class = if request {
        "RequestTypeSupport"
    } else {
        "ReplyTypeSupport"
    };
    let (header_in, header_class, set, get) = if request {
        (
            "HeaderInRequestType",
            "RequestHeader",
            "setRequestHeader",
            "requestHeader",
        )
    } else {
        (
            "HeaderInReplyType",
            "ReplyHeader",
            "setReplyHeader",
            "replyHeader",
        )
    };
    let empty = if request { "\"\"" } else { "null" };
    let (encode_header, decode_header) = if request {
        ("encodeRequestHeader", "decodeRequestHeader")
    } else {
        ("encodeReplyHeader", "decodeReplyHeader")
    };
    let members = crate::emit::type_pair_cpp::body_members(ast, node, pair.basic)?;
    let implements = if pair.basic {
        format!(
            "org.omg.dds.rpc.TypeSupport<{type_name}>, org.omg.dds.rpc.{header_in}<{type_name}>"
        )
    } else {
        format!("org.omg.dds.rpc.TypeSupport<{type_name}>")
    };
    let _ = write!(
        text,
        "
    /** The codec of {name}, the {kind} type of the pair. DDS-RPC 1.0 §7.3.1.4. */
    public static final class {class} implements {implements} {{
        @Override
        public byte[] serialize({type_name} value) {{
            return serialize(value, 0, org.omg.dds.rpc.WireEncoding.XCDR1);
        }}

        @Override
        public byte[] serialize({type_name} value, int offset, org.omg.dds.rpc.WireEncoding encoding) {{
            org.zerodds.wire.Writer w = new org.zerodds.wire.Writer(
                    org.zerodds.wire.Endianness.LITTLE, encoding.maxAlign(), null, false);
            // The alignment is counted from the start of the sample.
            for (int pad = 0; pad < offset; ++pad) {{
                w.writeU8(0);
            }}
",
        name = node.name,
        kind = if request { "request" } else { "reply" },
    );
    if pair.basic {
        let _ = write!(
            text,
            "            // A header nobody set is the unknown one.
            byte[] zdHeader = org.omg.dds.rpc.HeaderCodec.{encode_header}(
                    {header} == null ? new org.omg.dds.rpc.{header_class}(null, {empty}) : {header});
            for (byte zdOctet : zdHeader) {{
                w.writeU8(zdOctet & 0xFF);
            }}
",
            header = read_member("header"),
        );
    }
    for (name, member) in &members {
        let field = java::escape_keyword(name);
        let function = crate::emit::encode::codec_name::<Java>(ast, *member, Purpose::Data);
        let _ = write!(
            text,
            "            if (encoding == org.omg.dds.rpc.WireEncoding.XCDR1) {{
                ZdCodec.Xcdr1.{function}(w, {value});
            }} else {{
                ZdCodec.{function}(w, {value});
            }}
",
            value = read_member(&field),
        );
    }
    let _ = write!(
        text,
        "            byte[] zdAll = w.finish();
            return java.util.Arrays.copyOfRange(zdAll, offset, zdAll.length);
        }}

        @Override
        public {type_name} deserialize(byte[] octets, org.omg.dds.rpc.HeaderCodec.Cursor cursor) {{
            return deserialize(octets, cursor, org.omg.dds.rpc.WireEncoding.XCDR1);
        }}

        @Override
        public {type_name} deserialize(
                byte[] octets,
                org.omg.dds.rpc.HeaderCodec.Cursor cursor,
                org.omg.dds.rpc.WireEncoding encoding) {{
            org.zerodds.wire.Reader r = new org.zerodds.wire.Reader(
                    octets, org.zerodds.wire.Endianness.LITTLE, encoding.maxAlign(), null);
            {type_name} value = new {type_name}();
            int zdAt = cursor.at;
"
    );
    if pair.basic {
        let _ = write!(
            text,
            "            org.omg.dds.rpc.HeaderCodec.Cursor zdHeaderCursor = new org.omg.dds.rpc.HeaderCodec.Cursor(zdAt);
            {store}
            zdAt = zdHeaderCursor.at;
",
            store = write_member(
                "header",
                &format!("org.omg.dds.rpc.HeaderCodec.{decode_header}(octets, zdHeaderCursor)")
            ),
        );
    }
    let _ = writeln!(text, "            r.skip(zdAt);");
    for (name, member) in &members {
        let field = java::escape_keyword(name);
        let function = crate::emit::decode::decoder_name::<Java>(ast, *member);
        let _ = writeln!(
            text,
            "            {}",
            write_member(
                &field,
                &format!(
                    "encoding == org.omg.dds.rpc.WireEncoding.XCDR1
                    ? ZdCodec.Xcdr1.{function}(r, org.zerodds.wire.TryConstruct.DISCARD)
                    : ZdCodec.{function}(r, org.zerodds.wire.TryConstruct.DISCARD)"
                )
            )
        );
    }
    let _ = write!(
        text,
        "            cursor.at = r.position();
            return value;
        }}
"
    );
    if pair.basic {
        let _ = write!(
            text,
            "
        @Override
        public void {set}({type_name} value, org.omg.dds.rpc.{header_class} header) {{
            {store}
        }}

        @Override
        public org.omg.dds.rpc.{header_class} {get}({type_name} value) {{
            return {header};
        }}
",
            store = write_member("header", "header"),
            header = read_member("header"),
        );
    }
    text.push_str("    }\n");
    Ok(())
}

fn support_text(ast: &Ast, pair: &Pair) -> Result<String, String> {
    let request = ast
        .type_(pair.request)
        .map_err(|_| "a stale type".to_string())?;
    let reply = ast
        .type_(pair.reply)
        .map_err(|_| "a stale type".to_string())?;
    let PairNames {
        request_topic,
        reply_topic,
        service,
    } = pair_names(ast, pair)?;
    let scoped = |name: &str| {
        let mut out = String::new();
        for segment in &pair.scope {
            out.push_str(segment);
            out.push_str("::");
        }
        out.push_str(name);
        out
    };
    let mut text = String::new();
    let _ = write!(
        text,
        "
    /** A topic name or a type name. DDS-RPC 1.0 §7.4.1, §7.5.1.1.6. */
    public static final String REQUEST_TYPE_NAME = {};
    /** The DDS type name of the reply topic. */
    public static final String REPLY_TYPE_NAME = {};
    /** The {{@code <service_name>}} an annotation gives, {{@code \"\"}} for none. §7.4.1. */
    public static final String SERVICE_NAME = {};
    /** The whole name of the request topic an annotation gives, {{@code \"\"}} for none. §7.4.2.2. */
    public static final String REQUEST_TOPIC_NAME = {};
    /** The whole name of the reply topic an annotation gives, {{@code \"\"}} for none. §7.4.2.2. */
    public static final String REPLY_TOPIC_NAME = {};
",
        quoted(&scoped(&request.name)),
        quoted(&scoped(&reply.name)),
        quoted(service.as_deref().unwrap_or("")),
        quoted(request_topic.as_deref().unwrap_or("")),
        quoted(reply_topic.as_deref().unwrap_or("")),
    );
    type_support(ast, pair, pair.request, true, &mut text)?;
    type_support(ast, pair, pair.reply, false, &mut text)?;
    let mapping = if pair.basic {
        ""
    } else {
        "\n                .mapping(org.omg.dds.rpc.ServiceMapping.ENHANCED)"
    };
    let _ = write!(
        text,
        "
    /**
     * The params of a requester over the pair. The run-time service name
     * set on them takes precedence over the annotations. DDS-RPC 1.0
     * §7.4.2.3.
     */
    public static org.omg.dds.rpc.RequesterParams requesterParams(
            org.omg.dds.rpc.DomainParticipant participant) {{
        org.omg.dds.rpc.RequesterParams params = org.omg.dds.rpc.RequesterParams.create();
        params.domainParticipant(participant)
                .annotatedServiceName(SERVICE_NAME)
                .requestTopicName(REQUEST_TOPIC_NAME)
                .replyTopicName(REPLY_TOPIC_NAME)
                .requestTypeName(REQUEST_TYPE_NAME)
                .replyTypeName(REPLY_TYPE_NAME)
                .requestTypeSupport(new RequestTypeSupport())
                .replyTypeSupport(new ReplyTypeSupport()){mapping};
        return params;
    }}

    /** The params of a replier over the pair. */
    public static org.omg.dds.rpc.ReplierParams replierParams(
            org.omg.dds.rpc.DomainParticipant participant, String instanceName) {{
        org.omg.dds.rpc.ReplierParams params = org.omg.dds.rpc.ReplierParams.create();
        params.domainParticipant(participant)
                .instanceName(instanceName)
                .annotatedServiceName(SERVICE_NAME)
                .requestTopicName(REQUEST_TOPIC_NAME)
                .replyTopicName(REPLY_TOPIC_NAME)
                .requestTypeName(REQUEST_TYPE_NAME)
                .replyTypeName(REPLY_TYPE_NAME)
                .requestTypeSupport(new RequestTypeSupport())
                .replyTypeSupport(new ReplyTypeSupport()){mapping};
        return params;
    }}
"
    );
    Ok(text)
}

/// The members of `ZdServices` every pair of `ast` adds, with the chain of
/// module classes each nests in.
///
/// # Errors
/// A description of what in a pair is not a service definition of
/// §7.3.1.4.
pub fn java_type_pair_members(ast: &Ast) -> Result<Vec<(Vec<String>, java::Member)>, String> {
    let mut out = Vec::new();
    for pair in collect(ast)? {
        let request = ast
            .type_(pair.request)
            .map_err(|_| "a stale type".to_string())?;
        let name = format!("{}Support", java::escape_keyword(&request.name));
        let body = support_text(ast, &pair)?;
        let class = java::Member::Nested(Box::new(java::TypeDecl::Class(java::Class {
            doc: zerodds_lang_cst::common::Doc::line(
                "The type support of a service defined by a pair of types. DDS-RPC 1.0 §7.3.1.4.",
            ),
            annotations: Vec::new(),
            modifiers: vec![
                java::Modifier::Public,
                java::Modifier::Static,
                java::Modifier::Final,
            ],
            name: zerodds_lang_cst::common::Ident::new(&name),
            generics: Vec::new(),
            extends: None,
            implements: Vec::new(),
            members: vec![java::Member::Verbatim(body)],
        })));
        let chain = pair.scope.iter().map(|s| java::escape_keyword(s)).collect();
        out.push((chain, class));
    }
    Ok(out)
}

/// The member types the codec of the Java file has to reach for the bodies
/// of the pair types: [`java_type_pair_members`] calls the codec's
/// functions for them.
#[must_use]
pub fn body_roots(ast: &Ast) -> Vec<Id<TypeNode>> {
    collect(ast)
        .map(|pairs| {
            let mut roots: Vec<Id<TypeNode>> = Vec::new();
            for pair in &pairs {
                for id in body_member_ids(ast, pair) {
                    if !roots.contains(&id) {
                        roots.push(id);
                    }
                }
            }
            roots
        })
        .unwrap_or_default()
}

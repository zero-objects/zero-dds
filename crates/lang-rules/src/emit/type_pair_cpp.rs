// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A DDS-RPC 1.0 service defined by a pair of IDL types (§7.3.1.4), as a
//! C++ binding: the request type and the reply type become the topic types
//! of a `dds::rpc::Requester` and `dds::rpc::Replier`.
//!
//! In the Basic Service Mapping both types have a member `header`
//! (`dds::rpc::RequestHeader` and `dds::rpc::ReplyHeader`, §7.5.1.1.1);
//! those types are the runtime's (`<dds/rpc.hpp>`), so the declarations
//! the user's IDL gives them are not emitted and no codec is written for
//! them: the header is the first member, its layout is fixed by the clause,
//! and the traits write and read it with the runtime's own functions
//! (`encode_request_header`, `decode_request_header`) before the other
//! members, whose codecs are the shared codec walk's. In the Enhanced
//! Service Mapping neither type has a header and the sample is the bare
//! structure.
//!
//! What is written beside the structures:
//!
//! * `dds_type_traits` for each type, naming it in DDS by its scoped IDL
//!   name, with `set_request_header` / `set_reply_header` for the Basic
//!   mapping;
//! * `<Request>Support::requester_params` and `replier_params`, the
//!   params with the topic names the annotations (§7.4.2.2) or §7.4.1's
//!   request-reply style give and the correlation the mapping uses; the
//!   run-time service name and suffixes of §7.4.2.3 are set on the params
//!   by the caller and take precedence.
//!
//! The pair types are encoded as `@final` structures in both encodings:
//! the Basic mapping is plain classic CDR (§7.5.1.1), and a DHEADER in
//! front of the header would hide the identity from the runtime, which
//! reads it from the leading octets.

use std::fmt::Write as _;

use zerodds_ast::{Ast, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::cpp;

use crate::adapter::cpp::{cxx_name, nest_by_module};
use crate::emit::cpp::Cpp;
use crate::emit::type_pair_rust::{Pair, collect, service_name_of, string_argument};

/// The names DDS-RPC 1.0 §7.5.1.1.1 declares that the runtime defines too,
/// each in the module the clause puts it in: the identity in `dds`, the rest
/// in `dds::rpc`. A user's IDL that declares them again gets the runtime's
/// types, not a second declaration.
const RUNTIME_DEFINED: [(&[&str], &str); 8] = [
    (&["dds"], "SampleIdentity"),
    (&["dds"], "SequenceNumber_t"),
    (&["dds", "rpc"], "UnknownOperation"),
    (&["dds", "rpc"], "UnknownException"),
    (&["dds", "rpc"], "UnusedMember"),
    (&["dds", "rpc"], "RemoteExceptionCode_t"),
    (&["dds", "rpc"], "RequestHeader"),
    (&["dds", "rpc"], "ReplyHeader"),
];

/// The runtime-defined types whose C++ shape is not the structure the IDL
/// declares (`dds::SampleIdentity` holds the GUID as sixteen octets; the
/// headers use the names of the runtime). A generated codec cannot be
/// written over their IDL members, so the codec of each is written by hand,
/// laid out as §7.5.1.1.1 lays the plain types out. The other
/// runtime-defined names are an enumeration, aliases of `octet` and
/// `SequenceNumber_t`, whose members `high` and `low` are the IDL's, so
/// their generated codecs fit; the remaining common types (`GUID_t`,
/// `EntityId_t`, `GuidPrefix_t`, `InstanceName`) the runtime does not define
/// and are declared and encoded like any other type.
const RUNTIME_SHAPED: [(&[&str], &str); 3] = [
    (&["dds"], "SampleIdentity"),
    (&["dds", "rpc"], "RequestHeader"),
    (&["dds", "rpc"], "ReplyHeader"),
];

fn key() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr2, Endianness::Little)
}

/// The key of classic CDR (XCDR1), which the Basic mapping writes unless
/// the params say XCDR2: a nested `@appendable` member has no DHEADER and a
/// `@mutable` one is a parameter list (XTypes 1.3 §7.4.1.1, §7.4.1.2).
fn key_xcdr1() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr1, Endianness::Little)
}

/// The scope of a type as module names.
fn module_path(ast: &Ast, node: &TypeNode) -> Vec<String> {
    let mut chain = Vec::new();
    let mut current = Some(node.module);
    while let Some(id) = current {
        let Ok(module) = ast.module(id) else { break };
        if !module.name.is_empty() {
            chain.push(module.name.clone());
        }
        current = module.parent;
    }
    chain.reverse();
    chain
}

/// Whether the model defines a service by a pair of types.
#[must_use]
pub fn has_pair_types(ast: &Ast) -> bool {
    crate::emit::type_pair_rust::has_pair_types(ast)
}

/// The common types of §7.5.1.1.1 in the model whose shape in the runtime
/// is not the IDL's, found by name in `dds` and `dds::rpc`.
#[must_use]
pub fn runtime_shaped_types(ast: &Ast) -> Vec<Id<TypeNode>> {
    ast.types()
        .filter(|(_, node)| {
            let path = module_path(ast, node);
            RUNTIME_SHAPED
                .iter()
                .any(|(module, name)| path == *module && node.name == *name)
        })
        .map(|(id, _)| id)
        .collect()
}

/// The types the codec walk leaves out when the model defines pairs: the
/// common types whose shape the runtime gives (their codecs are written by
/// hand, `runtime_shaped_codecs`), and the pair types, whose codecs this
/// module writes. Empty without a pair.
#[must_use]
pub fn excluded_types(ast: &Ast) -> Vec<Id<TypeNode>> {
    if !has_pair_types(ast) {
        return Vec::new();
    }
    let pairs = collect(ast).unwrap_or_default();
    let mut out = runtime_shaped_types(ast);
    for (id, _) in ast.types() {
        let paired = pairs
            .iter()
            .any(|pair| pair.request == id || pair.reply == id);
        if paired && !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

fn is_common(module: &[&str], name: &str) -> bool {
    RUNTIME_DEFINED
        .iter()
        .any(|(here, defined)| module == *here && name == *defined)
}

/// Whether `module` and `name` are a common type the runtime defines and the model with a
/// pair therefore does not declare (the PSM layer of `--cpp-mapping psm` leaves them out too).
#[must_use]
pub fn is_runtime_defined(module: &[String], name: &str) -> bool {
    RUNTIME_DEFINED.iter().any(|(here, defined)| {
        *defined == name
            && here.len() == module.len()
            && here.iter().zip(module).all(|(a, b)| *a == b.as_str())
    })
}

/// Whether `function` is the namespace-level `swap` of a common type
/// (DDS-PSM-Cxx 1.0 §7.12), which goes with the type's class.
fn swaps_common(module: &[&str], function: &cpp::Function) -> bool {
    function.name.as_str() == "swap"
        && !function.params.is_empty()
        && function.params.iter().all(|param| match &param.type_ {
            cpp::Type::Ref(inner) => matches!(
                inner.as_ref(),
                cpp::Type::Named { path, .. }
                    if path.last().is_some_and(|last| is_common(module, last.as_str()))
            ),
            _ => false,
        })
}

/// Removes the declarations of the common types the runtime defines from
/// `items`: they are the runtime's. A namespace left empty goes with them. Only called for a
/// model with a pair.
///
/// Under `--cpp-mapping psm` the native declarations stand in the namespace
/// `zd_native` (DDS-PSM-Cxx 1.0 \u{a7}8.1); the walk looks into it as if it were the
/// root, and a using-declaration in `zd_native` names each dropped type again, so that
/// the native structures that hold one still find it, as the runtime's.
pub fn drop_common_declarations(items: &mut Vec<cpp::Decl>) {
    fn note(dropped: &mut Vec<(Vec<String>, String)>, path: &[&str], name: &str) {
        dropped.push((
            path.iter().map(|segment| (*segment).to_string()).collect(),
            name.to_string(),
        ));
    }
    /// zerodds-lint: recursion-depth 3 — the call below is reached only for
    /// `zd_native` at the root and the namespaces `dds` and `dds::rpc` (the
    /// guard returns before it otherwise), so the recursion is at most those
    /// three levels deep.
    fn keep(path: &[&str], decl: &mut cpp::Decl, dropped: &mut Vec<(Vec<String>, String)>) -> bool {
        let in_spec_scope = path == ["dds"] || path == ["dds", "rpc"];
        match decl {
            cpp::Decl::Namespace(namespace) => {
                let name = namespace.name.as_str().to_string();
                let mut inner: Vec<&str> = path.to_vec();
                // The native namespace is transparent: inside it the names are the spec's.
                if !(path.is_empty() && name == crate::adapter::cpp::NATIVE_NAMESPACE) {
                    inner.push(&name);
                    if !(inner == ["dds"] || inner == ["dds", "rpc"]) {
                        return true;
                    }
                }
                namespace
                    .decls
                    .retain_mut(|child| keep(&inner, child, dropped));
                !namespace.decls.is_empty()
            }
            cpp::Decl::Class(class) => {
                let drop = in_spec_scope && is_common(path, class.name.as_str());
                if drop {
                    note(dropped, path, class.name.as_str());
                }
                !drop
            }
            cpp::Decl::Enum(enumeration) => {
                let drop = in_spec_scope && is_common(path, enumeration.name.as_str());
                if drop {
                    note(dropped, path, enumeration.name.as_str());
                }
                !drop
            }
            cpp::Decl::Alias(alias) => {
                let drop = in_spec_scope && is_common(path, alias.name.as_str());
                if drop {
                    note(dropped, path, alias.name.as_str());
                }
                !drop
            }
            cpp::Decl::Forward { name, .. } => !(in_spec_scope && is_common(path, name.as_str())),
            cpp::Decl::Function(function) => !(in_spec_scope && swaps_common(path, function)),
            _ => true,
        }
    }
    let mut dropped = Vec::new();
    items.retain_mut(|decl| keep(&[], decl, &mut dropped));
    if crate::adapter::cpp::cpp_mapping() != crate::adapter::cpp::CppMapping::Psm {
        return;
    }
    dropped.sort();
    dropped.dedup();
    let mut text = String::new();
    for (path, name) in dropped {
        let open: String = path
            .iter()
            .map(|segment| format!("namespace {segment} {{ "))
            .collect();
        let close = "} ".repeat(path.len());
        text.push_str(&format!(
            "namespace {} {{ {open}using ::{}::{name}; {close}}}\n",
            crate::adapter::cpp::NATIVE_NAMESPACE,
            path.join("::"),
        ));
    }
    if !text.is_empty() {
        items.insert(0, cpp::Decl::Verbatim(text));
    }
}

fn cpp_scoped(scope: &[String], name: &str) -> String {
    let mut out = String::new();
    // Under `--cpp-mapping psm` the structures the codec reads and writes are
    // the native ones, in `zd_native` (DDS-PSM-Cxx 1.0 §8.1).
    if crate::adapter::cpp::cpp_mapping() == crate::adapter::cpp::CppMapping::Psm {
        out.push_str("::");
        out.push_str(crate::adapter::cpp::NATIVE_NAMESPACE);
    }
    for segment in scope {
        out.push_str("::");
        out.push_str(&cxx_name(segment));
    }
    out.push_str("::");
    out.push_str(&cxx_name(name));
    out
}

fn quoted(text: &str) -> String {
    format!("{text:?}")
}

/// The members of `node` after the header: the IDL name and the type.
pub(crate) fn body_members(
    ast: &Ast,
    node: &TypeNode,
    basic: bool,
) -> Result<Vec<(String, Id<TypeNode>)>, String> {
    let TypeKind::Structure { members, base } = &node.kind else {
        return Err(format!("{} is not a structure", node.name));
    };
    if base.is_some() {
        return Err(format!(
            "{} derives from another structure; a request or reply type is a plain structure",
            node.name
        ));
    }
    let mut out = Vec::new();
    for member in members {
        let member = ast
            .member(*member)
            .map_err(|_| "a stale member".to_string())?;
        if basic && member.name == "header" {
            continue;
        }
        let type_ = member
            .type_
            .resolved()
            .ok_or_else(|| format!("member `{}` of `{}` names no type", member.name, node.name))?;
        out.push((member.name.clone(), type_));
    }
    Ok(out)
}

/// The types of the members after the header, in both types of `pair`.
pub(crate) fn body_member_ids(ast: &Ast, pair: &Pair) -> Vec<Id<TypeNode>> {
    let mut out = Vec::new();
    for id in [pair.request, pair.reply] {
        let Ok(node) = ast.type_(id) else { continue };
        for (_, member) in body_members(ast, node, pair.basic).unwrap_or_default() {
            if !out.contains(&member) {
                out.push(member);
            }
        }
    }
    out
}

fn flat(scope: &[String], name: &str) -> String {
    let mut out = String::new();
    for segment in scope {
        out.push_str(segment);
        out.push('_');
    }
    out.push_str(name);
    out
}

fn struct_functions(
    ast: &Ast,
    pair: &Pair,
    id: Id<TypeNode>,
    text: &mut String,
) -> Result<Vec<(String, Id<TypeNode>)>, String> {
    let node = ast.type_(id).map_err(|_| "a stale type".to_string())?;
    let members = body_members(ast, node, pair.basic)?;
    let type_name = cpp_scoped(&pair.scope, &node.name);
    let suffix = flat(&pair.scope, &node.name);
    let purpose = key().purpose;
    let _ = write!(
        text,
        "
static void zd_pair_enc_{suffix}(zerodds::wire::Writer& w, const {type_name}& v) {{
  static_cast<void>(w);
  static_cast<void>(v);
"
    );
    for (name, member) in &members {
        let field = cxx_name(name);
        let _ = writeln!(
            text,
            "  {}(w, v.{field});",
            crate::emit::encode::codec_name::<Cpp>(ast, *member, purpose)
        );
    }
    let _ = write!(
        text,
        "}}

static void zd_pair_dec_{suffix}(zerodds::wire::Reader& r, {type_name}& v) {{
  static_cast<void>(r);
  static_cast<void>(v);
"
    );
    for (name, member) in &members {
        let field = cxx_name(name);
        let _ = writeln!(
            text,
            "  {}(r, v.{field}, zerodds::wire::TryConstruct::Discard);",
            crate::decode_cpp::decoder_name(ast, *member)
        );
    }
    text.push_str("}\n");
    Ok(members)
}

fn traits_text(ast: &Ast, pair: &Pair, id: Id<TypeNode>, request: bool) -> Result<String, String> {
    let node = ast.type_(id).map_err(|_| "a stale type".to_string())?;
    let type_name = cpp_scoped(&pair.scope, &node.name);
    let suffix = flat(&pair.scope, &node.name);
    let dds_name = if pair.scope.is_empty() {
        node.name.clone()
    } else {
        format!("{}::{}", pair.scope.join("::"), node.name)
    };
    let (header_type, encode_header, decode_header, setter) = if request {
        (
            "RequestHeader",
            "dds::rpc::core::encode_request_header",
            "dds::rpc::core::decode_request_header",
            "set_request_header",
        )
    } else {
        (
            "ReplyHeader",
            "dds::rpc::core::encode_reply_header",
            "dds::rpc::core::decode_reply_header",
            "set_reply_header",
        )
    };
    let (write_header, read_header, setters) = if pair.basic {
        (
            format!(
                "    {{
      const Octets zd_header = {encode_header}(value.header);
      for (const std::uint8_t zd_octet : zd_header) {{
        w.write_u8(zd_octet);
      }}
    }}
"
            ),
            format!(
                "    {{
      std::size_t zd_at = at;
      {header_type} zd_header;
      if (!{decode_header}(in, zd_at, zd_header)) {{
        return false;
      }}
      out.header = zd_header;
      r.seek(zd_at);
    }}
"
            ),
            format!(
                "  /// The header the runtime fills in (DDS-RPC 1.0 §7.3.1.4).
  static void {setter}({type_name}& value, const {header_type}& header) {{
    value.header = header;
  }}
"
            ),
        )
    } else {
        (String::new(), String::new(), String::new())
    };
    Ok(format!(
        "template <>
struct dds_type_traits<{type_name}> : public dds_type_traits_base<{type_name}> {{
  /// The DDS type name: the scoped name of the structure.
  static const char* type_name() {{ return {name}; }}

  static Octets serialize_at(const {type_name}& value, std::size_t, WireEncoding encoding) {{
    zerodds::wire::Writer w(zerodds::wire::Endianness::Little, max_alignment(encoding));
    static_cast<void>(value);
{write_header}    if (encoding == WireEncoding::XCDR1) {{
      zd_pair_xcdr1::zd_pair_enc_{suffix}(w, value);
    }} else {{
      zd_pair::zd_pair_enc_{suffix}(w, value);
    }}
    return w.bytes();
  }}

  static Octets serialize(const {type_name}& value) {{
    return serialize_at(value, 0, WireEncoding::XCDR1);
  }}

  static bool deserialize(const Octets& in, std::size_t& at, {type_name}& out,
                          WireEncoding encoding) {{
    zerodds::wire::Reader r(in, zerodds::wire::Endianness::Little, max_alignment(encoding));
    r.seek(at);
{read_header}    if (encoding == WireEncoding::XCDR1) {{
      zd_pair_xcdr1::zd_pair_dec_{suffix}(r, out);
    }} else {{
      zd_pair::zd_pair_dec_{suffix}(r, out);
    }}
    at = r.position();
    return r.reader_status() == zerodds::wire::Status::Ok;
  }}

  static bool deserialize(const Octets& in, std::size_t& at, {type_name}& out) {{
    return deserialize(in, at, out, WireEncoding::XCDR1);
  }}

{setters}}};
",
        name = quoted(&dds_name),
    ))
}

/// What the annotations on the two types of a pair name: the whole topic
/// names (§7.4.2.2) and the service name (§7.4.1).
pub(crate) struct PairNames {
    pub(crate) request_topic: Option<String>,
    pub(crate) reply_topic: Option<String>,
    pub(crate) service: Option<String>,
}

pub(crate) fn pair_names(ast: &Ast, pair: &Pair) -> Result<PairNames, String> {
    let request = ast
        .type_(pair.request)
        .map_err(|_| "a stale type".to_string())?;
    let reply = ast
        .type_(pair.reply)
        .map_err(|_| "a stale type".to_string())?;
    let service = match (
        service_name_of(ast, &request.facets)?,
        service_name_of(ast, &reply.facets)?,
    ) {
        (Some(a), Some(b)) if a != b => {
            return Err(format!(
                "the request type names the service {a:?} and the reply type {b:?}"
            ));
        }
        (Some(name), _) | (None, Some(name)) => Some(name),
        (None, None) => None,
    };
    Ok(PairNames {
        request_topic: string_argument(ast, &request.facets, "DDSRequestTopic", "name")?,
        reply_topic: string_argument(ast, &reply.facets, "DDSReplyTopic", "name")?,
        service,
    })
}

fn support_text(ast: &Ast, pair: &Pair) -> Result<String, String> {
    let request = ast
        .type_(pair.request)
        .map_err(|_| "a stale type".to_string())?;
    let reply = ast
        .type_(pair.reply)
        .map_err(|_| "a stale type".to_string())?;
    let req = cxx_name(&request.name);
    let rep = cxx_name(&reply.name);
    let PairNames {
        request_topic,
        reply_topic,
        service,
    } = pair_names(ast, pair)?;
    let correlation = if pair.basic {
        "HEADER_IN_TYPE"
    } else {
        "SAMPLE_IDENTITY"
    };
    let mut configure = String::new();
    if let Some(name) = &service {
        let _ = write!(
            configure,
            "\n        .annotated_service_name({})",
            quoted(name)
        );
    }
    let mut request_configure = configure.clone();
    if let Some(topic) = &request_topic {
        let _ = write!(
            request_configure,
            "\n        .request_topic_name({})",
            quoted(topic)
        );
    }
    let mut reply_configure = configure;
    if let Some(topic) = &reply_topic {
        let _ = write!(
            reply_configure,
            "\n        .reply_topic_name({})",
            quoted(topic)
        );
    }
    let both = |configure: &str, params: &str| {
        let mut text = configure.to_string();
        // Both params carry both topic annotations: a requester and a
        // replier have to agree on the two topics.
        if let Some(topic) = &request_topic {
            if !text.contains("request_topic_name") {
                let _ = write!(text, "\n        .request_topic_name({})", quoted(topic));
            }
        }
        if let Some(topic) = &reply_topic {
            if !text.contains("reply_topic_name") {
                let _ = write!(text, "\n        .reply_topic_name({})", quoted(topic));
            }
        }
        let _ = params;
        text
    };
    let requester_chain = both(&request_configure, "RequesterParams");
    let replier_chain = both(&reply_configure, "ReplierParams");
    Ok(format!(
        "/// The type support of the pair of types {req} and {rep}: the params that
/// carry the topic names the annotations give (DDS-RPC 1.0 §7.4.2.2) or the
/// request-reply style of §7.4.1, and the correlation the mapping uses. The
/// service name and the suffixes given at run time, on the params, take
/// precedence (§7.4.2.3).
struct {req}Support {{
  using Requester = dds::rpc::core::Requester<{req}, {rep}>;
  using Replier = dds::rpc::core::Replier<{req}, {rep}>;

  static dds::rpc::core::RequesterParams requester_params(
      std::shared_ptr<dds::rpc::DomainParticipant> participant) {{
    dds::rpc::core::RequesterParams params;
    params.domain_participant(std::move(participant))
        .binding_style(dds::rpc::BindingStyle::RequestReply)
        .correlation(dds::rpc::Correlation::{correlation}){requester_chain};
    return params;
  }}

  static dds::rpc::core::ReplierParams replier_params(
      std::shared_ptr<dds::rpc::DomainParticipant> participant) {{
    dds::rpc::core::ReplierParams params;
    params.domain_participant(std::move(participant))
        .binding_style(dds::rpc::BindingStyle::RequestReply)
        .correlation(dds::rpc::Correlation::{correlation}){replier_chain};
    return params;
  }}
}};
"
    ))
}

/// The include the structures of a pair need before they are declared:
/// their `header` members are the runtime's types.
pub const RUNTIME_INCLUDE: &str = "#include <dds/rpc.hpp>";

/// The hand-written codecs of the common types the runtime shapes itself
/// (`shaped`): `SampleIdentity`, `RequestHeader` and `ReplyHeader` of
/// §7.5.1.1.1, in the names the generated codec walk calls them by. They
/// lay the plain types out as the clause does and the runtime does (the
/// sixteen octets of the GUID, the two halves of the sequence number, the
/// instance name as a `string<255>`, the code as an enumeration of four
/// octets); they are `inline`, so a model that never refers to one has
/// nothing unused. `None` when `shaped` is empty.
fn runtime_shaped_codecs(ast: &Ast, shaped: &[Id<TypeNode>]) -> Option<String> {
    let purpose = key().purpose;
    let find = |name: &str| {
        shaped
            .iter()
            .copied()
            .find(|id| ast.type_(*id).is_ok_and(|node| node.name == name))
    };
    let identity = find("SampleIdentity")?;
    let enc = |id| crate::emit::encode::codec_name::<Cpp>(ast, id, purpose);
    let dec = |id| crate::decode_cpp::decoder_name(ast, id);
    let def = |id| crate::default_cpp::defaulter_name(ast, id);
    let (enc_identity, dec_identity, def_identity) = (enc(identity), dec(identity), def(identity));
    let mut text = format!(
        "
// DDS-RPC 1.0 \u{a7}7.5.1.1.1: the runtime's types for the common types.
inline void {enc_identity}(zerodds::wire::Writer& w, const ::dds::SampleIdentity& v) {{
  for (std::size_t i = 0; i < 16; ++i) {{
    w.write_u8(v.writer_guid[i]);
  }}
  w.write_i32(v.sequence_number.high);
  w.write_u32(v.sequence_number.low);
}}

inline void {dec_identity}(zerodds::wire::Reader& r, ::dds::SampleIdentity& v,
                           zerodds::wire::TryConstruct) {{
  for (std::size_t i = 0; i < 16; ++i) {{
    v.writer_guid[i] = r.read_u8();
  }}
  v.sequence_number.high = r.read_i32();
  v.sequence_number.low = r.read_u32();
}}

inline void {def_identity}(::dds::SampleIdentity& v) {{
  v = ::dds::SampleIdentity{{}};
}}
"
    );
    if let Some(id) = find("RequestHeader") {
        let (e, d, df) = (enc(id), dec(id), def(id));
        let _ = write!(
            text,
            "
inline void {e}(zerodds::wire::Writer& w, const ::dds::rpc::RequestHeader& v) {{
  {enc_identity}(w, v.requestId);
  w.write_string_bounded(v.instanceName, 255);
}}

inline void {d}(zerodds::wire::Reader& r, ::dds::rpc::RequestHeader& v,
                zerodds::wire::TryConstruct tc) {{
  {dec_identity}(r, v.requestId, tc);
  v.instanceName = r.read_string();
}}

inline void {df}(::dds::rpc::RequestHeader& v) {{
  v = ::dds::rpc::RequestHeader{{}};
}}
"
        );
    }
    if let Some(id) = find("ReplyHeader") {
        let (e, d, df) = (enc(id), dec(id), def(id));
        let _ = write!(
            text,
            "
inline void {e}(zerodds::wire::Writer& w, const ::dds::rpc::ReplyHeader& v) {{
  {enc_identity}(w, v.relatedRequestId);
  w.write_u32(static_cast<std::uint32_t>(v.remoteEx));
}}

inline void {d}(zerodds::wire::Reader& r, ::dds::rpc::ReplyHeader& v,
                zerodds::wire::TryConstruct tc) {{
  {dec_identity}(r, v.relatedRequestId, tc);
  v.remoteEx = static_cast<::dds::rpc::RemoteExceptionCode_t>(r.read_u32());
}}

inline void {df}(::dds::rpc::ReplyHeader& v) {{
  v = ::dds::rpc::ReplyHeader{{}};
}}
"
        );
    }
    Some(text)
}

/// The C++ declarations of every pair in `ast`, nested in their modules.
///
/// # Errors
/// A description of what in a pair is not a service definition of
/// §7.3.1.4.
pub fn cpp_type_pair_items(ast: &Ast) -> Result<Vec<cpp::Decl>, String> {
    let pairs = collect(ast)?;
    if pairs.is_empty() {
        return Ok(Vec::new());
    }
    let root = ast.root_module();
    let mut out: Vec<(Id<zerodds_ast::ModuleNode>, cpp::Decl)> = Vec::new();
    // The codecs of the members of every pair type, each once: two pairs
    // share the codecs of `long` and `string`.
    let mut roots: Vec<Id<TypeNode>> = Vec::new();
    let mut functions = Vec::new();
    for pair in &pairs {
        let mut ids = vec![pair.request];
        if pair.reply != pair.request {
            ids.push(pair.reply);
        }
        let mut text = String::new();
        for id in &ids {
            for (_, member) in struct_functions(ast, pair, *id, &mut text)? {
                if !roots.contains(&member) {
                    roots.push(member);
                }
            }
        }
        functions.push(text);
    }
    let shaped = runtime_shaped_types(ast);
    if let Some(text) = runtime_shaped_codecs(ast, &shaped) {
        out.push((root, cpp::Decl::Verbatim(text)));
    }
    // The codecs of the pair types' members in a namespace of their own
    // per version: a model that has an interface too defines the codecs of
    // the same types for it at the global scope.
    let mut modern: Vec<cpp::Decl> = Vec::new();
    let mut classic: Vec<cpp::Decl> = Vec::new();
    if !roots.is_empty() {
        let options = crate::emit::encode::CodecOptions::default();
        for (version, sink) in [(key(), &mut modern), (key_xcdr1(), &mut classic)] {
            let encoders = crate::emit::encode::codec_from_many_excluding::<Cpp>(
                ast, &roots, version, options, &shaped,
            )
            .map_err(|error| error.to_string())?;
            let decoders = crate::decode_cpp::cpp_decoder_from_many_excluding(
                ast, &roots, version, options, &shaped,
            )
            .map_err(|error| error.to_string())?;
            sink.extend(encoders.into_iter().chain(decoders));
        }
    }
    // The functions of the pair types are the same text under both
    // versions: the codecs they call are found in the namespace first.
    for sink in [&mut modern, &mut classic] {
        sink.extend(functions.iter().cloned().map(cpp::Decl::Verbatim));
    }
    for (name, doc, decls) in [
        (
            "zd_pair",
            "The codecs of the members of the pair types under XCDR2. DDS-RPC 1.0 \u{a7}7.5.1.1.",
            modern,
        ),
        (
            "zd_pair_xcdr1",
            "The same codecs under classic CDR (XCDR1), which the Basic mapping writes \
             unless the params say XCDR2. DDS-RPC 1.0 \u{a7}7.5.1.1, XTypes 1.3 \u{a7}7.4.1.",
            classic,
        ),
    ] {
        out.push((
            root,
            cpp::Decl::Namespace(cpp::Namespace {
                doc: zerodds_lang_cst::common::Doc::line(doc),
                name: zerodds_lang_cst::common::Ident::new(name),
                decls,
            }),
        ));
    }
    for pair in &pairs {
        let mut traits = String::from("namespace dds {\nnamespace rpc {\n\n");
        traits.push_str(&traits_text(ast, pair, pair.request, true)?);
        if pair.reply != pair.request {
            traits.push('\n');
            traits.push_str(&traits_text(ast, pair, pair.reply, false)?);
        }
        traits.push_str("\n}  // namespace rpc\n}  // namespace dds\n");
        out.push((root, cpp::Decl::Verbatim(traits)));
        out.push((pair.module, cpp::Decl::Verbatim(support_text(ast, pair)?)));
    }
    nest_by_module(ast, out).map_err(|error| format!("{error:?}"))
}

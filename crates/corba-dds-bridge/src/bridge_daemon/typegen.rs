// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The DDS types of a mapping, from the IDL of its interface (§5.2).
//!
//! The IDL a mapping names with `idl_file` is read, the operation is
//! looked up in the interface of `repo_id`, and one IDL file is written
//! from it: the types the operation uses as the source declares them, the
//! exceptions it raises as structures, each interface as a module, and in
//! it a module per mapped operation with `Request`, `Reply` and `Notify`.
//!
//! The layout is the one of §4.3, so the argument bytes pass through:
//! `Request` is the correlation member and the `in`/`inout` arguments,
//! `Reply` the correlation member and a union over the `ReplyStatusType`.
//! Its first two branches begin with the 32 bits that keep the rest
//! 8-aligned. In the branch of a user exception those 32 bits are the
//! enumeration `exception_kind`, and what follows is the exception's
//! members instead of its repository id: [`user_exception_body`] writes
//! the GIOP form from it.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::config::{DaemonConfig, Mapping};

use zerodds_ast::{
    AnyId, Ast, Bound, CharWidth, ExceptionNode, Extensibility, InterfaceNode, ParamDirection,
    PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref, RepositoryId};
use zerodds_ast_layout::{Encoding, Endianness as LayoutEndianness};
use zerodds_idl_cst::{CstKind, SyntaxNode, TokenKind};
use zerodds_idl_parse::FileLoader;
use zerodds_wire_codec::{ProbeRequest, decode_continuing, encode_continuing};

/// How deep modules and interfaces may nest in the IDL of a mapping, and how
/// many levels of base interfaces an operation is looked up through.
const MAX_NESTING: usize = 64;

/// One mapping's wish: the operation and what it needs of it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Wanted<'a> {
    /// Repository id of the interface.
    pub repo_id: &'a str,
    /// The operation, or `_get_<attribute>` / `_set_<attribute>`.
    pub operation: &'a str,
    /// Name of the correlation member (§4.3 step 5).
    pub correlation: &'a str,
    /// The mapping sends DDS samples to CORBA targets and needs `Notify`.
    pub notify: bool,
}

/// An exception an operation raises; its index is its `exception_kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raised {
    /// Its repository id, as the GIOP reply names it.
    pub repo_id: String,
    /// The structure with its members; `None` for one without members.
    pub type_name: Option<String>,
}

/// The generated types of one operation, by scoped name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct OpTypes {
    /// `<interface>::<operation>::Request`.
    pub request: String,
    /// `<interface>::<operation>::Reply`; empty for a `oneway` operation.
    pub reply: String,
    /// `<interface>::<operation>::Notify`; empty unless asked for.
    pub notify: String,
    /// The exceptions, in the order of `exception_kind`.
    pub raises: Vec<Raised>,
}

/// What one IDL file yields.
pub(crate) struct Generated {
    /// The IDL text.
    pub text: String,
    /// Its model, for [`user_exception_body`].
    pub ast: Arc<Ast>,
    /// One entry per [`Wanted`], in its order.
    pub ops: Vec<OpTypes>,
}

impl core::fmt::Debug for Generated {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Generated")
            .field("text", &self.text)
            .field("ops", &self.ops)
            .finish_non_exhaustive()
    }
}

struct Signature {
    ins: Vec<(String, String)>,
    outs: Vec<(String, String)>,
    result: Option<String>,
    raises: Vec<Id<ExceptionNode>>,
    oneway: bool,
}

struct Plan {
    /// Scoped names of the types the generated file declares.
    needed: BTreeSet<String>,
    /// Scoped names of the raised exceptions that have members.
    raised: BTreeSet<String>,
    /// Interface path and the module of one operation in it.
    modules: Vec<(Vec<String>, String)>,
}

fn diagnostics(composed: &zerodds_idl_compose::Composed) -> String {
    composed
        .diagnostics
        .iter()
        .filter(|d| d.severity == zerodds_idl_parse::Severity::Error)
        .map(|d| {
            let mut d = d.clone();
            composed.locate(&mut d);
            d.to_string()
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn scoped(ast: &Ast, any: AnyId) -> Result<Vec<String>, String> {
    ast.scoped_name_of(any)
        .map(|n| n.segments().to_vec())
        .ok_or_else(|| "a declaration without a scoped name".to_string())
}

fn repo_id_of(ast: &Ast, any: AnyId) -> Result<String, String> {
    if let Some(declared) = ast
        .facets(any)
        .and_then(|f| f.corba.as_ref())
        .and_then(|c| c.repository_id.as_ref())
    {
        return Ok(declared.0.clone());
    }
    Ok(format!("IDL:{}:1.0", scoped(ast, any)?.join("/")))
}

fn find_interface(ast: &Ast, repo_id: &str) -> Result<Id<InterfaceNode>, String> {
    if let Ok(AnyId::Interface(id, _)) = ast.find_by_repository_id(&RepositoryId(repo_id.into())) {
        return Ok(id);
    }
    for (id, node) in ast.interfaces() {
        if repo_id_of(ast, AnyId::Interface(id, node.node_id)).as_deref() == Ok(repo_id) {
            return Ok(id);
        }
    }
    Err(format!("no interface with the repository id {repo_id}"))
}

fn keyword(kind: PrimitiveKind) -> Result<&'static str, String> {
    Ok(match kind {
        PrimitiveKind::Boolean => "boolean",
        PrimitiveKind::Octet => "octet",
        PrimitiveKind::Char => "char",
        PrimitiveKind::Int8 => "int8",
        PrimitiveKind::UInt8 => "uint8",
        PrimitiveKind::Int16 => "short",
        PrimitiveKind::UInt16 => "unsigned short",
        PrimitiveKind::Int32 => "long",
        PrimitiveKind::UInt32 => "unsigned long",
        PrimitiveKind::Int64 => "long long",
        PrimitiveKind::UInt64 => "unsigned long long",
        PrimitiveKind::Float32 => "float",
        PrimitiveKind::Float64 => "double",
        PrimitiveKind::Float128 => "long double",
        PrimitiveKind::WChar => return Err(refused("wchar")),
    })
}

fn refused(what: &str) -> String {
    format!("{what} has no DDS form the GIOP bytes pass through to (zerodds-corba-bridge-1.0 §5.2)")
}

/// How a parameter's type is written in the generated module: by its
/// absolute name, so that nothing the module declares hides it.
fn spell(ast: &Ast, type_: &Ref<TypeNode>) -> Result<String, String> {
    let id = type_
        .resolved()
        .ok_or_else(|| "a type that does not resolve".to_string())?;
    let node = ast.type_(id).map_err(|e| e.to_string())?;
    match &node.kind {
        TypeKind::Primitive(kind) => keyword(*kind).map(str::to_string),
        TypeKind::String {
            width: CharWidth::Char,
            bound: Bound::Unbounded,
        } => Ok("string".to_string()),
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => Err(refused("wstring")),
        _ if !node.name.is_empty() => Ok(format!(
            "::{}",
            scoped(ast, AnyId::Type(id, node.node_id))?.join("::")
        )),
        _ => Err("a parameter of an anonymous type: declare it with a typedef".to_string()),
    }
}

/// Notes every type `type_` reaches and refuses what GIOP and XCDR1
/// encode differently or DDS has no type for.
fn close(ast: &Ast, type_: &Ref<TypeNode>, needed: &mut BTreeSet<String>) -> Result<(), String> {
    let id = type_
        .resolved()
        .ok_or_else(|| "a type that does not resolve".to_string())?;
    let reached = ast.reachable_types(id);
    for (other, node) in ast.types() {
        if !reached.contains(node.node_id) {
            continue;
        }
        let name = || {
            if node.name.is_empty() {
                String::new()
            } else {
                format!("{}: ", node.name)
            }
        };
        match &node.kind {
            TypeKind::Primitive(PrimitiveKind::WChar) => return Err(refused("wchar")),
            TypeKind::String {
                width: CharWidth::WChar,
                ..
            } => return Err(refused("wstring")),
            TypeKind::Fixed { .. } => return Err(format!("{}{}", name(), refused("fixed"))),
            TypeKind::Any => return Err(format!("{}{}", name(), refused("any"))),
            TypeKind::ObjectRef { .. } => {
                return Err(format!("{}{}", name(), refused("an object reference")));
            }
            TypeKind::Valuetype { .. } | TypeKind::ValueBox { .. } | TypeKind::ValueBase => {
                return Err(format!("{}{}", name(), refused("a valuetype")));
            }
            TypeKind::Native => return Err(format!("{}{}", name(), refused("a native type"))),
            TypeKind::Structure { .. } | TypeKind::Union { .. }
                if ast.effective_extensibility(other) == Extensibility::Mutable =>
            {
                return Err(format!("{}{}", name(), refused("a @mutable type")));
            }
            _ => {}
        }
        let declared = !matches!(
            node.kind,
            TypeKind::Primitive(_)
                | TypeKind::String { .. }
                | TypeKind::Sequence { .. }
                | TypeKind::Array { .. }
                | TypeKind::Map { .. }
        );
        if declared && !node.name.is_empty() {
            needed.insert(scoped(ast, AnyId::Type(other, node.node_id))?.join("::"));
        }
    }
    Ok(())
}

fn signature(
    ast: &Ast,
    interface: Id<InterfaceNode>,
    operation: &str,
    needed: &mut BTreeSet<String>,
) -> Result<Signature, String> {
    let mut seen = Vec::new();
    signature_in(ast, interface, operation, needed, &mut seen, 0)?
        .ok_or_else(|| format!("the interface has no operation {operation}"))
}

/// The signature of `operation` in `interface` or in one of its bases.
///
/// zerodds-lint: recursion-depth 64 (one level per base interface; refused
/// past `MAX_NESTING`)
fn signature_in(
    ast: &Ast,
    interface: Id<InterfaceNode>,
    operation: &str,
    needed: &mut BTreeSet<String>,
    seen: &mut Vec<Id<InterfaceNode>>,
    depth: usize,
) -> Result<Option<Signature>, String> {
    if depth > MAX_NESTING {
        return Err(format!(
            "the interface inherits through more than {MAX_NESTING} levels of base interfaces"
        ));
    }
    if seen.contains(&interface) {
        return Ok(None);
    }
    seen.push(interface);
    let node = ast.interface(interface).map_err(|e| e.to_string())?;
    for id in &node.operations {
        let op = ast.operation(*id).map_err(|e| e.to_string())?;
        if op.name != operation {
            continue;
        }
        let mut sig = Signature {
            ins: Vec::new(),
            outs: Vec::new(),
            result: None,
            raises: Vec::new(),
            oneway: op.oneway,
        };
        if let Some(result) = &op.return_type {
            close(ast, result, needed)?;
            sig.result = Some(spell(ast, result)?);
        }
        for param in &op.params {
            let param = ast.param(*param).map_err(|e| e.to_string())?;
            close(ast, &param.type_, needed).map_err(|e| format!("{}: {e}", param.name))?;
            let spelled = spell(ast, &param.type_).map_err(|e| format!("{}: {e}", param.name))?;
            if matches!(param.direction, ParamDirection::In | ParamDirection::InOut) {
                sig.ins.push((param.name.clone(), spelled.clone()));
            }
            if matches!(param.direction, ParamDirection::Out | ParamDirection::InOut) {
                sig.outs.push((param.name.clone(), spelled));
            }
        }
        for raised in &op.raises {
            sig.raises.push(
                raised
                    .resolved()
                    .ok_or_else(|| "a raised exception that does not resolve".to_string())?,
            );
        }
        return Ok(Some(sig));
    }
    // An attribute is the operations `_get_<name>` and `_set_<name>`
    // (CORBA 3.3 Part 2 §9.3.5).
    for id in &node.attributes {
        let attribute = ast.attribute(*id).map_err(|e| e.to_string())?;
        let getter = operation.strip_prefix("_get_") == Some(attribute.name.as_str());
        let setter = operation.strip_prefix("_set_") == Some(attribute.name.as_str());
        if !getter && !(setter && !attribute.readonly) {
            continue;
        }
        close(ast, &attribute.type_, needed)?;
        let spelled = spell(ast, &attribute.type_)?;
        let raises = if getter {
            &attribute.get_raises
        } else {
            &attribute.set_raises
        };
        let mut sig = Signature {
            ins: Vec::new(),
            outs: Vec::new(),
            result: None,
            raises: Vec::new(),
            oneway: false,
        };
        if getter {
            sig.result = Some(spelled);
        } else {
            sig.ins.push(("value".to_string(), spelled));
        }
        for raised in raises {
            sig.raises.push(
                raised
                    .resolved()
                    .ok_or_else(|| "a raised exception that does not resolve".to_string())?,
            );
        }
        return Ok(Some(sig));
    }
    for base in &node.bases {
        if let Some(base) = base.resolved()
            && let Some(sig) = signature_in(ast, base, operation, needed, seen, depth + 1)?
        {
            return Ok(Some(sig));
        }
    }
    Ok(None)
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The module of one operation and its types.
fn operation_module(
    ast: &Ast,
    interface: &[String],
    wanted: &Wanted<'_>,
    sig: &Signature,
    plan: &mut Plan,
) -> Result<(String, OpTypes), String> {
    if !is_identifier(wanted.correlation) {
        return Err(format!(
            "correlation_id `{}` is not an IDL identifier",
            wanted.correlation
        ));
    }
    let module = wanted.operation.trim_start_matches('_');
    let base = format!("{}::{module}", interface.join("::"));
    let mut types = OpTypes {
        request: format!("{base}::Request"),
        ..OpTypes::default()
    };
    let members = |list: &[(String, String)], taken: &[&str]| -> Result<String, String> {
        let mut out = String::new();
        for (name, spelled) in list {
            if taken.iter().any(|t| t.eq_ignore_ascii_case(name)) {
                return Err(format!(
                    "the parameter {name} has the name of a member the bridge writes"
                ));
            }
            out.push_str(&format!("    {spelled} {name};\n"));
        }
        Ok(out)
    };
    let mut text = format!("module {module} {{\n");
    text.push_str(&format!(
        "@final struct Request {{\n    unsigned long long {};\n{}}};\n",
        wanted.correlation,
        members(&sig.ins, &[wanted.correlation])?
    ));
    if wanted.notify {
        if sig.ins.is_empty() {
            return Err(
                "an operation without in arguments has no notify sample to declare".to_string(),
            );
        }
        text.push_str(&format!(
            "@final struct Notify {{\n{}}};\n",
            members(&sig.ins, &[])?
        ));
        types.notify = format!("{base}::Notify");
    }
    if !sig.oneway {
        let mut kinds = Vec::new();
        let mut cases = String::new();
        for id in &sig.raises {
            let node = ast.exception(*id).map_err(|e| e.to_string())?;
            let any = AnyId::Exception(*id, node.node_id);
            let path = scoped(ast, any)?;
            let kind = path.join("_");
            let type_name = if node.members.is_empty() {
                None
            } else {
                for member in &node.members {
                    let member = ast.member(*member).map_err(|e| e.to_string())?;
                    close(ast, &member.type_, &mut plan.needed)
                        .map_err(|e| format!("{}::{}: {e}", node.name, member.name))?;
                }
                plan.raised.insert(path.join("::"));
                cases.push_str(&format!(
                    "    case {kind}: ::{} as_{kind};\n",
                    path.join("::")
                ));
                Some(path.join("::"))
            };
            types.raises.push(Raised {
                repo_id: repo_id_of(ast, any)?,
                type_name,
            });
            kinds.push(kind);
        }
        let user = if kinds.is_empty() {
            ""
        } else if cases.is_empty() {
            "    case 1: exception_kind raised;\n"
        } else {
            "    case 1: UserException raised;\n"
        };
        if !kinds.is_empty() {
            text.push_str(&format!(
                "enum exception_kind {{ {} }};\n",
                kinds.join(", ")
            ));
        }
        if !cases.is_empty() {
            text.push_str(&format!(
                "@final union UserException switch (exception_kind) {{\n{cases}}};\n"
            ));
        }
        let mut result = sig.outs.clone();
        if let Some(spelled) = &sig.result {
            let mut name = "return_value".to_string();
            while sig.outs.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                name.push('_');
            }
            result.insert(0, (name, spelled.clone()));
        }
        text.push_str(&format!(
            "@final struct Result {{\n    unsigned long reserved;\n{}}};\n",
            members(&result, &["reserved"])?
        ));
        text.push_str(
            "@final struct SystemException {\n    unsigned long reserved;\n    string exception_id;\n    unsigned long minor_code;\n    unsigned long completion_status;\n};\n",
        );
        text.push_str(&format!(
            "@final union Outcome switch (long) {{\n    case 0: Result returned;\n{user}    case 2: SystemException failed;\n}};\n"
        ));
        text.push_str(&format!(
            "@final struct Reply {{\n    unsigned long long {};\n    Outcome status;\n}};\n",
            wanted.correlation
        ));
        types.reply = format!("{base}::Reply");
    }
    text.push_str("};\n");
    Ok((text, types))
}

fn ident(node: &SyntaxNode) -> Option<String> {
    node.syntax_tokens()
        .into_iter()
        .find(|(_, t)| t.kind() == TokenKind::Ident)
        .map(|(_, t)| t.text().trim_start_matches('_').to_string())
}

/// The tokens of a declaration, one space apart.
fn tokens(node: &Arc<SyntaxNode>) -> Vec<String> {
    let mut all: Vec<(u32, String)> = node
        .descendants()
        .iter()
        .flat_map(|n| n.syntax_tokens())
        .filter(|(_, t)| t.kind() != TokenKind::Eof)
        .map(|(span, t)| (span.start, t.text().to_string()))
        .collect();
    all.sort_by_key(|(at, _)| *at);
    all.into_iter().map(|(_, text)| text).collect()
}

fn declaration(mut tokens: Vec<String>) -> String {
    if tokens.last().map(String::as_str) != Some(";") {
        tokens.push(";".to_string());
    }
    let mut out = tokens.join(" ");
    out.push('\n');
    out
}

/// Writes the declarations of `node` that the plan keeps.
///
/// zerodds-lint: recursion-depth 64 (one level per nested module or
/// interface; refused past `MAX_NESTING`)
fn render(
    node: &Arc<SyntaxNode>,
    scope: &mut Vec<String>,
    plan: &Plan,
    out: &mut String,
) -> Result<(), String> {
    if scope.len() > MAX_NESTING {
        return Err(format!(
            "{}: modules and interfaces nested deeper than {MAX_NESTING} levels",
            scope.join("::")
        ));
    }
    // Annotations ahead of a declaration belong to it.
    let mut annotations = String::new();
    for child in node.children() {
        let kind = child.kind();
        if kind == CstKind::Annotation {
            annotations.push_str(&tokens(&child).join(" "));
            annotations.push(' ');
            continue;
        }
        let ahead = core::mem::take(&mut annotations);
        let name = ident(&child).unwrap_or_default();
        let path = || {
            let mut path = scope.clone();
            path.push(name.clone());
            path.join("::")
        };
        let kept: Option<Vec<String>> = match kind {
            CstKind::Module | CstKind::InterfaceDecl => {
                scope.push(name.clone());
                let mut inner = String::new();
                render(&child, scope, plan, &mut inner)?;
                if kind == CstKind::InterfaceDecl {
                    for (interface, module) in &plan.modules {
                        if interface == scope {
                            inner.push_str(module);
                        }
                    }
                }
                scope.pop();
                if !inner.is_empty() {
                    out.push_str(&format!("module {name} {{\n{inner}}};\n"));
                }
                None
            }
            CstKind::EnumDecl | CstKind::ConstDecl => Some(tokens(&child)),
            CstKind::StructDecl
            | CstKind::UnionDecl
            | CstKind::BitmaskDecl
            | CstKind::BitsetDecl => plan.needed.contains(&path()).then(|| tokens(&child)),
            CstKind::ForwardDecl => {
                let tokens = tokens(&child);
                let aggregate = tokens.iter().any(|t| t == "struct" || t == "union");
                (aggregate && plan.needed.contains(&path())).then_some(tokens)
            }
            CstKind::TypedefDecl => {
                let mut names: Vec<String> = child
                    .descendants()
                    .iter()
                    .filter(|n| n.kind() == CstKind::Declarator)
                    .filter_map(|n| ident(n))
                    .collect();
                if names.is_empty() {
                    names = tokens(&child);
                }
                let declares = names.iter().any(|n| {
                    let mut path = scope.clone();
                    path.push(n.trim_start_matches('_').to_string());
                    plan.needed.contains(&path.join("::"))
                });
                declares.then(|| tokens(&child))
            }
            CstKind::ExceptionDecl => plan.raised.contains(&path()).then(|| {
                let mut tokens = tokens(&child);
                if let Some(first) = tokens.iter_mut().find(|t| *t == "exception") {
                    *first = "struct".to_string();
                }
                tokens
            }),
            _ => None,
        };
        if let Some(tokens) = kept {
            out.push_str(&ahead);
            out.push_str(&declaration(tokens));
        }
    }
    Ok(())
}

/// Reads `source` (the IDL at `path`) and generates the types of every
/// operation in `wanted`.
///
/// # Errors
/// The IDL does not compile, an interface or operation is not in it, a
/// type has no DDS form the bytes pass through to, or the generated IDL
/// does not compile.
pub(crate) fn generate(
    path: &str,
    source: &str,
    loader: &dyn FileLoader,
    wanted: &[Wanted<'_>],
) -> Result<Generated, String> {
    let composed = zerodds_idl_compose::compose_source(path, source, loader);
    if composed.failed() {
        return Err(format!("{path}: {}", diagnostics(&composed)));
    }
    let ast = &composed.ast;
    let mut plan = Plan {
        needed: BTreeSet::new(),
        raised: BTreeSet::new(),
        modules: Vec::new(),
    };
    let mut ops = Vec::new();
    for w in wanted {
        let context = |e: String| format!("{path}: {} {}: {e}", w.repo_id, w.operation);
        let interface = find_interface(ast, w.repo_id).map_err(context)?;
        let node = ast
            .interface(interface)
            .map_err(|e| context(e.to_string()))?;
        let at = scoped(ast, AnyId::Interface(interface, node.node_id)).map_err(context)?;
        let sig = signature(ast, interface, w.operation, &mut plan.needed).map_err(context)?;
        let (module, types) = operation_module(ast, &at, w, &sig, &mut plan).map_err(context)?;
        // Two mappings of one operation share its module.
        if !plan.modules.iter().any(|(i, m)| *i == at && *m == module) {
            if plan
                .modules
                .iter()
                .any(|(i, m)| *i == at && m.lines().next() == module.lines().next())
            {
                return Err(context(
                    "mapped twice with another correlation_id or direction".to_string(),
                ));
            }
            plan.modules.push((at, module));
        }
        ops.push(types);
    }
    let preprocessed = zerodds_idl_parse::preprocess_with(path, source, loader, &[]);
    let parsed = zerodds_idl_parse::parse_preprocessed(&preprocessed);
    let mut text = String::from(
        "// Generated by zerodds-corba-bridged from the IDL of its mappings\n// (zerodds-corba-bridge-1.0 §5.2). The samples are XCDR1.\n",
    );
    render(&parsed.syntax(), &mut Vec::new(), &plan, &mut text)?;
    let generated = zerodds_idl_compose::compose_source(
        "generated.idl",
        &text,
        &zerodds_idl_parse::MemoryLoader::new(),
    );
    if generated.failed() {
        return Err(format!(
            "{path}: the generated IDL does not compile: {}",
            diagnostics(&generated)
        ));
    }
    Ok(Generated {
        text,
        ast: Arc::new(generated.ast),
        ops,
    })
}

/// The IDL of one `idl_file` and the mappings it serves.
pub(crate) struct Unit {
    /// The `idl_file`.
    pub source: String,
    /// What it yields; `ops` is in the order of `mappings`.
    pub generated: Generated,
    /// Indices into the mappings it was generated for.
    pub mappings: Vec<usize>,
}

/// Generates the types of every mapping that names an `idl_file`, one
/// [`Unit`] per file. A quoted `#include` resolves beside the file.
///
/// # Errors
/// The file cannot be read, or as [`generate`].
pub(crate) fn units(mappings: &[Mapping]) -> Result<Vec<Unit>, String> {
    let mut sources: Vec<(String, Vec<usize>)> = Vec::new();
    for (index, m) in mappings.iter().enumerate() {
        let Some(file) = &m.idl_file else { continue };
        match sources.iter_mut().find(|(source, _)| source == file) {
            Some((_, indices)) => indices.push(index),
            None => sources.push((file.clone(), vec![index])),
        }
    }
    let mut out = Vec::new();
    for (source, indices) in sources {
        let text =
            std::fs::read_to_string(&source).map_err(|e| format!("idl_file {source}: {e}"))?;
        let wanted: Vec<Wanted<'_>> = indices
            .iter()
            .filter_map(|i| mappings.get(*i))
            .map(|m| Wanted {
                repo_id: &m.repo_id,
                operation: &m.operation,
                correlation: &m.correlation_id,
                notify: m.sends_giop(),
            })
            .collect();
        let generated = generate(
            &source,
            &text,
            &zerodds_idl_compose::PathLoader::bare(),
            &wanted,
        )?;
        out.push(Unit {
            source,
            generated,
            mappings: indices,
        });
    }
    Ok(out)
}

/// Gives every mapping with an `idl_file` its generated types and the
/// exceptions of its operation.
///
/// # Errors
/// As [`units`].
pub(crate) fn apply(mappings: &mut [Mapping]) -> Result<(), String> {
    for unit in units(mappings)? {
        for (types, index) in unit.generated.ops.iter().zip(&unit.mappings) {
            let Some(m) = mappings.get_mut(*index) else {
                continue;
            };
            m.request_topic.dds_type.clone_from(&types.request);
            if !types.reply.is_empty() {
                m.reply_topic.dds_type.clone_from(&types.reply);
            }
            if !types.notify.is_empty() {
                m.notify_topic.dds_type.clone_from(&types.notify);
            }
            m.raises.clone_from(&types.raises);
        }
    }
    Ok(())
}

/// The files `corba.idl_dir` gets: one per `idl_file` of the daemon's and
/// the tenants' mappings, under the name of that file.
///
/// # Errors
/// As [`units`]; two `idl_file`s of one name; a file that would replace
/// its own source.
pub(crate) fn files(cfg: &DaemonConfig) -> Result<Vec<(std::path::PathBuf, String)>, String> {
    let mut all = cfg.mappings.clone();
    for t in &cfg.tenants {
        all.extend(t.mappings.iter().cloned());
    }
    let mut out: Vec<(std::path::PathBuf, String)> = Vec::new();
    for unit in units(&all)? {
        let source = std::path::Path::new(&unit.source);
        let stem = source
            .file_stem()
            .ok_or_else(|| format!("idl_file {}: not a file name", unit.source))?;
        let mut target = std::path::Path::new(&cfg.idl_dir).join(stem);
        target.set_extension("idl");
        if out.iter().any(|(other, _)| *other == target) {
            return Err(format!(
                "idl_file {}: another idl_file has this name, both would be {}",
                unit.source,
                target.display()
            ));
        }
        if let (Ok(a), Ok(b)) = (source.canonicalize(), target.canonicalize())
            && a == b
        {
            return Err(format!(
                "idl_file {}: corba.idl_dir is its directory, the generated file would replace it",
                unit.source
            ));
        }
        out.push((target, unit.generated.text));
    }
    Ok(out)
}

/// The GIOP body of a user exception (CORBA 3.3 Part 2 §9.4.3.2: the
/// repository id, then the members) from the reply sample `payload`,
/// whose octets 12 to 16 are the `exception_kind` and whose members begin
/// at octet 16.
///
/// # Errors
/// The kind is none of the operation's, or the members do not decode.
pub(crate) fn user_exception_body(
    ast: &Ast,
    raises: &[Raised],
    payload: &[u8],
    big_endian: bool,
) -> Result<Vec<u8>, String> {
    let kind: [u8; 4] = payload
        .get(12..16)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| "a reply sample shorter than its header".to_string())?;
    let kind = if big_endian {
        u32::from_be_bytes(kind)
    } else {
        u32::from_le_bytes(kind)
    };
    let raised = usize::try_from(kind)
        .ok()
        .and_then(|k| raises.get(k))
        .ok_or_else(|| format!("exception_kind {kind} is none of the operation's"))?;
    let length = u32::try_from(raised.repo_id.len() + 1).map_err(|e| e.to_string())?;
    let mut body = if big_endian {
        length.to_be_bytes().to_vec()
    } else {
        length.to_le_bytes().to_vec()
    };
    body.extend_from_slice(raised.repo_id.as_bytes());
    body.push(0);
    let Some(type_name) = &raised.type_name else {
        return Ok(body);
    };
    let endianness = if big_endian {
        LayoutEndianness::Big
    } else {
        LayoutEndianness::Little
    };
    let request = ProbeRequest::new(type_name, Encoding::PlainCdr1, endianness);
    let value: serde_json::Value =
        decode_continuing(ast, &request, payload, 16).map_err(|e| format!("{type_name}: {e}"))?;
    let members = encode_continuing(ast, &request, &value, body.len())
        .map_err(|e| format!("{type_name}: {e}"))?;
    body.extend_from_slice(&members);
    Ok(body)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_idl_parse::MemoryLoader;

    const CALC: &str = r#"
module Demo {
    struct Unused { any payload; };
    typedef sequence<long> Longs;
    enum Mode { FAST, EXACT };
    struct Terms { Longs values; Mode how; };
    exception DivideByZero {};
    interface Calc {
        exception Overflow { long long limit; string why; };
        long long add(in long long a, in long long b) raises (Overflow, DivideByZero);
        void sum(in Terms input, out long long total, inout string note);
        oneway void reset(in long seed);
        attribute long precision;
        wstring name();
    };
};
"#;

    fn wanted(operation: &str) -> Wanted<'_> {
        Wanted {
            repo_id: "IDL:Demo/Calc:1.0",
            operation,
            correlation: "request_id",
            notify: false,
        }
    }

    fn calc(wanted: &[Wanted<'_>]) -> Result<Generated, String> {
        generate("calc.idl", CALC, &MemoryLoader::new(), wanted)
    }

    fn flat(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn corbabridge_5_2_an_operation_behind_more_than_64_base_interfaces_is_refused() {
        // I0 declares ping; I<n> inherits I<n-1>. From I64 the lookup goes
        // through 64 levels of bases, from I65 through 65.
        let mut idl = String::from("module Deep {\n    interface I0 { void ping(); };\n");
        for n in 1..=65 {
            idl.push_str(&format!("    interface I{n} : I{} {{}};\n", n - 1));
        }
        idl.push_str("};\n");
        let ask = |repo_id: &'static str| {
            generate(
                "deep.idl",
                &idl,
                &MemoryLoader::new(),
                &[Wanted {
                    repo_id,
                    operation: "ping",
                    correlation: "request_id",
                    notify: false,
                }],
            )
        };
        assert!(ask("IDL:Deep/I64:1.0").is_ok());
        let error = ask("IDL:Deep/I65:1.0").err().unwrap_or_default();
        assert!(
            error.contains("more than 64 levels of base interfaces"),
            "{error}"
        );
    }

    #[test]
    fn corbabridge_5_2_modules_nested_deeper_than_64_levels_are_refused() {
        let nested = |levels: usize| {
            let mut idl = String::new();
            for _ in 0..levels {
                idl.push_str("module M { ");
            }
            idl.push_str("interface Calc { void ping(); }; ");
            for _ in 0..levels {
                idl.push_str("}; ");
            }
            let repo_id = format!("IDL:{}Calc:1.0", "M/".repeat(levels));
            let wanted = [Wanted {
                repo_id: &repo_id,
                operation: "ping",
                correlation: "request_id",
                notify: false,
            }];
            generate("nested.idl", &idl, &MemoryLoader::new(), &wanted)
        };
        assert!(nested(63).is_ok(), "{:?}", nested(63).err());
        let error = nested(65).err().unwrap_or_default();
        assert!(error.contains("nested deeper than 64 levels"), "{error}");
    }

    #[test]
    fn corbabridge_5_2_request_and_reply_are_generated_from_the_operation() {
        let g = calc(&[wanted("add")]).unwrap();
        assert_eq!(g.ops[0].request, "Demo::Calc::add::Request");
        assert_eq!(g.ops[0].reply, "Demo::Calc::add::Reply");
        let text = flat(&g.text);
        assert!(
            text.contains(
                "@final struct Request { unsigned long long request_id; long long a; long long b; };"
            ),
            "{}",
            g.text
        );
        assert!(
            text.contains(
                "@final struct Result { unsigned long reserved; long long return_value; };"
            ),
            "{}",
            g.text
        );
        assert!(
            text.contains(
                "@final struct Reply { unsigned long long request_id; Outcome status; };"
            ),
            "{}",
            g.text
        );
        // What the operation does not use is not declared: `Unused`
        // holds an `any`.
        assert!(!text.contains("Unused"), "{}", g.text);
    }

    #[test]
    fn corbabridge_5_2_the_exceptions_are_the_variant_exception_kind() {
        let g = calc(&[wanted("add")]).unwrap();
        let text = flat(&g.text);
        assert!(
            text.contains("enum exception_kind { Demo_Calc_Overflow, Demo_DivideByZero };"),
            "{}",
            g.text
        );
        assert!(
            text.contains(
                "@final union UserException switch (exception_kind) { case Demo_Calc_Overflow: ::Demo::Calc::Overflow as_Demo_Calc_Overflow; };"
            ),
            "{}",
            g.text
        );
        assert!(
            text.contains("struct Overflow { long long limit ; string why ; } ;"),
            "{}",
            g.text
        );
        assert_eq!(
            g.ops[0].raises,
            vec![
                Raised {
                    repo_id: "IDL:Demo/Calc/Overflow:1.0".into(),
                    type_name: Some("Demo::Calc::Overflow".into()),
                },
                Raised {
                    repo_id: "IDL:Demo/DivideByZero:1.0".into(),
                    type_name: None,
                },
            ]
        );
    }

    #[test]
    fn corbabridge_5_2_out_and_inout_arguments_and_the_used_types() {
        let g = calc(&[wanted("sum")]).unwrap();
        let text = flat(&g.text);
        assert!(
            text.contains(
                "@final struct Request { unsigned long long request_id; ::Demo::Terms input; string note; };"
            ),
            "{}",
            g.text
        );
        assert!(
            text.contains(
                "@final struct Result { unsigned long reserved; long long total; string note; };"
            ),
            "{}",
            g.text
        );
        for declared in [
            "typedef sequence < long > Longs ;",
            "enum Mode",
            "struct Terms",
        ] {
            assert!(text.contains(declared), "{declared}: {}", g.text);
        }
        // No exception: the reply has no branch for one.
        assert!(!text.contains("case 1"), "{}", g.text);
    }

    #[test]
    fn corbabridge_5_2_oneway_attributes_and_notify() {
        let g = calc(&[
            wanted("reset"),
            wanted("_get_precision"),
            wanted("_set_precision"),
            Wanted {
                notify: true,
                ..wanted("sum")
            },
        ])
        .unwrap();
        assert_eq!(g.ops[0].reply, "");
        assert_eq!(g.ops[1].request, "Demo::Calc::get_precision::Request");
        assert_eq!(g.ops[3].notify, "Demo::Calc::sum::Notify");
        let text = flat(&g.text);
        assert!(
            text.contains(
                "module get_precision { @final struct Request { unsigned long long request_id; };"
            ),
            "{}",
            g.text
        );
        assert!(
            text.contains(
                "module set_precision { @final struct Request { unsigned long long request_id; long value; };"
            ),
            "{}",
            g.text
        );
        assert!(
            text.contains("@final struct Notify { ::Demo::Terms input; string note; };"),
            "{}",
            g.text
        );
    }

    #[test]
    fn corbabridge_4_3_the_correlation_member_has_the_name_of_correlation_id() {
        let g = calc(&[Wanted {
            correlation: "call_id",
            ..wanted("add")
        }])
        .unwrap();
        let text = flat(&g.text);
        assert!(
            text.contains("@final struct Request { unsigned long long call_id; long long a;"),
            "{}",
            g.text
        );
        assert!(
            text.contains("@final struct Reply { unsigned long long call_id; Outcome status; };"),
            "{}",
            g.text
        );
        let e = calc(&[Wanted {
            correlation: "a",
            ..wanted("add")
        }])
        .unwrap_err();
        assert!(e.contains("the parameter a"), "{e}");
        let e = calc(&[Wanted {
            correlation: "not an id",
            ..wanted("add")
        }])
        .unwrap_err();
        assert!(e.contains("not an IDL identifier"), "{e}");
    }

    #[test]
    fn corbabridge_5_2_what_has_no_dds_form_is_refused() {
        let e = calc(&[wanted("name")]).unwrap_err();
        assert!(e.contains("wstring"), "{e}");
        let e = calc(&[wanted("missing")]).unwrap_err();
        assert!(e.contains("no operation missing"), "{e}");
        let e = calc(&[Wanted {
            repo_id: "IDL:Demo/Other:1.0",
            ..wanted("add")
        }])
        .unwrap_err();
        assert!(e.contains("no interface"), "{e}");
        let e = generate(
            "any.idl",
            "interface Bag { void put(in any item); };",
            &MemoryLoader::new(),
            &[Wanted {
                repo_id: "IDL:Bag:1.0",
                ..wanted("put")
            }],
        )
        .unwrap_err();
        assert!(e.contains("any"), "{e}");
        let e = generate(
            "broken.idl",
            "interface Bag { void put(in Missing item); };",
            &MemoryLoader::new(),
            &[],
        )
        .unwrap_err();
        assert!(e.starts_with("broken.idl: "), "{e}");
    }

    /// The members of `Overflow` (`long long limit; string why;`) behind
    /// the kind in a reply sample, and the GIOP body they become: the
    /// repository id `IDL:Demo/Calc/Overflow:1.0` (27 octets with its
    /// NUL, so the members begin at 31 and `limit` is padded to 32).
    #[test]
    fn corbabridge_5_2_a_user_exception_becomes_its_giop_body() {
        let g = calc(&[wanted("add")]).unwrap();
        let mut sample = Vec::new();
        sample.extend_from_slice(&7u64.to_le_bytes());
        sample.extend_from_slice(&1i32.to_le_bytes());
        sample.extend_from_slice(&0u32.to_le_bytes());
        sample.extend_from_slice(&900i64.to_le_bytes());
        sample.extend_from_slice(&4u32.to_le_bytes());
        sample.extend_from_slice(b"big\0");
        let body = user_exception_body(&g.ast, &g.ops[0].raises, &sample, false).unwrap();
        let mut expected = Vec::new();
        expected.extend_from_slice(&27u32.to_le_bytes());
        expected.extend_from_slice(b"IDL:Demo/Calc/Overflow:1.0\0");
        expected.push(0);
        expected.extend_from_slice(&900i64.to_le_bytes());
        expected.extend_from_slice(&4u32.to_le_bytes());
        expected.extend_from_slice(b"big\0");
        assert_eq!(body, expected);

        // The second kind has no members: the body is its repository id.
        sample[12..16].copy_from_slice(&1u32.to_le_bytes());
        let body = user_exception_body(&g.ast, &g.ops[0].raises, &sample[..16], false).unwrap();
        let mut expected = 26u32.to_le_bytes().to_vec();
        expected.extend_from_slice(b"IDL:Demo/DivideByZero:1.0\0");
        assert_eq!(body, expected);

        sample[12..16].copy_from_slice(&2u32.to_le_bytes());
        let e = user_exception_body(&g.ast, &g.ops[0].raises, &sample, false).unwrap_err();
        assert!(e.contains("exception_kind 2"), "{e}");
    }
}

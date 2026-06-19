// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CST → typed AST Builder (T5.2).
//!
//! Walks a [`CstNode`] (production IDs from [`crate::grammar::idl42`])
//! and produces a [`Specification`]. Each production has a
//! corresponding `build_<production>` function. List productions
//! (cons-recursive with an optional empty alternative) are converted
//! to a Vec via [`flatten_list`].
//!
//! # Error model
//! Builder errors indicate *bugs* — the CST comes from successful
//! recognition, so the structure should always match the grammar.
//! Nonetheless [`BuilderError`] instead of `panic!`, so defects are
//! catchable instead of process-aborting.

use crate::ast::types::*;
use crate::cst::node::{CstKind, CstNode};
use crate::errors::Span;
use crate::grammar::idl42::*;
use crate::grammar::{ProductionId, TokenKind};

/// Builder-specific error. Only occurs on a CST/grammar mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuilderError {
    /// Short description of the mismatch.
    pub message: String,
    /// Span in the source where the CST node begins.
    pub span: Span,
}

impl BuilderError {
    fn new(msg: impl Into<String>, span: Span) -> Self {
        Self {
            message: msg.into(),
            span,
        }
    }
}

impl core::fmt::Display for BuilderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "AST builder error @ {}: {}", self.span, self.message)
    }
}

impl std::error::Error for BuilderError {}

/// Builder-Result-Alias.
pub type BuildResult<T> = Result<T, BuilderError>;

/// Maximum nesting depth for modules + template-module body.
///
/// OMG IDL 4.2 imposes no hard upper bound, but all practically
/// occurring IDL bodies stay well below it. The cap protects against
/// stack overflow with adversarial inputs (TS-1 finding 1,
/// `docs/test-harness/plan.md`).
///
/// 256 is well above realistic use cases (typical
/// module hierarchies go 4-6 deep) and below the default
/// test-thread stack limit (2 MB / ~1 KB frame = ~2000 frames; per
/// module level we recurse over 3 functions → ~750 safe frames).
pub const MAX_MODULE_NESTING_DEPTH: usize = 256;

/// Top level: convert the CST root to a [`Specification`].
///
/// # Errors
/// [`BuilderError`] if the CST structure does not match the expected
/// grammar (should not occur with a correctly built CST).
pub fn build(root: &CstNode<'_>) -> BuildResult<Specification> {
    expect_production(root, ID_SPECIFICATION, "specification")?;
    let definitions = build_definition_list_from_spec(root)?;
    Ok(Specification {
        definitions,
        span: root.span,
    })
}

// ============================================================================
// CST-Helpers
// ============================================================================

fn expect_production(node: &CstNode<'_>, expected: ProductionId, name: &str) -> BuildResult<()> {
    if node.production() == Some(expected) {
        Ok(())
    } else {
        Err(BuilderError::new(
            format!("expected {name} (id {})", expected.0),
            node.span,
        ))
    }
}

fn first_internal_child<'a, 'src>(
    node: &'a CstNode<'src>,
    prod: ProductionId,
) -> Option<&'a CstNode<'src>> {
    node.children.iter().find(|c| c.production() == Some(prod))
}

fn require_internal_child<'a, 'src>(
    node: &'a CstNode<'src>,
    prod: ProductionId,
    name: &str,
) -> BuildResult<&'a CstNode<'src>> {
    first_internal_child(node, prod)
        .ok_or_else(|| BuilderError::new(format!("missing child production {name}"), node.span))
}

/// Collects all token-leaf texts under `node` in source order,
/// whitespace-separated. Serves to reconstruct the raw construct for
/// delta-inserted [`VendorExtension`] nodes (whose internal `text` can be
/// empty), so consumers like the key-pragma apply can extract type + fields
/// from it.
fn collect_token_texts(node: &CstNode<'_>) -> String {
    /// zerodds-lint: recursion-depth 64 (CST-Walk; bounded by IDL nesting)
    fn walk(node: &CstNode<'_>, out: &mut String) {
        if matches!(node.kind, CstKind::Token(_)) {
            if !node.text.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(node.text);
            }
            return;
        }
        for c in &node.children {
            walk(c, out);
        }
    }
    let mut out = String::new();
    walk(node, &mut out);
    out
}

fn first_token_text<'src>(node: &CstNode<'src>, kind: TokenKind) -> Option<&'src str> {
    node.children.iter().find_map(|c| match c.kind {
        CstKind::Token(k) if k == kind => Some(c.text),
        _ => None,
    })
}

/// Recursively searches for a `Keyword(kw)` token in the entire subtree.
/// Necessary for productions like value_inheritance_spec, whose
/// keyword tokens sit inside repeat/choice wrappers.
///
/// zerodds-lint: recursion-depth 64 (CST-Walk; bounded by IDL nesting)
fn has_keyword_in_tree(node: &CstNode<'_>, kw: &'static str) -> bool {
    for c in &node.children {
        if let CstKind::Token(TokenKind::Keyword(k)) = c.kind {
            if k == kw {
                return true;
            }
        }
        if c.is_internal() && has_keyword_in_tree(c, kw) {
            return true;
        }
    }
    false
}

/// Finds the first identifier in a node's children — either
/// as a direct `Ident` token or via an `<identifier>` production wrapper.
fn first_ident_text<'src>(node: &CstNode<'src>) -> Option<(&'src str, Span)> {
    for c in &node.children {
        if let CstKind::Token(TokenKind::Ident) = c.kind {
            return Some((c.text, c.span));
        }
        if c.production() == Some(ID_IDENTIFIER) {
            return ident_token_in(c);
        }
    }
    None
}

fn ident_token_in<'src>(ident_node: &CstNode<'src>) -> Option<(&'src str, Span)> {
    ident_node.children.iter().find_map(|c| {
        if let CstKind::Token(TokenKind::Ident) = c.kind {
            Some((c.text, c.span))
        } else {
            None
        }
    })
}

fn token_text_of<'src>(child: &CstNode<'src>) -> Option<&'src str> {
    if matches!(child.kind, CstKind::Token(_)) {
        Some(child.text)
    } else {
        None
    }
}

/// Flatten a cons-recursive list.
///
/// Assumption: the production has alternatives of the form
/// `single ::= item` and `cons ::= list separator? item`. The
/// recursion is always in the *first* child.
fn flatten_list<'a, 'src>(node: &'a CstNode<'src>) -> Vec<&'a CstNode<'src>> {
    let mut out = Vec::new();
    flatten_list_into(node, &mut out);
    out
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn flatten_list_into<'a, 'src>(node: &'a CstNode<'src>, out: &mut Vec<&'a CstNode<'src>>) {
    if node.children.is_empty() {
        return; // empty alternative
    }
    let first = &node.children[0];
    if first.production() == node.production() {
        flatten_list_into(first, out);
        for child in &node.children[1..] {
            if child.is_internal() {
                out.push(child);
            }
        }
    } else {
        for child in &node.children {
            if child.is_internal() {
                out.push(child);
            }
        }
    }
}

// ============================================================================
// Specification + Definition
// ============================================================================

fn build_definition_list_from_spec(spec: &CstNode<'_>) -> BuildResult<Vec<Definition>> {
    // <specification> ::= <definition_list>
    let list = require_internal_child(spec, ID_DEFINITION_LIST, "definition_list")?;
    build_definition_list(list, 0)
}

fn build_definition_list(list: &CstNode<'_>, depth: usize) -> BuildResult<Vec<Definition>> {
    if depth > MAX_MODULE_NESTING_DEPTH {
        return Err(BuilderError::new(
            format!(
                "module/template nesting exceeds {MAX_MODULE_NESTING_DEPTH} — refusing to build to \
                 protect against stack overflow"
            ),
            list.span,
        ));
    }
    let items = flatten_list(list);
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        if it.production() == Some(ID_DEFINITION) {
            out.push(build_definition(it, depth)?);
        }
    }
    Ok(out)
}

/// zerodds-lint: recursion-depth 64 (a template module body can contain definitions recursively; bounded by IDL nesting)
fn build_definition(node: &CstNode<'_>, depth: usize) -> BuildResult<Definition> {
    // <definition> has 7 alternatives, each beginning with
    // annotation_appl_seq + inner_dcl + ";".
    let annotations = build_annotation_seq(node)?;
    // Find the inner production — one of the known decls.
    for c in &node.children {
        match c.production() {
            Some(p) if p == ID_MODULE_DCL => {
                let mut m = build_module_dcl(c, node.span, depth + 1)?;
                m.annotations = annotations;
                return Ok(Definition::Module(m));
            }
            Some(p) if p == ID_TYPE_DCL => {
                let mut td = build_type_dcl(c)?;
                set_type_dcl_annotations(&mut td, annotations);
                return Ok(Definition::Type(td));
            }
            Some(p) if p == ID_CONST_DCL => {
                let mut cd = build_const_dcl(c)?;
                cd.annotations = annotations;
                return Ok(Definition::Const(cd));
            }
            Some(p) if p == ID_EXCEPT_DCL => {
                let mut e = build_except_dcl(c)?;
                e.annotations = annotations;
                return Ok(Definition::Except(e));
            }
            Some(p) if p == ID_INTERFACE_DCL => {
                let mut i = build_interface_dcl(c)?;
                set_interface_annotations(&mut i, annotations);
                return Ok(Definition::Interface(i));
            }
            Some(p) if p == ID_VALUE_BOX_DCL => {
                let mut v = build_value_box_dcl(c)?;
                v.annotations = annotations;
                return Ok(Definition::ValueBox(v));
            }
            Some(p) if p == ID_VALUE_FORWARD_DCL => {
                return Ok(Definition::ValueForward(build_value_forward_dcl(c)?));
            }
            Some(p) if p == ID_VALUE_DEF => {
                let mut v = build_value_def(c)?;
                v.annotations = annotations;
                return Ok(Definition::ValueDef(v));
            }
            Some(p) if p == ID_TYPE_ID_DCL => {
                return Ok(Definition::TypeId(build_type_id_dcl(c)?));
            }
            Some(p) if p == ID_TYPE_PREFIX_DCL => {
                return Ok(Definition::TypePrefix(build_type_prefix_dcl(c)?));
            }
            Some(p) if p == ID_IMPORT_DCL => {
                return Ok(Definition::Import(build_import_dcl(c)?));
            }
            Some(p) if p == ID_COMPONENT_DCL => {
                let mut cd = build_component_dcl(c)?;
                set_component_annotations(&mut cd, annotations);
                return Ok(Definition::Component(cd));
            }
            Some(p) if p == ID_HOME_DCL => {
                let mut h = build_home_dcl(c)?;
                set_home_annotations(&mut h, annotations);
                return Ok(Definition::Home(h));
            }
            Some(p) if p == ID_EVENT_DCL => {
                let mut ev = build_event_dcl(c)?;
                set_event_annotations(&mut ev, annotations);
                return Ok(Definition::Event(ev));
            }
            Some(p) if p == ID_PORTTYPE_DCL => {
                return Ok(Definition::Porttype(build_porttype_dcl(c)?));
            }
            Some(p) if p == ID_CONNECTOR_DCL => {
                return Ok(Definition::Connector(build_connector_dcl(c)?));
            }
            Some(p) if p == ID_TEMPLATE_MODULE_DCL => {
                return Ok(Definition::TemplateModule(build_template_module_dcl(
                    c,
                    depth + 1,
                )?));
            }
            Some(p) if p == ID_TEMPLATE_MODULE_INST => {
                return Ok(Definition::TemplateModuleInst(build_template_module_inst(
                    c,
                )?));
            }
            Some(p) if p == ID_ANNOTATION_DCL => {
                return Ok(Definition::Annotation(build_annotation_dcl(c)?));
            }
            _ => {}
        }
    }
    // Fallback for delta-inserted constructs (T6.5): if no
    // known production matched, capture the innermost internal-child
    // production as a VendorExtension. Stays tolerant without every
    // delta bringing a dedicated AST builder.
    if let Some(c) = node.children.iter().find(|c| c.is_internal()) {
        let production_name = format!(
            "production_id={}",
            c.production().map(|p| p.0).unwrap_or(u32::MAX)
        );
        return Ok(Definition::VendorExtension(VendorExtension {
            production_name,
            raw: collect_token_texts(c),
            span: node.span,
        }));
    }
    Err(BuilderError::new("unrecognized definition kind", node.span))
}

fn set_type_dcl_annotations(td: &mut TypeDecl, annotations: Vec<Annotation>) {
    match td {
        TypeDecl::Constr(c) => set_constr_annotations(c, annotations),
        TypeDecl::Typedef(t) => t.annotations = annotations,
        // native carries no annotations in our model (opaque type).
        TypeDecl::Native(_) => {}
    }
}

fn set_constr_annotations(c: &mut ConstrTypeDecl, annotations: Vec<Annotation>) {
    match c {
        ConstrTypeDecl::Struct(StructDcl::Def(s)) => s.annotations = annotations,
        ConstrTypeDecl::Struct(StructDcl::Forward(_)) => {}
        ConstrTypeDecl::Union(UnionDcl::Def(u)) => u.annotations = annotations,
        ConstrTypeDecl::Union(UnionDcl::Forward(_)) => {}
        ConstrTypeDecl::Enum(e) => e.annotations = annotations,
        ConstrTypeDecl::Bitset(b) => b.annotations = annotations,
        ConstrTypeDecl::Bitmask(b) => b.annotations = annotations,
    }
}

fn set_interface_annotations(i: &mut InterfaceDcl, annotations: Vec<Annotation>) {
    if let InterfaceDcl::Def(def) = i {
        def.annotations = annotations;
    }
}

// ============================================================================
// Module
// ============================================================================

fn build_module_dcl(node: &CstNode<'_>, outer_span: Span, depth: usize) -> BuildResult<ModuleDef> {
    // module <ident> { <definition_list> }
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("module without identifier", node.span))?;
    let definitions = match first_internal_child(node, ID_DEFINITION_LIST) {
        Some(list) => build_definition_list(list, depth)?,
        None => Vec::new(),
    };
    Ok(ModuleDef {
        name: Identifier::new(text, span),
        definitions,
        annotations: Vec::new(),
        span: outer_span,
    })
}

// ============================================================================
// Type-Decl (Constr | Typedef)
// ============================================================================

fn build_type_dcl(node: &CstNode<'_>) -> BuildResult<TypeDecl> {
    if let Some(c) = first_internal_child(node, ID_CONSTR_TYPE_DCL) {
        Ok(TypeDecl::Constr(build_constr_type_dcl(c)?))
    } else if let Some(c) = first_internal_child(node, ID_TYPEDEF_DCL) {
        Ok(TypeDecl::Typedef(build_typedef_dcl(c)?))
    } else if let Some(c) = first_internal_child(node, ID_NATIVE_DCL) {
        Ok(TypeDecl::Native(build_native_dcl(c)?))
    } else {
        Err(BuilderError::new(
            "type_dcl without constr/typedef/native",
            node.span,
        ))
    }
}

/// `native <simple_declarator>;` (§7.4.1.3 Rule 61).
fn build_native_dcl(node: &CstNode<'_>) -> BuildResult<NativeDecl> {
    let sd = require_internal_child(node, ID_SIMPLE_DECLARATOR, "simple_declarator")?;
    let (text, span) = first_ident_text(sd)
        .ok_or_else(|| BuilderError::new("native_dcl without ident", sd.span))?;
    Ok(NativeDecl {
        name: Identifier::new(text, span),
        span: node.span,
    })
}

fn build_constr_type_dcl(node: &CstNode<'_>) -> BuildResult<ConstrTypeDecl> {
    if let Some(c) = first_internal_child(node, ID_STRUCT_DCL) {
        Ok(ConstrTypeDecl::Struct(build_struct_dcl(c)?))
    } else if let Some(c) = first_internal_child(node, ID_UNION_DCL) {
        Ok(ConstrTypeDecl::Union(build_union_dcl(c)?))
    } else if let Some(c) = first_internal_child(node, ID_ENUM_DCL) {
        Ok(ConstrTypeDecl::Enum(build_enum_dcl(c)?))
    } else if let Some(c) = first_internal_child(node, ID_BITSET_DCL) {
        Ok(ConstrTypeDecl::Bitset(build_bitset_dcl(c)?))
    } else if let Some(c) = first_internal_child(node, ID_BITMASK_DCL) {
        Ok(ConstrTypeDecl::Bitmask(build_bitmask_dcl(c)?))
    } else {
        Err(BuilderError::new(
            "constr_type_dcl without struct/union/enum/bitset/bitmask",
            node.span,
        ))
    }
}

// ============================================================================
// Struct
// ============================================================================

fn build_struct_dcl(node: &CstNode<'_>) -> BuildResult<StructDcl> {
    if let Some(c) = first_internal_child(node, ID_STRUCT_DEF) {
        Ok(StructDcl::Def(build_struct_def(c)?))
    } else if let Some(c) = first_internal_child(node, ID_STRUCT_FORWARD_DCL) {
        Ok(StructDcl::Forward(build_struct_forward(c)?))
    } else {
        Err(BuilderError::new(
            "struct_dcl without def/forward",
            node.span,
        ))
    }
}

fn build_struct_def(node: &CstNode<'_>) -> BuildResult<StructDef> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("struct without identifier", node.span))?;
    let base = first_internal_child(node, ID_SCOPED_NAME)
        .map(build_scoped_name)
        .transpose()?;
    let members = match first_internal_child(node, ID_MEMBER_LIST) {
        Some(list) => {
            let items = flatten_list(list);
            items
                .iter()
                .map(|it| build_member(it))
                .collect::<Result<_, _>>()?
        }
        None => Vec::new(),
    };
    Ok(StructDef {
        name: Identifier::new(text, span),
        base,
        members,
        annotations: Vec::new(),
        span: node.span,
    })
}

fn build_struct_forward(node: &CstNode<'_>) -> BuildResult<StructForwardDecl> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("struct fwd without identifier", node.span))?;
    Ok(StructForwardDecl {
        name: Identifier::new(text, span),
        span: node.span,
    })
}

fn build_member(node: &CstNode<'_>) -> BuildResult<Member> {
    let annotations = build_annotation_seq(node)?;
    let ts = require_internal_child(node, ID_TYPE_SPEC, "type_spec")?;
    let dl = require_internal_child(node, ID_DECLARATORS, "declarators")?;
    Ok(Member {
        type_spec: build_type_spec(ts)?,
        declarators: build_declarator_list(dl)?,
        annotations,
        span: node.span,
    })
}

fn build_declarator_list(node: &CstNode<'_>) -> BuildResult<Vec<Declarator>> {
    let items = flatten_list(node);
    items.iter().map(|it| build_declarator(it)).collect()
}

fn build_declarator(node: &CstNode<'_>) -> BuildResult<Declarator> {
    if let Some(arr) = first_internal_child(node, ID_ARRAY_DECLARATOR) {
        return Ok(Declarator::Array(build_array_declarator(arr)?));
    }
    if let Some(simple) = first_internal_child(node, ID_SIMPLE_DECLARATOR) {
        let (text, span) = first_ident_text(simple)
            .ok_or_else(|| BuilderError::new("simple_declarator without ident", simple.span))?;
        return Ok(Declarator::Simple(Identifier::new(text, span)));
    }
    Err(BuilderError::new(
        "declarator without alternative",
        node.span,
    ))
}

fn build_array_declarator(node: &CstNode<'_>) -> BuildResult<ArrayDeclarator> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("array_declarator without ident", node.span))?;
    let mut sizes = Vec::new();
    if let Some(list) = first_internal_child(node, ID_FIXED_ARRAY_SIZES) {
        for sz in flatten_list(list) {
            if sz.production() == Some(ID_FIXED_ARRAY_SIZE) {
                if let Some(pic) = first_internal_child(sz, ID_POSITIVE_INT_CONST) {
                    sizes.push(build_positive_int_const(pic)?);
                }
            }
        }
    }
    Ok(ArrayDeclarator {
        name: Identifier::new(text, span),
        sizes,
        span: node.span,
    })
}

// ============================================================================
// Union
// ============================================================================

fn build_union_dcl(node: &CstNode<'_>) -> BuildResult<UnionDcl> {
    if let Some(c) = first_internal_child(node, ID_UNION_DEF) {
        Ok(UnionDcl::Def(build_union_def(c)?))
    } else if let Some(c) = first_internal_child(node, ID_UNION_FORWARD_DCL) {
        let (text, span) =
            first_ident_text(c).ok_or_else(|| BuilderError::new("union fwd no ident", c.span))?;
        Ok(UnionDcl::Forward(UnionForwardDecl {
            name: Identifier::new(text, span),
            span: c.span,
        }))
    } else {
        Err(BuilderError::new(
            "union_dcl without def/forward",
            node.span,
        ))
    }
}

fn build_union_def(node: &CstNode<'_>) -> BuildResult<UnionDef> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("union without identifier", node.span))?;
    let switch = require_internal_child(node, ID_SWITCH_TYPE_SPEC, "switch_type_spec")?;
    let switch_type = build_switch_type_spec(switch)?;
    let cases = match first_internal_child(node, ID_SWITCH_BODY) {
        Some(body) => flatten_list(body)
            .iter()
            .filter(|c| c.production() == Some(ID_CASE))
            .map(|c| build_case(c))
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    Ok(UnionDef {
        name: Identifier::new(text, span),
        switch_type,
        cases,
        annotations: Vec::new(),
        span: node.span,
    })
}

fn build_switch_type_spec(node: &CstNode<'_>) -> BuildResult<SwitchTypeSpec> {
    if let Some(int) = first_internal_child(node, ID_INTEGER_TYPE) {
        return Ok(SwitchTypeSpec::Integer(build_integer_type(int)?));
    }
    if first_internal_child(node, ID_CHAR_TYPE).is_some() {
        return Ok(SwitchTypeSpec::Char);
    }
    if first_internal_child(node, ID_BOOLEAN_TYPE).is_some() {
        return Ok(SwitchTypeSpec::Boolean);
    }
    if first_internal_child(node, ID_OCTET_TYPE).is_some() {
        return Ok(SwitchTypeSpec::Octet);
    }
    if let Some(sn) = first_internal_child(node, ID_SCOPED_NAME) {
        return Ok(SwitchTypeSpec::Scoped(build_scoped_name(sn)?));
    }
    Err(BuilderError::new(
        "switch_type_spec unrecognized",
        node.span,
    ))
}

fn build_case(node: &CstNode<'_>) -> BuildResult<Case> {
    let annotations = build_annotation_seq(node)?;
    let labels = match first_internal_child(node, ID_CASE_LABELS) {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_CASE_LABEL))
            .map(|c| build_case_label(c))
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    let element_node = require_internal_child(node, ID_ELEMENT_SPEC, "element_spec")?;
    let element = build_element_spec(element_node)?;
    Ok(Case {
        labels,
        element,
        annotations,
        span: node.span,
    })
}

fn build_case_label(node: &CstNode<'_>) -> BuildResult<CaseLabel> {
    // case_label has alts: case + const_expr + ":" | "default" + ":"
    if let Some(expr) = first_internal_child(node, ID_CONST_EXPR) {
        Ok(CaseLabel::Value(build_const_expr(expr)?))
    } else {
        Ok(CaseLabel::Default)
    }
}

fn build_element_spec(node: &CstNode<'_>) -> BuildResult<ElementSpec> {
    let annotations = build_annotation_seq(node)?;
    let ts = require_internal_child(node, ID_TYPE_SPEC, "type_spec")?;
    let decl = require_internal_child(node, ID_DECLARATOR, "declarator")?;
    Ok(ElementSpec {
        type_spec: build_type_spec(ts)?,
        declarator: build_declarator(decl)?,
        annotations,
        span: node.span,
    })
}

// ============================================================================
// Enum
// ============================================================================

fn build_enum_dcl(node: &CstNode<'_>) -> BuildResult<EnumDef> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("enum without identifier", node.span))?;
    let enumerators: Vec<Enumerator> = match first_internal_child(node, ID_ENUMERATOR_LIST) {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_ENUMERATOR))
            .map(|c| build_enumerator(c))
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    // §7.4.1.4.4.4.3 — Enum-Limit 2^32 enumerators (Spec-Constraint).
    // Never reached in practice (4.3 billion values), but a spec-conform check.
    if enumerators.len() > (u32::MAX as usize) {
        return Err(BuilderError::new(
            format!(
                "enum '{text}' exceeds spec limit of 2^32 enumerators ({} given)",
                enumerators.len()
            ),
            node.span,
        ));
    }
    Ok(EnumDef {
        name: Identifier::new(text, span),
        enumerators,
        annotations: Vec::new(),
        span: node.span,
    })
}

fn build_enumerator(node: &CstNode<'_>) -> BuildResult<Enumerator> {
    let annotations = build_annotation_seq(node)?;
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("enumerator without ident", node.span))?;
    Ok(Enumerator {
        name: Identifier::new(text, span),
        annotations,
        span: node.span,
    })
}

// ============================================================================
// Bitset / Bitmask
// ============================================================================

fn build_bitset_dcl(node: &CstNode<'_>) -> BuildResult<BitsetDecl> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("bitset without ident", node.span))?;
    let base = first_internal_child(node, ID_SCOPED_NAME)
        .map(build_scoped_name)
        .transpose()?;
    let bitfields = match first_internal_child(node, ID_BITFIELD_LIST) {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_BITFIELD))
            .map(|c| build_bitfield(c))
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    Ok(BitsetDecl {
        name: Identifier::new(text, span),
        base,
        bitfields,
        annotations: Vec::new(),
        span: node.span,
    })
}

fn build_bitfield(node: &CstNode<'_>) -> BuildResult<Bitfield> {
    let annotations = build_annotation_seq(node)?;
    let spec_node = require_internal_child(node, ID_BITFIELD_SPEC, "bitfield_spec")?;
    let spec = build_bitfield_spec(spec_node)?;
    let name = first_ident_text(node).map(|(t, s)| Identifier::new(t, s));
    Ok(Bitfield {
        spec,
        name,
        annotations,
        span: node.span,
    })
}

fn build_bitfield_spec(node: &CstNode<'_>) -> BuildResult<BitfieldSpec> {
    let width = first_internal_child(node, ID_POSITIVE_INT_CONST)
        .ok_or_else(|| BuilderError::new("bitfield_spec without width", node.span))?;
    let dest_type = first_internal_child(node, ID_BASE_TYPE_SPEC)
        .map(build_base_type_spec)
        .transpose()?
        .and_then(|t| match t {
            TypeSpec::Primitive(p) => Some(p),
            _ => None,
        });
    Ok(BitfieldSpec {
        width: build_positive_int_const(width)?,
        dest_type,
        span: node.span,
    })
}

/// `<positive_int_const>` wraps `<integer_literal>`.
fn build_positive_int_const(node: &CstNode<'_>) -> BuildResult<ConstExpr> {
    let lit = first_internal_child(node, ID_INTEGER_LITERAL).ok_or_else(|| {
        BuilderError::new("positive_int_const without integer_literal", node.span)
    })?;
    Ok(ConstExpr::Literal(build_literal(lit)?))
}

fn build_bitmask_dcl(node: &CstNode<'_>) -> BuildResult<BitmaskDecl> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("bitmask without ident", node.span))?;
    let mut values = Vec::new();
    if let Some(list) = first_internal_child(node, ID_BIT_VALUE_LIST) {
        // bit_value_list: cons-style without an explicit item production —
        // each entry is an annotation_appl_seq + identifier.
        // Walking: deepest left-most is the first element.
        collect_bit_values(list, &mut values)?;
    }
    Ok(BitmaskDecl {
        name: Identifier::new(text, span),
        values,
        annotations: Vec::new(),
        span: node.span,
    })
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn collect_bit_values(list: &CstNode<'_>, out: &mut Vec<BitValue>) -> BuildResult<()> {
    if list.children.is_empty() {
        return Ok(());
    }
    let first = &list.children[0];
    if first.production() == list.production() {
        collect_bit_values(first, out)?;
    } else {
        // base case: first annotation_appl_seq + ident
        out.push(extract_bit_value(list)?);
        return Ok(());
    }
    // cons case: the rest is "," annotation_appl_seq <ident>
    out.push(extract_bit_value_after_first(list)?);
    Ok(())
}

fn extract_bit_value(node: &CstNode<'_>) -> BuildResult<BitValue> {
    let annotations = build_annotation_seq(node)?;
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("bit_value without ident", node.span))?;
    Ok(BitValue {
        name: Identifier::new(text, span),
        annotations,
        span: node.span,
    })
}

fn extract_bit_value_after_first(node: &CstNode<'_>) -> BuildResult<BitValue> {
    // Children: [list-recursion, ",", annotation_appl_seq, <ident>]
    let mut iter = node.children.iter();
    let _list = iter.next();
    let _comma = iter.next();
    let mut annotations = Vec::new();
    let mut ident: Option<(&str, Span)> = None;
    for c in iter {
        if c.production() == Some(ID_ANNOTATION_APPL_SEQ) {
            annotations = build_annotation_seq_node(c)?;
        } else if let CstKind::Token(TokenKind::Ident) = c.kind {
            ident = Some((c.text, c.span));
        } else if c.production() == Some(ID_IDENTIFIER) {
            ident = ident_token_in(c);
        }
    }
    let (text, span) =
        ident.ok_or_else(|| BuilderError::new("bit_value cons without ident", node.span))?;
    Ok(BitValue {
        name: Identifier::new(text, span),
        annotations,
        span: node.span,
    })
}

// ============================================================================
// Typedef
// ============================================================================

fn build_typedef_dcl(node: &CstNode<'_>) -> BuildResult<TypedefDecl> {
    let td = require_internal_child(node, ID_TYPE_DECLARATOR, "type_declarator")?;
    // type_declarator has two alternatives: simple_type_spec or
    // template_type_spec (each + any_declarators). NO type_spec wrapper.
    let type_spec = if let Some(s) = first_internal_child(td, ID_SIMPLE_TYPE_SPEC) {
        build_simple_type_spec(s)?
    } else if let Some(t) = first_internal_child(td, ID_TEMPLATE_TYPE_SPEC) {
        build_template_type_spec(t)?
    } else {
        return Err(BuilderError::new(
            "type_declarator without simple/template type_spec",
            td.span,
        ));
    };
    let ad = require_internal_child(td, ID_ANY_DECLARATORS, "any_declarators")?;
    let mut declarators = Vec::new();
    for it in flatten_list(ad) {
        if it.production() == Some(ID_ANY_DECLARATOR) {
            declarators.push(build_any_declarator(it)?);
        }
    }
    Ok(TypedefDecl {
        type_spec,
        declarators,
        annotations: Vec::new(),
        span: node.span,
    })
}

fn build_any_declarator(node: &CstNode<'_>) -> BuildResult<Declarator> {
    if let Some(arr) = first_internal_child(node, ID_ARRAY_DECLARATOR) {
        return Ok(Declarator::Array(build_array_declarator(arr)?));
    }
    if let Some(simple) = first_internal_child(node, ID_SIMPLE_DECLARATOR) {
        let (text, span) = first_ident_text(simple)
            .ok_or_else(|| BuilderError::new("simple_declarator without ident", simple.span))?;
        return Ok(Declarator::Simple(Identifier::new(text, span)));
    }
    // any_declarator could directly contain ident
    if let Some((text, span)) = first_ident_text(node) {
        return Ok(Declarator::Simple(Identifier::new(text, span)));
    }
    Err(BuilderError::new("any_declarator without alt", node.span))
}

// ============================================================================
// Const-Decl + ConstExpr
// ============================================================================

fn build_const_dcl(node: &CstNode<'_>) -> BuildResult<ConstDecl> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("const_dcl without ident", node.span))?;
    let ct = require_internal_child(node, ID_CONST_TYPE, "const_type")?;
    let ce = require_internal_child(node, ID_CONST_EXPR, "const_expr")?;
    Ok(ConstDecl {
        name: Identifier::new(text, span),
        type_: build_const_type(ct)?,
        value: build_const_expr(ce)?,
        annotations: Vec::new(),
        span: node.span,
    })
}

fn build_const_type(node: &CstNode<'_>) -> BuildResult<ConstType> {
    if let Some(c) = first_internal_child(node, ID_INTEGER_TYPE) {
        return Ok(ConstType::Integer(build_integer_type(c)?));
    }
    if let Some(c) = first_internal_child(node, ID_FLOATING_PT_TYPE) {
        return Ok(ConstType::Floating(build_floating_type(c)?));
    }
    if first_internal_child(node, ID_CHAR_TYPE).is_some() {
        return Ok(ConstType::Char);
    }
    if first_internal_child(node, ID_WIDE_CHAR_TYPE).is_some() {
        return Ok(ConstType::WideChar);
    }
    if first_internal_child(node, ID_BOOLEAN_TYPE).is_some() {
        return Ok(ConstType::Boolean);
    }
    if first_internal_child(node, ID_OCTET_TYPE).is_some() {
        return Ok(ConstType::Octet);
    }
    if let Some(c) = first_internal_child(node, ID_STRING_TYPE) {
        let bound = first_internal_child(c, ID_POSITIVE_INT_CONST).is_some();
        let _ = bound; // bound semantics held by StringType, ConstType only signals kind
        return Ok(ConstType::String { wide: false });
    }
    if first_internal_child(node, ID_WIDE_STRING_TYPE).is_some() {
        return Ok(ConstType::String { wide: true });
    }
    if first_internal_child(node, ID_FIXED_PT_TYPE).is_some() {
        return Ok(ConstType::Fixed);
    }
    if let Some(sn) = first_internal_child(node, ID_SCOPED_NAME) {
        return Ok(ConstType::Scoped(build_scoped_name(sn)?));
    }
    Err(BuilderError::new("const_type unrecognized", node.span))
}

fn build_const_expr(node: &CstNode<'_>) -> BuildResult<ConstExpr> {
    // const_expr ::= or_expr — just pass through.
    let or = require_internal_child(node, ID_OR_EXPR, "or_expr")?;
    build_binary_chain(
        or,
        ID_OR_EXPR,
        ID_XOR_EXPR,
        "|",
        BinaryOp::Or,
        build_xor_expr,
    )
}

fn build_xor_expr(node: &CstNode<'_>) -> BuildResult<ConstExpr> {
    build_binary_chain(
        node,
        ID_XOR_EXPR,
        ID_AND_EXPR,
        "^",
        BinaryOp::Xor,
        build_and_expr,
    )
}

fn build_and_expr(node: &CstNode<'_>) -> BuildResult<ConstExpr> {
    build_binary_chain(
        node,
        ID_AND_EXPR,
        ID_SHIFT_EXPR,
        "&",
        BinaryOp::And,
        build_shift_expr,
    )
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn build_shift_expr(node: &CstNode<'_>) -> BuildResult<ConstExpr> {
    // shift_expr has TWO operators (>> and <<). Special case.
    if node.children.len() == 1 {
        return build_add_expr(&node.children[0]);
    }
    if node.children.is_empty() {
        return Err(BuilderError::new("empty shift_expr", node.span));
    }
    // children: [shift_expr, "<<", add_expr] OR [shift_expr, ">", ">", add_expr]
    let lhs_node = &node.children[0];
    let lhs = if lhs_node.production() == Some(ID_SHIFT_EXPR) {
        build_shift_expr(lhs_node)?
    } else {
        build_add_expr(lhs_node)?
    };
    // Find op + rhs
    let mut iter = node.children.iter().skip(1).peekable();
    let mut acc = lhs;
    while let Some(tok) = iter.next() {
        let op = match token_text_of(tok) {
            Some("<<") => BinaryOp::Shl,
            Some(">") => {
                // double-> for ">>"
                if iter.peek().and_then(|n| token_text_of(n)) == Some(">") {
                    iter.next();
                }
                BinaryOp::Shr
            }
            _ => continue,
        };
        let rhs_node = iter
            .next()
            .ok_or_else(|| BuilderError::new("shift_expr without rhs", node.span))?;
        let rhs = build_add_expr(rhs_node)?;
        acc = ConstExpr::Binary {
            op,
            lhs: Box::new(acc),
            rhs: Box::new(rhs),
            span: node.span,
        };
    }
    Ok(acc)
}

fn build_add_expr(node: &CstNode<'_>) -> BuildResult<ConstExpr> {
    build_binary_chain_dual(
        node,
        ID_ADD_EXPR,
        ID_MULT_EXPR,
        ("+", BinaryOp::Add),
        ("-", BinaryOp::Sub),
        build_mult_expr,
    )
}

fn build_mult_expr(node: &CstNode<'_>) -> BuildResult<ConstExpr> {
    build_binary_chain_triple(
        node,
        ID_MULT_EXPR,
        ID_UNARY_EXPR,
        ("*", BinaryOp::Mul),
        ("/", BinaryOp::Div),
        ("%", BinaryOp::Mod),
        build_unary_expr,
    )
}

fn build_unary_expr(node: &CstNode<'_>) -> BuildResult<ConstExpr> {
    // unary_expr ::= (+ | - | ~)? primary_expr
    let mut op: Option<UnaryOp> = None;
    let mut primary: Option<&CstNode<'_>> = None;
    for c in &node.children {
        if c.production() == Some(ID_PRIMARY_EXPR) {
            primary = Some(c);
        } else if let CstKind::Token(tk) = &c.kind {
            match tk {
                TokenKind::Punct("+") => op = Some(UnaryOp::Plus),
                TokenKind::Punct("-") => op = Some(UnaryOp::Minus),
                TokenKind::Punct("~") => op = Some(UnaryOp::BitNot),
                _ => {}
            }
        }
    }
    let prim = primary.ok_or_else(|| BuilderError::new("unary_expr without primary", node.span))?;
    let base = build_primary_expr(prim)?;
    Ok(match op {
        Some(op) => ConstExpr::Unary {
            op,
            operand: Box::new(base),
            span: node.span,
        },
        None => base,
    })
}

fn build_primary_expr(node: &CstNode<'_>) -> BuildResult<ConstExpr> {
    if let Some(lit) = first_internal_child(node, ID_LITERAL) {
        return Ok(ConstExpr::Literal(build_literal(lit)?));
    }
    if let Some(sn) = first_internal_child(node, ID_SCOPED_NAME) {
        return Ok(ConstExpr::Scoped(build_scoped_name(sn)?));
    }
    if let Some(ce) = first_internal_child(node, ID_CONST_EXPR) {
        // parenthesized
        return build_const_expr(ce);
    }
    Err(BuilderError::new("primary_expr unrecognized", node.span))
}

fn build_literal(node: &CstNode<'_>) -> BuildResult<Literal> {
    // <literal> wraps a sub-production wrapper (e.g.
    // <integer_literal>) which in turn carries the actual
    // token leaf. Boolean is a special case: keyword TRUE/FALSE.
    if let Some((kind, text, span)) = find_literal_token(node) {
        let lk = match kind {
            TokenKind::IntegerLiteral => LiteralKind::Integer,
            TokenKind::FloatLiteral => LiteralKind::Floating,
            TokenKind::FixedPtLiteral => LiteralKind::Fixed,
            TokenKind::CharLiteral => LiteralKind::Char,
            TokenKind::WideCharLiteral => LiteralKind::WideChar,
            TokenKind::StringLiteral => LiteralKind::String,
            TokenKind::WideStringLiteral => LiteralKind::WideString,
            TokenKind::BoolLiteral => LiteralKind::Boolean,
            TokenKind::Keyword("TRUE" | "FALSE") => LiteralKind::Boolean,
            _ => return Err(BuilderError::new("literal token unrecognized", node.span)),
        };
        return Ok(Literal {
            kind: lk,
            raw: text.to_string(),
            span,
        });
    }
    Err(BuilderError::new("literal without token", node.span))
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn find_literal_token<'src>(node: &CstNode<'src>) -> Option<(TokenKind, &'src str, Span)> {
    for c in &node.children {
        match c.kind {
            CstKind::Token(k) => return Some((k, c.text, c.span)),
            CstKind::Internal { .. } => {
                if let Some(found) = find_literal_token(c) {
                    return Some(found);
                }
            }
            CstKind::Error => {}
        }
    }
    None
}

// Generic helper for left-associative single-op binary chains.
/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn build_binary_chain<F>(
    node: &CstNode<'_>,
    self_id: ProductionId,
    next_id: ProductionId,
    op_text: &str,
    op_kind: BinaryOp,
    next: F,
) -> BuildResult<ConstExpr>
where
    F: Fn(&CstNode<'_>) -> BuildResult<ConstExpr> + Copy,
{
    if node.children.len() == 1 {
        return next(&node.children[0]);
    }
    let _ = self_id;
    let _ = next_id;
    // children: [self_id, op_token, next_id]
    let lhs = if node.children[0].production() == node.production() {
        build_binary_chain(&node.children[0], self_id, next_id, op_text, op_kind, next)?
    } else {
        next(&node.children[0])?
    };
    let mut iter = node.children.iter().skip(1).peekable();
    let mut acc = lhs;
    while let Some(tok) = iter.next() {
        if token_text_of(tok) == Some(op_text) {
            let rhs_node = iter
                .next()
                .ok_or_else(|| BuilderError::new("binary chain missing rhs", node.span))?;
            let rhs = next(rhs_node)?;
            acc = ConstExpr::Binary {
                op: op_kind,
                lhs: Box::new(acc),
                rhs: Box::new(rhs),
                span: node.span,
            };
        }
    }
    Ok(acc)
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn build_binary_chain_dual<F>(
    node: &CstNode<'_>,
    self_id: ProductionId,
    next_id: ProductionId,
    a: (&str, BinaryOp),
    b: (&str, BinaryOp),
    next: F,
) -> BuildResult<ConstExpr>
where
    F: Fn(&CstNode<'_>) -> BuildResult<ConstExpr> + Copy,
{
    if node.children.len() == 1 {
        return next(&node.children[0]);
    }
    let _ = self_id;
    let _ = next_id;
    let lhs = if node.children[0].production() == node.production() {
        build_binary_chain_dual(&node.children[0], self_id, next_id, a, b, next)?
    } else {
        next(&node.children[0])?
    };
    let mut acc = lhs;
    let mut iter = node.children.iter().skip(1).peekable();
    while let Some(tok) = iter.next() {
        let op = match token_text_of(tok) {
            t if t == Some(a.0) => Some(a.1),
            t if t == Some(b.0) => Some(b.1),
            _ => None,
        };
        if let Some(op) = op {
            let rhs_node = iter
                .next()
                .ok_or_else(|| BuilderError::new("dual chain missing rhs", node.span))?;
            let rhs = next(rhs_node)?;
            acc = ConstExpr::Binary {
                op,
                lhs: Box::new(acc),
                rhs: Box::new(rhs),
                span: node.span,
            };
        }
    }
    Ok(acc)
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn build_binary_chain_triple<F>(
    node: &CstNode<'_>,
    self_id: ProductionId,
    next_id: ProductionId,
    a: (&str, BinaryOp),
    b: (&str, BinaryOp),
    c: (&str, BinaryOp),
    next: F,
) -> BuildResult<ConstExpr>
where
    F: Fn(&CstNode<'_>) -> BuildResult<ConstExpr> + Copy,
{
    if node.children.len() == 1 {
        return next(&node.children[0]);
    }
    let _ = self_id;
    let _ = next_id;
    let lhs = if node.children[0].production() == node.production() {
        build_binary_chain_triple(&node.children[0], self_id, next_id, a, b, c, next)?
    } else {
        next(&node.children[0])?
    };
    let mut acc = lhs;
    let mut iter = node.children.iter().skip(1).peekable();
    while let Some(tok) = iter.next() {
        let op = match token_text_of(tok) {
            t if t == Some(a.0) => Some(a.1),
            t if t == Some(b.0) => Some(b.1),
            t if t == Some(c.0) => Some(c.1),
            _ => None,
        };
        if let Some(op) = op {
            let rhs_node = iter
                .next()
                .ok_or_else(|| BuilderError::new("triple chain missing rhs", node.span))?;
            let rhs = next(rhs_node)?;
            acc = ConstExpr::Binary {
                op,
                lhs: Box::new(acc),
                rhs: Box::new(rhs),
                span: node.span,
            };
        }
    }
    Ok(acc)
}

// ============================================================================
// Exception
// ============================================================================

fn build_except_dcl(node: &CstNode<'_>) -> BuildResult<ExceptDecl> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("exception without ident", node.span))?;
    let members = match first_internal_child(node, ID_MEMBER_LIST) {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_MEMBER))
            .map(|c| build_member(c))
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    Ok(ExceptDecl {
        name: Identifier::new(text, span),
        members,
        annotations: Vec::new(),
        span: node.span,
    })
}

// ============================================================================
// Interface
// ============================================================================

fn build_interface_dcl(node: &CstNode<'_>) -> BuildResult<InterfaceDcl> {
    if let Some(c) = first_internal_child(node, ID_INTERFACE_DEF) {
        Ok(InterfaceDcl::Def(build_interface_def(c)?))
    } else if let Some(c) = first_internal_child(node, ID_INTERFACE_FORWARD_DCL) {
        Ok(InterfaceDcl::Forward(build_interface_forward(c)?))
    } else {
        Err(BuilderError::new("interface_dcl without alt", node.span))
    }
}

fn build_interface_def(node: &CstNode<'_>) -> BuildResult<InterfaceDef> {
    let header = require_internal_child(node, ID_INTERFACE_HEADER, "interface_header")?;
    let kind = header
        .children
        .iter()
        .find(|c| c.production() == Some(ID_INTERFACE_KIND))
        .map(build_interface_kind)
        .transpose()?
        .unwrap_or(InterfaceKind::Plain);
    let (text, span) = first_ident_text(header)
        .ok_or_else(|| BuilderError::new("interface header without ident", header.span))?;
    let bases = match first_internal_child(header, ID_INTERFACE_INHERITANCE_SPEC) {
        Some(spec) => match first_internal_child(spec, ID_INTERFACE_NAME_LIST) {
            Some(list) => flatten_list(list)
                .iter()
                .filter(|c| c.production() == Some(ID_SCOPED_NAME))
                .map(|c| build_scoped_name(c))
                .collect::<Result<_, _>>()?,
            None => Vec::new(),
        },
        None => Vec::new(),
    };
    let exports: Vec<Export> = match first_internal_child(node, ID_INTERFACE_BODY)
        .and_then(|b| first_internal_child(b, ID_EXPORT_LIST))
    {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_EXPORT))
            .map(|c| build_export(c))
            .collect::<Result<Vec<Vec<Export>>, _>>()?
            .into_iter()
            .flatten()
            .collect(),
        None => Vec::new(),
    };
    Ok(InterfaceDef {
        kind,
        name: Identifier::new(text, span),
        bases,
        exports,
        annotations: Vec::new(),
        span: node.span,
    })
}

fn build_interface_forward(node: &CstNode<'_>) -> BuildResult<InterfaceForwardDecl> {
    let kind = node
        .children
        .iter()
        .find(|c| c.production() == Some(ID_INTERFACE_KIND))
        .map(build_interface_kind)
        .transpose()?
        .unwrap_or(InterfaceKind::Plain);
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("interface fwd without ident", node.span))?;
    Ok(InterfaceForwardDecl {
        kind,
        name: Identifier::new(text, span),
        span: node.span,
    })
}

fn build_interface_kind(node: &CstNode<'_>) -> BuildResult<InterfaceKind> {
    if first_token_text(node, TokenKind::Keyword("abstract")).is_some() {
        Ok(InterfaceKind::Abstract)
    } else if first_token_text(node, TokenKind::Keyword("local")).is_some() {
        Ok(InterfaceKind::Local)
    } else {
        Ok(InterfaceKind::Plain)
    }
}

/// Returns `Vec<Export>` because §7.4.3.1 multi-name attr_dcl can produce
/// multiple AttrDecl exports from a single `attribute long a, b, c;` line.
/// Other export alternatives always return exactly one
/// entry.
fn build_export(node: &CstNode<'_>) -> BuildResult<Vec<Export>> {
    let annotations = build_annotation_seq(node)?;
    if let Some(c) = first_internal_child(node, ID_OP_DCL) {
        let mut o = build_op_dcl(c)?;
        o.annotations = annotations;
        return Ok(vec![Export::Op(o)]);
    }
    // `op_with_context` (§7.4.6.3 Rule 123): op_dcl + context_expr. The op_dcl
    // sits nested, not as a direct export child — handle separately.
    if let Some(owc) = first_internal_child(node, ID_OP_WITH_CONTEXT) {
        let op_node = first_internal_child(owc, ID_OP_DCL)
            .ok_or_else(|| BuilderError::new("op_with_context without op_dcl", owc.span))?;
        let mut o = build_op_dcl(op_node)?;
        o.annotations = annotations;
        if let Some(ce) = first_internal_child(owc, ID_CONTEXT_EXPR) {
            o.context = collect_string_literals(ce);
        }
        return Ok(vec![Export::Op(o)]);
    }
    if let Some(c) = first_internal_child(node, ID_ATTR_DCL) {
        let attrs = build_attr_dcl(c)?;
        return Ok(attrs
            .into_iter()
            .map(|mut a| {
                a.annotations = annotations.clone();
                Export::Attr(a)
            })
            .collect());
    }
    if let Some(c) = first_internal_child(node, ID_TYPE_DCL) {
        let mut t = build_type_dcl(c)?;
        set_type_dcl_annotations(&mut t, annotations);
        return Ok(vec![Export::Type(t)]);
    }
    if let Some(c) = first_internal_child(node, ID_CONST_DCL) {
        let mut cd = build_const_dcl(c)?;
        cd.annotations = annotations;
        return Ok(vec![Export::Const(cd)]);
    }
    if let Some(c) = first_internal_child(node, ID_EXCEPT_DCL) {
        let mut e = build_except_dcl(c)?;
        e.annotations = annotations;
        return Ok(vec![Export::Except(e)]);
    }
    Err(BuilderError::new("export without alt", node.span))
}

fn build_op_dcl(node: &CstNode<'_>) -> BuildResult<OpDecl> {
    let oneway = first_token_text(node, TokenKind::Keyword("oneway")).is_some();
    let ot = require_internal_child(node, ID_OP_TYPE_SPEC, "op_type_spec")?;
    let return_type = if first_token_text(ot, TokenKind::Keyword("void")).is_some() {
        None
    } else {
        first_internal_child(ot, ID_TYPE_SPEC)
            .map(build_type_spec)
            .transpose()?
    };
    // Identifier: first Ident token *after* op_type_spec.
    let (text, span) = node
        .children
        .iter()
        .skip_while(|c| c.production() != Some(ID_OP_TYPE_SPEC))
        .skip(1)
        .find_map(|c| {
            if let CstKind::Token(TokenKind::Ident) = c.kind {
                Some((c.text, c.span))
            } else if c.production() == Some(ID_IDENTIFIER) {
                ident_token_in(c)
            } else {
                None
            }
        })
        .ok_or_else(|| BuilderError::new("op_dcl without ident", node.span))?;
    let params = match first_internal_child(node, ID_PARAMETER_DCLS)
        .and_then(|p| first_internal_child(p, ID_PARAM_DCL_LIST))
    {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_PARAM_DCL))
            .map(|c| build_param_dcl(c))
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    let raises = match first_internal_child(node, ID_RAISES_EXPR)
        .and_then(|r| first_internal_child(r, ID_SCOPED_NAME_LIST))
    {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_SCOPED_NAME))
            .map(|c| build_scoped_name(c))
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    Ok(OpDecl {
        name: Identifier::new(text, span),
        oneway,
        return_type,
        params,
        raises,
        context: Vec::new(),
        annotations: Vec::new(),
        span: node.span,
    })
}

fn build_param_dcl(node: &CstNode<'_>) -> BuildResult<ParamDecl> {
    let annotations = build_annotation_seq(node)?;
    let attr = require_internal_child(node, ID_PARAM_ATTRIBUTE, "param_attribute")?;
    let attribute = if first_token_text(attr, TokenKind::Keyword("in")).is_some() {
        ParamAttribute::In
    } else if first_token_text(attr, TokenKind::Keyword("out")).is_some() {
        ParamAttribute::Out
    } else if first_token_text(attr, TokenKind::Keyword("inout")).is_some() {
        ParamAttribute::InOut
    } else {
        return Err(BuilderError::new("param_attribute unknown", attr.span));
    };
    let ts = require_internal_child(node, ID_TYPE_SPEC, "type_spec")?;
    let sd = require_internal_child(node, ID_SIMPLE_DECLARATOR, "simple_declarator")?;
    let (text, span) = first_ident_text(sd)
        .ok_or_else(|| BuilderError::new("param_dcl without ident", sd.span))?;
    Ok(ParamDecl {
        attribute,
        type_spec: build_type_spec(ts)?,
        name: Identifier::new(text, span),
        annotations,
        span: node.span,
    })
}

/// Returns `Vec<AttrDecl>` because §7.4.3.1 Rule 91/93 allow multi-name:
/// `attribute long a, b, c;` is semantically equivalent to three
/// individual attribute declarations with a shared type/readonly/
/// annotations/raises.
fn build_attr_dcl(node: &CstNode<'_>) -> BuildResult<Vec<AttrDecl>> {
    if let Some(spec) = first_internal_child(node, ID_READONLY_ATTR_SPEC) {
        return build_readonly_attr_spec(spec, node.span);
    }
    if let Some(spec) = first_internal_child(node, ID_ATTR_SPEC) {
        return build_attr_spec(spec, node.span);
    }
    Err(BuilderError::new("attr_dcl without spec alt", node.span))
}

fn build_readonly_attr_spec(spec: &CstNode<'_>, outer_span: Span) -> BuildResult<Vec<AttrDecl>> {
    let ts = require_internal_child(spec, ID_TYPE_SPEC, "type_spec")?;
    let type_spec = build_type_spec(ts)?;
    let decl = require_internal_child(
        spec,
        ID_READONLY_ATTR_DECLARATOR,
        "readonly_attr_declarator",
    )?;
    let names = collect_simple_declarator_names(decl)?;
    if names.is_empty() {
        return Err(BuilderError::new(
            "readonly_attr_declarator without name",
            decl.span,
        ));
    }
    // Rule 91 alt-1: `simple_declarator <raises_expr>` (read-only `raises (..)`).
    let get_raises = match first_internal_child(decl, ID_RAISES_EXPR)
        .and_then(|r| first_internal_child(r, ID_SCOPED_NAME_LIST))
    {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_SCOPED_NAME))
            .map(|c| build_scoped_name(c))
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    Ok(names
        .into_iter()
        .map(|(text, span)| AttrDecl {
            name: Identifier::new(text, span),
            type_spec: type_spec.clone(),
            readonly: true,
            get_raises: get_raises.clone(),
            set_raises: Vec::new(),
            annotations: Vec::new(),
            span: outer_span,
        })
        .collect())
}

fn build_attr_spec(spec: &CstNode<'_>, outer_span: Span) -> BuildResult<Vec<AttrDecl>> {
    let ts = require_internal_child(spec, ID_TYPE_SPEC, "type_spec")?;
    let type_spec = build_type_spec(ts)?;
    let decl = require_internal_child(spec, ID_ATTR_DECLARATOR, "attr_declarator")?;
    let names = collect_simple_declarator_names(decl)?;
    if names.is_empty() {
        return Err(BuilderError::new("attr_declarator without name", decl.span));
    }
    // attr_raises_expr can sit under the attr_declarator node or further
    // down in an `_alt_X` wrapper — we search recursively.
    let (get_raises, set_raises) = match find_descendant(decl, ID_ATTR_RAISES_EXPR) {
        Some(rx) => {
            let g = match find_descendant(rx, ID_GET_EXCEP_EXPR) {
                Some(g) => collect_exception_list(g)?,
                None => Vec::new(),
            };
            let s = match find_descendant(rx, ID_SET_EXCEP_EXPR) {
                Some(s) => collect_exception_list(s)?,
                None => Vec::new(),
            };
            (g, s)
        }
        None => (Vec::new(), Vec::new()),
    };
    Ok(names
        .into_iter()
        .map(|(text, span)| AttrDecl {
            name: Identifier::new(text, span),
            type_spec: type_spec.clone(),
            readonly: false,
            get_raises: get_raises.clone(),
            set_raises: set_raises.clone(),
            annotations: Vec::new(),
            span: outer_span,
        })
        .collect())
}

/// Collects all `simple_declarator` nodes in source order.
///
/// Rules 91/93 use `Symbol::Repeat(ZeroOrMore, [",", simple_declarator])`,
/// which expands in the compile pass to a synthesized `_rep_X` production.
/// The repeated simple_declarators therefore sit
/// not directly under the `*_attr_declarator` but in a nested
/// `_rep_X` wrapper. We therefore walk all internal children and
/// collect recursively.
fn collect_simple_declarator_names<'a>(decl: &'a CstNode<'a>) -> BuildResult<Vec<(&'a str, Span)>> {
    let mut out = Vec::new();
    collect_simple_declarators_into(decl, &mut out)?;
    Ok(out)
}

/// zerodds-lint: recursion-depth 64 (CST-Walk; bounded by IDL nesting)
fn collect_simple_declarators_into<'a>(
    node: &'a CstNode<'a>,
    out: &mut Vec<(&'a str, Span)>,
) -> BuildResult<()> {
    for c in &node.children {
        if c.production() == Some(ID_SIMPLE_DECLARATOR) {
            let (text, span) = first_ident_text(c)
                .ok_or_else(|| BuilderError::new("simple_declarator without ident", c.span))?;
            out.push((text, span));
        } else if c.is_internal() {
            collect_simple_declarators_into(c, out)?;
        }
    }
    Ok(())
}

fn collect_exception_list(excep_node: &CstNode<'_>) -> BuildResult<Vec<ScopedName>> {
    // exception_list can be nested under `_alt_X` wrappers.
    let list = find_descendant(excep_node, ID_EXCEPTION_LIST)
        .and_then(|el| find_descendant(el, ID_SCOPED_NAME_LIST));
    match list {
        Some(snl) => flatten_list(snl)
            .iter()
            .filter(|c| c.production() == Some(ID_SCOPED_NAME))
            .map(|c| build_scoped_name(c))
            .collect::<Result<Vec<_>, _>>(),
        None => Ok(Vec::new()),
    }
}

/// Depth search for the first production of a given ID. Unlike
/// [`first_internal_child`] it goes through synthesized `_rep_X` /
/// `_alt_X` / `_chc_X` wrappers that the compile pass produces from
/// `Symbol::Repeat`/`Choice`.
///
/// zerodds-lint: recursion-depth 64 (CST-Walk; bounded by IDL nesting)
fn find_descendant<'a, 'src>(
    node: &'a CstNode<'src>,
    target: ProductionId,
) -> Option<&'a CstNode<'src>> {
    for c in &node.children {
        if c.production() == Some(target) {
            return Some(c);
        }
        if c.is_internal() {
            if let Some(found) = find_descendant(c, target) {
                return Some(found);
            }
        }
    }
    None
}

// ============================================================================
// Valuetype (minimal)
// ============================================================================

fn build_value_box_dcl(node: &CstNode<'_>) -> BuildResult<ValueBoxDecl> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("value_box without ident", node.span))?;
    let ts = require_internal_child(node, ID_TYPE_SPEC, "type_spec")?;
    Ok(ValueBoxDecl {
        name: Identifier::new(text, span),
        type_spec: build_type_spec(ts)?,
        annotations: Vec::new(),
        span: node.span,
    })
}

fn build_value_forward_dcl(node: &CstNode<'_>) -> BuildResult<ValueForwardDecl> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("value_fwd without ident", node.span))?;
    Ok(ValueForwardDecl {
        name: Identifier::new(text, span),
        span: node.span,
    })
}

// ============================================================================
// User-Defined Annotation Declarations (§7.4.15)
// ============================================================================

fn build_annotation_dcl(node: &CstNode<'_>) -> BuildResult<AnnotationDcl> {
    let header = require_internal_child(node, ID_ANNOTATION_HEADER, "annotation_header")?;
    let (text, span) = first_ident_text(header)
        .ok_or_else(|| BuilderError::new("annotation_header without ident", header.span))?;
    let mut members = Vec::new();
    let mut embedded_types = Vec::new();
    let mut embedded_consts = Vec::new();
    if let Some(body) = first_internal_child(node, ID_ANNOTATION_BODY) {
        // The body contains a `_rep_X`-synthesized production with
        // multiple `annotation_body_member` nodes as children. We
        // collect all annotation_body_member recursively.
        let mut collected: Vec<&CstNode<'_>> = Vec::new();
        collect_annotation_body_members(body, &mut collected);
        for bm in collected {
            if let Some(am) = first_internal_child(bm, ID_ANNOTATION_MEMBER) {
                members.push(build_annotation_member(am)?);
            } else if let Some(ed) = first_internal_child(bm, ID_ENUM_DCL) {
                let enum_def = build_enum_dcl(ed)?;
                embedded_types.push(TypeDecl::Constr(ConstrTypeDecl::Enum(enum_def)));
            } else if let Some(td) = first_internal_child(bm, ID_TYPEDEF_DCL) {
                embedded_types.push(TypeDecl::Typedef(build_typedef_dcl(td)?));
            } else if let Some(cd) = first_internal_child(bm, ID_CONST_DCL) {
                embedded_consts.push(build_const_dcl(cd)?);
            }
        }
    }
    Ok(AnnotationDcl {
        name: Identifier::new(text, span),
        members,
        embedded_types,
        embedded_consts,
        span: node.span,
    })
}

/// zerodds-lint: recursion-depth 64 (CST-Walk; bounded by IDL nesting)
fn collect_annotation_body_members<'a, 'src>(
    node: &'a CstNode<'src>,
    out: &mut Vec<&'a CstNode<'src>>,
) {
    for c in &node.children {
        if c.production() == Some(ID_ANNOTATION_BODY_MEMBER) {
            out.push(c);
        } else if c.is_internal() {
            collect_annotation_body_members(c, out);
        }
    }
}

fn build_annotation_member(node: &CstNode<'_>) -> BuildResult<AnnotationMember> {
    let ct = require_internal_child(node, ID_CONST_TYPE, "const_type")?;
    let type_spec = build_const_type(ct)?;
    let sd = require_internal_child(node, ID_SIMPLE_DECLARATOR, "simple_declarator")?;
    let (text, span) = first_ident_text(sd)
        .ok_or_else(|| BuilderError::new("annotation_member without ident", sd.span))?;
    let default = match first_internal_child(node, ID_CONST_EXPR) {
        Some(ce) => Some(build_const_expr(ce)?),
        None => None,
    };
    Ok(AnnotationMember {
        name: Identifier::new(text, span),
        type_spec,
        default,
        span: node.span,
    })
}

// ============================================================================
// Type-Spec
// ============================================================================

fn build_type_spec(node: &CstNode<'_>) -> BuildResult<TypeSpec> {
    if let Some(simple) = first_internal_child(node, ID_SIMPLE_TYPE_SPEC) {
        return build_simple_type_spec(simple);
    }
    if let Some(tmpl) = first_internal_child(node, ID_TEMPLATE_TYPE_SPEC) {
        return build_template_type_spec(tmpl);
    }
    Err(BuilderError::new("type_spec without alt", node.span))
}

fn build_simple_type_spec(node: &CstNode<'_>) -> BuildResult<TypeSpec> {
    if let Some(b) = first_internal_child(node, ID_BASE_TYPE_SPEC) {
        return build_base_type_spec(b);
    }
    if let Some(s) = first_internal_child(node, ID_SCOPED_NAME) {
        return Ok(TypeSpec::Scoped(build_scoped_name(s)?));
    }
    Err(BuilderError::new("simple_type_spec without alt", node.span))
}

fn build_base_type_spec(node: &CstNode<'_>) -> BuildResult<TypeSpec> {
    if let Some(c) = first_internal_child(node, ID_INTEGER_TYPE) {
        return Ok(TypeSpec::Primitive(PrimitiveType::Integer(
            build_integer_type(c)?,
        )));
    }
    if let Some(c) = first_internal_child(node, ID_FLOATING_PT_TYPE) {
        return Ok(TypeSpec::Primitive(PrimitiveType::Floating(
            build_floating_type(c)?,
        )));
    }
    if first_internal_child(node, ID_CHAR_TYPE).is_some() {
        return Ok(TypeSpec::Primitive(PrimitiveType::Char));
    }
    if first_internal_child(node, ID_WIDE_CHAR_TYPE).is_some() {
        return Ok(TypeSpec::Primitive(PrimitiveType::WideChar));
    }
    if first_internal_child(node, ID_BOOLEAN_TYPE).is_some() {
        return Ok(TypeSpec::Primitive(PrimitiveType::Boolean));
    }
    if first_internal_child(node, ID_OCTET_TYPE).is_some() {
        return Ok(TypeSpec::Primitive(PrimitiveType::Octet));
    }
    if first_internal_child(node, ID_ANY_TYPE).is_some() {
        return Ok(TypeSpec::Any);
    }
    // `Object` — CORBA object-reference pseudo-type (§7.4.6.3 Rule 117), as a
    // direct keyword terminal in base_type_spec. Like the inheritance builder
    // (build_interface_type), modeled as the scoped name `Object` — the codegen
    // maps this reserved name to the object reference (IOR wire), without
    // every TypeSpec consumer having to handle a new variant.
    if first_token_text(node, TokenKind::Keyword("Object")).is_some() {
        return Ok(TypeSpec::Scoped(ScopedName {
            absolute: false,
            parts: vec![Identifier::new("Object", node.span)],
            span: node.span,
        }));
    }
    Err(BuilderError::new("base_type_spec unrecognized", node.span))
}

fn build_template_type_spec(node: &CstNode<'_>) -> BuildResult<TypeSpec> {
    if let Some(c) = first_internal_child(node, ID_SEQUENCE_TYPE) {
        return Ok(TypeSpec::Sequence(build_sequence_type(c)?));
    }
    if let Some(c) = first_internal_child(node, ID_STRING_TYPE) {
        return Ok(TypeSpec::String(build_string_type(c, false)?));
    }
    if let Some(c) = first_internal_child(node, ID_WIDE_STRING_TYPE) {
        return Ok(TypeSpec::String(build_string_type(c, true)?));
    }
    if let Some(c) = first_internal_child(node, ID_FIXED_PT_TYPE) {
        return Ok(TypeSpec::Fixed(build_fixed_pt_type(c)?));
    }
    if let Some(c) = first_internal_child(node, ID_MAP_TYPE) {
        return Ok(TypeSpec::Map(build_map_type(c)?));
    }
    Err(BuilderError::new(
        "template_type_spec unrecognized",
        node.span,
    ))
}

fn build_sequence_type(node: &CstNode<'_>) -> BuildResult<SequenceType> {
    let elem_node = require_internal_child(node, ID_TYPE_SPEC, "type_spec")?;
    let elem = build_type_spec(elem_node)?;
    let bound = first_internal_child(node, ID_POSITIVE_INT_CONST)
        .map(build_positive_int_const)
        .transpose()?;
    Ok(SequenceType {
        elem: Box::new(elem),
        bound,
        span: node.span,
    })
}

fn build_string_type(node: &CstNode<'_>, wide: bool) -> BuildResult<StringType> {
    let bound = first_internal_child(node, ID_POSITIVE_INT_CONST)
        .map(build_positive_int_const)
        .transpose()?;
    Ok(StringType {
        wide,
        bound,
        span: node.span,
    })
}

fn build_fixed_pt_type(node: &CstNode<'_>) -> BuildResult<FixedPtType> {
    let parts: Vec<&CstNode<'_>> = node
        .children
        .iter()
        .filter(|c| c.production() == Some(ID_POSITIVE_INT_CONST))
        .collect();
    let digits_node = parts
        .first()
        .ok_or_else(|| BuilderError::new("fixed_pt_type without digits", node.span))?;
    let scale_node = parts
        .get(1)
        .ok_or_else(|| BuilderError::new("fixed_pt_type without scale", node.span))?;
    Ok(FixedPtType {
        digits: build_positive_int_const(digits_node)?,
        scale: build_positive_int_const(scale_node)?,
        span: node.span,
    })
}

fn build_map_type(node: &CstNode<'_>) -> BuildResult<MapType> {
    let type_specs: Vec<&CstNode<'_>> = node
        .children
        .iter()
        .filter(|c| c.production() == Some(ID_TYPE_SPEC))
        .collect();
    let key = type_specs
        .first()
        .ok_or_else(|| BuilderError::new("map_type without key", node.span))?;
    let value = type_specs
        .get(1)
        .ok_or_else(|| BuilderError::new("map_type without value", node.span))?;
    let bound = first_internal_child(node, ID_POSITIVE_INT_CONST)
        .map(build_positive_int_const)
        .transpose()?;
    Ok(MapType {
        key: Box::new(build_type_spec(key)?),
        value: Box::new(build_type_spec(value)?),
        bound,
        span: node.span,
    })
}

// ============================================================================
// Integer / Floating
// ============================================================================

fn build_integer_type(node: &CstNode<'_>) -> BuildResult<IntegerType> {
    if let Some(s) = first_internal_child(node, ID_SIGNED_INT) {
        return integer_from_keywords(s, true);
    }
    if let Some(u) = first_internal_child(node, ID_UNSIGNED_INT) {
        return integer_from_keywords(u, false);
    }
    Err(BuilderError::new("integer_type unrecognized", node.span))
}

fn integer_from_keywords(node: &CstNode<'_>, signed: bool) -> BuildResult<IntegerType> {
    let kws: Vec<&str> = node
        .children
        .iter()
        .filter_map(|c| match c.kind {
            CstKind::Token(TokenKind::Keyword(k)) => Some(k),
            _ => None,
        })
        .collect();
    Ok(match (signed, kws.as_slice()) {
        (true, ["short"]) => IntegerType::Short,
        (true, ["long"]) => IntegerType::Long,
        (true, ["long", "long"]) => IntegerType::LongLong,
        (true, ["int8"]) => IntegerType::Int8,
        (true, ["int16"]) => IntegerType::Int16,
        (true, ["int32"]) => IntegerType::Int32,
        (true, ["int64"]) => IntegerType::Int64,
        (false, ["unsigned", "short"]) => IntegerType::UShort,
        (false, ["unsigned", "long"]) => IntegerType::ULong,
        (false, ["unsigned", "long", "long"]) => IntegerType::ULongLong,
        (false, ["uint8"]) => IntegerType::UInt8,
        (false, ["uint16"]) => IntegerType::UInt16,
        (false, ["uint32"]) => IntegerType::UInt32,
        (false, ["uint64"]) => IntegerType::UInt64,
        _ => {
            return Err(BuilderError::new(
                format!("integer keywords unrecognized: {kws:?}"),
                node.span,
            ));
        }
    })
}

fn build_floating_type(node: &CstNode<'_>) -> BuildResult<FloatingType> {
    let kws: Vec<&str> = node
        .children
        .iter()
        .filter_map(|c| match c.kind {
            CstKind::Token(TokenKind::Keyword(k)) => Some(k),
            _ => None,
        })
        .collect();
    Ok(match kws.as_slice() {
        ["float"] => FloatingType::Float,
        ["double"] => FloatingType::Double,
        ["long", "double"] => FloatingType::LongDouble,
        _ => {
            return Err(BuilderError::new(
                format!("float keywords unrecognized: {kws:?}"),
                node.span,
            ));
        }
    })
}

// ============================================================================
// ScopedName
// ============================================================================

fn build_scoped_name(node: &CstNode<'_>) -> BuildResult<ScopedName> {
    let mut absolute = false;
    let mut parts = Vec::new();
    collect_scoped_name_parts(node, &mut absolute, &mut parts);
    if parts.is_empty() {
        return Err(BuilderError::new("scoped_name empty", node.span));
    }
    Ok(ScopedName {
        absolute,
        parts,
        span: node.span,
    })
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn collect_scoped_name_parts(node: &CstNode<'_>, absolute: &mut bool, parts: &mut Vec<Identifier>) {
    for c in &node.children {
        match c.kind {
            CstKind::Token(TokenKind::Punct("::")) if parts.is_empty() => {
                *absolute = true;
            }
            CstKind::Token(TokenKind::Punct("::")) => {
                // separator inside tail
            }
            CstKind::Token(TokenKind::Ident) => {
                parts.push(Identifier::new(c.text, c.span));
            }
            _ => {
                if c.production() == Some(ID_IDENTIFIER) {
                    if let Some((t, s)) = ident_token_in(c) {
                        parts.push(Identifier::new(t, s));
                    }
                } else if c.is_internal() {
                    collect_scoped_name_parts(c, absolute, parts);
                }
            }
        }
    }
}

// ============================================================================
// Annotations
// ============================================================================

fn build_annotation_seq(parent: &CstNode<'_>) -> BuildResult<Vec<Annotation>> {
    match first_internal_child(parent, ID_ANNOTATION_APPL_SEQ) {
        Some(seq) => build_annotation_seq_node(seq),
        None => Ok(Vec::new()),
    }
}

fn build_annotation_seq_node(node: &CstNode<'_>) -> BuildResult<Vec<Annotation>> {
    let mut out = Vec::new();
    collect_annotations(node, &mut out)?;
    Ok(out)
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn collect_annotations(node: &CstNode<'_>, out: &mut Vec<Annotation>) -> BuildResult<()> {
    if node.children.is_empty() {
        return Ok(());
    }
    let first = &node.children[0];
    if first.production() == node.production() {
        collect_annotations(first, out)?;
    }
    for c in &node.children {
        if c.production() == Some(ID_ANNOTATION_APPL) {
            out.push(build_annotation_appl(c)?);
        }
    }
    Ok(())
}

/// The name after `@`: either a `<scoped_name>` (identifier) or a
/// single keyword token — built-in annotations like `@default` whose
/// name is a reserved keyword (OMG IDL 4.2 / XTypes 1.3).
fn build_annotation_appl_name(node: &CstNode<'_>) -> BuildResult<ScopedName> {
    if let Some(sn) = first_internal_child(node, ID_SCOPED_NAME) {
        return build_scoped_name(sn);
    }
    for c in &node.children {
        if let CstKind::Token(TokenKind::Keyword(_)) = c.kind {
            return Ok(ScopedName {
                absolute: false,
                parts: vec![Identifier::new(c.text, c.span)],
                span: node.span,
            });
        }
    }
    Err(BuilderError::new("annotation_appl_name leer", node.span))
}

fn build_annotation_appl(node: &CstNode<'_>) -> BuildResult<Annotation> {
    let name_node = require_internal_child(node, ID_ANNOTATION_APPL_NAME, "annotation_appl_name")?;
    let name = build_annotation_appl_name(name_node)?;
    let params = if let Some(p) = first_internal_child(node, ID_ANNOTATION_APPL_PARAMS) {
        build_annotation_appl_params(p)?
    } else {
        // Either no_params or empty_params alternative.
        let has_open = node
            .children
            .iter()
            .any(|c| matches!(c.kind, CstKind::Token(TokenKind::Punct("("))));
        if has_open {
            AnnotationParams::Empty
        } else {
            AnnotationParams::None
        }
    };
    Ok(Annotation {
        name,
        params,
        span: node.span,
    })
}

fn build_annotation_appl_params(node: &CstNode<'_>) -> BuildResult<AnnotationParams> {
    if let Some(list) = first_internal_child(node, ID_ANNOTATION_APPL_PARAM_LIST) {
        let items = flatten_list(list);
        let named: Vec<NamedParam> = items
            .iter()
            .filter(|c| c.production() == Some(ID_ANNOTATION_APPL_PARAM))
            .map(|c| build_named_param(c))
            .collect::<Result<_, _>>()?;
        return Ok(AnnotationParams::Named(named));
    }
    if let Some(expr) = first_internal_child(node, ID_CONST_EXPR) {
        return Ok(AnnotationParams::Single(build_const_expr(expr)?));
    }
    Ok(AnnotationParams::Empty)
}

fn build_named_param(node: &CstNode<'_>) -> BuildResult<NamedParam> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("named_param without ident", node.span))?;
    let expr = require_internal_child(node, ID_CONST_EXPR, "const_expr")?;
    Ok(NamedParam {
        name: Identifier::new(text, span),
        value: build_const_expr(expr)?,
        span: node.span,
    })
}

// ============================================================================
// ValueDef (§7.4.5.4 Rules 99-110 + §7.4.7.3 Rules 125-128 abstract/custom)
// ============================================================================

fn build_value_def(node: &CstNode<'_>) -> BuildResult<ValueDef> {
    let header = require_internal_child(node, ID_VALUE_HEADER, "value_header")?;
    let kind = if has_keyword_in_tree(header, "abstract") {
        ValueKind::Abstract
    } else if has_keyword_in_tree(header, "custom") {
        ValueKind::Custom
    } else {
        ValueKind::Concrete
    };
    let (text, span) = first_ident_text(header)
        .ok_or_else(|| BuilderError::new("value_header without ident", header.span))?;
    let inheritance = match find_descendant(header, ID_VALUE_INHERITANCE_SPEC) {
        Some(spec) => Some(build_value_inheritance_spec(spec)?),
        None => None,
    };
    let mut elements = Vec::new();
    for c in &node.children {
        if c.production() == Some(ID_VALUE_ELEMENT) {
            elements.extend(build_value_element(c)?);
        } else if c.is_internal() && c.production() != Some(ID_VALUE_HEADER) {
            collect_value_elements_into(c, &mut elements)?;
        }
    }
    Ok(ValueDef {
        name: Identifier::new(text, span),
        kind,
        inheritance,
        elements,
        annotations: Vec::new(),
        span: node.span,
    })
}

/// zerodds-lint: recursion-depth 64 (CST-Walk; bounded by IDL nesting)
fn collect_value_elements_into(node: &CstNode<'_>, out: &mut Vec<ValueElement>) -> BuildResult<()> {
    for c in &node.children {
        if c.production() == Some(ID_VALUE_ELEMENT) {
            out.extend(build_value_element(c)?);
        } else if c.is_internal() {
            collect_value_elements_into(c, out)?;
        }
    }
    Ok(())
}

fn build_value_inheritance_spec(node: &CstNode<'_>) -> BuildResult<ValueInheritanceSpec> {
    let truncatable = has_keyword_in_tree(node, "truncatable");
    let bases = match find_descendant(node, ID_VALUE_INHERITANCE_NAME_LIST) {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_SCOPED_NAME))
            .map(|c| build_scoped_name(c))
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    let supports = match find_descendant(node, ID_INTERFACE_NAME_LIST) {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_SCOPED_NAME))
            .map(|c| build_scoped_name(c))
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    Ok(ValueInheritanceSpec {
        truncatable,
        bases,
        supports,
        span: node.span,
    })
}

fn build_value_element(node: &CstNode<'_>) -> BuildResult<Vec<ValueElement>> {
    if let Some(ex) = first_internal_child(node, ID_EXPORT) {
        let exports = build_export(ex)?;
        return Ok(exports.into_iter().map(ValueElement::Export).collect());
    }
    if let Some(sm) = first_internal_child(node, ID_STATE_MEMBER) {
        return Ok(vec![ValueElement::State(build_state_member(sm)?)]);
    }
    if let Some(init) = first_internal_child(node, ID_INIT_DCL) {
        return Ok(vec![ValueElement::Init(build_init_dcl(init)?)]);
    }
    Err(BuilderError::new("value_element without alt", node.span))
}

fn build_state_member(node: &CstNode<'_>) -> BuildResult<StateMember> {
    let visibility = if first_token_text(node, TokenKind::Keyword("private")).is_some() {
        StateVisibility::Private
    } else {
        StateVisibility::Public
    };
    let ts = require_internal_child(node, ID_TYPE_SPEC, "type_spec")?;
    let type_spec = build_type_spec(ts)?;
    let dl = require_internal_child(node, ID_DECLARATORS, "declarators")?;
    let declarators = build_declarator_list(dl)?;
    Ok(StateMember {
        visibility,
        type_spec,
        declarators,
        annotations: Vec::new(),
        span: node.span,
    })
}

fn build_init_dcl(node: &CstNode<'_>) -> BuildResult<InitDcl> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("init_dcl without ident", node.span))?;
    let params = match find_descendant(node, ID_INIT_PARAM_DCLS) {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_INIT_PARAM_DCL))
            .map(|c| build_init_param_dcl(c))
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    let raises = match first_internal_child(node, ID_RAISES_EXPR)
        .and_then(|r| first_internal_child(r, ID_SCOPED_NAME_LIST))
    {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_SCOPED_NAME))
            .map(|c| build_scoped_name(c))
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    Ok(InitDcl {
        name: Identifier::new(text, span),
        params,
        raises,
        span: node.span,
    })
}

fn build_init_param_dcl(node: &CstNode<'_>) -> BuildResult<ParamDecl> {
    let ts = require_internal_child(node, ID_TYPE_SPEC, "type_spec")?;
    let sd = require_internal_child(node, ID_SIMPLE_DECLARATOR, "simple_declarator")?;
    let (text, span) = first_ident_text(sd)
        .ok_or_else(|| BuilderError::new("init_param_dcl without ident", sd.span))?;
    Ok(ParamDecl {
        attribute: ParamAttribute::In,
        type_spec: build_type_spec(ts)?,
        name: Identifier::new(text, span),
        annotations: Vec::new(),
        span: node.span,
    })
}

// ============================================================================
// CORBA-Specific Top-Level (§7.4.6 Rules 111-117)
// ============================================================================

fn build_type_id_dcl(node: &CstNode<'_>) -> BuildResult<TypeIdDcl> {
    let sn = require_internal_child(node, ID_SCOPED_NAME, "scoped_name")?;
    let target = build_scoped_name(sn)?;
    let str_node = require_internal_child(node, ID_STRING_LITERAL, "string_literal")?;
    let repository_id = string_literal_value(str_node)?;
    Ok(TypeIdDcl {
        target,
        repository_id,
        span: node.span,
    })
}

fn build_type_prefix_dcl(node: &CstNode<'_>) -> BuildResult<TypePrefixDcl> {
    let sn = require_internal_child(node, ID_SCOPED_NAME, "scoped_name")?;
    let target = build_scoped_name(sn)?;
    let str_node = require_internal_child(node, ID_STRING_LITERAL, "string_literal")?;
    let prefix = string_literal_value(str_node)?;
    Ok(TypePrefixDcl {
        target,
        prefix,
        span: node.span,
    })
}

fn build_import_dcl(node: &CstNode<'_>) -> BuildResult<ImportDcl> {
    let scope_node = require_internal_child(node, ID_IMPORTED_SCOPE, "imported_scope")?;
    let imported = if let Some(sn) = first_internal_child(scope_node, ID_SCOPED_NAME) {
        ImportedScope::Scoped(build_scoped_name(sn)?)
    } else if let Some(sl) = first_internal_child(scope_node, ID_STRING_LITERAL) {
        ImportedScope::Repository(string_literal_value(sl)?)
    } else {
        return Err(BuilderError::new(
            "imported_scope without alt",
            scope_node.span,
        ));
    };
    Ok(ImportDcl {
        imported,
        span: node.span,
    })
}

/// Extracts the unescaped value from a `<string_literal>` wrapper —
/// strips the enclosing quotes.
fn string_literal_value(node: &CstNode<'_>) -> BuildResult<String> {
    let raw = node
        .children
        .iter()
        .find_map(|c| match c.kind {
            CstKind::Token(TokenKind::StringLiteral) => Some(c.text),
            _ => None,
        })
        .ok_or_else(|| BuilderError::new("string_literal without token", node.span))?;
    Ok(strip_string_quotes(raw))
}

fn strip_string_quotes(raw: &str) -> String {
    let trimmed = raw.trim_start_matches('L');
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Recursively collects all `StringLiteral` token texts (unquoted) under `node`
/// in document order. Used for the `context (...)` clause, whose
/// `context_string_list` is left-recursive (§7.4.6.3 Rule 124).
fn collect_string_literals(node: &CstNode<'_>) -> Vec<String> {
    let mut out = Vec::new();
    collect_string_literals_into(node, &mut out);
    out
}

/// zerodds-lint: recursion-depth 32
fn collect_string_literals_into(node: &CstNode<'_>, out: &mut Vec<String>) {
    for c in &node.children {
        if matches!(c.kind, CstKind::Token(TokenKind::StringLiteral)) {
            out.push(strip_string_quotes(c.text));
        } else {
            collect_string_literals_into(c, out);
        }
    }
}

// ============================================================================
// Component / Home / Event / Porttype / Connector (§7.4.8-§7.4.11)
// ============================================================================

fn build_component_dcl(node: &CstNode<'_>) -> BuildResult<ComponentDcl> {
    if let Some(fwd) = first_internal_child(node, ID_COMPONENT_FORWARD_DCL) {
        let (text, span) = first_ident_text(fwd)
            .ok_or_else(|| BuilderError::new("component_forward without ident", fwd.span))?;
        return Ok(ComponentDcl::Forward(
            Identifier::new(text, span),
            node.span,
        ));
    }
    if let Some(def) = first_internal_child(node, ID_COMPONENT_DEF) {
        return Ok(ComponentDcl::Def(build_component_def(def, node.span)?));
    }
    Err(BuilderError::new("component_dcl without alt", node.span))
}

fn build_component_def(node: &CstNode<'_>, outer_span: Span) -> BuildResult<ComponentDef> {
    let header = require_internal_child(node, ID_COMPONENT_HEADER, "component_header")?;
    let (text, span) = first_ident_text(header)
        .ok_or_else(|| BuilderError::new("component_header without ident", header.span))?;
    let base = match find_descendant(header, ID_COMPONENT_INHERITANCE_SPEC)
        .and_then(|h| find_descendant(h, ID_SCOPED_NAME))
    {
        Some(sn) => Some(build_scoped_name(sn)?),
        None => None,
    };
    let supports = collect_supported_interfaces(header)?;
    let mut body = Vec::new();
    if let Some(b) = first_internal_child(node, ID_COMPONENT_BODY) {
        for c in collect_descendants(b, ID_COMPONENT_EXPORT) {
            body.extend(build_component_export(c)?);
        }
    }
    Ok(ComponentDef {
        name: Identifier::new(text, span),
        base,
        supports,
        body,
        annotations: Vec::new(),
        span: outer_span,
    })
}

fn collect_supported_interfaces(header: &CstNode<'_>) -> BuildResult<Vec<ScopedName>> {
    match find_descendant(header, ID_SUPPORTED_INTERFACE_SPEC)
        .and_then(|s| find_descendant(s, ID_SCOPED_NAME_LIST))
    {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_SCOPED_NAME))
            .map(|c| build_scoped_name(c))
            .collect::<Result<Vec<_>, _>>(),
        None => Ok(Vec::new()),
    }
}

/// zerodds-lint: recursion-depth 64 (CST-Walk; bounded by IDL nesting)
fn collect_descendants<'a, 'src>(
    node: &'a CstNode<'src>,
    target: ProductionId,
) -> Vec<&'a CstNode<'src>> {
    let mut out = Vec::new();
    collect_descendants_into(node, target, &mut out);
    out
}

/// zerodds-lint: recursion-depth 64 (CST-Walk; bounded by IDL nesting)
fn collect_descendants_into<'a, 'src>(
    node: &'a CstNode<'src>,
    target: ProductionId,
    out: &mut Vec<&'a CstNode<'src>>,
) {
    for c in &node.children {
        if c.production() == Some(target) {
            out.push(c);
        } else if c.is_internal() {
            collect_descendants_into(c, target, out);
        }
    }
}

fn build_component_export(node: &CstNode<'_>) -> BuildResult<Vec<ComponentExport>> {
    if let Some(p) = first_internal_child(node, ID_PROVIDES_DCL) {
        let (type_spec, name) = build_provides_dcl(p)?;
        return Ok(vec![ComponentExport::Provides {
            type_spec,
            name,
            span: node.span,
        }]);
    }
    if let Some(u) = first_internal_child(node, ID_USES_DCL) {
        let (type_spec, name, multiple) = build_uses_dcl(u)?;
        return Ok(vec![ComponentExport::Uses {
            type_spec,
            name,
            multiple,
            span: node.span,
        }]);
    }
    if let Some(a) = first_internal_child(node, ID_ATTR_DCL) {
        return Ok(build_attr_dcl(a)?
            .into_iter()
            .map(attr_decl_to_dcl)
            .map(ComponentExport::Attribute)
            .collect());
    }
    if let Some(e) = first_internal_child(node, ID_EMITS_DCL) {
        let (type_spec, name) = build_event_port_dcl(e)?;
        return Ok(vec![ComponentExport::Emits {
            type_spec,
            name,
            span: node.span,
        }]);
    }
    if let Some(p) = first_internal_child(node, ID_PUBLISHES_DCL) {
        let (type_spec, name) = build_event_port_dcl(p)?;
        return Ok(vec![ComponentExport::Publishes {
            type_spec,
            name,
            span: node.span,
        }]);
    }
    if let Some(c) = first_internal_child(node, ID_CONSUMES_DCL) {
        let (type_spec, name) = build_event_port_dcl(c)?;
        return Ok(vec![ComponentExport::Consumes {
            type_spec,
            name,
            span: node.span,
        }]);
    }
    if let Some(p) = first_internal_child(node, ID_PORT_DCL) {
        let (type_spec, name, mirror) = build_port_dcl(p)?;
        return Ok(vec![ComponentExport::Port {
            type_spec,
            name,
            mirror,
            span: node.span,
        }]);
    }
    Err(BuilderError::new("component_export without alt", node.span))
}

fn build_provides_dcl(node: &CstNode<'_>) -> BuildResult<(ScopedName, Identifier)> {
    let it = require_internal_child(node, ID_INTERFACE_TYPE, "interface_type")?;
    let type_spec = build_interface_type(it)?;
    let (text, span) = ident_after(node, ID_INTERFACE_TYPE)?;
    Ok((type_spec, Identifier::new(text, span)))
}

fn build_uses_dcl(node: &CstNode<'_>) -> BuildResult<(ScopedName, Identifier, bool)> {
    let multiple = first_token_text(node, TokenKind::Keyword("multiple")).is_some();
    let it = require_internal_child(node, ID_INTERFACE_TYPE, "interface_type")?;
    let type_spec = build_interface_type(it)?;
    let (text, span) = ident_after(node, ID_INTERFACE_TYPE)?;
    Ok((type_spec, Identifier::new(text, span), multiple))
}

fn build_interface_type(node: &CstNode<'_>) -> BuildResult<ScopedName> {
    if let Some(sn) = first_internal_child(node, ID_SCOPED_NAME) {
        return build_scoped_name(sn);
    }
    if first_token_text(node, TokenKind::Keyword("Object")).is_some() {
        return Ok(ScopedName {
            absolute: false,
            parts: vec![Identifier::new("Object", node.span)],
            span: node.span,
        });
    }
    Err(BuilderError::new("interface_type without alt", node.span))
}

fn build_event_port_dcl(node: &CstNode<'_>) -> BuildResult<(ScopedName, Identifier)> {
    let sn = require_internal_child(node, ID_SCOPED_NAME, "scoped_name")?;
    let type_spec = build_scoped_name(sn)?;
    let (text, span) = ident_after(node, ID_SCOPED_NAME)?;
    Ok((type_spec, Identifier::new(text, span)))
}

fn build_port_dcl(node: &CstNode<'_>) -> BuildResult<(ScopedName, Identifier, bool)> {
    let mirror = first_token_text(node, TokenKind::Keyword("mirrorport")).is_some();
    let sn = require_internal_child(node, ID_SCOPED_NAME, "scoped_name")?;
    let type_spec = build_scoped_name(sn)?;
    let (text, span) = ident_after(node, ID_SCOPED_NAME)?;
    Ok((type_spec, Identifier::new(text, span), mirror))
}

/// Returns the first identifier *after* the first occurrence of a
/// Marker production. Handy for productions like `provides
/// <interface_type> <identifier>`.
fn ident_after<'src>(node: &CstNode<'src>, marker: ProductionId) -> BuildResult<(&'src str, Span)> {
    let mut seen = false;
    for c in &node.children {
        if c.production() == Some(marker) {
            seen = true;
            continue;
        }
        if !seen {
            continue;
        }
        if let CstKind::Token(TokenKind::Ident) = c.kind {
            return Ok((c.text, c.span));
        }
        if c.production() == Some(ID_IDENTIFIER) {
            if let Some((t, s)) = ident_token_in(c) {
                return Ok((t, s));
            }
        }
    }
    Err(BuilderError::new("ident not found after marker", node.span))
}

fn build_home_dcl(node: &CstNode<'_>) -> BuildResult<HomeDcl> {
    let header = require_internal_child(node, ID_HOME_HEADER, "home_header")?;
    let (text, span) = first_ident_text(header)
        .ok_or_else(|| BuilderError::new("home_header without ident", header.span))?;
    let base = match find_descendant(header, ID_HOME_INHERITANCE_SPEC)
        .and_then(|h| find_descendant(h, ID_SCOPED_NAME))
    {
        Some(sn) => Some(build_scoped_name(sn)?),
        None => None,
    };
    let supports = collect_supported_interfaces(header)?;
    // `manages <scoped_name>` — the manage scoped-name comes as the second
    // <scoped_name> node in the header (the first is in the optional inheritance/
    // supports). We find the `manages` keyword index and take the
    // first ScopedName *after* it.
    let manages = find_manages_scoped_name(header)?;
    let primary_key = match find_descendant(header, ID_PRIMARY_KEY_SPEC)
        .and_then(|p| find_descendant(p, ID_SCOPED_NAME))
    {
        Some(sn) => Some(build_scoped_name(sn)?),
        None => None,
    };
    Ok(HomeDcl::Def(HomeDef {
        name: Identifier::new(text, span),
        base,
        supports,
        manages,
        primary_key,
        annotations: Vec::new(),
        span: node.span,
    }))
}

fn find_manages_scoped_name(header: &CstNode<'_>) -> BuildResult<ScopedName> {
    // `manages` comes as a keyword token in the header. ScopedName directly
    // after it (not via inheritance/supports).
    let mut after_manages = false;
    for c in &header.children {
        if let CstKind::Token(TokenKind::Keyword("manages")) = c.kind {
            after_manages = true;
            continue;
        }
        if !after_manages {
            continue;
        }
        if c.production() == Some(ID_SCOPED_NAME) {
            return build_scoped_name(c);
        }
        if c.is_internal() {
            if let Some(sn) = find_descendant(c, ID_SCOPED_NAME) {
                return build_scoped_name(sn);
            }
        }
    }
    Err(BuilderError::new(
        "home_header without manages",
        header.span,
    ))
}

fn build_event_dcl(node: &CstNode<'_>) -> BuildResult<EventDcl> {
    if let Some(fwd) = first_internal_child(node, ID_EVENT_FORWARD_DCL) {
        let (text, span) = first_ident_text(fwd)
            .ok_or_else(|| BuilderError::new("event_forward without ident", fwd.span))?;
        return Ok(EventDcl::Forward(Identifier::new(text, span), node.span));
    }
    if let Some(def) = first_internal_child(node, ID_EVENT_DEF) {
        return Ok(EventDcl::Def(build_event_def(def, node.span)?));
    }
    Err(BuilderError::new("event_dcl without alt", node.span))
}

fn build_event_def(node: &CstNode<'_>, outer_span: Span) -> BuildResult<EventDef> {
    let header = require_internal_child(node, ID_EVENT_HEADER, "event_header")?;
    let kind = if has_keyword_in_tree(header, "abstract") {
        ValueKind::Abstract
    } else if has_keyword_in_tree(header, "custom") {
        ValueKind::Custom
    } else {
        ValueKind::Concrete
    };
    let (text, span) = first_ident_text(header)
        .ok_or_else(|| BuilderError::new("event_header without ident", header.span))?;
    let inheritance = match find_descendant(header, ID_VALUE_INHERITANCE_SPEC) {
        Some(spec) => Some(build_value_inheritance_spec(spec)?),
        None => None,
    };
    let mut elements = Vec::new();
    for c in collect_descendants(node, ID_VALUE_ELEMENT) {
        elements.extend(build_value_element(c)?);
    }
    Ok(EventDef {
        name: Identifier::new(text, span),
        kind,
        inheritance,
        elements,
        annotations: Vec::new(),
        span: outer_span,
    })
}

fn build_porttype_dcl(node: &CstNode<'_>) -> BuildResult<PorttypeDcl> {
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("porttype without ident", node.span))?;
    // The forward variant has no body.
    if first_internal_child(node, ID_PORT_BODY).is_none() {
        return Ok(PorttypeDcl::Forward(Identifier::new(text, span), node.span));
    }
    let mut body = Vec::new();
    for c in collect_descendants(node, ID_PORT_REF) {
        body.extend(build_port_ref(c)?);
    }
    for c in collect_descendants(node, ID_ATTR_DCL) {
        // attr exports under port_export are captured via descendants;
        // here we filter only those that sit directly in the PORT_BODY
        // (not in nested constructs).
        body.extend(
            build_attr_dcl(c)?
                .into_iter()
                .map(attr_decl_to_dcl)
                .map(ComponentExport::Attribute),
        );
    }
    Ok(PorttypeDcl::Def(PorttypeDef {
        name: Identifier::new(text, span),
        body,
        span: node.span,
    }))
}

fn build_port_ref(node: &CstNode<'_>) -> BuildResult<Vec<ComponentExport>> {
    if let Some(p) = first_internal_child(node, ID_PROVIDES_DCL) {
        let (type_spec, name) = build_provides_dcl(p)?;
        return Ok(vec![ComponentExport::Provides {
            type_spec,
            name,
            span: node.span,
        }]);
    }
    if let Some(u) = first_internal_child(node, ID_USES_DCL) {
        let (type_spec, name, multiple) = build_uses_dcl(u)?;
        return Ok(vec![ComponentExport::Uses {
            type_spec,
            name,
            multiple,
            span: node.span,
        }]);
    }
    if let Some(p) = first_internal_child(node, ID_PORT_DCL) {
        let (type_spec, name, mirror) = build_port_dcl(p)?;
        return Ok(vec![ComponentExport::Port {
            type_spec,
            name,
            mirror,
            span: node.span,
        }]);
    }
    Err(BuilderError::new("port_ref without alt", node.span))
}

fn build_connector_dcl(node: &CstNode<'_>) -> BuildResult<ConnectorDcl> {
    let header = require_internal_child(node, ID_CONNECTOR_HEADER, "connector_header")?;
    let (text, span) = first_ident_text(header)
        .ok_or_else(|| BuilderError::new("connector_header without ident", header.span))?;
    let base = match find_descendant(header, ID_CONNECTOR_INHERIT_SPEC)
        .and_then(|h| find_descendant(h, ID_SCOPED_NAME))
    {
        Some(sn) => Some(build_scoped_name(sn)?),
        None => None,
    };
    let mut body = Vec::new();
    for c in collect_descendants(node, ID_CONNECTOR_EXPORT) {
        body.extend(build_connector_export(c)?);
    }
    Ok(ConnectorDcl {
        name: Identifier::new(text, span),
        base,
        body,
        span: node.span,
    })
}

fn build_connector_export(node: &CstNode<'_>) -> BuildResult<Vec<ComponentExport>> {
    if let Some(p) = first_internal_child(node, ID_PROVIDES_DCL) {
        let (type_spec, name) = build_provides_dcl(p)?;
        return Ok(vec![ComponentExport::Provides {
            type_spec,
            name,
            span: node.span,
        }]);
    }
    if let Some(u) = first_internal_child(node, ID_USES_DCL) {
        let (type_spec, name, multiple) = build_uses_dcl(u)?;
        return Ok(vec![ComponentExport::Uses {
            type_spec,
            name,
            multiple,
            span: node.span,
        }]);
    }
    if let Some(p) = first_internal_child(node, ID_PORT_DCL) {
        let (type_spec, name, mirror) = build_port_dcl(p)?;
        return Ok(vec![ComponentExport::Port {
            type_spec,
            name,
            mirror,
            span: node.span,
        }]);
    }
    if let Some(a) = first_internal_child(node, ID_ATTR_DCL) {
        return Ok(build_attr_dcl(a)?
            .into_iter()
            .map(attr_decl_to_dcl)
            .map(ComponentExport::Attribute)
            .collect());
    }
    Err(BuilderError::new("connector_export without alt", node.span))
}

// ============================================================================
// Template Modules (§7.4.12 Rules 184-194)
// ============================================================================

/// zerodds-lint: recursion-depth 64 (a template module body can contain further template_module_dcls; bounded by IDL nesting)
fn build_template_module_dcl(node: &CstNode<'_>, depth: usize) -> BuildResult<TemplateModuleDcl> {
    if depth > MAX_MODULE_NESTING_DEPTH {
        return Err(BuilderError::new(
            format!(
                "module/template nesting exceeds {MAX_MODULE_NESTING_DEPTH} — refusing to build to \
                 protect against stack overflow"
            ),
            node.span,
        ));
    }
    let (text, span) = first_ident_text(node)
        .ok_or_else(|| BuilderError::new("template_module without ident", node.span))?;
    let mut formal_params = Vec::new();
    if let Some(params) = first_internal_child(node, ID_FORMAL_PARAMETERS) {
        for fp in flatten_list(params)
            .iter()
            .filter(|c| c.production() == Some(ID_FORMAL_PARAMETER))
        {
            formal_params.push(build_formal_parameter(fp)?);
        }
    }
    // Body is <tpl_definition>+; each tpl_definition wraps either
    // <definition> or <template_module_ref>. For the AST foundation
    // we only walk the <definition> children.
    let mut definitions = Vec::new();
    for tpl in collect_descendants(node, ID_TPL_DEFINITION) {
        if let Some(def) = first_internal_child(tpl, ID_DEFINITION) {
            definitions.push(build_definition(def, depth)?);
        }
        // We skip template_module_ref alias constructs here;
        // they are handled separately by the validator pass.
    }
    Ok(TemplateModuleDcl {
        name: Identifier::new(text, span),
        formal_params,
        definitions,
        span: node.span,
    })
}

fn build_formal_parameter(node: &CstNode<'_>) -> BuildResult<FormalParam> {
    let kind_node =
        require_internal_child(node, ID_FORMAL_PARAMETER_TYPE, "formal_parameter_type")?;
    let kind = build_formal_parameter_kind(kind_node)?;
    let (text, span) = node
        .children
        .iter()
        .skip_while(|c| c.production() != Some(ID_FORMAL_PARAMETER_TYPE))
        .skip(1)
        .find_map(|c| {
            if let CstKind::Token(TokenKind::Ident) = c.kind {
                Some((c.text, c.span))
            } else if c.production() == Some(ID_IDENTIFIER) {
                ident_token_in(c)
            } else {
                None
            }
        })
        .ok_or_else(|| BuilderError::new("formal_parameter without ident", node.span))?;
    Ok(FormalParam {
        kind,
        name: Identifier::new(text, span),
        span: node.span,
    })
}

fn build_formal_parameter_kind(node: &CstNode<'_>) -> BuildResult<FormalParamKind> {
    if first_token_text(node, TokenKind::Keyword("typename")).is_some() {
        return Ok(FormalParamKind::Typename);
    }
    if first_token_text(node, TokenKind::Keyword("interface")).is_some() {
        return Ok(FormalParamKind::Interface);
    }
    if first_token_text(node, TokenKind::Keyword("valuetype")).is_some() {
        return Ok(FormalParamKind::Valuetype);
    }
    if first_token_text(node, TokenKind::Keyword("eventtype")).is_some() {
        return Ok(FormalParamKind::Eventtype);
    }
    if first_token_text(node, TokenKind::Keyword("struct")).is_some() {
        return Ok(FormalParamKind::Struct);
    }
    if first_token_text(node, TokenKind::Keyword("union")).is_some() {
        return Ok(FormalParamKind::Union);
    }
    if first_token_text(node, TokenKind::Keyword("exception")).is_some() {
        return Ok(FormalParamKind::Exception);
    }
    if first_token_text(node, TokenKind::Keyword("enum")).is_some() {
        return Ok(FormalParamKind::Enum);
    }
    if first_token_text(node, TokenKind::Keyword("sequence")).is_some() {
        return Ok(FormalParamKind::Sequence);
    }
    if let Some(ct) = first_internal_child(node, ID_CONST_TYPE) {
        return Ok(FormalParamKind::Const(build_const_type(ct)?));
    }
    Err(BuilderError::new(
        "formal_parameter_type unrecognized",
        node.span,
    ))
}

fn build_template_module_inst(node: &CstNode<'_>) -> BuildResult<TemplateModuleInst> {
    let sn = require_internal_child(node, ID_SCOPED_NAME, "scoped_name")?;
    let template = build_scoped_name(sn)?;
    // The last identifier is the instance name. Spec §7.4.12 Rule 189:
    // `module <scoped_name> "<" <actual_parameters> ">" <identifier>`.
    let (text, span) = node
        .children
        .iter()
        .rev()
        .find_map(|c| {
            if let CstKind::Token(TokenKind::Ident) = c.kind {
                Some((c.text, c.span))
            } else if c.production() == Some(ID_IDENTIFIER) {
                ident_token_in(c)
            } else {
                None
            }
        })
        .ok_or_else(|| {
            BuilderError::new("template_module_inst without instance ident", node.span)
        })?;
    let actual_params = match first_internal_child(node, ID_ACTUAL_PARAMETERS) {
        Some(list) => flatten_list(list)
            .iter()
            .filter(|c| c.production() == Some(ID_ACTUAL_PARAMETER))
            .map(|c| build_actual_parameter(c))
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    Ok(TemplateModuleInst {
        template,
        actual_params,
        instance_name: Identifier::new(text, span),
        span: node.span,
    })
}

fn build_actual_parameter(node: &CstNode<'_>) -> BuildResult<ConstExpr> {
    if let Some(ce) = first_internal_child(node, ID_CONST_EXPR) {
        return build_const_expr(ce);
    }
    if let Some(ts) = first_internal_child(node, ID_TYPE_SPEC) {
        // Spec §7.4.12 Rule 191 allows both type_spec and
        // const_expr as actual_parameter. ScopedName path: vendor-
        // Reference to an existing type. Primitive path (`long`,
        // `string`, ...): we synthesize a ScopedName from the
        // keyword token set so the AST node has a uniform
        // target. The validator pass can discriminate it later.
        if let Some(scoped) = find_descendant(ts, ID_SCOPED_NAME) {
            return Ok(ConstExpr::Scoped(build_scoped_name(scoped)?));
        }
        let parts = collect_keyword_idents(ts);
        if !parts.is_empty() {
            return Ok(ConstExpr::Scoped(ScopedName {
                absolute: false,
                parts,
                span: ts.span,
            }));
        }
        return Err(BuilderError::new(
            "actual_parameter type_spec without ident",
            ts.span,
        ));
    }
    Err(BuilderError::new("actual_parameter without alt", node.span))
}

/// Collects all keyword tokens in the subtree as pseudo-identifiers — a helper
/// for constructs that want to convert a primitive type-spec like
/// `long`/`unsigned long` into an identifier list.
///
/// zerodds-lint: recursion-depth 64 (CST-Walk; bounded by IDL nesting)
fn collect_keyword_idents(node: &CstNode<'_>) -> Vec<Identifier> {
    let mut out = Vec::new();
    collect_keyword_idents_into(node, &mut out);
    out
}

/// zerodds-lint: recursion-depth 64 (CST-Walk; bounded by IDL nesting)
fn collect_keyword_idents_into(node: &CstNode<'_>, out: &mut Vec<Identifier>) {
    for c in &node.children {
        if let CstKind::Token(TokenKind::Keyword(_)) = c.kind {
            out.push(Identifier::new(c.text, c.span));
        } else if c.is_internal() {
            collect_keyword_idents_into(c, out);
        }
    }
}

/// Adapter: full [`AttrDecl`] (interface pipeline) → lean
/// [`AttrDcl`] (component/home/connector). Drops raises/annotations
/// — these fields are outside the AST in the component pipeline
/// (component validators in §7.4.8.4 use the lean form).
fn attr_decl_to_dcl(d: AttrDecl) -> AttrDcl {
    AttrDcl {
        readonly: d.readonly,
        type_spec: d.type_spec,
        name: d.name,
        span: d.span,
    }
}

// ============================================================================
// Annotation helpers for new definition variants
// ============================================================================

fn set_component_annotations(c: &mut ComponentDcl, annotations: Vec<Annotation>) {
    if let ComponentDcl::Def(d) = c {
        d.annotations = annotations;
    }
}

fn set_home_annotations(h: &mut HomeDcl, annotations: Vec<Annotation>) {
    if let HomeDcl::Def(d) = h {
        d.annotations = annotations;
    }
}

fn set_event_annotations(e: &mut EventDcl, annotations: Vec<Annotation>) {
    if let EventDcl::Def(d) = e {
        d.annotations = annotations;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use super::*;
    use crate::cst::build::build_cst;
    use crate::engine::Engine;
    use crate::grammar::idl42::IDL_42;
    use crate::lexer::Tokenizer;

    fn parse_to_ast(src: &str) -> Specification {
        let tokenizer = Tokenizer::for_grammar(&IDL_42);
        let stream = tokenizer.tokenize(src).expect("lex");
        let engine = Engine::new(&IDL_42);
        let result = engine.recognize(stream.tokens()).expect("parse");
        let cst = build_cst(engine.compiled_grammar(), stream.tokens(), &result).expect("cst");
        build(&cst).expect("ast")
    }

    #[test]
    fn builds_empty_module() {
        let ast = parse_to_ast("module Empty {};");
        assert_eq!(ast.definitions.len(), 1);
        match &ast.definitions[0] {
            Definition::Module(m) => {
                assert_eq!(m.name.text, "Empty");
                assert!(m.definitions.is_empty());
            }
            _ => panic!("expected module"),
        }
    }

    #[test]
    fn builds_struct_with_primitive_member() {
        let ast = parse_to_ast("struct Point { long x; long y; };");
        let def = match &ast.definitions[0] {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => s,
            _ => panic!("expected struct def"),
        };
        assert_eq!(def.name.text, "Point");
        assert_eq!(def.members.len(), 2);
        assert!(matches!(
            def.members[0].type_spec,
            TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::Long))
        ));
    }

    #[test]
    fn builds_struct_forward_decl() {
        let ast = parse_to_ast("struct Foo;");
        match &ast.definitions[0] {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Forward(f)))) => {
                assert_eq!(f.name.text, "Foo");
            }
            _ => panic!("expected struct forward"),
        }
    }

    #[test]
    fn builds_typedef_with_sequence() {
        let ast = parse_to_ast("typedef sequence<long> LongSeq;");
        let td = match &ast.definitions[0] {
            Definition::Type(TypeDecl::Typedef(t)) => t,
            _ => panic!("expected typedef"),
        };
        match &td.type_spec {
            TypeSpec::Sequence(s) => {
                assert!(matches!(
                    *s.elem,
                    TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::Long))
                ));
                assert!(s.bound.is_none());
            }
            _ => panic!("expected sequence"),
        }
        assert_eq!(td.declarators.len(), 1);
        assert_eq!(td.declarators[0].name().text, "LongSeq");
    }

    #[test]
    fn builds_const_decl_with_literal() {
        let ast = parse_to_ast("const long MAX = 100;");
        let cd = match &ast.definitions[0] {
            Definition::Const(c) => c,
            _ => panic!("expected const"),
        };
        assert_eq!(cd.name.text, "MAX");
        assert!(matches!(cd.type_, ConstType::Integer(IntegerType::Long)));
        match &cd.value {
            ConstExpr::Literal(l) => {
                assert_eq!(l.kind, LiteralKind::Integer);
                assert_eq!(l.raw, "100");
            }
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn builds_const_decl_with_arithmetic() {
        let ast = parse_to_ast("const long V = 1 + 2 * 3;");
        let cd = match &ast.definitions[0] {
            Definition::Const(c) => c,
            _ => panic!("expected const"),
        };
        // 1 + (2 * 3): outermost is Add
        match &cd.value {
            ConstExpr::Binary {
                op: BinaryOp::Add, ..
            } => {}
            other => panic!("expected Add at root, got {other:?}"),
        }
    }

    #[test]
    fn builds_enum_with_enumerators() {
        let ast = parse_to_ast("enum Color { RED, GREEN, BLUE };");
        let e = match &ast.definitions[0] {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => e,
            _ => panic!("expected enum"),
        };
        assert_eq!(e.name.text, "Color");
        let names: Vec<_> = e.enumerators.iter().map(|n| n.name.text.as_str()).collect();
        assert_eq!(names, vec!["RED", "GREEN", "BLUE"]);
    }

    #[test]
    fn builds_annotated_struct() {
        let ast = parse_to_ast("@topic struct Foo { @key long id; };");
        let s = match &ast.definitions[0] {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => s,
            _ => panic!("expected struct"),
        };
        assert_eq!(s.annotations.len(), 1);
        assert_eq!(s.annotations[0].name.parts[0].text, "topic");
        assert_eq!(s.members[0].annotations.len(), 1);
        assert_eq!(s.members[0].annotations[0].name.parts[0].text, "key");
    }

    #[test]
    fn builds_interface_with_op() {
        let ast = parse_to_ast("interface Calc { long add(in long a, in long b); };");
        let i = match &ast.definitions[0] {
            Definition::Interface(InterfaceDcl::Def(i)) => i,
            _ => panic!("expected interface"),
        };
        assert_eq!(i.name.text, "Calc");
        assert_eq!(i.exports.len(), 1);
        match &i.exports[0] {
            Export::Op(o) => {
                assert_eq!(o.name.text, "add");
                assert!(!o.oneway);
                assert_eq!(o.params.len(), 2);
                assert_eq!(o.params[0].attribute, ParamAttribute::In);
                assert_eq!(o.params[0].name.text, "a");
            }
            _ => panic!("expected op"),
        }
    }

    #[test]
    fn builds_interface_with_attr() {
        let ast = parse_to_ast("interface Counter { readonly attribute long value; };");
        let i = match &ast.definitions[0] {
            Definition::Interface(InterfaceDcl::Def(i)) => i,
            _ => panic!("expected interface"),
        };
        match &i.exports[0] {
            Export::Attr(a) => {
                assert_eq!(a.name.text, "value");
                assert!(a.readonly);
            }
            _ => panic!("expected attr"),
        }
    }

    #[test]
    fn builds_interface_with_multi_name_attr() {
        // §7.4.3.1 Rule 93 Multi-Name flatten: `attribute long a, b, c;`
        // → drei AttrDecl-Exports (gemeinsamer type/raises/annotations).
        let ast = parse_to_ast("interface Bag { attribute long a, b, c; };");
        let i = match &ast.definitions[0] {
            Definition::Interface(InterfaceDcl::Def(i)) => i,
            _ => panic!("expected interface"),
        };
        assert_eq!(i.exports.len(), 3);
        let names: Vec<&str> = i
            .exports
            .iter()
            .map(|e| match e {
                Export::Attr(a) => a.name.text.as_str(),
                _ => panic!("expected attr"),
            })
            .collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn builds_interface_with_attr_get_set_raises() {
        // §7.4.3.1 Rules 94-97: getraises + setraises in writable attribute.
        let ast = parse_to_ast(
            "interface I { exception E1 {}; exception E2 {}; \
             attribute long val getraises (E1) setraises (E2); };",
        );
        let i = match &ast.definitions[0] {
            Definition::Interface(InterfaceDcl::Def(i)) => i,
            _ => panic!("expected interface"),
        };
        let attr = i
            .exports
            .iter()
            .find_map(|e| match e {
                Export::Attr(a) => Some(a),
                _ => None,
            })
            .expect("expected one attr");
        assert_eq!(attr.name.text, "val");
        assert!(!attr.readonly);
        assert_eq!(attr.get_raises.len(), 1);
        assert_eq!(attr.get_raises[0].parts[0].text, "E1");
        assert_eq!(attr.set_raises.len(), 1);
        assert_eq!(attr.set_raises[0].parts[0].text, "E2");
    }

    #[test]
    fn builds_readonly_attr_with_raises() {
        // §7.4.3.1 Rule 91 alt-1: readonly attribute with raises_expr.
        let ast = parse_to_ast(
            "interface I { exception Err {}; \
             readonly attribute long val raises (Err); };",
        );
        let i = match &ast.definitions[0] {
            Definition::Interface(InterfaceDcl::Def(i)) => i,
            _ => panic!("expected interface"),
        };
        let attr = i
            .exports
            .iter()
            .find_map(|e| match e {
                Export::Attr(a) => Some(a),
                _ => None,
            })
            .expect("expected one attr");
        assert!(attr.readonly);
        assert_eq!(attr.get_raises.len(), 1);
        assert_eq!(attr.get_raises[0].parts[0].text, "Err");
        assert!(attr.set_raises.is_empty());
    }

    #[test]
    fn builds_interface_with_embedded_exception_and_op() {
        // §7.4.4 Rule 97: type/const/except embedded in interface body.
        let ast = parse_to_ast(
            "interface Service { exception NotFound {}; \
             long lookup(in long key) raises (NotFound); };",
        );
        let i = match &ast.definitions[0] {
            Definition::Interface(InterfaceDcl::Def(i)) => i,
            _ => panic!("expected interface"),
        };
        assert_eq!(i.exports.len(), 2);
        match &i.exports[0] {
            Export::Except(e) => assert_eq!(e.name.text, "NotFound"),
            other => panic!("expected exception, got {other:?}"),
        }
        match &i.exports[1] {
            Export::Op(o) => {
                assert_eq!(o.name.text, "lookup");
                assert_eq!(o.raises.len(), 1);
                assert_eq!(o.raises[0].parts[0].text, "NotFound");
            }
            other => panic!("expected op, got {other:?}"),
        }
    }

    #[test]
    fn builds_module_with_nested_struct() {
        let ast = parse_to_ast("module svc { struct Foo { long x; }; };");
        let m = match &ast.definitions[0] {
            Definition::Module(m) => m,
            _ => panic!("expected module"),
        };
        assert_eq!(m.definitions.len(), 1);
    }

    #[test]
    fn builds_struct_with_inheritance() {
        let ast = parse_to_ast("struct Derived : Base { long x; };");
        let s = match &ast.definitions[0] {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => s,
            _ => panic!("expected struct"),
        };
        let base = s.base.as_ref().expect("base set");
        assert_eq!(base.parts[0].text, "Base");
    }

    #[test]
    fn builds_map_typedef() {
        let ast = parse_to_ast("typedef map<string, long> Index;");
        let td = match &ast.definitions[0] {
            Definition::Type(TypeDecl::Typedef(t)) => t,
            _ => panic!("expected typedef"),
        };
        match &td.type_spec {
            TypeSpec::Map(m) => {
                assert!(matches!(
                    *m.value,
                    TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::Long))
                ));
            }
            _ => panic!("expected map"),
        }
    }

    #[test]
    fn builds_bitmask() {
        let ast = parse_to_ast("bitmask Perms { READ, WRITE };");
        let b = match &ast.definitions[0] {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(b))) => b,
            _ => panic!("expected bitmask"),
        };
        let names: Vec<_> = b.values.iter().map(|n| n.name.text.as_str()).collect();
        assert_eq!(names, vec!["READ", "WRITE"]);
    }

    #[test]
    fn builds_union() {
        let ast = parse_to_ast("union V switch (long) { case 1: long a; default: string b; };");
        let u = match &ast.definitions[0] {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => u,
            _ => panic!("expected union"),
        };
        assert_eq!(u.name.text, "V");
        assert_eq!(u.cases.len(), 2);
        assert!(matches!(u.switch_type, SwitchTypeSpec::Integer(_)));
    }

    #[test]
    fn builds_exception() {
        let ast = parse_to_ast(r#"exception NotFound { string what; };"#);
        let e = match &ast.definitions[0] {
            Definition::Except(e) => e,
            _ => panic!("expected exception"),
        };
        assert_eq!(e.name.text, "NotFound");
        assert_eq!(e.members.len(), 1);
    }

    #[test]
    fn builds_value_box_dcl() {
        let ast = parse_to_ast("valuetype MyBox long;");
        match &ast.definitions[0] {
            Definition::ValueBox(v) => {
                assert_eq!(v.name.text, "MyBox");
                assert!(matches!(
                    v.type_spec,
                    TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::Long))
                ));
            }
            _ => panic!("expected value_box"),
        }
    }

    #[test]
    fn annotation_with_single_const_expr() {
        let ast = parse_to_ast("struct S { @id(7) long x; };");
        let s = match &ast.definitions[0] {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => s,
            _ => panic!("expected struct"),
        };
        let ann = &s.members[0].annotations[0];
        match &ann.params {
            AnnotationParams::Single(_) => {}
            other => panic!("expected single, got {other:?}"),
        }
    }

    #[test]
    fn annotation_with_named_params() {
        let ast = parse_to_ast("struct S { @range(min=0, max=100) long x; };");
        let s = match &ast.definitions[0] {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => s,
            _ => panic!("expected struct"),
        };
        let ann = &s.members[0].annotations[0];
        match &ann.params {
            AnnotationParams::Named(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].name.text, "min");
                assert_eq!(items[1].name.text, "max");
            }
            other => panic!("expected named, got {other:?}"),
        }
    }

    // ========================================================================
    // K1 follow-up: builder routing tests for 11 definition variants
    // ========================================================================

    #[test]
    fn builds_value_def_concrete_minimal() {
        let ast = parse_to_ast("valuetype V {};");
        match &ast.definitions[0] {
            Definition::ValueDef(v) => {
                assert_eq!(v.name.text, "V");
                assert_eq!(v.kind, ValueKind::Concrete);
                assert!(v.inheritance.is_none());
                assert!(v.elements.is_empty());
            }
            other => panic!("expected ValueDef, got {other:?}"),
        }
    }

    #[test]
    fn builds_value_def_abstract() {
        let ast = parse_to_ast("abstract valuetype A {};");
        match &ast.definitions[0] {
            Definition::ValueDef(v) => {
                assert_eq!(v.kind, ValueKind::Abstract);
            }
            other => panic!("expected ValueDef, got {other:?}"),
        }
    }

    #[test]
    fn builds_value_def_custom() {
        let ast = parse_to_ast("custom valuetype C { public long x; };");
        match &ast.definitions[0] {
            Definition::ValueDef(v) => {
                assert_eq!(v.kind, ValueKind::Custom);
                assert_eq!(v.elements.len(), 1);
                matches!(v.elements[0], ValueElement::State(_));
            }
            other => panic!("expected ValueDef, got {other:?}"),
        }
    }

    #[test]
    fn builds_value_def_with_truncatable_inheritance() {
        let ast = parse_to_ast("valuetype V : truncatable Base {};");
        match &ast.definitions[0] {
            Definition::ValueDef(v) => {
                let inh = v.inheritance.as_ref().expect("inheritance");
                assert!(inh.truncatable);
                assert_eq!(inh.bases.len(), 1);
                assert_eq!(inh.bases[0].parts[0].text, "Base");
            }
            other => panic!("expected ValueDef, got {other:?}"),
        }
    }

    #[test]
    fn builds_typeid_dcl() {
        let ast = parse_to_ast("typeid Foo \"IDL:Foo:1.0\";");
        match &ast.definitions[0] {
            Definition::TypeId(t) => {
                assert_eq!(t.target.parts[0].text, "Foo");
                assert_eq!(t.repository_id, "IDL:Foo:1.0");
            }
            other => panic!("expected TypeId, got {other:?}"),
        }
    }

    #[test]
    fn builds_typeprefix_dcl() {
        let ast = parse_to_ast("typeprefix Mod \"com/acme\";");
        match &ast.definitions[0] {
            Definition::TypePrefix(t) => {
                assert_eq!(t.target.parts[0].text, "Mod");
                assert_eq!(t.prefix, "com/acme");
            }
            other => panic!("expected TypePrefix, got {other:?}"),
        }
    }

    #[test]
    fn builds_import_dcl_scoped() {
        let ast = parse_to_ast("import ::A::B;");
        match &ast.definitions[0] {
            Definition::Import(i) => match &i.imported {
                ImportedScope::Scoped(sn) => {
                    assert!(sn.absolute);
                    assert_eq!(sn.parts.len(), 2);
                    assert_eq!(sn.parts[0].text, "A");
                    assert_eq!(sn.parts[1].text, "B");
                }
                other => panic!("expected scoped, got {other:?}"),
            },
            other => panic!("expected Import, got {other:?}"),
        }
    }

    #[test]
    fn builds_import_dcl_repository() {
        let ast = parse_to_ast("import \"IDL:Foo:1.0\";");
        match &ast.definitions[0] {
            Definition::Import(i) => match &i.imported {
                ImportedScope::Repository(s) => assert_eq!(s, "IDL:Foo:1.0"),
                other => panic!("expected repository, got {other:?}"),
            },
            other => panic!("expected Import, got {other:?}"),
        }
    }

    #[test]
    fn builds_component_minimal() {
        let ast = parse_to_ast("component C {};");
        match &ast.definitions[0] {
            Definition::Component(ComponentDcl::Def(d)) => {
                assert_eq!(d.name.text, "C");
                assert!(d.base.is_none());
                assert!(d.supports.is_empty());
                assert!(d.body.is_empty());
            }
            other => panic!("expected Component-Def, got {other:?}"),
        }
    }

    #[test]
    fn builds_component_forward_dcl() {
        let ast = parse_to_ast("component Fwd;");
        match &ast.definitions[0] {
            Definition::Component(ComponentDcl::Forward(name, _)) => {
                assert_eq!(name.text, "Fwd");
            }
            other => panic!("expected Component-Forward, got {other:?}"),
        }
    }

    #[test]
    fn builds_component_with_provides_uses() {
        let ast = parse_to_ast("interface I {}; component C { provides I p; uses I u; };");
        let comp = match &ast.definitions[1] {
            Definition::Component(ComponentDcl::Def(d)) => d,
            other => panic!("expected Component, got {other:?}"),
        };
        assert_eq!(comp.body.len(), 2);
        match &comp.body[0] {
            ComponentExport::Provides { name, .. } => assert_eq!(name.text, "p"),
            other => panic!("expected Provides, got {other:?}"),
        }
        match &comp.body[1] {
            ComponentExport::Uses { name, multiple, .. } => {
                assert_eq!(name.text, "u");
                assert!(!multiple);
            }
            other => panic!("expected Uses, got {other:?}"),
        }
    }

    #[test]
    fn builds_home_with_manages() {
        let ast = parse_to_ast("component C {}; home H manages C {};");
        match &ast.definitions[1] {
            Definition::Home(HomeDcl::Def(d)) => {
                assert_eq!(d.name.text, "H");
                assert_eq!(d.manages.parts[0].text, "C");
                assert!(d.primary_key.is_none());
            }
            other => panic!("expected Home-Def, got {other:?}"),
        }
    }

    #[test]
    fn builds_home_with_primary_key() {
        let ast = parse_to_ast("component C {}; valuetype K {}; home H manages C primarykey K {};");
        match &ast.definitions[2] {
            Definition::Home(HomeDcl::Def(d)) => {
                let pk = d.primary_key.as_ref().expect("primary_key");
                assert_eq!(pk.parts[0].text, "K");
            }
            other => panic!("expected Home-Def, got {other:?}"),
        }
    }

    #[test]
    fn builds_eventtype_minimal() {
        let ast = parse_to_ast("eventtype E {};");
        match &ast.definitions[0] {
            Definition::Event(EventDcl::Def(d)) => {
                assert_eq!(d.name.text, "E");
                assert_eq!(d.kind, ValueKind::Concrete);
            }
            other => panic!("expected Event-Def, got {other:?}"),
        }
    }

    #[test]
    fn builds_eventtype_abstract_forward() {
        let ast = parse_to_ast("abstract eventtype AE;");
        match &ast.definitions[0] {
            Definition::Event(EventDcl::Forward(name, _)) => {
                assert_eq!(name.text, "AE");
            }
            other => panic!("expected Event-Forward, got {other:?}"),
        }
    }

    #[test]
    fn builds_porttype_def() {
        let ast = parse_to_ast("interface I {}; porttype P { provides I p; };");
        match &ast.definitions[1] {
            Definition::Porttype(PorttypeDcl::Def(d)) => {
                assert_eq!(d.name.text, "P");
                assert_eq!(d.body.len(), 1);
                matches!(d.body[0], ComponentExport::Provides { .. });
            }
            other => panic!("expected Porttype-Def, got {other:?}"),
        }
    }

    #[test]
    fn builds_porttype_forward_dcl() {
        let ast = parse_to_ast("porttype Pf;");
        match &ast.definitions[0] {
            Definition::Porttype(PorttypeDcl::Forward(name, _)) => {
                assert_eq!(name.text, "Pf");
            }
            other => panic!("expected Porttype-Forward, got {other:?}"),
        }
    }

    #[test]
    fn builds_connector_minimal() {
        let ast = parse_to_ast("interface I {}; connector C { provides I p; };");
        match &ast.definitions[1] {
            Definition::Connector(d) => {
                assert_eq!(d.name.text, "C");
                assert_eq!(d.body.len(), 1);
            }
            other => panic!("expected Connector, got {other:?}"),
        }
    }

    #[test]
    fn builds_template_module_with_typename_param() {
        let ast = parse_to_ast("module M<typename T> { typedef T Alias; };");
        match &ast.definitions[0] {
            Definition::TemplateModule(t) => {
                assert_eq!(t.name.text, "M");
                assert_eq!(t.formal_params.len(), 1);
                assert_eq!(t.formal_params[0].name.text, "T");
                matches!(t.formal_params[0].kind, FormalParamKind::Typename);
                assert_eq!(t.definitions.len(), 1);
            }
            other => panic!("expected TemplateModule, got {other:?}"),
        }
    }

    #[test]
    fn builds_template_module_inst() {
        let ast = parse_to_ast("module M<typename T> { typedef T A; }; module M<long> ML;");
        match &ast.definitions[1] {
            Definition::TemplateModuleInst(i) => {
                assert_eq!(i.template.parts[0].text, "M");
                assert_eq!(i.instance_name.text, "ML");
                assert_eq!(i.actual_params.len(), 1);
            }
            other => panic!("expected TemplateModuleInst, got {other:?}"),
        }
    }

    // ========================================================================
    // K1.9 coverage audit: missing tests for §7.4.10/11 r158/r182
    // ========================================================================

    #[test]
    fn builds_component_with_uses_multiple() {
        // §7.4.10.3 r158 — `uses multiple <iface_type> <name>`.
        let ast = parse_to_ast("interface I {}; component C { uses multiple I src; };");
        let comp = match &ast.definitions[1] {
            Definition::Component(ComponentDcl::Def(d)) => d,
            other => panic!("expected Component, got {other:?}"),
        };
        assert_eq!(comp.body.len(), 1);
        match &comp.body[0] {
            ComponentExport::Uses { name, multiple, .. } => {
                assert_eq!(name.text, "src");
                assert!(*multiple, "expected multiple=true");
            }
            other => panic!("expected Uses, got {other:?}"),
        }
    }

    #[test]
    fn builds_connector_with_inheritance() {
        // §7.4.11.3 r182 — `connector_inherit_spec ::= ":" scoped_name`.
        let ast = parse_to_ast(
            "interface I {}; connector Base { provides I p; }; \
             connector Derived : Base { provides I q; };",
        );
        let conn = match &ast.definitions[2] {
            Definition::Connector(c) => c,
            other => panic!("expected Connector, got {other:?}"),
        };
        assert_eq!(conn.name.text, "Derived");
        let base = conn.base.as_ref().expect("base");
        assert_eq!(base.parts[0].text, "Base");
    }

    #[test]
    fn builds_porttype_with_attr_export() {
        // §7.4.11.3 r177 — `port_export ::= port_ref | attr_dcl ";"`.
        let ast =
            parse_to_ast("interface I {}; porttype P { provides I p; attribute long counter; };");
        let pt = match &ast.definitions[1] {
            Definition::Porttype(PorttypeDcl::Def(d)) => d,
            other => panic!("expected Porttype-Def, got {other:?}"),
        };
        assert_eq!(pt.body.len(), 2);
        let has_attr = pt
            .body
            .iter()
            .any(|ex| matches!(ex, ComponentExport::Attribute(_)));
        assert!(has_attr, "expected Attribute export, got {:?}", pt.body);
    }
}

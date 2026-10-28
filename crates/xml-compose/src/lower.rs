// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The DDS-XML `<types>` tree lowered into the common AST.
//!
//! Two passes over every document, then two completion passes over the
//! model.
//!
//! **Declare** creates one node per named type, so that a member written
//! before its type is declared has something to point at. Enumerations
//! and bitmasks are finished here — neither names another type — and
//! structs, unions, aliases and bitsets are created forward and given
//! their bodies in the second pass.
//!
//! **Define** fills those bodies. Every type reference is resolved
//! against the names the first pass recorded, innermost scope first, so
//! an anonymous collection built at the use site holds a resolved
//! element rather than a pending name that no enclosing-scope walk could
//! later reach — the anonymous node sits in the root module and its
//! scope is not the writing member's.
//!
//! **Resolve** then runs `Ast::resolve_all_by_name` for whatever is
//! left, and **complete** fills what the document left implicit: the
//! caller's default extensibility where the document said nothing, and
//! every member id, by the same rule and with the same hash the IDL
//! front end uses. Two declarations of one type, one in `.idl` and one
//! in `.xml`, have to reach the same `TypeIdentifier`, and they cannot
//! if the two paths number members differently.

use std::collections::{BTreeMap, BTreeSet};

use zerodds_ast::node::ModuleNode;
use zerodds_ast::{
    AnyId, AppliedAnnotation, AppliedArgument, Ast, AutoId, Bound, CaseLabel, CharWidth, ConstExpr,
    ConstExprNode, EvalError, Extensibility, Facets, Literal, MemberIdAssignment, MemberIdOrigin,
    MemberSpec, PrimitiveKind, SourceFacet, TryConstruct, TypeKind, TypeNode, VerbatimDirective,
    VerbatimPlacement, XTypesFacet, XmlFacet,
};
use zerodds_ast_core::{Id, PendingRef, Ref, ScopedName};
use zerodds_xml::{DdsXmlDocument, XmlElement, parse_xml_tree};

use crate::ComposeOptions;
use crate::expr::{self, Expression};

/// How bad one finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The document says something this loader cannot carry, and the
    /// model is missing it.
    Error,
    /// The model is whole; something was noted on the way.
    Warning,
}

/// One thing wrong with a document.
///
/// Not `zerodds_idl_parse::Diagnostic`: that one carries a byte span
/// into an IDL token stream and a `DiagnosticCode` naming IDL grammar
/// productions. An XML finding points at an element path, and saying so
/// is better than a span of `0..0` that pretends to be a source
/// location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// How bad it is.
    pub severity: Severity,
    /// What is wrong, in a sentence.
    pub message: String,
    /// The document, and the element path inside it —
    /// `types/extensibility.xml: dds/types/module[Test]/struct[struct_m1]`.
    pub at: String,
    /// The clause the rule comes from.
    pub spec_reference: &'static str,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {} [{}]", self.at, self.message, self.spec_reference)
    }
}

/// Lower every unit into one model, and report what went wrong.
pub(crate) fn lower_all(
    ast: &mut Ast,
    sources: &[(String, String)],
    options: &ComposeOptions,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let units = load_units(sources, &options.include_paths, &mut diagnostics);

    let mut lowering = Lowering {
        ast,
        options: options.clone(),
        unit: String::new(),
        diagnostics,
        declared: BTreeMap::new(),
        builtin: BTreeMap::new(),
    };

    // Every named type in every unit exists before any body is filled,
    // so a forward reference within a file and one across two files are
    // the same case.
    for (name, doc) in &units {
        lowering.unit = name.clone();
        lowering.walk_units(doc, Pass::Declare);
    }
    for (name, doc) in &units {
        lowering.unit = name.clone();
        lowering.walk_units(doc, Pass::Define);
    }
    lowering.unit = units
        .first()
        .map_or_else(|| String::from("<bundle>"), |(name, _)| name.clone());
    lowering.resolve();
    lowering.complete();
    lowering.diagnostics
}

/// The documents to lower, each ahead of the ones that include it.
///
/// DDS-XTypes 1.3 §7.3.2.1.1: "A file inclusion specified as in this Type
/// Representation shall be considered equivalent to an IDL #include of the
/// same file", and "Conformant Type Representation compilers need not
/// support the inclusion of files of other Type Representations from
/// within an XML Type Representation document". So an `<include
/// file="x.xml"/>` brings that document into the model, once however many
/// documents include it, and an `<include>` of a file of another
/// representation is reported. The file is looked for beside the document
/// that names it.
fn load_units(
    sources: &[(String, String)],
    include_paths: &[std::path::PathBuf],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<(String, DdsXmlDocument)> {
    let mut units = Vec::with_capacity(sources.len());
    // What the caller names is loaded in its own turn, so an include of
    // it is not a second copy.
    let mut seen: BTreeSet<String> = sources.iter().map(|(name, _)| document_key(name)).collect();
    for (name, text) in sources {
        load_one(
            name,
            text,
            &mut units,
            &mut Loading {
                seen: &mut seen,
                include_paths,
            },
            diagnostics,
            0,
        );
    }
    units
}

/// How deeply includes nest before the chain is taken for a runaway.
const MAX_INCLUDE_DEPTH: usize = 32;

/// What a load carries from document to document.
struct Loading<'a> {
    /// The documents already taken, so that one included twice is one.
    seen: &'a mut BTreeSet<String>,
    /// The directories searched after the including document's own
    /// (DDS-XTypes 1.3 §7.5.2.2.13).
    include_paths: &'a [std::path::PathBuf],
}

/// Where an included file is: beside the document that names it, then in
/// each include path; the first that exists, or the first candidate when
/// none does (the read then names the file that was expected).
fn locate_include(
    including: &str,
    file: &str,
    include_paths: &[std::path::PathBuf],
) -> std::path::PathBuf {
    let beside = std::path::Path::new(including)
        .parent()
        .map_or_else(|| std::path::PathBuf::from(file), |dir| dir.join(file));
    let mut candidates = vec![beside];
    candidates.extend(include_paths.iter().map(|dir| dir.join(file)));
    candidates
        .iter()
        .find(|candidate| candidate.is_file())
        .cloned()
        .unwrap_or_else(|| candidates.swap_remove(0))
}

/// zerodds-lint: recursion-depth 32 (an include nests at most
/// `MAX_INCLUDE_DEPTH` levels; a deeper one is an error diagnostic)
fn load_one(
    name: &str,
    text: &str,
    units: &mut Vec<(String, DdsXmlDocument)>,
    loading: &mut Loading<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    depth: usize,
) {
    let document = match parse_xml_tree(text) {
        Ok(document) => document,
        Err(error) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!("the document is not well-formed DDS-XML: {error:?}"),
                at: name.to_owned(),
                spec_reference: "OMG DDS-XML 1.0 §7.1",
            });
            return;
        }
    };
    let mut files = Vec::new();
    let mut deeper = false;
    if let Ok(libraries) = document.library_elements("types") {
        for library in libraries {
            collect_includes(library, &mut files, 0, &mut deeper);
            check_identifier_names(library, name, diagnostics);
        }
    }
    if deeper {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: format!("modules nest deeper than {MAX_INCLUDE_DEPTH} levels; the includes below are not read"),
            at: name.to_owned(),
            spec_reference: "OMG DDS-XTypes 1.3 §7.3.2.1.1",
        });
    }
    for file in files {
        let path = locate_include(name, &file, loading.include_paths);
        let included = path.to_string_lossy().into_owned();
        if !std::path::Path::new(&file)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
        {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!(
                    "`{file}` is not an XML document; including a file of another Type \
                     Representation is not supported, and §7.3.2.1.1 does not require it"
                ),
                at: format!("{name}: include[{file}]"),
                spec_reference: "OMG DDS-XTypes 1.3 §7.3.2.1.1",
            });
            continue;
        }
        if !loading.seen.insert(document_key(&included)) {
            continue;
        }
        if depth >= MAX_INCLUDE_DEPTH {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!("includes nest deeper than {MAX_INCLUDE_DEPTH} levels"),
                at: format!("{name}: include[{file}]"),
                spec_reference: "OMG DDS-XTypes 1.3 §7.3.2.1.1",
            });
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(included_text) => {
                load_one(
                    &included,
                    &included_text,
                    units,
                    loading,
                    diagnostics,
                    depth + 1,
                );
            }
            Err(error) => diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!("the included file cannot be read: {error}"),
                at: format!("{name}: include[{file}]"),
                spec_reference: "OMG DDS-XTypes 1.3 §7.3.2.1.1",
            }),
        }
    }
    units.push((name.to_owned(), document));
}

/// The attributes whose type in the schema of Annex A is `identifierName`
/// (`([a-zA-Z]|::)([a-zA-Z_0-9]|::)*`, S. 243): the `name` of every
/// declaration, member, enumerator, flag and application, and the three
/// attributes that name a type.
const IDENTIFIER_ATTRIBUTES: [&str; 4] = [
    "name",
    "baseType",
    "nonBasicTypeName",
    "mapKeyNonBasicTypeName",
];

/// Reports every `identifierName` attribute of a `<types>` library that does
/// not match the pattern of Annex A, an error each, with the path of the
/// element.
///
/// The library's own `name` is no `identifierName` in the schema (`typeLibrary`
/// has no attributes) and is left alone.
fn check_identifier_names(library: &XmlElement, document: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut stack: Vec<&XmlElement> = library.children.iter().collect();
    while let Some(element) = stack.pop() {
        for attribute in IDENTIFIER_ATTRIBUTES {
            if let Some(value) = element.attribute(attribute) {
                if !zerodds_xml::is_identifier_name(value) {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "{}@{attribute}=\"{value}\" is no identifierName (a letter or `::` \
                             first, then letters, digits, `_` and `::`)",
                            element.path
                        ),
                        at: document.to_owned(),
                        spec_reference: "OMG DDS-XTypes 1.3 Annex A, identifierName",
                    });
                }
            }
        }
        stack.extend(element.children.iter());
    }
}

/// The `file` of every `<include>` in a library, inside modules too.
///
/// zerodds-lint: recursion-depth 32 (module nesting; past `MAX_INCLUDE_DEPTH`
/// modules the walk stops and says so in `deeper`)
fn collect_includes(element: &XmlElement, out: &mut Vec<String>, depth: usize, deeper: &mut bool) {
    if depth > MAX_INCLUDE_DEPTH {
        *deeper = true;
        return;
    }
    for child in &element.children {
        match child.name.as_str() {
            "include" => {
                if let Some(file) = child.attribute("file") {
                    out.push(file.to_owned());
                }
            }
            "module" => collect_includes(child, out, depth + 1, deeper),
            _ => {}
        }
    }
}

/// A name one document is known by, however a path to it is spelled.
fn document_key(name: &str) -> String {
    std::fs::canonicalize(name).map_or_else(
        |_| name.to_owned(),
        |path| path.to_string_lossy().into_owned(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pass {
    Declare,
    Define,
}

struct Lowering<'a> {
    ast: &'a mut Ast,
    options: ComposeOptions,
    unit: String,
    diagnostics: Vec<Diagnostic>,
    /// Every named type, by its absolute path from the root.
    declared: BTreeMap<Vec<String>, Id<TypeNode>>,
    /// One node per primitive per model, as the IDL front end does it: a
    /// document with a hundred `int32` members must not hold a hundred
    /// primitive types.
    builtin: BTreeMap<PrimitiveKind, Id<TypeNode>>,
}

/// Where in the document a node sits, for both name resolution and
/// diagnostics.
#[derive(Clone)]
struct Scope {
    /// The module path from the root, outermost first.
    path: Vec<String>,
    /// The module node itself.
    module: Id<ModuleNode>,
    /// The element path, for a diagnostic.
    trail: String,
}

impl<'a> Lowering<'a> {
    // -- walking ---------------------------------------------------------

    fn walk_units(&mut self, doc: &DdsXmlDocument, pass: Pass) {
        let root = self.ast.root_module();
        let scope = Scope {
            path: Vec::new(),
            module: root,
            trail: format!("{}: {}", self.unit, doc.root.name),
        };
        match doc.library_elements("types") {
            Ok(libraries) => {
                for library in libraries {
                    let scope = Scope {
                        trail: format!("{}/types", scope.trail),
                        ..scope.clone()
                    };
                    if pass == Pass::Declare {
                        self.constants_in_order(&library.children, &scope, None);
                    }
                    for child in &library.children {
                        self.declaration(child, &scope, pass);
                    }
                }
            }
            Err(error) => self.error(
                &scope.trail,
                format!("no <types> to read: {error:?}"),
                "OMG DDS-XML 1.0 §7.3.3.3",
            ),
        }
    }

    /// The `<const>` children of one scope, each after the constants its
    /// value names (§7.3.2.1.3: a `<const_exp>` may name a constant; the
    /// document may write them in any order). A constant that waits on
    /// itself or on a cycle is lowered anyway, in document order, and the
    /// resolve pass reports the name that does not resolve.
    fn constants_in_order(&mut self, children: &[XmlElement], scope: &Scope, within: Option<&str>) {
        let mut remaining: Vec<&XmlElement> =
            children.iter().filter(|c| c.name == "const").collect();
        while !remaining.is_empty() {
            let declared: Vec<String> = remaining
                .iter()
                .filter_map(|c| c.attribute("name").map(str::to_owned))
                .collect();
            let ready = remaining.iter().position(|c| {
                let own = c.attribute("name");
                let mut named = Vec::new();
                if let Some(Ok(parsed)) = c.attribute("value").map(expr::parse) {
                    names_in(&parsed, &mut named);
                }
                !named
                    .iter()
                    .any(|name| Some(name.as_str()) != own && declared.contains(name))
            });
            let at = ready.unwrap_or(0);
            let el = remaining.remove(at);
            self.constant(el, scope, within);
        }
    }

    /// One `<types>` child, in whichever pass is running.
    fn declaration(&mut self, el: &XmlElement, scope: &Scope, pass: Pass) {
        match el.name.as_str() {
            "module" => self.module(el, scope, pass),
            "struct" => self.structure(el, scope, pass),
            "union" => self.union(el, scope, pass),
            "enum" => match pass {
                Pass::Declare => self.enumeration(el, scope),
                // Annotations are applied once every `<annotation>` of the
                // bundle is declared.
                Pass::Define => self.annotate_enumerated(el, scope, "enumerator"),
            },
            "bitmask" => match pass {
                Pass::Declare => self.bitmask(el, scope),
                Pass::Define => self.annotate_enumerated(el, scope, "flag"),
            },
            "typedef" => self.alias(el, scope, pass),
            "forward_dcl" | "forwardDcl" => {
                if pass == Pass::Declare {
                    self.forward_declaration(el, scope);
                }
            }
            // Read before the passes (`load_units`): the included document
            // is one of the units, ahead of this one.
            "include" => {}
            // Declared ahead of the other children of their scope, in the
            // order their values need (`constants_in_order`).
            "const" => {}
            "bitset" => self.bitset(el, scope, pass),
            "annotation" => {
                if pass == Pass::Declare {
                    self.annotation_declaration(el, scope);
                }
            }
            other => {
                if pass == Pass::Declare {
                    self.error(
                        &format!("{}/{other}", scope.trail),
                        format!("<{other}> is not a type declaration"),
                        "OMG DDS-XTypes 1.3 Annex A",
                    );
                }
            }
        }
    }

    fn module(&mut self, el: &XmlElement, scope: &Scope, pass: Pass) {
        let Some(name) = self.named(el, scope) else {
            return;
        };
        let Ok(module) = self.ast.ensure_module(scope.module, &name) else {
            self.error(
                &scope.trail,
                format!("the model refused a module named `{name}`"),
                "zerodds-ast-1.0 §3.1",
            );
            return;
        };
        let mut path = scope.path.clone();
        path.push(name.clone());
        let inner = Scope {
            trail: format!("{}/module[{name}]", scope.trail),
            path,
            module,
        };
        if pass == Pass::Declare {
            if let Ok(node_id) = self.ast.module(module).map(|node| node.node_id) {
                self.set_xml_facet(AnyId::Module(module, node_id), el);
                // Annex A `moduleDecl/@autoid`: "Module declarations" are
                // among the targets of `@autoid` (§7.3.1.2.1.1, Table 21),
                // and the value governs the aggregated types declared in the
                // module that carry none of their own.
                if let Some(autoid) = self.autoid_of(el, &inner.trail) {
                    if let Some(facets) = self.ast.facets_mut(AnyId::Module(module, node_id)) {
                        facets.xtypes_mut().autoid = Some(autoid);
                    }
                }
            }
        }
        if pass == Pass::Declare {
            self.constants_in_order(&el.children, &inner, None);
        }
        for child in &el.children {
            self.declaration(child, &inner, pass);
        }
    }

    // -- declarations ----------------------------------------------------

    fn structure(&mut self, el: &XmlElement, scope: &Scope, pass: Pass) {
        let Some(name) = self.named(el, scope) else {
            return;
        };
        let trail = format!("{}/struct[{name}]", scope.trail);
        if pass == Pass::Declare {
            let placeholder = TypeKind::Structure {
                base: None,
                members: Vec::new(),
            };
            self.declare(&name, scope, placeholder, el, &trail);
            return;
        }
        let Some(id) = self.declared_here(&name, scope) else {
            return;
        };
        let base = el
            .attribute("baseType")
            .or_else(|| el.attribute("base_type"))
            .map(|written| self.named_ref(written, scope, &trail));
        if self
            .ast
            .complete_type(
                id,
                TypeKind::Structure {
                    base,
                    members: Vec::new(),
                },
            )
            .is_err()
        {
            self.error(
                &trail,
                format!("`{name}` is declared twice"),
                "OMG IDL 4.2 §7.2.3",
            );
            return;
        }
        // The names a constant or a bound inside the structure resolves
        // against start at the structure: `MyStructure::MY_CONSTANT`.
        let mut inner_path = scope.path.clone();
        inner_path.push(name.clone());
        let inner = Scope {
            path: inner_path,
            ..scope.clone()
        };
        let annotations = self.applied_annotations(el, &inner, &trail);
        let verbatim = self.verbatim_directives(el, &trail);
        self.attach_to_type(id, annotations, verbatim);
        // Constants first: a member's bound may name one declared after it.
        self.constants_in_order(&el.children, &inner, Some(&name));
        for child in &el.children {
            match child.name.as_str() {
                "member" => {
                    let spec = self.member_spec(child, &inner, &trail);
                    let Some(spec) = spec else { continue };
                    if self.ast.add_member(id, spec).is_err() {
                        self.error(
                            &trail,
                            String::from("the model refused a member"),
                            "zerodds-ast-1.0 §3.3",
                        );
                    }
                }
                // Read above: the constants, and the annotations and
                // verbatim text of the structure as a whole.
                "const" | "annotate" | "verbatim" => {}
                other => self.warning(
                    &trail,
                    format!("<{other}> is no child of a <struct> and is not read"),
                    "OMG DDS-XTypes 1.3 Annex A",
                ),
            }
        }
    }

    fn union(&mut self, el: &XmlElement, scope: &Scope, pass: Pass) {
        let Some(name) = self.named(el, scope) else {
            return;
        };
        let trail = format!("{}/union[{name}]", scope.trail);
        if pass == Pass::Declare {
            let placeholder = TypeKind::Union {
                discriminator: Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(
                    Vec::new(),
                ))),
                cases: Vec::new(),
                default_case: None,
            };
            self.declare(&name, scope, placeholder, el, &trail);
            return;
        }
        let Some(id) = self.declared_here(&name, scope) else {
            return;
        };
        let Some(discriminator_el) = el.child("discriminator") else {
            self.error(
                &trail,
                String::from("a union needs a <discriminator>"),
                "OMG DDS-XTypes 1.3 Annex A",
            );
            return;
        };
        // `key="true"` on a union discriminator: the one construct this
        // loader answers on an axis rather than by itself. Table 21
        // permits it, so `KeyOnUnionDiscriminator::Carried` — the
        // default — records it; `Refused`, for a tool that has to stay
        // within what the measured toolchains accept, reports it here,
        // exactly as the IDL front end does for `@key` in the same
        // position. The two paths have to
        // answer alike: a type declared once in IDL and once in XML must
        // reach the same `TypeIdentifier`, and the key bit is part of
        // it.
        let discriminator_is_key = truthy(discriminator_el.attribute("key"));
        if discriminator_is_key
            && self.options.key_on_union_discriminator == crate::KeyOnUnionDiscriminator::Refused
        {
            self.error(
                &trail,
                String::from(
                    "`key=\"true\"` on a union discriminator is not carried under \
                     `KeyOnUnionDiscriminator::Refused`: the clause permits it and no measured \
                     implementation accepts it, so this setting reports it rather than \
                     computing a KeyHash no peer computes",
                ),
                "DDS-XTypes 1.3 §7.2.2.4.4.4.8, Table 21",
            );
            // The node stays in the model with a broken discriminator
            // rather than as a forward declaration nobody completes: a
            // reference left pending would be reported a second time by
            // the resolve pass, under a clause that has nothing to do
            // with the refusal, and the reader would chase the wrong
            // rule.
            let _ = self.ast.complete_type(
                id,
                TypeKind::Union {
                    discriminator: broken("key discriminator"),
                    cases: Vec::new(),
                    default_case: None,
                },
            );
            return;
        }
        let discriminator = self.type_reference(
            discriminator_el,
            scope,
            &trail,
            "type",
            "nonBasicTypeName",
            "stringMaxLength",
        );
        if self
            .ast
            .complete_type(
                id,
                TypeKind::Union {
                    discriminator: discriminator.clone(),
                    cases: Vec::new(),
                    default_case: None,
                },
            )
            .is_err()
        {
            self.error(
                &trail,
                format!("`{name}` is declared twice"),
                "OMG IDL 4.2 §7.2.3",
            );
            return;
        }
        if let Some(behaviour) = self.try_construct(discriminator_el, &trail) {
            self.set_type_xtypes(id, |x| x.discriminator_try_construct = Some(behaviour));
        }
        // §7.3.1.2.5: annotations "can be applied to the implicit
        // discriminator member of a union type"; the schema's
        // `discriminatorDecl` holds `annotate` for them.
        let on_discriminator = self.applied_annotations(discriminator_el, scope, &trail);
        if !on_discriminator.is_empty() {
            self.set_type_xtypes(id, |x| x.discriminator_annotations.extend(on_discriminator));
        }
        let annotations = self.applied_annotations(el, scope, &trail);
        let verbatim = self.verbatim_directives(el, &trail);
        self.attach_to_type(id, annotations, verbatim);
        if discriminator_is_key {
            self.set_type_xtypes(id, |x| x.discriminator_is_key = true);
        }

        let discriminator_type = discriminator.resolved();
        for case in el.children_named("case") {
            let mut labels = Vec::new();
            for label in case.children_named("caseDiscriminator") {
                let Some(written) = label.attribute("value") else {
                    self.error(
                        &trail,
                        String::from("a <caseDiscriminator> needs a `value`"),
                        "OMG DDS-XTypes 1.3 Annex A",
                    );
                    continue;
                };
                match self.case_label(written, discriminator_type, &trail) {
                    Some(parsed) => labels.push(parsed),
                    // `case 1: case 2: long x;` reached by one label is
                    // a different union: the value the lost label
                    // selected now falls to `default:`, or to nothing.
                    // The whole case goes rather than half of it, and
                    // `case_label` has already said which label and why.
                    None => {
                        labels.clear();
                        break;
                    }
                }
            }
            if labels.is_empty() {
                self.error(
                    &trail,
                    String::from("a <case> needs at least one <caseDiscriminator>"),
                    "OMG DDS-XTypes 1.3 Annex A",
                );
                continue;
            }
            let Some(member_el) = case.child("member") else {
                self.error(
                    &trail,
                    String::from("a <case> needs a <member>"),
                    "OMG DDS-XTypes 1.3 Annex A",
                );
                continue;
            };
            let Some(spec) = self.member_spec(member_el, scope, &trail) else {
                continue;
            };
            if self.ast.add_case(id, labels, spec).is_err() {
                self.error(
                    &trail,
                    String::from("the model refused a case"),
                    "zerodds-ast-1.0 §3.3",
                );
            }
        }
    }

    fn enumeration(&mut self, el: &XmlElement, scope: &Scope) {
        let Some(name) = self.named(el, scope) else {
            return;
        };
        let trail = format!("{}/enum[{name}]", scope.trail);
        let kind = TypeKind::Enumeration {
            enumerators: Vec::new(),
        };
        let Some(id) = self.declare_now(&name, scope, kind, el, &trail) else {
            return;
        };
        for literal in el.children_named("enumerator") {
            let Some(literal_name) = literal.attribute("name") else {
                self.error(
                    &trail,
                    String::from("an <enumerator> needs a `name`"),
                    "OMG DDS-XTypes 1.3 Annex A",
                );
                continue;
            };
            let literal_name = literal_name.to_owned();
            // Absent, not zero: an enumerator without a value is
            // positional, and `Ast::effective_enumerator_value` applies
            // the rule. Writing a value in would erase the difference
            // between `value="0"` and silence.
            let value = match literal.attribute("value") {
                // §7.3.2.7.1: "The syntax of the value shall be defined by
                // the <const_exp> production in the IDL grammar". A value
                // that does not parse must not leave the enumerator
                // looking positional: `value="7"` silently becoming
                // "previous plus one" changes the ordinal on the wire.
                Some(written) => {
                    match self.const_exp(written, scope, &trail, "OMG DDS-XTypes 1.3 §7.3.2.7.1") {
                        Some(expr) => Some(expr),
                        None => continue,
                    }
                }
                None => None,
            };
            let Ok(enumerator) = self.ast.add_enumerator(id, &literal_name, value) else {
                self.error(
                    &trail,
                    format!("the model refused enumerator `{literal_name}`"),
                    "zerodds-ast-1.0 §3.3",
                );
                continue;
            };
            if truthy(literal.attribute("defaultLiteral"))
                || truthy(literal.attribute("default_literal"))
            {
                if let Ok(node_id) = self.ast.enumerator(enumerator).map(|node| node.node_id) {
                    if let Some(facets) =
                        self.ast.facets_mut(AnyId::Enumerator(enumerator, node_id))
                    {
                        facets.xtypes_mut().is_default_literal = true;
                    }
                }
            }
        }
    }

    fn bitmask(&mut self, el: &XmlElement, scope: &Scope) {
        let Some(name) = self.named(el, scope) else {
            return;
        };
        let trail = format!("{}/bitmask[{name}]", scope.trail);
        let kind = TypeKind::Bitmask { bits: Vec::new() };
        let Some(id) = self.declare_now(&name, scope, kind, el, &trail) else {
            return;
        };
        // The OMG suite writes `<flag name="…" value="…"/>`; DDS-XML's
        // own Types building block and `zerodds-xml`'s parser write
        // `<bit_value name="…" position="…"/>`. Both spellings name the
        // same construct, and a loader that took only one would read
        // half the corpus.
        let bits: Vec<&XmlElement> = el
            .children
            .iter()
            .filter(|c| c.name == "flag" || c.name == "bit_value" || c.name == "bit")
            .collect();
        for bit in bits {
            let Some(bit_name) = bit.attribute("name") else {
                self.error(
                    &trail,
                    format!("a <{}> needs a `name`", bit.name),
                    "OMG DDS-XTypes 1.3 Annex A",
                );
                continue;
            };
            let bit_name = bit_name.to_owned();
            let written = bit.attribute("position").or_else(|| bit.attribute("value"));
            let position = match written {
                Some(written) => match integer(written).and_then(|v| u16::try_from(v).ok()) {
                    Some(position) => Some(position),
                    None => {
                        self.error(
                            &trail,
                            format!("`{written}` is not a bit position"),
                            "OMG DDS-XTypes 1.3 §7.3.1.2.1.9",
                        );
                        continue;
                    }
                },
                None => None,
            };
            if self.ast.add_bit_value(id, &bit_name, position).is_err() {
                self.error(
                    &trail,
                    format!("the model refused bit `{bit_name}`"),
                    "zerodds-ast-1.0 §3.3",
                );
            }
        }
    }

    fn alias(&mut self, el: &XmlElement, scope: &Scope, pass: Pass) {
        let Some(name) = self.named(el, scope) else {
            return;
        };
        let trail = format!("{}/typedef[{name}]", scope.trail);
        if pass == Pass::Declare {
            let placeholder = TypeKind::Alias {
                aliased: Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(Vec::new()))),
            };
            self.declare(&name, scope, placeholder, el, &trail);
            return;
        }
        let Some(id) = self.declared_here(&name, scope) else {
            return;
        };
        let aliased = self.type_reference(
            el,
            scope,
            &trail,
            "type",
            "nonBasicTypeName",
            "stringMaxLength",
        );
        let aliased = self.collections_around(el, aliased, scope, &trail);
        if self
            .ast
            .complete_type(id, TypeKind::Alias { aliased })
            .is_err()
        {
            self.error(
                &trail,
                format!("`{name}` is declared twice"),
                "OMG IDL 4.2 §7.2.3",
            );
        }
    }

    /// `<forward_dcl name="T" kind="STRUCT"/>` — a name with no body.
    ///
    /// The node stays `Completeness::Forward` until a `<struct>` or
    /// `<union>` of the same name completes it, which is what makes two
    /// mutually recursive types expressible.
    fn forward_declaration(&mut self, el: &XmlElement, scope: &Scope) {
        let Some(name) = self.named(el, scope) else {
            return;
        };
        let trail = format!("{}/forward_dcl[{name}]", scope.trail);
        let kind = match el.attribute("kind").unwrap_or("STRUCT") {
            "UNION" | "union" => TypeKind::Union {
                discriminator: Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(
                    Vec::new(),
                ))),
                cases: Vec::new(),
                default_case: None,
            },
            _ => TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        };
        self.declare(&name, scope, kind, el, &trail);
    }

    // -- members ---------------------------------------------------------

    /// A `<member>` of a struct or a union case.
    fn member_spec(&mut self, el: &XmlElement, scope: &Scope, trail: &str) -> Option<MemberSpec> {
        let name = match el.attribute("name") {
            Some(name) => name.to_owned(),
            None => {
                self.error(
                    trail,
                    String::from("a <member> needs a `name`"),
                    "OMG DDS-XTypes 1.3 Annex A",
                );
                return None;
            }
        };
        let trail = format!("{trail}/member[{name}]");
        let element = self.type_reference(
            el,
            scope,
            &trail,
            "type",
            "nonBasicTypeName",
            "stringMaxLength",
        );
        let type_ = self.collections_around(el, element, scope, &trail);

        // DDS-XTypes 1.3 §7.3.2.4: "The element_external property of the
        // Type System Model shall be represented by an attribute
        // external." For a member that is a collection — a sequence, a
        // map or an array — the attribute is the elements', as
        // `sequence<@external Foo>` is (§7.3.1.2.1.6); for any other
        // member it is the member's.
        let collection = el.attribute("sequenceMaxLength").is_some()
            || el.attribute("mapKeyType").is_some()
            || el.attribute("arrayDimensions").is_some();
        let external = truthy(el.attribute("external"));
        let mut xtypes = XTypesFacet {
            is_key: truthy(el.attribute("key")),
            is_optional: truthy(el.attribute("optional")),
            is_external: external && !collection,
            element_is_external: external && collection,
            is_must_understand: truthy(el.attribute("mustUnderstand"))
                || truthy(el.attribute("must_understand")),
            is_non_serialized: truthy(el.attribute("nonSerialized")),
            ..XTypesFacet::default()
        };
        // Table 17's `ARRAY_TYPE` row knows no member-level TryConstruct
        // behaviour — only the element's — so an array member's
        // annotation goes to `element_try_construct`, exactly where the
        // IDL front end puts it.
        if let Some(behaviour) = self.try_construct(el, &trail) {
            if el
                .attribute("arrayDimensions")
                .is_some_and(|written| !written.trim().is_empty())
            {
                xtypes.element_try_construct = Some(behaviour);
            } else {
                xtypes.try_construct = Some(behaviour);
            }
        }
        // `@id(n)` first, then `@hashid` — the same order §7.3.1.2.1.1
        // fixes. The number a hash produces is filled in by the
        // completion pass, so that both front ends compute it once, in
        // one place, from `zerodds_idl_lower::hashed_member_id`.
        if let Some(written) = el.attribute("id") {
            match integer(written).and_then(|v| u32::try_from(v).ok()) {
                Some(id) => {
                    xtypes.member_id = Some(MemberIdAssignment {
                        effective: id,
                        origin: MemberIdOrigin::Declared,
                    });
                }
                None => self.error(
                    &trail,
                    format!("`{written}` is not a member id"),
                    "DDS-XTypes 1.3 §7.3.1.2.1.1",
                ),
            }
        } else if let Some(written) = el.attribute("hashid") {
            // `hashid=""` and `hashid="true"` both mean "hash this
            // member's own name"; anything else is the name to hash,
            // which is `@hashid("x1")` in IDL.
            let source_name = if written.is_empty() || written.eq_ignore_ascii_case("true") {
                name.clone()
            } else {
                written.to_owned()
            };
            xtypes.member_id = Some(MemberIdAssignment {
                effective: 0,
                origin: MemberIdOrigin::Hashed { source_name },
            });
        }

        // §7.3.2.5.1.2 and Annex A `memberDecl`: a member holds applied
        // annotations of its own.
        let applied = self.applied_annotations(el, scope, &trail);
        let mut facets = self.facets(el, Some(xtypes));
        facets.annotations.extend(applied);
        Some(MemberSpec {
            name,
            type_,
            facets,
        })
    }

    // -- type references -------------------------------------------------

    /// The element type an attribute triple names.
    fn type_reference(
        &mut self,
        el: &XmlElement,
        scope: &Scope,
        trail: &str,
        type_attribute: &str,
        nonbasic_attribute: &str,
        bound_attribute: &str,
    ) -> Ref<TypeNode> {
        let Some(written) = el.attribute(type_attribute) else {
            self.error(
                trail,
                format!("no `{type_attribute}` attribute to read a type from"),
                "OMG DDS-XTypes 1.3 Annex A",
            );
            return broken(type_attribute);
        };
        // DDS-XTypes 1.3 §7.3.2.2: "It is an error to include this attribute
        // if the type attribute does not indicate a non-basic type."
        if !matches!(written, "nonBasic" | "nonbasic") && el.attribute(nonbasic_attribute).is_some()
        {
            self.error(
                trail,
                format!(
                    "`{nonbasic_attribute}` on an element whose `{type_attribute}` is \
                     `{written}`, which is not `nonBasic`"
                ),
                "OMG DDS-XTypes 1.3 §7.3.2.2",
            );
            return broken(nonbasic_attribute);
        }
        match written {
            "nonBasic" | "nonbasic" => match el.attribute(nonbasic_attribute) {
                Some(name) => self.named_ref(name, scope, trail),
                None => {
                    self.error(
                        trail,
                        format!(
                            "`{type_attribute}=\"nonBasic\"` without a `{nonbasic_attribute}` \
                             names nothing"
                        ),
                        "OMG DDS-XTypes 1.3 Annex A",
                    );
                    broken(nonbasic_attribute)
                }
            },
            "string" | "wstring" => {
                let width = if written == "string" {
                    CharWidth::Char
                } else {
                    CharWidth::WChar
                };
                let bound = self.bound(el.attribute(bound_attribute), scope, trail);
                self.anonymous(TypeKind::String { width, bound })
            }
            other => match primitive_of(other) {
                Some(kind) => self.primitive_ref(kind),
                None => {
                    // A `type=` that is neither a primitive nor
                    // `nonBasic` is still a name in every DDS-XML
                    // dialect that writes one, and refusing it outright
                    // would lose a type that resolves perfectly well.
                    self.named_ref(other, scope, trail)
                }
            },
        }
    }

    /// The sequence, array and map wrappers a member's attributes put
    /// around its element type.
    ///
    /// DDS-XTypes 1.3 §7.3.2.4.4 orders them "from most-tightly-binding
    /// to least-tightly-binding": the sequence designations, then the
    /// array designations, then the map designations. "A member
    /// specifying all of these would define a map whose values are
    /// arrays of sequences of strings", and "to indicate a type composed
    /// in a different order [...] it is necessary to interpose an alias
    /// definition". `stringMaxLength="10" arrayDimensions="10"` is
    /// `string<10> x1[10]`, an array of bounded strings, as the suite's
    /// own `arrays.idl` writes it.
    fn collections_around(
        &mut self,
        el: &XmlElement,
        element: Ref<TypeNode>,
        scope: &Scope,
        trail: &str,
    ) -> Ref<TypeNode> {
        let mut current = element;
        if let Some(written) = el.attribute("sequenceMaxLength") {
            let bound = self.bound(Some(written), scope, trail);
            current = self.anonymous(TypeKind::Sequence {
                element: current,
                bound,
            });
        }
        if let Some(written) = el.attribute("arrayDimensions") {
            match self.dimensions(written, scope, trail) {
                Some(dims) if !dims.is_empty() => {
                    current = self.anonymous(TypeKind::Array {
                        element: current,
                        dims,
                    });
                }
                Some(_) => {}
                None => return broken("arrayDimensions"),
            }
        }
        if let Some(key_written) = el.attribute("mapKeyType") {
            let key = self.type_reference(
                el,
                scope,
                trail,
                "mapKeyType",
                "mapKeyNonBasicTypeName",
                "mapKeyStringMaxLength",
            );
            // §7.3.2.4.3: "If the "key" type is basic, the
            // mapKeyNonBasicTypeName attribute is not allowed", and
            // mapKeyStringMaxLength only goes with a string key. A
            // non-basic key may be an alias of a string, so that one is
            // not judged here.
            let key_non_basic = matches!(key_written, "nonBasic" | "nonbasic");
            if !key_non_basic && el.attribute("mapKeyNonBasicTypeName").is_some() {
                self.error(
                    trail,
                    format!(
                        "`mapKeyNonBasicTypeName` is not allowed beside the basic \
                         `mapKeyType=\"{key_written}\"`"
                    ),
                    "OMG DDS-XTypes 1.3 §7.3.2.4.3",
                );
            }
            if !key_non_basic
                && !matches!(key_written, "string" | "wstring")
                && el.attribute("mapKeyStringMaxLength").is_some()
            {
                self.error(
                    trail,
                    format!(
                        "`mapKeyStringMaxLength` is not allowed beside the key type \
                         `{key_written}`, which is no string"
                    ),
                    "OMG DDS-XTypes 1.3 §7.3.2.4.3",
                );
            }
            // §7.3.2.4.3: "This attribute is required for all map types."
            if el.attribute("mapMaxLength").is_none() {
                self.error(
                    trail,
                    String::from("a map member needs `mapMaxLength`; \"-1\" is unbounded"),
                    "OMG DDS-XTypes 1.3 §7.3.2.4.3",
                );
            }
            let bound = self.bound(el.attribute("mapMaxLength"), scope, trail);
            current = self.anonymous(TypeKind::Map {
                key,
                value: current,
                bound,
            });
        } else if el.attribute("mapMaxLength").is_some() {
            self.error(
                trail,
                String::from("`mapMaxLength` without `mapKeyType` names no key type"),
                "OMG DDS-XTypes 1.3 §7.3.2.4.3",
            );
        }
        current
    }

    /// `arrayDimensions`: a comma-delimited list of `<const_exp>`,
    /// "in the same order in which those bounds would be given in IDL"
    /// (§7.3.2.4.1). `None` when the list does not parse or a dimension
    /// is not a positive integer; the diagnostic is given here.
    fn dimensions(
        &mut self,
        written: &str,
        scope: &Scope,
        trail: &str,
    ) -> Option<Vec<Id<ConstExprNode>>> {
        let parsed = match expr::parse_list(written) {
            Ok(parsed) => parsed,
            Err(why) => {
                self.error(
                    trail,
                    format!("`arrayDimensions=\"{written}\"` is not a list of bounds: {why}"),
                    "OMG DDS-XTypes 1.3 §7.3.2.4.1",
                );
                return None;
            }
        };
        let mut out = Vec::with_capacity(parsed.len());
        for one in &parsed {
            let id = self.expression(one, scope, trail)?;
            match self.ast.eval_integral(id) {
                Ok(value) if value <= 0 => {
                    self.error(
                        trail,
                        format!("an array dimension of {value} is no size"),
                        "OMG DDS-XTypes 1.3 §7.3.2.4.1",
                    );
                    return None;
                }
                Ok(_) => {}
                // A constant declared further on: the model evaluates the
                // expression when it needs the number.
                Err(EvalError::Unresolved { .. }) => {}
                Err(why) => {
                    self.error(
                        trail,
                        format!("an array dimension cannot be evaluated: {why:?}"),
                        "OMG DDS-XTypes 1.3 §7.3.2.4.1",
                    );
                    return None;
                }
            }
            out.push(id);
        }
        Some(out)
    }

    /// A named type, resolved against the names the declare pass
    /// recorded, innermost enclosing scope first.
    fn named_ref(&mut self, written: &str, scope: &Scope, trail: &str) -> Ref<TypeNode> {
        let absolute = written.starts_with("::");
        let segments: Vec<String> = written
            .split("::")
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        if segments.is_empty() {
            self.error(
                trail,
                String::from("an empty type name names nothing"),
                "OMG IDL 4.2 §7.5",
            );
            return broken("");
        }
        if absolute {
            if let Some(id) = self.declared.get(&segments) {
                return Ref::Resolved(*id);
            }
            return Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(segments)));
        }
        for depth in (0..=scope.path.len()).rev() {
            let mut candidate: Vec<String> = scope.path.iter().take(depth).cloned().collect();
            candidate.extend(segments.iter().cloned());
            if let Some(id) = self.declared.get(&candidate) {
                return Ref::Resolved(*id);
            }
        }
        // Not declared in this bundle. Left pending rather than broken:
        // `Ast::resolve_all_by_name` gets its own attempt against the
        // model's name index, and only then is it reported.
        Ref::Pending(PendingRef::ScopedName(ScopedName::relative(segments)))
    }

    fn primitive_ref(&mut self, kind: PrimitiveKind) -> Ref<TypeNode> {
        if let Some(id) = self.builtin.get(&kind) {
            return Ref::Resolved(*id);
        }
        let root = self.ast.root_module();
        match self
            .ast
            .add_type(root, kind.as_str(), TypeKind::Primitive(kind))
        {
            Ok(id) => {
                self.builtin.insert(kind, id);
                Ref::Resolved(id)
            }
            Err(_) => broken(kind.as_str()),
        }
    }

    /// An unnamed type, which a collection attribute introduces at the
    /// use site.
    fn anonymous(&mut self, kind: TypeKind) -> Ref<TypeNode> {
        let root = self.ast.root_module();
        let what = kind.as_str();
        self.ast
            .add_type(root, "", kind)
            .map_or_else(|_| broken(what), Ref::Resolved)
    }

    /// A collection bound.
    ///
    /// DDS-XTypes 1.3 §7.3.2.4: the value "may be any constant
    /// expression as defined by the <const_exp> production in the IDL
    /// grammar [IDL]. The literal expression "-1" shall indicate an
    /// unbounded collection; no other "negative" value is permitted."
    /// `0` stays unbounded as well — DDS-XML 1.0 §7.2.2 and the suite's
    /// own `sequences.xml`, which comments `-1` as "unlimited (0 or -1)".
    ///
    /// An expression that names a constant declared further on is kept
    /// as written and evaluated by whoever needs the number.
    fn bound(&mut self, written: Option<&str>, scope: &Scope, trail: &str) -> Bound {
        let Some(written) = written else {
            return Bound::Unbounded;
        };
        let Some(id) = self.const_exp(written, scope, trail, "OMG DDS-XTypes 1.3 §7.3.2.4") else {
            return Bound::Unbounded;
        };
        match self.ast.eval_integral(id) {
            Ok(value) if value < -1 => {
                self.error(
                    trail,
                    format!("`{written}` is a negative bound other than -1"),
                    "OMG DDS-XTypes 1.3 §7.3.2.4",
                );
                Bound::Unbounded
            }
            Ok(-1) => Bound::Unbounded,
            Ok(0) if self.options.zero_bound_is_unbounded => Bound::Unbounded,
            Ok(0) => {
                self.error(
                    trail,
                    format!(
                        "`{written}` is a bound of zero; \"-1\" is unbounded, and a bound is \
                         a positive integer"
                    ),
                    "OMG DDS-XTypes 1.3 §7.3.2.4",
                );
                Bound::Unbounded
            }
            Ok(_) | Err(EvalError::Unresolved { .. }) => Bound::Const(id),
            Err(why) => {
                self.error(
                    trail,
                    format!("`{written}` cannot be evaluated as a bound: {why:?}"),
                    "OMG DDS-XTypes 1.3 §7.3.2.4",
                );
                Bound::Unbounded
            }
        }
    }

    fn try_construct(&mut self, el: &XmlElement, trail: &str) -> Option<TryConstruct> {
        let written = el
            .attribute("tryConstruct")
            .or_else(|| el.attribute("try_construct"))?;
        match written.to_ascii_lowercase().as_str() {
            "trim" => Some(TryConstruct::Trim),
            "discard" => Some(TryConstruct::Discard),
            "use_default" | "usedefault" | "default" => Some(TryConstruct::UseDefault),
            other => {
                self.error(
                    trail,
                    format!("`{other}` is not a try-construct behaviour"),
                    "DDS-XTypes 1.3 §7.3.1.2.1.12",
                );
                None
            }
        }
    }

    /// One `<caseDiscriminator value="…">`.
    fn case_label(
        &mut self,
        written: &str,
        discriminator: Option<Id<TypeNode>>,
        trail: &str,
    ) -> Option<CaseLabel> {
        if written.eq_ignore_ascii_case("default") {
            return Some(CaseLabel::Default);
        }
        // An enumerator name, when the discriminator is an enumeration.
        // Tried before the numeric reading so that an enumerator called
        // `1` — which no file writes — could still only be a number.
        if let Some(discriminator) = discriminator {
            let enumerators = self.ast.enumerators_of(discriminator).to_vec();
            for enumerator in enumerators {
                if self
                    .ast
                    .enumerator(enumerator)
                    .is_ok_and(|node| node.name == written)
                {
                    return match self.ast.add_expr(ConstExpr::Literal(Literal::Enumerator(
                        Ref::Resolved(enumerator),
                    ))) {
                        Ok(expr) => Some(CaseLabel::Value(expr)),
                        Err(_) => {
                            self.error(
                                trail,
                                format!("the model refused the case label `{written}`"),
                                "zerodds-ast-1.0 §3.5",
                            );
                            None
                        }
                    };
                }
            }
        }
        if let Some(value) = integer(written) {
            return match self.ast.add_int_expr(value) {
                Ok(expr) => Some(CaseLabel::Value(expr)),
                Err(_) => {
                    self.error(
                        trail,
                        format!("the model refused the case label `{written}`"),
                        "zerodds-ast-1.0 §3.5",
                    );
                    None
                }
            };
        }
        let literal = match written {
            "TRUE" | "true" => Some(Literal::Boolean(true)),
            "FALSE" | "false" => Some(Literal::Boolean(false)),
            _ => None,
        };
        if let Some(literal) = literal {
            return match self.ast.add_expr(ConstExpr::Literal(literal)) {
                Ok(expr) => Some(CaseLabel::Value(expr)),
                Err(_) => {
                    self.error(
                        trail,
                        format!("the model refused the case label `{written}`"),
                        "zerodds-ast-1.0 §3.5",
                    );
                    None
                }
            };
        }
        self.error(
            trail,
            format!("`{written}` is not a case label the discriminator can take"),
            "OMG IDL 4.2 §7.4.1.4.4.3",
        );
        None
    }

    // -- model plumbing --------------------------------------------------

    fn named(&mut self, el: &XmlElement, scope: &Scope) -> Option<String> {
        match el.attribute("name") {
            Some(name) if !name.is_empty() => Some(name.to_owned()),
            _ => {
                let what = el.name.clone();
                self.error(
                    &scope.trail,
                    format!("<{what}> needs a `name`"),
                    "OMG DDS-XML 1.0 §7.1.4",
                );
                None
            }
        }
    }

    /// Declare a type forward, to be completed in the second pass.
    fn declare(&mut self, name: &str, scope: &Scope, kind: TypeKind, el: &XmlElement, trail: &str) {
        let mut path = scope.path.clone();
        path.push(name.to_owned());
        if self.declared.contains_key(&path) {
            // A `<forward_dcl>` followed by the definition is the
            // ordinary case and not a duplicate: the second pass
            // completes the node the first one made.
            return;
        }
        match self.ast.add_forward_type(scope.module, name, kind) {
            Ok(id) => {
                self.declared.insert(path, id);
                self.record(id, el, trail);
            }
            Err(_) => self.error(
                trail,
                format!("the model refused a type named `{name}`"),
                "zerodds-ast-1.0 §3.2",
            ),
        }
    }

    /// Declare a type that is complete as soon as it is created — an
    /// enumeration or a bitmask, neither of which names another type.
    fn declare_now(
        &mut self,
        name: &str,
        scope: &Scope,
        kind: TypeKind,
        el: &XmlElement,
        trail: &str,
    ) -> Option<Id<TypeNode>> {
        let mut path = scope.path.clone();
        path.push(name.to_owned());
        match self.ast.add_type(scope.module, name, kind) {
            Ok(id) => {
                self.declared.insert(path, id);
                self.record(id, el, trail);
                Some(id)
            }
            Err(_) => {
                self.error(
                    trail,
                    format!("the model refused a type named `{name}`"),
                    "zerodds-ast-1.0 §3.2",
                );
                None
            }
        }
    }

    fn declared_here(&mut self, name: &str, scope: &Scope) -> Option<Id<TypeNode>> {
        let mut path = scope.path.clone();
        path.push(name.to_owned());
        self.declared.get(&path).copied()
    }

    /// Everything a type node carries from its element: provenance, the
    /// attributes as written, and the XTypes properties an aggregate or
    /// an enumerated type declares on itself.
    fn record(&mut self, id: Id<TypeNode>, el: &XmlElement, trail: &str) {
        let Ok(node_id) = self.ast.type_(id).map(|node| node.node_id) else {
            return;
        };
        let any = AnyId::Type(id, node_id);
        // Each of the three below is a value whose *absence* has a
        // specified meaning — appendable, bit bound 32, sequential ids.
        // A spelling this loader cannot read is therefore reported and
        // not folded into the absent case: `extensibility="fnial"`
        // quietly producing an appendable type is a wire format nobody
        // asked for and nothing names.
        let mut xtypes = XTypesFacet {
            is_nested: truthy(el.attribute("nested")),
            nested_explicit: el.attribute("nested").is_some(),
            ignore_literal_names: truthy(el.attribute("ignoreLiteralNames")),
            ..XTypesFacet::default()
        };
        if let Some(written) = el.attribute("extensibility") {
            match extensibility_of(written) {
                Some(extensibility) => xtypes.extensibility = Some(extensibility),
                None => self.error(
                    trail,
                    format!("`{written}` is not final, appendable or mutable"),
                    "DDS-XTypes 1.3 §7.3.1.2.1.8",
                ),
            }
        }
        if let Some(written) = el
            .attribute("bitBound")
            .or_else(|| el.attribute("bit_bound"))
        {
            match integer(written).and_then(|v| u16::try_from(v).ok()) {
                Some(bound) => xtypes.bit_bound = Some(bound),
                None => self.error(
                    trail,
                    format!("`{written}` is not a bit bound"),
                    "DDS-XTypes 1.3 §7.3.1.2.1.9",
                ),
            }
        }
        xtypes.autoid = self.autoid_of(el, trail);
        let facets = self.facets(el, Some(xtypes));
        if let Some(slot) = self.ast.facets_mut(any) {
            *slot = facets;
        }
    }

    /// The `autoid` of a module, a structure or a union (Annex A `autoIdKind`).
    fn autoid_of(&mut self, el: &XmlElement, trail: &str) -> Option<AutoId> {
        let written = el.attribute("autoid")?;
        match written.to_ascii_lowercase().as_str() {
            "hash" => Some(AutoId::Hash),
            // Annex A spells the enumeration `sequencial` (S. 265);
            // IDL and the rest of the specification spell it
            // `sequential`. Both are read.
            "sequential" | "sequencial" => Some(AutoId::Sequential),
            other => {
                self.error(
                    trail,
                    format!("`{other}` is not HASH or SEQUENTIAL"),
                    "DDS-XTypes 1.3 §7.3.1.2.1.1",
                );
                None
            }
        }
    }

    fn facets(&self, el: &XmlElement, xtypes: Option<XTypesFacet>) -> Facets {
        Facets {
            source: SourceFacet::file(self.unit.clone(), None),
            xtypes,
            xml: Some(XmlFacet {
                element: el.name.clone(),
                attributes: el
                    .attributes
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                ..XmlFacet::default()
            }),
            ..Facets::default()
        }
    }

    fn set_xml_facet(&mut self, any: AnyId, el: &XmlElement) {
        let facets = self.facets(el, None);
        if let Some(slot) = self.ast.facets_mut(any) {
            *slot = facets;
        }
    }

    fn set_type_xtypes(&mut self, id: Id<TypeNode>, apply: impl FnOnce(&mut XTypesFacet)) {
        let Ok(node_id) = self.ast.type_(id).map(|node| node.node_id) else {
            return;
        };
        if let Some(facets) = self.ast.facets_mut(AnyId::Type(id, node_id)) {
            apply(facets.xtypes_mut());
        }
    }

    // -- after the walk --------------------------------------------------

    /// Whatever the declare pass could not bind, against the model's own
    /// name index. A name that still does not resolve is an error.
    fn resolve(&mut self) {
        let Ok(report) = self.ast.resolve_all_by_name() else {
            self.error(
                &self.unit.clone(),
                String::from("the model could not run a resolve pass"),
                "OMG IDL 4.2 §7.5",
            );
            return;
        };
        let broken: Vec<String> = report
            .broken
            .iter()
            .map(|(node, cause)| format!("{cause:?} (node {node:?})"))
            .collect();
        for one in broken {
            self.error(
                &self.unit.clone(),
                format!("a name does not resolve: {one}"),
                "OMG IDL 4.2 §7.5",
            );
        }
        let pending = self.ast.pending_count();
        if pending > 0 {
            self.error(
                &self.unit.clone(),
                format!("{pending} type reference(s) never resolved"),
                "OMG IDL 4.2 §7.5",
            );
        }
    }

    /// What the document left implicit.
    ///
    /// The same two rules `zerodds_idl_lower::complete` applies, in the
    /// same order and over the same facets: the caller's default
    /// extensibility where the document said nothing, then member ids
    /// base-first.
    fn complete(&mut self) {
        let types: Vec<Id<TypeNode>> = self.ast.types().map(|(id, _)| id).collect();
        for id in self.base_first(&types) {
            self.complete_one(id);
        }
    }

    fn complete_one(&mut self, id: Id<TypeNode>) {
        let Ok(node) = self.ast.type_(id) else {
            return;
        };
        let node_id = node.node_id;
        let any = AnyId::Type(id, node_id);
        let is_aggregate = matches!(
            node.kind,
            TypeKind::Structure { .. } | TypeKind::Union { .. } | TypeKind::Valuetype { .. }
        );
        if !is_aggregate {
            return;
        }
        // The caller's default fills silence only. What the document
        // said stays what it said, so "appendable because the file says
        // so" and "appendable because nobody said otherwise" remain two
        // different facts. A structure with a base is left out: silent, it
        // has its base's extensibility (DDS-XTypes 1.3 §7.2.2.4.5), which
        // `Ast::effective_extensibility` follows.
        let derives = matches!(node.kind, TypeKind::Structure { base: Some(_), .. });
        if let (Some(extensibility), false) = (self.options.default_extensibility, derives) {
            let declared = self
                .ast
                .facets(any)
                .and_then(|facets| facets.xtypes.as_ref())
                .and_then(|x| x.extensibility);
            if declared.is_none() {
                if let Some(facets) = self.ast.facets_mut(any) {
                    facets.xtypes_mut().extensibility = Some(extensibility);
                }
            }
        }
        self.member_ids(id);
    }

    /// Member ids: an `id` attribute, else a `hashid`, else the
    /// aggregate's `autoid`, else the previous member plus one, starting
    /// at zero — or, for a derived type, at the base's last id plus one.
    /// DDS-XTypes 1.3 §7.3.1.2.1.1.
    fn member_ids(&mut self, id: Id<TypeNode>) {
        let autoid = self.ast.effective_autoid(id);
        let members: Vec<_> = self.ast.members_of(id).to_vec();
        let mut previous: Option<u32> = self.last_base_member_id(id);
        for member in members {
            let Ok(node) = self.ast.member(member) else {
                continue;
            };
            let member_any = AnyId::Member(member, node.node_id);
            let name = node.name.clone();
            let declared = self
                .ast
                .facets(member_any)
                .and_then(|facets| facets.xtypes.as_ref())
                .and_then(|x| x.member_id.clone());
            let assignment = match declared {
                Some(assignment) if assignment.origin == MemberIdOrigin::Declared => assignment,
                Some(MemberIdAssignment {
                    origin: MemberIdOrigin::Hashed { source_name },
                    ..
                }) => MemberIdAssignment {
                    effective: zerodds_idl_lower::hashed_member_id(&source_name),
                    origin: MemberIdOrigin::Hashed { source_name },
                },
                _ => match autoid {
                    Some(AutoId::Hash) => MemberIdAssignment {
                        effective: zerodds_idl_lower::hashed_member_id(&name),
                        origin: MemberIdOrigin::AutoHash,
                    },
                    Some(AutoId::Sequential) => MemberIdAssignment {
                        effective: next_id(previous),
                        origin: MemberIdOrigin::AutoSequential,
                    },
                    None => MemberIdAssignment {
                        effective: next_id(previous),
                        origin: MemberIdOrigin::PreviousPlusOne,
                    },
                },
            };
            let hashed = matches!(
                assignment.origin,
                MemberIdOrigin::Hashed { .. } | MemberIdOrigin::AutoHash
            );
            if !hashed
                || self.options.hashed_id_advances_the_counter
                    == zerodds_idl_lower::HashedIdCounter::Advances
            {
                previous = Some(assignment.effective);
            }
            if let Some(facets) = self.ast.facets_mut(member_any) {
                facets.xtypes_mut().member_id = Some(assignment);
            }
        }
    }

    fn last_base_member_id(&self, id: Id<TypeNode>) -> Option<u32> {
        let base = self.ast.base_of(id)?;
        let last = *self.ast.all_members_of(base).last()?;
        let node_id = self.ast.member(last).ok()?.node_id;
        self.ast
            .facets(AnyId::Member(last, node_id))
            .and_then(|facets| facets.xtypes.as_ref())
            .and_then(|x| x.member_id.as_ref())
            .map(|assignment| assignment.effective)
    }

    /// The types, every base ahead of what derives from it: sequential
    /// member ids continue across an inheritance boundary.
    fn base_first(&self, types: &[Id<TypeNode>]) -> Vec<Id<TypeNode>> {
        let mut out: Vec<Id<TypeNode>> = Vec::with_capacity(types.len());
        for id in types.iter().copied() {
            self.push_base_first(id, &mut out, 0);
        }
        out
    }

    fn push_base_first(&self, id: Id<TypeNode>, out: &mut Vec<Id<TypeNode>>, depth: usize) {
        if depth > MAX_INHERITANCE_DEPTH || out.contains(&id) {
            return;
        }
        if let Some(base) = self.ast.base_of(id) {
            self.push_base_first(base, out, depth + 1);
        }
        out.push(id);
    }

    fn warning(&mut self, at: &str, message: String, spec_reference: &'static str) {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message,
            at: at.to_owned(),
            spec_reference,
        });
    }

    // -- expressions -----------------------------------------------------

    /// Parse a `<const_exp>` attribute and put it into the model.
    fn const_exp(
        &mut self,
        written: &str,
        scope: &Scope,
        trail: &str,
        spec_reference: &'static str,
    ) -> Option<Id<ConstExprNode>> {
        match expr::parse(written) {
            Ok(parsed) => self.expression(&parsed, scope, trail),
            Err(why) => {
                self.error(
                    trail,
                    format!("`{written}` is not a <const_exp>: {why}"),
                    spec_reference,
                );
                None
            }
        }
    }

    /// One parsed expression as model nodes. A name is resolved against
    /// the scopes of the document, innermost first (IDL 4.2 §7.5); one
    /// that names nothing yet stays a pending reference, which the
    /// resolve pass reports when it is still unresolved.
    fn expression(
        &mut self,
        parsed: &Expression,
        scope: &Scope,
        trail: &str,
    ) -> Option<Id<ConstExprNode>> {
        let node = match parsed {
            Expression::Integer(value) => ConstExpr::Literal(Literal::Integer(*value)),
            Expression::Float { mantissa, exponent } => ConstExpr::Literal(Literal::Float {
                mantissa: mantissa.clone(),
                exponent: exponent.clone(),
            }),
            Expression::String(text) => ConstExpr::Literal(Literal::String(text.clone())),
            Expression::Char(c) => ConstExpr::Literal(Literal::Char(*c)),
            Expression::Boolean(value) => ConstExpr::Literal(Literal::Boolean(*value)),
            Expression::Name { absolute, segments } => {
                match self.lookup(*absolute, segments, scope, |any| {
                    matches!(any, AnyId::Const(..) | AnyId::Enumerator(..))
                }) {
                    Some(AnyId::Const(id, _)) => ConstExpr::NameRef(Ref::Resolved(id)),
                    Some(AnyId::Enumerator(id, _)) => {
                        ConstExpr::Literal(Literal::Enumerator(Ref::Resolved(id)))
                    }
                    _ => ConstExpr::NameRef(Ref::Pending(PendingRef::ScopedName(scoped(
                        *absolute, segments,
                    )))),
                }
            }
            Expression::Unary(op, inner) => ConstExpr::Unary {
                op: *op,
                operand: self.expression(inner, scope, trail)?,
            },
            Expression::Binary(op, left, right) => {
                let lhs = self.expression(left, scope, trail)?;
                let rhs = self.expression(right, scope, trail)?;
                ConstExpr::Binary { op: *op, lhs, rhs }
            }
        };
        match self.ast.add_expr(node) {
            Ok(id) => Some(id),
            Err(_) => {
                self.error(
                    trail,
                    String::from("the model refused an expression"),
                    "zerodds-ast-1.0 §3.5",
                );
                None
            }
        }
    }

    /// The node a written name denotes: the name is tried from the
    /// innermost enclosing scope outwards, or from the root when it
    /// starts with `::`.
    fn lookup(
        &self,
        absolute: bool,
        segments: &[String],
        scope: &Scope,
        wanted: impl Fn(&AnyId) -> bool,
    ) -> Option<AnyId> {
        let depths: Vec<usize> = if absolute {
            vec![0]
        } else {
            (0..=scope.path.len()).rev().collect()
        };
        for depth in depths {
            let mut candidate: Vec<String> = scope.path.iter().take(depth).cloned().collect();
            candidate.extend(segments.iter().cloned());
            if let Ok(any) = self
                .ast
                .find_by_scoped_name(&ScopedName::absolute(candidate))
            {
                if wanted(&any) {
                    return Some(any);
                }
            }
        }
        None
    }

    // -- constants, bitsets, annotations ---------------------------------

    /// `<const name type value>`; DDS-XTypes 1.3 §7.3.2.1.3. "Constants
    /// are not reflected directly in the Type System" — the model holds
    /// the constant and the expressions that name it, and the type
    /// identifiers see only the evaluated numbers.
    ///
    /// `within` is the structure the constant is declared in, which is a
    /// segment of its scoped name.
    fn constant(&mut self, el: &XmlElement, scope: &Scope, within: Option<&str>) {
        let Some(name) = self.named(el, scope) else {
            return;
        };
        let trail = format!("{}/const[{name}]", scope.trail);
        let type_ = self.type_reference(
            el,
            scope,
            &trail,
            "type",
            "nonBasicTypeName",
            "stringMaxLength",
        );
        let Some(written) = el.attribute("value") else {
            self.error(
                &trail,
                String::from("a <const> needs a `value`"),
                "OMG DDS-XTypes 1.3 Annex A",
            );
            return;
        };
        let Some(expression) =
            self.const_exp(written, scope, &trail, "OMG DDS-XTypes 1.3 §7.3.2.1.3")
        else {
            return;
        };
        let Ok(id) = self.ast.add_const(scope.module, &name, type_, expression) else {
            self.error(
                &trail,
                format!("the model refused a constant named `{name}`"),
                "zerodds-ast-1.0 §3.5",
            );
            return;
        };
        let Ok(node_id) = self.ast.constant(id).map(|c| c.node_id) else {
            return;
        };
        let any = AnyId::Const(id, node_id);
        if let Some(owner) = within {
            if self.ast.set_declaration_scope(any, owner).is_err() {
                self.error(
                    &trail,
                    format!("the model refused the scope `{owner}` for `{name}`"),
                    "zerodds-ast-1.0 §3.5",
                );
            }
        }
        self.set_xml_facet(any, el);
    }

    /// `<bitset>` with `<bitfield name type bits|mask>`, the form
    /// `zerodds-xml` reads: a field is as wide as `bits` says or as the
    /// mask has bits set; a field without a `name` is padding.
    fn bitset(&mut self, el: &XmlElement, scope: &Scope, pass: Pass) {
        let Some(name) = self.named(el, scope) else {
            return;
        };
        let trail = format!("{}/bitset[{name}]", scope.trail);
        if pass == Pass::Declare {
            let placeholder = TypeKind::Bitset {
                base: None,
                fields: Vec::new(),
            };
            self.declare(&name, scope, placeholder, el, &trail);
            return;
        }
        let Some(id) = self.declared_here(&name, scope) else {
            return;
        };
        let base = el
            .attribute("baseType")
            .or_else(|| el.attribute("base_type"))
            .map(|written| self.named_ref(written, scope, &trail));
        if self
            .ast
            .complete_type(
                id,
                TypeKind::Bitset {
                    base,
                    fields: Vec::new(),
                },
            )
            .is_err()
        {
            self.error(
                &trail,
                format!("`{name}` is declared twice"),
                "OMG IDL 4.2 §7.2.3",
            );
            return;
        }
        let annotations = self.applied_annotations(el, scope, &trail);
        let verbatim = self.verbatim_directives(el, &trail);
        self.attach_to_type(id, annotations, verbatim);
        for field in &el.children {
            if field.name != "bitfield" {
                if !matches!(field.name.as_str(), "annotate" | "verbatim") {
                    self.warning(
                        &trail,
                        format!("<{}> is no child of a <bitset> and is not read", field.name),
                        "OMG IDL 4.2 §7.4.13.4.3",
                    );
                }
                continue;
            }
            let field_name = field.attribute("name").map(str::to_owned);
            let label = field_name
                .clone()
                .unwrap_or_else(|| String::from("<padding>"));
            let width = match (field.attribute("bits"), field.attribute("mask")) {
                (Some(written), _) => integer(written).and_then(|v| u16::try_from(v).ok()),
                (None, Some(mask)) => integer(mask)
                    .and_then(|v| u64::try_from(v).ok())
                    .and_then(|v| u16::try_from(v.count_ones()).ok()),
                (None, None) => None,
            };
            let Some(width) = width.filter(|w| (1..=64).contains(w)) else {
                self.error(
                    &trail,
                    format!("bitfield `{label}` needs a `bits` or a `mask` of 1 to 64 bits"),
                    "OMG IDL 4.2 §7.4.13.4.3",
                );
                continue;
            };
            let holder = field.attribute("type").is_some().then(|| {
                self.type_reference(
                    field,
                    scope,
                    &trail,
                    "type",
                    "nonBasicTypeName",
                    "stringMaxLength",
                )
            });
            match self
                .ast
                .add_bitfield(id, field_name.as_deref(), width, holder)
            {
                Ok(bitfield) => {
                    if let Ok(node_id) = self.ast.bitfield(bitfield).map(|b| b.node_id) {
                        self.set_xml_facet(AnyId::Bitfield(bitfield, node_id), field);
                    }
                }
                Err(_) => self.error(
                    &trail,
                    format!("the model refused bitfield `{label}`"),
                    "zerodds-ast-1.0 §3.3",
                ),
            }
        }
    }

    /// `<annotation name><member name type/></annotation>`;
    /// DDS-XTypes 1.3 §7.3.2.9 and Annex A `annotationTypeDecl`.
    fn annotation_declaration(&mut self, el: &XmlElement, scope: &Scope) {
        let Some(name) = self.named(el, scope) else {
            return;
        };
        let trail = format!("{}/annotation[{name}]", scope.trail);
        let Ok(def) = self.ast.add_annotation_def(scope.module, &name) else {
            self.error(
                &trail,
                format!("the model refused an annotation named `{name}`"),
                "zerodds-ast-1.0 §3.6",
            );
            return;
        };
        if let Ok(node_id) = self.ast.annotation_def(def).map(|d| d.node_id) {
            self.set_xml_facet(AnyId::AnnotationDef(def, node_id), el);
        }
        // Annex A `annotationTypeDecl/@baseType`. The model's declaration has
        // no base, so the members of the base are the first members of this
        // declaration, in the base's order, as an inheriting annotation type
        // has them (IDL 4.2 §7.4.15.3, `<annotation_inheritance_spec>`).
        if let Some(written) = el.attribute("baseType") {
            let absolute = written.starts_with("::");
            let segments: Vec<String> = written
                .split("::")
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            let base = self.lookup(absolute, &segments, scope, |any| {
                matches!(any, AnyId::AnnotationDef(..))
            });
            let Some(AnyId::AnnotationDef(base, _)) = base else {
                self.error(
                    &trail,
                    format!(
                        "`baseType=\"{written}\"` names no annotation declared before this one"
                    ),
                    "OMG DDS-XTypes 1.3 Annex A",
                );
                return;
            };
            let inherited: Vec<_> = match self.ast.annotation_def(base) {
                Ok(node) => node.members.clone(),
                Err(_) => {
                    self.error(
                        &trail,
                        format!("the model does not hold the base annotation `{written}`"),
                        "zerodds-ast-1.0 §3.6",
                    );
                    return;
                }
            };
            for member in inherited {
                let Ok(node) = self.ast.annotation_member(member) else {
                    self.error(
                        &trail,
                        format!("a member of the base annotation `{written}` is not in the model"),
                        "zerodds-ast-1.0 §3.6",
                    );
                    continue;
                };
                let (member_name, member_type, member_default) =
                    (node.name.clone(), node.type_.clone(), node.default);
                if self
                    .ast
                    .add_annotation_member(def, &member_name, member_type, member_default)
                    .is_err()
                {
                    self.error(
                        &trail,
                        format!(
                            "the model refused the inherited annotation member `{member_name}`"
                        ),
                        "zerodds-ast-1.0 §3.6",
                    );
                }
            }
        }
        for child in &el.children {
            if child.name != "member" {
                self.warning(
                    &trail,
                    format!(
                        "<{}> is no child of an <annotation> and is not read",
                        child.name
                    ),
                    "OMG DDS-XTypes 1.3 Annex A",
                );
                continue;
            }
            let Some(member) = child.attribute("name") else {
                self.error(
                    &trail,
                    String::from("a <member> needs a `name`"),
                    "OMG DDS-XTypes 1.3 Annex A",
                );
                continue;
            };
            let member = member.to_owned();
            let type_ = self.type_reference(
                child,
                scope,
                &format!("{trail}/member[{member}]"),
                "type",
                "nonBasicTypeName",
                "stringMaxLength",
            );
            if self
                .ast
                .add_annotation_member(def, &member, type_, None)
                .is_err()
            {
                self.error(
                    &trail,
                    format!("the model refused annotation member `{member}`"),
                    "zerodds-ast-1.0 §3.6",
                );
            }
        }
    }

    /// The `<annotate name><member name value/></annotate>` children of
    /// an element, as applied annotations (§7.3.2.9). A built-in
    /// annotation is an attribute of the element in this representation
    /// (§7.3.2.5) and is reported when it is written as `<annotate>`.
    fn applied_annotations(
        &mut self,
        el: &XmlElement,
        scope: &Scope,
        trail: &str,
    ) -> Vec<AppliedAnnotation> {
        let mut out = Vec::new();
        for note in el.children_named("annotate") {
            let Some(written) = note.attribute("name") else {
                self.error(
                    trail,
                    String::from("an <annotate> needs a `name`"),
                    "OMG DDS-XTypes 1.3 Annex A",
                );
                continue;
            };
            let at = format!("{trail}/annotate[{written}]");
            let absolute = written.starts_with("::");
            let segments: Vec<String> = written
                .split("::")
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            let user_declared = self.lookup(absolute, &segments, scope, |any| {
                matches!(any, AnyId::AnnotationDef(..))
            });
            let definition = match user_declared {
                Some(AnyId::AnnotationDef(id, _)) => Ref::Resolved(id),
                _ => {
                    if segments.len() == 1
                        && zerodds_ast::node::annotation::BUILTIN_ANNOTATIONS.contains(&written)
                    {
                        self.error(
                            &at,
                            format!(
                                "`{written}` is a built-in annotation; this representation \
                                 writes it as an attribute of the element"
                            ),
                            "OMG DDS-XTypes 1.3 §7.3.2.5",
                        );
                        continue;
                    }
                    // Every `<annotation>` of the bundle was declared in the
                    // first pass, so a name that is not found now is not
                    // declared anywhere.
                    self.error(
                        &at,
                        format!("no <annotation> named `{written}` is declared"),
                        "OMG DDS-XTypes 1.3 §7.3.2.9",
                    );
                    continue;
                }
            };
            let mut arguments = Vec::new();
            for argument in note.children_named("member") {
                let (Some(member), Some(value)) =
                    (argument.attribute("name"), argument.attribute("value"))
                else {
                    self.error(
                        &at,
                        String::from("an annotation <member> needs a `name` and a `value`"),
                        "OMG DDS-XTypes 1.3 Annex A",
                    );
                    continue;
                };
                let member = member.to_owned();
                if let Some(expression) =
                    self.const_exp(value, scope, &at, "OMG DDS-XTypes 1.3 §7.3.2.9")
                {
                    arguments.push(AppliedArgument {
                        name: Some(member),
                        value: expression,
                    });
                }
            }
            out.push(AppliedAnnotation {
                name: written.to_owned(),
                definition,
                arguments,
            });
        }
        out
    }

    /// The `<verbatim language placement>text</verbatim>` children of an
    /// element (§7.3.2.5.1.1). The text is the `<text>` child of Annex A's
    /// `verbatimDecl` or, as the clause's example writes it, the element's
    /// own content; the placement defaults to `before-declaration` and
    /// the language to `*`, as `@verbatim` does in IDL 4.2 §8.3.5.1.
    fn verbatim_directives(&mut self, el: &XmlElement, trail: &str) -> Vec<VerbatimDirective> {
        let mut out = Vec::new();
        for verbatim in el.children_named("verbatim") {
            let placement = match verbatim.attribute("placement") {
                Some(written) => match zerodds_idl_lower::verbatim_placement(written) {
                    Some(placement) => placement,
                    None => {
                        self.error(
                            trail,
                            format!("`{written}` is none of the six `@verbatim` placements"),
                            "OMG IDL 4.2 §8.3.5.1",
                        );
                        continue;
                    }
                },
                None => VerbatimPlacement::BeforeDeclaration,
            };
            let text = verbatim
                .child("text")
                .map_or_else(|| verbatim.text.clone(), |t| t.text.clone());
            out.push(VerbatimDirective {
                language: verbatim
                    .attribute("language")
                    .map_or_else(|| String::from("*"), str::to_owned),
                placement,
                text,
            });
        }
        out
    }

    /// Annotations and verbatim text of a type declaration, onto its node.
    fn attach_to_type(
        &mut self,
        id: Id<TypeNode>,
        annotations: Vec<AppliedAnnotation>,
        verbatim: Vec<VerbatimDirective>,
    ) {
        let Ok(node_id) = self.ast.type_(id).map(|node| node.node_id) else {
            return;
        };
        if let Some(facets) = self.ast.facets_mut(AnyId::Type(id, node_id)) {
            facets.annotations.extend(annotations);
            if !verbatim.is_empty() {
                facets.xtypes_mut().verbatim.extend(verbatim);
            }
        }
    }

    /// Annotations of an enumeration or a bitmask and of its literals,
    /// applied once every `<annotation>` of the bundle is declared.
    /// `literal` is the element name of a literal (`enumerator`, `flag`).
    fn annotate_enumerated(&mut self, el: &XmlElement, scope: &Scope, literal: &str) {
        let Some(name) = el.attribute("name") else {
            return;
        };
        let name = name.to_owned();
        let trail = format!("{}/{}[{name}]", scope.trail, el.name);
        let Some(id) = self.declared_here(&name, scope) else {
            return;
        };
        let annotations = self.applied_annotations(el, scope, &trail);
        let verbatim = self.verbatim_directives(el, &trail);
        self.attach_to_type(id, annotations, verbatim);
        let Ok(node) = self.ast.type_(id) else {
            return;
        };
        let literals: Vec<(String, AnyId)> = match &node.kind {
            TypeKind::Enumeration { enumerators } => enumerators
                .iter()
                .filter_map(|e| {
                    let node = self.ast.enumerator(*e).ok()?;
                    Some((node.name.clone(), AnyId::Enumerator(*e, node.node_id)))
                })
                .collect(),
            TypeKind::Bitmask { bits } => bits
                .iter()
                .filter_map(|b| {
                    let node = self.ast.bit_value(*b).ok()?;
                    Some((node.name.clone(), AnyId::BitValue(*b, node.node_id)))
                })
                .collect(),
            _ => Vec::new(),
        };
        for child in el.children_named(literal) {
            let Some(literal_name) = child.attribute("name") else {
                continue;
            };
            let applied = self.applied_annotations(child, scope, &trail);
            if applied.is_empty() {
                continue;
            }
            if let Some((_, any)) = literals.iter().find(|(n, _)| n == literal_name) {
                if let Some(facets) = self.ast.facets_mut(*any) {
                    facets.annotations.extend(applied);
                }
            }
        }
    }

    fn error(&mut self, at: &str, message: String, spec_reference: &'static str) {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message,
            at: at.to_owned(),
            spec_reference,
        });
    }
}

/// A written name as a scoped name: from the root when it started with
/// `::`, otherwise relative to the node that names it.
/// The last segment of every name an expression writes.
///
/// zerodds-lint: recursion-depth 64 (the parser refuses an expression
/// nested deeper than its `MAX_DEPTH`)
fn names_in(parsed: &Expression, out: &mut Vec<String>) {
    match parsed {
        Expression::Name { segments, .. } => {
            if let Some(last) = segments.last() {
                out.push(last.clone());
            }
        }
        Expression::Unary(_, inner) => names_in(inner, out),
        Expression::Binary(_, left, right) => {
            names_in(left, out);
            names_in(right, out);
        }
        _ => {}
    }
}

fn scoped(absolute: bool, segments: &[String]) -> ScopedName {
    if absolute {
        ScopedName::absolute(segments.to_vec())
    } else {
        ScopedName::relative(segments.to_vec())
    }
}

/// How far an inheritance chain is walked before it is taken for a
/// cycle, matching `Ast::all_members_of`.
const MAX_INHERITANCE_DEPTH: usize = 64;

fn next_id(previous: Option<u32>) -> u32 {
    previous.map_or(0, |last| last.saturating_add(1))
}

/// A reference that will not be retried, naming what was wanted.
///
/// `BreakCause` carries the key rather than a sentence so that a
/// consumer can print the name the document wrote; a broken reference
/// with no name in it is a diagnostic nobody can act on.
fn broken(what: &str) -> Ref<TypeNode> {
    Ref::Broken(zerodds_ast_core::BreakCause::NotFound {
        what: PendingRef::ScopedName(ScopedName::absolute(vec![what.to_owned()])),
    })
}

/// The XTypes primitive a `type=` attribute names.
///
/// Both vocabularies, because both are written: DDS-XTypes 1.3 Annex A's
/// width-carrying spellings (`int32`, `float64`, `char8`) and OMG IDL
/// 4.2 §7.4.1.4's keywords (`long`, `double`, `char`), which DDS-XML
/// documents in the field use interchangeably. `zerodds-xml`'s own
/// `PrimitiveType::from_xml` carries most of this table and is not
/// reused only because it has no `int8` and no `uint8`, which the OMG
/// suite's `primitives.xml` writes fourteen times.
fn primitive_of(written: &str) -> Option<PrimitiveKind> {
    Some(match written {
        "boolean" | "bool" => PrimitiveKind::Boolean,
        "byte" | "octet" => PrimitiveKind::Octet,
        "uint8" => PrimitiveKind::UInt8,
        "int8" => PrimitiveKind::Int8,
        "char8" | "char" => PrimitiveKind::Char,
        "char16" | "wchar" => PrimitiveKind::WChar,
        "int16" | "short" => PrimitiveKind::Int16,
        "uint16" | "ushort" | "unsignedShort" => PrimitiveKind::UInt16,
        "int32" | "long" => PrimitiveKind::Int32,
        "uint32" | "ulong" | "unsignedLong" => PrimitiveKind::UInt32,
        "int64" | "longlong" | "longLong" => PrimitiveKind::Int64,
        "uint64" | "ulonglong" | "unsignedLongLong" => PrimitiveKind::UInt64,
        "float32" | "float" => PrimitiveKind::Float32,
        "float64" | "double" => PrimitiveKind::Float64,
        "float128" | "longdouble" | "longDouble" => PrimitiveKind::Float128,
        _ => return None,
    })
}

/// `byte`/`octet` and `uint8` are two type kinds, not two spellings of
/// one: XTypes 1.3 §7.3.4.2 gives `TK_BYTE` (0x02) and `TK_UINT8`
/// (0x0D), and §7.2.4.4.4.4 Table 15 makes a primitive assignable only
/// to its own kind. The DDS-XML vocabulary writes `byte` for the first
/// and `uint8` for the second, which is the split above; the IDL front
/// end splits them the same way — `octet` reaches
/// `PrimitiveKind::Octet` and `uint8` reaches `PrimitiveKind::UInt8` —
/// so the two loaders agree.
const _: () = ();

fn extensibility_of(written: &str) -> Option<Extensibility> {
    match written.to_ascii_lowercase().as_str() {
        "final" => Some(Extensibility::Final),
        "appendable" | "extensible" => Some(Extensibility::Appendable),
        "mutable" => Some(Extensibility::Mutable),
        _ => None,
    }
}

fn truthy(written: Option<&str>) -> bool {
    matches!(
        written.map(str::trim),
        Some("true" | "TRUE" | "True" | "1" | "yes")
    )
}

/// An integer as DDS-XML writes one: decimal, `0x`-hexadecimal, or
/// either with a sign.
fn integer(written: &str) -> Option<i128> {
    let text = written.trim();
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let value = if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        i128::from_str_radix(hex, 16).ok()?
    } else if let Some(bin) = rest.strip_prefix("0b").or_else(|| rest.strip_prefix("0B")) {
        i128::from_str_radix(bin, 2).ok()?
    } else {
        rest.parse::<i128>().ok()?
    };
    Some(if negative { -value } else { value })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn an_integer_reads_decimal_hexadecimal_and_a_sign() {
        assert_eq!(integer("10"), Some(10));
        assert_eq!(integer("0x0e"), Some(14));
        assert_eq!(integer("0X1F"), Some(31));
        assert_eq!(integer("-1"), Some(-1));
        assert_eq!(integer("VAL0"), None);
    }

    #[test]
    fn the_two_primitive_vocabularies_agree() {
        assert_eq!(primitive_of("int32"), primitive_of("long"));
        assert_eq!(primitive_of("byte"), primitive_of("octet"));
        assert_eq!(primitive_of("float128"), primitive_of("longdouble"));
        assert_eq!(primitive_of("int8"), Some(PrimitiveKind::Int8));
        assert_eq!(primitive_of("nonBasic"), None);
    }

    /// XTypes 1.3 §7.3.4.2 gives `octet` and `uint8` the distinct type
    /// kinds `TK_BYTE` and `TK_UINT8`. A loader that merged them
    /// announced `TK_BYTE` for a `uint8` member, which made the two
    /// mutually assignable under §7.2.4.4.4.4 Table 15 — the two OMG
    /// `dds-xtypes` cases `struct_uint8_byte` and `struct_byte_uint8`
    /// expect `INCONSISTENT_TOPIC` and got a match.
    #[test]
    fn octet_and_uint8_are_two_kinds() {
        assert_eq!(primitive_of("octet"), Some(PrimitiveKind::Octet));
        assert_eq!(primitive_of("byte"), Some(PrimitiveKind::Octet));
        assert_eq!(primitive_of("uint8"), Some(PrimitiveKind::UInt8));
        assert_ne!(primitive_of("octet"), primitive_of("uint8"));
    }
}

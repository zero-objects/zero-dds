// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CORBA IDL for one type, printed from the model.
//!
//! XCDR1 *is* CORBA CDR. Every alignment rule, every primitive's
//! encoding, the string and sequence forms and the encapsulation byte
//! order come from CORBA 3.3 part 2 §9.3, and XTypes 1.3 changes them
//! only where §7.4 says so. A CORBA ORB is therefore a reference for the
//! whole XCDR1 base, and three of them are reachable — omniORB 4.3.3,
//! JacORB 3.9 and TAO 2.5.24.
//!
//! An ORB's IDL front end is not an XTypes one. It refuses `@final`, it
//! refuses `@mutable`, it has no `bitmask`, no `bitset` and no `map`,
//! and it has no `int8`. So the corpus IDL cannot be handed to it as
//! written.
//!
//! ## Printed from the model, not edited as text
//!
//! The alternative was a textual filter that deleted annotations. A
//! filter deletes what it recognises and passes what it does not, which
//! is exactly backwards: an annotation nobody taught it about would
//! reach the ORB, the ORB would refuse it, and the case would be
//! recorded as "the ORB cannot take this construct" when the truth was
//! that the filter could not.
//!
//! Printing from the model inverts the failure. Every construct is
//! reached by an arm of one `match`, so a construct with no CORBA form
//! is refused **by name**, with the reason, and the run says which
//! construct and where. Nothing falls through.
//!
//! ## What is dropped, and why dropping it is byte-neutral
//!
//! | annotation | why it can go |
//! |---|---|
//! | `@final` | rule (17): the members in declaration order and no framing, which is what a CORBA `struct` is |
//! | `@appendable` | rule (29): under encoding version 1 an appendable type "is not delimited", so its bytes are the final form's — inventory row H2, three vendors, 18 bytes both |
//! | `@key` | the key holder and the KeyHash are computed beside the sample; §7.6.8 puts nothing in the payload |
//! | `@id`, `@hashid` | member ids appear on the wire only in a `@mutable` type's headers, and `@mutable` is refused below |
//! | `@must_understand` | `FLAG_MUST_UNDERSTAND` likewise lives in a parameter header |
//! | `@external` | §7.3.1.2.1.10 gives it no wire effect at all |
//! | `@try_construct`, `@default` | both are the **receiver's** answer to a value; neither changes what a writer emits |
//! | `@nested` | B6: `@nested` changes nothing in §7.4's rules this project can observe |
//!
//! Everything else is refused rather than dropped.

use crate::contract;
use crate::encode::ProbeError;
use crate::target_keywords;

use std::fmt::Write as _;
use zerodds_ast::{
    Ast, Bound, CaseLabel, CharWidth, Extensibility, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::Id;

/// Refuse a construct that has no CORBA IDL form.
fn refuse(path: &str, kind: &'static str) -> ProbeError {
    ProbeError::Unsupported {
        path: path.to_string(),
        kind,
    }
}

/// The same, for a construct this file has only the model's name for.
///
/// The catch-all arms reach a `TypeKind` and nothing else, and a bare
/// noun as a whole message says neither what is wrong nor whose gap it
/// is. [`ProbeError::Unfilled`] supplies the sentence.
fn unfilled(path: &str, kind: &'static str) -> ProbeError {
    ProbeError::Unfilled {
        path: path.to_string(),
        kind,
    }
}

/// The declarations one type needs, as CORBA IDL an ORB's front end
/// accepts.
///
/// # Errors
/// [`ProbeError::Unsupported`], naming the construct that has no CORBA
/// form and the path it sits at.
pub fn corba_idl(ast: &Ast, root: Id<TypeNode>) -> Result<String, ProbeError> {
    let mut printer = Printer::default();
    printer.declare(ast, root, &contract::root_path(ast, root))?;
    Ok(printer.finish())
}

/// The scoped name a type takes in CORBA IDL, absolute.
///
/// `::common::util::Location`, which is what `--type` names and what the
/// generated C++ and Java both derive their own spelling from.
#[must_use]
pub fn corba_scoped_name(ast: &Ast, type_: Id<TypeNode>) -> Option<Vec<String>> {
    ast.any_of_type(type_)
        .ok()
        .and_then(|any| ast.scoped_name_of(any))
        .map(|scoped| scoped.segments().to_vec())
}

/// Declarations in dependency order, each with the module path it sits
/// in.
#[derive(Default)]
struct Printer {
    /// Types already declared, so a diamond is emitted once.
    done: Vec<Id<TypeNode>>,
    /// Types on the current path, so a cycle is refused rather than
    /// recursed into.
    open: Vec<Id<TypeNode>>,
    /// `(module path, declaration text)`, in emission order.
    declarations: Vec<(Vec<String>, String)>,
    /// The name given to each anonymous collection that needed one, so
    /// two uses of the same type share one typedef.
    named: Vec<(Id<TypeNode>, String)>,
    /// The module path of the declaration being emitted, so an invented
    /// typedef can be placed beside its use rather than at global scope.
    scope: Vec<String>,
}

impl Printer {
    /// The IDL, with each declaration wrapped in its module path.
    ///
    /// A module is reopened per declaration rather than grouped. All
    /// three front ends accept a reopened module — CORBA 3.3 part 1
    /// §7.4.2 requires it — and grouping would have to reorder
    /// declarations, which is the one thing dependency order forbids.
    fn finish(self) -> String {
        let mut out = String::new();
        for (scope, text) in self.declarations {
            for segment in &scope {
                let _ = writeln!(out, "module {segment} {{");
            }
            out.push_str(&text);
            for _ in &scope {
                out.push_str("};\n");
            }
        }
        out
    }

    /// Emit `type_` and everything it depends on, once.
    fn declare(&mut self, ast: &Ast, type_: Id<TypeNode>, path: &str) -> Result<(), ProbeError> {
        if self.done.contains(&type_) {
            return Ok(());
        }
        if self.open.contains(&type_) {
            // A recursive type reaches its own declaration. CORBA IDL
            // has a forward declaration for exactly this, but the CDR
            // form of a recursive struct is not something XCDR1 shares —
            // §7.4.1.1 has no recursion rule — so there is nothing to
            // compare and the case is refused instead of guessed at.
            return Err(refuse(
                path,
                "a recursive type, which CORBA CDR frames its own way",
            ));
        }
        self.open.push(type_);
        // `declare_now` switches `self.scope` to this type's module path;
        // putting it back here keeps the caller's own members — spelled
        // after this dependency returns — in the caller's scope.
        let enclosing = self.scope.clone();
        let result = self.declare_now(ast, type_, path);
        self.scope = enclosing;
        self.open.pop();
        result?;
        self.done.push(type_);
        Ok(())
    }

    fn declare_now(
        &mut self,
        ast: &Ast,
        type_: Id<TypeNode>,
        path: &str,
    ) -> Result<(), ProbeError> {
        let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
            detail: format!("{error}"),
        })?;
        let kind = node.kind.clone();
        let Some(scoped) = corba_scoped_name(ast, type_) else {
            // Anonymous: it is spelled at its use site, and its element
            // type still has to be declared.
            return self.declare_dependencies(ast, &kind, path);
        };
        let Some((name, scope)) = scoped.split_last() else {
            return self.declare_dependencies(ast, &kind, path);
        };
        let scope = scope.to_vec();
        self.scope = scope.clone();

        match &kind {
            // A named primitive is not a declaration; it is spelled
            // where it is used.
            TypeKind::Primitive(_)
            | TypeKind::String { .. }
            | TypeKind::Fixed { .. }
            | TypeKind::Sequence { .. }
            | TypeKind::Array { .. } => self.declare_dependencies(ast, &kind, path),
            TypeKind::Alias { aliased } => {
                let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the typedef target is unresolved"),
                })?;
                self.declare(ast, target, path)?;
                // `typedef long Arr[3];` puts the dimensions after the
                // name, so an aliased array cannot use the plain form.
                let text = match ast.type_(target).map(|n| n.kind.clone()) {
                    Ok(TypeKind::Array { element, dims }) => {
                        let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                            detail: format!("{path}: the array element is unresolved"),
                        })?;
                        let counts = contract::array_dimensions(ast, &dims, path)?;
                        let brackets: String =
                            counts.iter().map(|count| format!("[{count}]")).collect();
                        format!(
                            "typedef {} {name}{brackets};\n",
                            self.spell(ast, element, path)?
                        )
                    }
                    _ => format!("typedef {} {name};\n", self.spell(ast, target, path)?),
                };
                self.declarations.push((scope, text));
                Ok(())
            }
            TypeKind::Enumeration { .. } => {
                let text = self.enumeration(ast, type_, name, path)?;
                self.declarations.push((scope, text));
                Ok(())
            }
            TypeKind::Structure { base, members } => {
                if base.is_some() {
                    // Measured 2026-08-10 on the Linux test host:
                    // omniidl 1.0 "Syntax error in struct definition",
                    // tao_idl 2.5.24 "Missing '{' or illegal syntax
                    // following struct identifier", JacORB 3.9 "Syntax
                    // error". Struct inheritance is IDL 4.2 and none of
                    // the three parses it.
                    return Err(refuse(
                        path,
                        "a structure with a base, which CORBA IDL has no syntax for",
                    ));
                }
                match ast.effective_extensibility(type_) {
                    Extensibility::Final | Extensibility::Appendable => {}
                    Extensibility::Mutable => {
                        return Err(refuse(
                            path,
                            "a @mutable type, whose PL_CDR1 parameter list is XTypes' and not CORBA's",
                        ));
                    }
                }
                let ids: Vec<_> = members.clone();
                let mut body = format!("struct {name} {{\n");
                for id in ids {
                    let member = ast.member(id).map_err(|error| ProbeError::NoLayout {
                        detail: format!("{error}"),
                    })?;
                    let member_path = contract::join(path, &member.name);
                    if member.facets.is_optional() {
                        return Err(refuse(
                            &member_path,
                            "an @optional member, whose presence marker is XTypes' and not CORBA's",
                        ));
                    }
                    if is_non_serialized(&member.facets) {
                        return Err(refuse(
                            &member_path,
                            "a @non_serialized member, which CORBA IDL cannot leave off the wire",
                        ));
                    }
                    let type_id = member
                        .type_
                        .resolved()
                        .ok_or_else(|| ProbeError::NoLayout {
                            detail: format!("{member_path}: the member type is unresolved"),
                        })?;
                    self.declare(ast, type_id, &member_path)?;
                    let _ = writeln!(
                        body,
                        "  {}",
                        self.member_declaration(ast, type_id, &member.name, &member_path)?
                    );
                }
                body.push_str("};\n");
                self.declarations.push((scope, body));
                Ok(())
            }
            TypeKind::Union { discriminator, .. } => {
                match ast.effective_extensibility(type_) {
                    Extensibility::Final | Extensibility::Appendable => {}
                    Extensibility::Mutable => {
                        return Err(refuse(
                            path,
                            "a @mutable union, whose per-member headers are XTypes' and not CORBA's",
                        ));
                    }
                }
                let discriminator =
                    discriminator
                        .resolved()
                        .ok_or_else(|| ProbeError::NoLayout {
                            detail: format!("{path}: the discriminator is unresolved"),
                        })?;
                self.declare(ast, discriminator, path)?;
                let mut body = format!(
                    "union {name} switch ({}) {{\n",
                    self.spell(ast, discriminator, path)?
                );
                for case_id in ast.cases_of(type_) {
                    let case = ast.case(*case_id).map_err(|error| ProbeError::NoLayout {
                        detail: format!("{error}"),
                    })?;
                    let member = ast
                        .member(case.member)
                        .map_err(|error| ProbeError::NoLayout {
                            detail: format!("{error}"),
                        })?;
                    let member_path = contract::join(path, &member.name);
                    let type_id = member
                        .type_
                        .resolved()
                        .ok_or_else(|| ProbeError::NoLayout {
                            detail: format!("{member_path}: the case member type is unresolved"),
                        })?;
                    self.declare(ast, type_id, &member_path)?;
                    for label in &case.labels {
                        match label {
                            CaseLabel::Default => body.push_str("  default:\n"),
                            CaseLabel::Value(expr) => {
                                let value = ast.eval_integral(*expr).map_err(|_| {
                                    refuse(&member_path, "a case label that does not evaluate")
                                })?;
                                let _ = writeln!(
                                    body,
                                    "  case {}:",
                                    case_label(ast, discriminator, value, &member_path)?
                                );
                            }
                        }
                    }
                    let _ = writeln!(
                        body,
                        "    {}",
                        self.member_declaration(ast, type_id, &member.name, &member_path)?
                    );
                }
                body.push_str("};\n");
                self.declarations.push((scope, body));
                Ok(())
            }
            TypeKind::Bitmask { .. } => Err(refuse(
                path,
                "a bitmask, which is XTypes 1.3 §7.3.1.2.1.4 and not CORBA IDL",
            )),
            TypeKind::Bitset { .. } => Err(refuse(
                path,
                "a bitset, which is XTypes 1.3 §7.3.1.2.1.3 and not CORBA IDL",
            )),
            // A named map is not a declaration; like a sequence it is
            // spelled where it is used, and its key and value types
            // still have to be declared.
            TypeKind::Map { .. } => self.declare_dependencies(ast, &kind, path),
            TypeKind::Valuetype { .. } => Err(refuse(
                path,
                "a valuetype, whose CDR form is CORBA 3.3 §9.3.4's tagged one and not §9.3.2's",
            )),
            other => Err(unfilled(path, other.as_str())),
        }
    }

    /// Declare whatever an anonymous type is built out of.
    fn declare_dependencies(
        &mut self,
        ast: &Ast,
        kind: &TypeKind,
        path: &str,
    ) -> Result<(), ProbeError> {
        match kind {
            TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => {
                let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the element type is unresolved"),
                })?;
                self.declare(ast, element, path)
            }
            TypeKind::Map { key, value, .. } => {
                let key = key.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the map key type is unresolved"),
                })?;
                let value = value.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the map value type is unresolved"),
                })?;
                self.declare(ast, key, path)?;
                self.declare(ast, value, path)
            }
            _ => Ok(()),
        }
    }

    /// `<type> <name>` for a member, with an array's dimensions moved
    /// behind the name the way IDL writes them.
    fn member_declaration(
        &mut self,
        ast: &Ast,
        type_: Id<TypeNode>,
        name: &str,
        path: &str,
    ) -> Result<String, ProbeError> {
        // A member whose declared type *is* an array writes the
        // dimensions after the name, which is the only place IDL has for
        // them. A member whose type is a typedef of an array is
        // `TypeKind::Alias` here, not `TypeKind::Array`, so it takes the
        // name and the typedef carries the dimensions — which is what
        // the source IDL wrote in both cases.
        if let Ok(TypeKind::Array { element, dims }) = ast.type_(type_).map(|n| n.kind.clone()) {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = contract::array_dimensions(ast, &dims, path)?;
            let brackets: String = counts.iter().map(|count| format!("[{count}]")).collect();
            return Ok(format!(
                "{} {name}{brackets};",
                self.spell(ast, element, path)?
            ));
        }
        Ok(format!("{} {name};", self.spell(ast, type_, path)?))
    }

    fn enumeration(
        &mut self,
        ast: &Ast,
        type_: Id<TypeNode>,
        name: &str,
        path: &str,
    ) -> Result<String, ProbeError> {
        // CORBA 3.3 part 2 §9.3.2.5: an enumeration is an unsigned long
        // whose value is the ordinal position of the enumerator. There
        // is no `@value` and no `@bit_bound`, and both change the wire —
        // inventory rows E1 and E2 — so an enumeration that uses either
        // is refused rather than printed without them.
        if ast.effective_bit_bound(type_) != 32 {
            return Err(refuse(
                path,
                "an enumeration with a @bit_bound, where CORBA's holder is always a 32-bit unsigned long",
            ));
        }
        let mut names = Vec::new();
        for (position, id) in ast.enumerators_of(type_).iter().enumerate() {
            let enumerator = ast.enumerator(*id).map_err(|error| ProbeError::NoLayout {
                detail: format!("{error}"),
            })?;
            let value = ast.effective_enumerator_value(*id);
            if value != Some(i128::try_from(position).unwrap_or(i128::MAX)) {
                return Err(refuse(
                    path,
                    "an enumerator with an explicit @value, where CORBA's is its ordinal position",
                ));
            }
            names.push(enumerator.name.clone());
        }
        Ok(format!("enum {name} {{ {} }};\n", names.join(", ")))
    }

    /// Give an anonymous collection a name, and remember it.
    ///
    /// `shape` is the declarator with `@NAME@` where the name goes, so
    /// an array can put its dimensions behind it and a sequence cannot.
    /// The typedef is emitted in the **module of the declaration that
    /// uses it** and referred to absolutely, so a use inside a module
    /// cannot pick up a same-named type of its own. Not at global scope:
    /// the IDL-to-Java mapping puts a global name in Java's default
    /// package, which a class inside a named package has no way to
    /// reference — JacORB's generated `SHelper` on
    /// `resolve-anonymous-nested-sequence` said `cannot find symbol:
    /// zd_anon_1Helper` for exactly that.
    ///
    /// Rule (7) makes a typedef transparent on the wire — inventory row
    /// F1, measured against Cyclone and RTI — so the name costs no byte.
    fn name_it(&mut self, type_: Id<TypeNode>, shape: &str) -> Result<String, ProbeError> {
        if let Some((_, name)) = self.named.iter().find(|(known, _)| *known == type_) {
            return Ok(name.clone());
        }
        let name = format!("zd_anon_{}", self.named.len() + 1);
        self.declarations.push((
            self.scope.clone(),
            format!("typedef {};\n", shape.replace("@NAME@", &name)),
        ));
        let mut absolute = String::new();
        for segment in &self.scope {
            let _ = write!(absolute, "::{segment}");
        }
        let _ = write!(absolute, "::{name}");
        self.named.push((type_, absolute.clone()));
        Ok(absolute)
    }

    /// How a type is written as the **element** of a collection.
    ///
    /// A collection element that is itself an anonymous collection has
    /// no IDL spelling at all: `sequence<sequence<long>>` is a syntax
    /// error in omniidl 1.0 and in JacORB 3.9 alike, measured
    /// 2026-08-10. IDL puts a name in that position and nothing else, so
    /// the printer supplies one. An array element takes the same route
    /// through [`Self::spell`], which has nowhere to put its dimensions
    /// either.
    fn spell_element(
        &mut self,
        ast: &Ast,
        type_: Id<TypeNode>,
        path: &str,
    ) -> Result<String, ProbeError> {
        let spelled = self.spell(ast, type_, path)?;
        // Already a name — a typedef, a struct, a union, an enum, or a
        // typedef this walk invented a moment ago.
        if spelled.starts_with("::") {
            return Ok(spelled);
        }
        if matches!(
            ast.type_(type_).map(|node| node.kind.clone()),
            Ok(TypeKind::Sequence { .. } | TypeKind::Map { .. })
        ) {
            return self.name_it(type_, &format!("{spelled} @NAME@"));
        }
        Ok(spelled)
    }

    /// How a type is written where it is used.
    fn spell(&mut self, ast: &Ast, type_: Id<TypeNode>, path: &str) -> Result<String, ProbeError> {
        if let Some(scoped) = corba_scoped_name(ast, type_) {
            let named = matches!(
                ast.type_(type_).map(|n| n.kind.clone()),
                Ok(TypeKind::Structure { .. }
                    | TypeKind::Union { .. }
                    | TypeKind::Enumeration { .. }
                    | TypeKind::Alias { .. })
            );
            if named {
                return Ok(format!("::{}", scoped.join("::")));
            }
        }
        let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
            detail: format!("{error}"),
        })?;
        match &node.kind {
            TypeKind::Primitive(kind) => primitive(*kind, path).map(ToString::to_string),
            TypeKind::String { width, bound } => {
                let keyword = match width {
                    CharWidth::Char => "string",
                    CharWidth::WChar => "wstring",
                };
                match self.bound_value(ast, *bound, path)? {
                    Some(n) => Ok(format!("{keyword}<{n}>")),
                    None => Ok(keyword.to_string()),
                }
            }
            TypeKind::Sequence { element, bound } => {
                let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the sequence element is unresolved"),
                })?;
                let spelled = self.spell_element(ast, element, path)?;
                match self.bound_value(ast, *bound, path)? {
                    Some(n) => Ok(format!("sequence<{spelled}, {n}>")),
                    None => Ok(format!("sequence<{spelled}>")),
                }
            }
            // CORBA 3.3 part 2 §9.3.2.8 gives `fixed` a BCD form and
            // IDL 4.2 §7.4.1.4.4.3.4 the syntax. It is one of the two
            // constructs this whole exercise exists for: XTypes has no
            // fixed-point type at all, so an ORB is the only witness.
            TypeKind::Fixed { digits, scale } => Ok(format!("fixed<{digits},{scale}>")),
            // An array is the one type whose IDL declaration puts part
            // of itself after the name, so `sequence<long[3]>` is not
            // IDL at all — `typedef long A[3]; sequence<A>` is. Rule (7)
            // makes a typedef transparent on the wire (inventory row
            // F1, measured against Cyclone and RTI), so the typedef the
            // printer invents here changes nothing an ORB serialises.
            //
            // Inventing it rather than refusing the case is what keeps
            // `sequence<long[3]>` measurable at all: it is the broadest
            // reading available and it costs no byte.
            TypeKind::Array { element, dims } => {
                let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the array element is unresolved"),
                })?;
                let counts = contract::array_dimensions(ast, dims, path)?;
                let spelled = self.spell_element(ast, element, path)?;
                let brackets: String = counts.iter().map(|count| format!("[{count}]")).collect();
                self.name_it(type_, &format!("{spelled} @NAME@{brackets}"))
            }
            TypeKind::Alias { aliased } => {
                let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the typedef target is unresolved"),
                })?;
                self.spell(ast, target, path)
            }
            // IDL 4.2 §7.4.13.4.3, rule (199): `map<K, V>` and the
            // bounded `map<K, V, N>`. Printed rather than refused, so
            // each front end answers for itself: tao_idl 2.5.24 parses
            // it and generates `std::map` (measured on the test host —
            // the opendds column's map cells are byte-equal with fastdds
            // and zerodds), omniidl 4.3.3 says "Error in look-up of
            // 'map': 'map' not found" and JacORB 3.9 "Syntax error" at
            // the keyword, both measured on the test host too.
            TypeKind::Map { key, value, bound } => {
                let key = key.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the map key type is unresolved"),
                })?;
                let value = value.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the map value type is unresolved"),
                })?;
                let key_spelled = self.spell_element(ast, key, path)?;
                let value_spelled = self.spell_element(ast, value, path)?;
                match self.bound_value(ast, *bound, path)? {
                    Some(n) => Ok(format!("map<{key_spelled}, {value_spelled}, {n}>")),
                    None => Ok(format!("map<{key_spelled}, {value_spelled}>")),
                }
            }
            TypeKind::Bitmask { .. } => Err(refuse(
                path,
                "a bitmask, which is XTypes 1.3 §7.3.1.2.1.4 and not CORBA IDL",
            )),
            TypeKind::Bitset { .. } => Err(refuse(
                path,
                "a bitset, which is XTypes 1.3 §7.3.1.2.1.3 and not CORBA IDL",
            )),
            other => Err(unfilled(path, other.as_str())),
        }
    }

    fn bound_value(&self, ast: &Ast, bound: Bound, path: &str) -> Result<Option<i128>, ProbeError> {
        match bound.expr() {
            None => Ok(None),
            Some(expr) => ast
                .eval_integral(expr)
                .map(Some)
                .map_err(|_| refuse(path, "a bound that does not evaluate to a count")),
        }
    }
}

/// Whether a member carries `@non_serialized`.
fn is_non_serialized(facets: &zerodds_ast::Facets) -> bool {
    facets
        .xtypes
        .as_ref()
        .is_some_and(|xtypes| xtypes.is_non_serialized)
}

/// One primitive, in CORBA IDL's own spelling.
///
/// CORBA 3.3 part 2 Table 9-1 and §9.3.1.
///
/// ## `int8` and `uint8` become `octet`
///
/// Both are IDL 4.2 §7.4.1.4.4.2's extended integers and all three front
/// ends refuse the keywords — measured 2026-08-10: omniidl "'int8' not
/// found", tao_idl "error in lookup of symbol: int8", JacORB "Undefined
/// name: int8".
///
/// They are spelled `octet` rather than refused. XTypes Table 31 gives
/// Int8, UInt8 and Byte the same size 1 and the same alignment 1, and
/// §9.3.1.1 gives `octet` the same, so the octet on the wire is the same
/// octet whichever of the three names it — which is the shared rule an
/// ORB is being asked about. What differs is only how the *value* is
/// read back, and that is the fill's and the dump's job: the C++
/// backends cast, and the IDL-to-Java mapping already makes `octet` a
/// signed `byte`.
///
/// Refusing them would have cost 24 corpus cases to keep a distinction
/// that exists in the model and not on the wire.
///
/// ## `long double` is refused
///
/// It is CORBA's own type and the ORBs accept the keyword, but the C++
/// mapping makes it the platform's `long double`, which on x86-64 is x87
/// 80-bit extended inside a sixteen-byte object and not the binary128
/// §9.3.1.3 Figure 9.2 specifies. That is the same trap `fill_cpp.rs`
/// documents for Fast DDS. It is measured on its own, outside the
/// corpus, rather than folded into a case whose other members would then
/// be unreadable — `docs/corba-column.md`.
fn primitive(kind: PrimitiveKind, path: &str) -> Result<&'static str, ProbeError> {
    Ok(match kind {
        PrimitiveKind::Boolean => "boolean",
        PrimitiveKind::Octet | PrimitiveKind::Int8 | PrimitiveKind::UInt8 => "octet",
        PrimitiveKind::Char => "char",
        PrimitiveKind::WChar => "wchar",
        PrimitiveKind::Int16 => "short",
        PrimitiveKind::UInt16 => "unsigned short",
        PrimitiveKind::Int32 => "long",
        PrimitiveKind::UInt32 => "unsigned long",
        PrimitiveKind::Int64 => "long long",
        PrimitiveKind::UInt64 => "unsigned long long",
        PrimitiveKind::Float32 => "float",
        PrimitiveKind::Float64 => "double",
        PrimitiveKind::Float128 => {
            // Measured: `omniidl -bcxx` and `tao_idl` both take
            // `long double` and generate. What differs is the octets —
            // CORBA's C++ mapping makes it the platform's x87 extended
            // precision and not §9.3.1.3's binary128 — so this is a
            // divergence of the mapping and not an absence in the
            // grammar. Recorded as a missing form it said CORBA 3.3 has
            // no long double, which it does.
            return Err(ProbeError::MappingDivergence {
                path: path.to_string(),
                kind: "a long double, whose C++ mapping is x87 extended and not \
                       §9.3.1.3's binary128",
            });
        }
    })
}

/// A union case label, in the spelling the discriminator's type needs.
///
/// An integer would compile against a `long` discriminator and fail
/// against an `enum` one, and against a `char` one it would compile and
/// select a different case.
fn case_label(
    ast: &Ast,
    discriminator: Id<TypeNode>,
    value: i128,
    path: &str,
) -> Result<String, ProbeError> {
    let resolved = contract::resolve_alias(ast, discriminator);
    let node = ast.type_(resolved).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Enumeration { .. } => {
            for id in ast.enumerators_of(resolved) {
                if ast.effective_enumerator_value(*id) == Some(value) {
                    let enumerator = ast.enumerator(*id).map_err(|error| ProbeError::NoLayout {
                        detail: format!("{error}"),
                    })?;
                    let scope = corba_scoped_name(ast, resolved)
                        .and_then(|segments| {
                            let (_, scope) = segments.split_last()?;
                            Some(scope.to_vec())
                        })
                        .unwrap_or_default();
                    let mut spelled = String::from("::");
                    for segment in scope {
                        spelled.push_str(&segment);
                        spelled.push_str("::");
                    }
                    spelled.push_str(&enumerator.name);
                    return Ok(spelled);
                }
            }
            Err(refuse(path, "a case label that names no enumerator"))
        }
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            Ok(if value == 0 { "FALSE" } else { "TRUE" }.to_string())
        }
        // IDL 4.2 §7.2.6.2's escapes, not Rust's. `char::escape_default`
        // writes `\u{e4}`, which no IDL front end reads, and a literal
        // `ä` byte would depend on the front end's source character set.
        // `\xHH` and `\uHHHH` are the two forms the grammar has.
        TypeKind::Primitive(PrimitiveKind::Char) => {
            let code = u8::try_from(value)
                .map_err(|_| refuse(path, "a char case label outside one octet"))?;
            Ok(format!("'\\x{code:02x}'"))
        }
        TypeKind::Primitive(PrimitiveKind::WChar) => {
            let code = u16::try_from(value)
                .map_err(|_| refuse(path, "a wchar case label outside sixteen bits"))?;
            Ok(format!("L'\\u{code:04x}'"))
        }
        _ => Ok(format!("{value}")),
    }
}

/// The Java name a type takes in JacORB's generated code.
///
/// The OMG IDL-to-Java mapping (CORBA 3.3 part 3 §1.5.1) makes each IDL
/// module a Java package, so `::common::util::Location` is
/// `common.util.Location`.
///
/// Each segment is a Java identifier and goes through
/// [`crate::target_keywords::java`] on the way: measured 2026-08-11 from
/// JacORB 3.9's output for
/// `tests/interop-corpus/identifier-target-keywords`, the IDL module
/// `package` becomes the Java package `_package` and the enumeration
/// `class` the class `_class`, so the type's Java name there is
/// `_package._class` and nothing else finds the generated class. The
/// name the IDL itself carries is untouched — [`corba_idl`] prints the
/// segments as the corpus wrote them, because that file is the input the
/// renaming is a consequence of.
#[must_use]
pub fn java_name(ast: &Ast, type_: Id<TypeNode>) -> String {
    corba_scoped_name(ast, type_).map_or_else(
        || String::from("UNKNOWN"),
        |scoped| {
            scoped
                .iter()
                .map(|segment| target_keywords::java(segment).into_owned())
                .collect::<Vec<String>>()
                .join(".")
        },
    )
}

/// The C++ name a type takes in omniORB's and TAO's generated code.
///
/// The OMG IDL-to-C++ mapping (CORBA 3.3 part 3 §1.2.2) makes each IDL
/// module a C++ namespace.
///
/// Each segment is a C++ identifier and goes through
/// [`crate::target_keywords::cxx`], for [`java_name`]'s reason and from
/// the same measurement: omniidl and `tao_idl` generate
/// `enum _cxx_class` for the corpus enumeration `class`, so the C++ name
/// of `::package::class` is `package::_cxx_class` — the module is left
/// alone, `package` being no C++ keyword, and the type is not.
#[must_use]
pub fn cpp_name(ast: &Ast, type_: Id<TypeNode>) -> String {
    corba_scoped_name(ast, type_).map_or_else(
        || String::from("UNKNOWN"),
        |scoped| {
            scoped
                .iter()
                .map(|segment| target_keywords::cxx(segment).into_owned())
                .collect::<Vec<String>>()
                .join("::")
        },
    )
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The fill and the dump for the OMG IDL-to-Java mapping.
//!
//! JacORB 3.9 generates against CORBA 3.3 part 3 §1.5. It is the third
//! ORB and the only one of the three that is not C++, which is the
//! reason it is worth having: where omniORB and TAO could share a
//! mistake through the C++ mapping, a Java witness cannot.
//!
//! ```text
//! sample.x = 1;                        // struct member — §1.5.10, public fields
//! sample.picked.whole(1);              // union case member — §1.5.11
//! sample.rows = new int[3];            // sequence and array are both Java arrays
//! ```
//!
//! ## Why this is not a self-oracle
//!
//! The statements place values into JacORB's own generated classes and
//! read them back out; the bytes come from JacORB's `CDROutputStream`
//! through the generated `Helper.write`. Nothing here decides a wire
//! question.
//!
//! ## What the Java mapping makes of each construct
//!
//! | IDL | Java | consequence here |
//! |---|---|---|
//! | `octet` | `byte`, **signed** | which is exactly what `int8` wants, and what `uint8` has to mask |
//! | `char`, `wchar` | `char` | one UTF-16 code unit either way |
//! | `string`, `wstring` | `java.lang.String` | |
//! | `sequence<T>`, `T[N]` | a Java array | so a sequence is allocated, not lengthened |
//! | `enum` | a final class with one constant per enumerator | `Colour.RED`, and `value()` for the ordinal |
//! | `union` | a class with a modifier per case and `discriminator()` | §1.5.11 |
//! | `fixed<P,S>` | `java.math.BigDecimal` | §1.5.6 |
//!
//! ## The one thing the Java mapping cannot express
//!
//! There is no `long double`. §1.5.2 maps every other primitive and
//! stops there, so a case with one is refused rather than approximated
//! into a `double` — which would put eight bytes on the wire where the
//! type says sixteen.
//!
//! ## Identifiers JacORB renames
//!
//! An IDL module called `package` is not a Java package declaration, so
//! JacORB emits `_package` instead; the enumeration `class` becomes the
//! class `_class`. Measured 2026-08-11 over
//! `tests/interop-corpus/identifier-target-keywords`. Every identifier
//! this file pastes into an identifier position therefore goes through
//! [`crate::target_keywords::java`] — and only a Java keyword moves, so
//! the enumerators `template`, `virtual` and `friend`, which the two C++
//! front ends do rename, keep their spelling here. The JSON keys the
//! dump prints and the paths in a [`ProbeError`] stay as the IDL wrote
//! them.

use crate::contract::{self, Consumed};
use crate::corba_idl::java_name;
use crate::encode::ProbeError;
use crate::target_keywords::java;
use serde_json::Value;
use std::fmt::Write as _;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// Where one value goes.
enum Place {
    /// A public field, or an array element.
    Field(String),
    /// A union case member: `object.name(value)`.
    Modifier {
        /// The expression naming the union.
        object: String,
        /// The case member's name.
        name: String,
    },
    /// §1.5.11's second modifier, `object.name(discriminator, value)`,
    /// which a case with more than one label or a `default:` needs.
    ModifierWithDiscriminator {
        /// The expression naming the union.
        object: String,
        /// The case member's name.
        name: String,
        /// The discriminator literal to write beside the value.
        discriminator: String,
    },
}

impl Place {
    fn assign(&self, literal: &str) -> String {
        match self {
            Self::Field(expression) => format!("{expression} = {literal};"),
            Self::Modifier { object, name } => format!("{object}.{name}({literal});"),
            Self::ModifierWithDiscriminator {
                object,
                name,
                discriminator,
            } => format!("{object}.{name}({discriminator}, {literal});"),
        }
    }
}

/// The statements, plus a counter for the temporaries a nested
/// allocation needs.
#[derive(Debug, Default)]
struct Java {
    statements: String,
    next: usize,
    seen: Consumed,
}

impl Java {
    fn temporary(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("zd_{stem}_{}", self.next)
    }

    fn line(&mut self, text: &str) {
        self.statements.push_str(text);
        self.statements.push('\n');
    }
}

/// Emit the Java statements that fill `sample` with `values`.
///
/// # Errors
/// [`ProbeError`], naming the path in the value.
pub fn fill_corba_java(
    ast: &Ast,
    type_: Id<TypeNode>,
    values: &Value,
) -> Result<String, ProbeError> {
    fill_corba_java_recording(ast, type_, values, &mut Consumed::new())
}

/// As [`fill_corba_java`], recording which scalars of the value were
/// read.
///
/// # Errors
/// As [`fill_corba_java`].
pub fn fill_corba_java_recording(
    ast: &Ast,
    type_: Id<TypeNode>,
    values: &Value,
    seen: &mut Consumed,
) -> Result<String, ProbeError> {
    // Before the first hop; see `contract::reject_alias_cycle`.
    contract::reject_alias_cycle(ast)?;
    let mut out = Java::default();
    let root = contract::root_path(ast, type_);
    let result = fill_at(
        ast,
        type_,
        values,
        &Place::Field(String::from("sample")),
        &mut out,
        &root,
    );
    *seen = core::mem::take(&mut out.seen);
    result?;
    Ok(out.statements)
}

/// The Java type of one IDL type, for `new` and for a local.
///
/// # Errors
/// [`ProbeError::Unsupported`] for a construct with no Java binding.
///
/// zerodds-lint: recursion-depth 64 — the type graph's own nesting. An
/// aggregate is a leaf here: a struct, a union, an enumeration and a
/// valuetype are all spelled by `java_name` and never descended into, so
/// only the collection and typedef arms recurse. A collection nests as
/// deeply as the IDL text writes it, and a typedef chain that returns to
/// itself has been refused by `contract::reject_alias_cycle` at the entry
/// point.
fn java_type(ast: &Ast, type_: Id<TypeNode>, path: &str) -> Result<String, ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => java_primitive(*kind, path)?.to_string(),
        TypeKind::String { .. } => "java.lang.String".to_string(),
        TypeKind::Fixed { .. } => "java.math.BigDecimal".to_string(),
        TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the element type is unresolved"),
            })?;
            format!("{}[]", java_type(ast, element, path)?)
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            java_type(ast, target, path)?
        }
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Valuetype { .. } => java_name(ast, type_),
        // The same refusal the direct-member arms state, so a map as an
        // array or sequence element names JacORB's grammar gap instead
        // of falling into the catch-all — `array-1d-map` and
        // `sequence-map` reached this arm through the element type.
        TypeKind::Map { .. } => {
            return Err(ProbeError::Unsupported {
                path: path.to_string(),
                kind: "a map, which is IDL 4.2 §7.4.13.4.3 and outside JacORB 3.9's grammar — its idl says `Syntax error` at the keyword, measured on the test host",
            });
        }
        other => {
            return Err(ProbeError::Unfilled {
                path: path.to_string(),
                kind: other.as_str(),
            });
        }
    })
}

/// zerodds-lint: recursion-depth 128 — the sample's own nesting. Every arm
/// but the typedef one descends into a strictly smaller part of the value,
/// so the walk ends because the sample is finite: a recursive type has
/// finite values, because the sequence that closes its cycle is eventually
/// empty. The typedef arm keeps the value and is finite because
/// `contract::reject_alias_cycle` at the entry point has refused a chain
/// that returns to itself. A value file is `serde_json`'s to parse and its
/// `RECURSION_LIMIT` is 128.
#[allow(clippy::too_many_lines)]
fn fill_at(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    place: &Place,
    out: &mut Java,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Primitive(kind) => {
            let literal = primitive_literal(*kind, value, path)?;
            out.seen.scalar(path);
            let statement = place.assign(&literal);
            out.line(&statement);
            Ok(())
        }
        // A `wstring` is a `String` too. The units the sample carries
        // are UTF-16 and a Java `String` *is* UTF-16, so a code point
        // above the BMP goes in as its surrogate pair without the
        // emitter arranging one.
        TypeKind::String { .. } => {
            let text = contract::text(value, path)?;
            out.seen.scalar(path);
            let statement = place.assign(&java_string(text));
            out.line(&statement);
            Ok(())
        }
        TypeKind::Fixed { .. } => {
            let text = contract::text(value, path)?;
            out.seen.scalar(path);
            let statement =
                place.assign(&format!("new java.math.BigDecimal({})", java_string(text)));
            out.line(&statement);
            Ok(())
        }
        TypeKind::Enumeration { .. } => {
            let (name, _) = contract::enumerator(ast, type_, value, path)?;
            out.seen.scalar(path);
            // §1.5.9's constant, whose name is the enumerator's — so it
            // is renamed exactly when the enumerator is a Java keyword,
            // which none of the corpus's three are.
            let statement = place.assign(&format!("{}.{}", java_name(ast, type_), java(&name)));
            out.line(&statement);
            Ok(())
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            fill_at(ast, target, value, place, out, path)
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let object = contract::object(value, path)?;
            let members = contract::members(ast, type_);
            let declared: Vec<String> = members.iter().map(|member| member.name.clone()).collect();
            contract::reject_unknown_keys(object, &declared, path)?;
            // A Java aggregate is a reference, so it is built in a local
            // and assigned once. That is also what makes a union case
            // member reachable: the modifier takes a whole object and
            // there is no reference to descend into.
            let class = java_type(ast, type_, path)?;
            let local = out.temporary("obj");
            out.line(&format!("{class} {local} = new {class}();"));
            for member in &members {
                let Some(member_value) = contract::member_value(object, member, path)? else {
                    continue;
                };
                fill_at(
                    ast,
                    member.type_,
                    member_value,
                    // §1.5.10's public field, under the name Java has it
                    // by; the path keeps the name the IDL wrote.
                    &Place::Field(format!("{local}.{}", java(&member.name))),
                    out,
                    &contract::join(path, &member.name),
                )?;
            }
            let statement = place.assign(&local);
            out.line(&statement);
            Ok(())
        }
        TypeKind::Union { discriminator, .. } => {
            let object = contract::object(value, path)?;
            let discriminant = contract::discriminator(object, path)?;
            let class = java_type(ast, type_, path)?;
            let local = out.temporary("union");
            out.line(&format!("{class} {local} = new {class}();"));
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the discriminator is unresolved"),
                })?;
            let disc_literal = discriminator_literal(ast, discriminator, discriminant, path)?;

            let mut selected = false;
            for (name, member_value) in object {
                if name == "_d" {
                    continue;
                }
                let Some(case) = union_case(ast, type_, name) else {
                    return Err(ProbeError::Unknown {
                        path: contract::join(path, name),
                        declared: union_member_names(ast, type_),
                    });
                };
                contract::reject_null(member_value, &contract::join(path, name))?;
                selected = true;
                // §1.5.11 gives a case member one modifier, and a second
                // taking the discriminator, "if the branch has more than
                // one case label or is the default". The one-argument
                // form sets the discriminator to the case's *first*
                // label — measured in JacORB 3.9's own output — so a
                // sample whose `_d` is the second label of a case, or a
                // value that reaches `default:`, needs the second form
                // or the discriminator on the wire would be the wrong
                // one.
                let member_place = if case.explicit_labels == 1 && !case.is_default {
                    Place::Modifier {
                        object: local.clone(),
                        name: java(name).into_owned(),
                    }
                } else {
                    Place::ModifierWithDiscriminator {
                        object: local.clone(),
                        name: java(name).into_owned(),
                        discriminator: disc_literal.clone(),
                    }
                };
                fill_at(
                    ast,
                    case.type_,
                    member_value,
                    &member_place,
                    out,
                    &contract::join(path, name),
                )?;
            }
            if !selected {
                // A discriminator that selects no member at all. §1.5.11
                // gives the union `__default(int)` for exactly this, and
                // there is no other way in — every modifier is attached
                // to a member.
                out.line(&format!("{local}.__default({disc_literal});"));
            }
            let statement = place.assign(&local);
            out.line(&statement);
            Ok(())
        }
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = contract::array_dimensions(ast, dims, path)?;
            let local = out.temporary("arr");
            let brackets: String = counts.iter().map(|count| format!("[{count}]")).collect();
            let (declared, allocation) =
                java_array(&java_type(ast, element, path)?, counts.len(), &brackets);
            out.line(&format!("{declared} {local} = {allocation};"));
            fill_array(ast, element, value, &local, out, path, &counts)?;
            let statement = place.assign(&local);
            out.line(&statement);
            Ok(())
        }
        TypeKind::Sequence { element, bound } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the sequence element is unresolved"),
            })?;
            let items = contract::bounded_items(ast, value, *bound, "elements", path)?;
            let local = out.temporary("seq");
            let (declared, allocation) = java_array(
                &java_type(ast, element, path)?,
                1,
                &format!("[{}]", items.len()),
            );
            out.line(&format!("{declared} {local} = {allocation};"));
            for (index, item) in items.iter().enumerate() {
                fill_at(
                    ast,
                    element,
                    item,
                    &Place::Field(format!("{local}[{index}]")),
                    out,
                    &format!("{path}[{index}]"),
                )?;
            }
            let statement = place.assign(&local);
            out.line(&statement);
            Ok(())
        }
        TypeKind::Bitmask { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a bitmask, which is XTypes 1.3 §7.3.1.2.1.4 and not CORBA IDL",
        }),
        TypeKind::Bitset { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a bitset, which is XTypes 1.3 §7.3.1.2.1.3 and not CORBA IDL",
        }),
        TypeKind::Map { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a map, which is IDL 4.2 §7.4.13.4.3 and outside JacORB 3.9's grammar — its idl says `Syntax error` at the keyword, measured on the test host",
        }),
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// zerodds-lint: recursion-depth 8 — one frame per array dimension.
/// `counts` drops its head at each frame and the innermost element goes to
/// `fill_at`, which is bounded by the sample.
#[allow(clippy::too_many_arguments)]
fn fill_array(
    ast: &Ast,
    element: Id<TypeNode>,
    value: &Value,
    lvalue: &str,
    out: &mut Java,
    path: &str,
    counts: &[usize],
) -> Result<(), ProbeError> {
    let Some((&count, rest)) = counts.split_first() else {
        return fill_at(
            ast,
            element,
            value,
            &Place::Field(lvalue.to_string()),
            out,
            path,
        );
    };
    let items = contract::array(value, path)?;
    if items.len() != count {
        return Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: format!("exactly {count} elements"),
            found: format!("{}", items.len()),
        });
    }
    for (index, item) in items.iter().enumerate() {
        fill_array(
            ast,
            element,
            item,
            &format!("{lvalue}[{index}]"),
            out,
            &format!("{path}[{index}]"),
            rest,
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------- dump

/// The statements, ready to paste into a decode driver.
#[derive(Debug, Default)]
struct Dump {
    body: String,
    depth: usize,
    next: usize,
    /// The aggregates on a cycle, which this walk refuses rather than
    /// expands. See [`crate::recursion::recursive_aggregates`] and the
    /// arms in [`dump_at`] that read it.
    rec: Vec<Id<TypeNode>>,
}

impl Dump {
    fn line(&mut self, text: &str) {
        for _ in 0..self.depth {
            self.body.push_str("    ");
        }
        self.body.push_str(text);
        self.body.push('\n');
    }

    fn open(&mut self, text: &str) {
        self.line(text);
        self.depth += 1;
    }

    fn close(&mut self, text: &str) {
        self.depth = self.depth.saturating_sub(1);
        self.line(text);
    }

    fn raw(&mut self, literal: &str) {
        self.line(&format!("Zdj.raw({});", java_string(literal)));
    }

    fn temporary(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("zd_{stem}_{}", self.next)
    }
}

/// Emit the Java statements that print `sample` as canonical JSON.
///
/// # Errors
/// [`ProbeError`], naming the path in the type.
pub fn dump_corba_java(ast: &Ast, type_: Id<TypeNode>) -> Result<String, ProbeError> {
    // Before `recursive_aggregates`; see `contract::reject_alias_cycle`.
    contract::reject_alias_cycle(ast)?;
    let mut out = Dump {
        rec: crate::recursion::recursive_aggregates(ast, type_),
        ..Dump::default()
    };
    dump_at(ast, type_, "sample", &mut out, "")?;
    out.line("Zdj.raw(\"\\n\");");
    Ok(out.body)
}

/// zerodds-lint: recursion-depth 64 — the type graph's own nesting,
/// which is finite because the walk never enters a cycle: an aggregate
/// on one is refused in the two arms below, and a typedef chain that
/// returns to itself has been refused by `reject_alias_cycle` before the
/// first hop. Every other arm strictly descends the graph, so the depth
/// is the IDL's own and the corpus's deepest is four.
///
/// Refused and not expanded into a function, as `corba_cpp.rs` does for
/// the same walk in C++: `dump.inc` lands inside a Java method body,
/// where a method cannot be declared, and `corba_idl` refuses a
/// recursive type for this column anyway — "a recursive type, which
/// CORBA CDR frames its own way". Until this arm existed, this emitter
/// did not refuse it either: it followed
/// `A → sequence<B> → B → sequence<A>` until the
/// stack ran out, which the corpus case `recursive-mutual` reproduces.
/// The C++ half of the same file carries the `rec` machinery; this half
/// was left behind when it was added.
#[allow(clippy::too_many_lines)]
fn dump_at(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Primitive(kind) => {
            out.line(&primitive_print(*kind, expr, path)?);
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => {
            out.line(&format!("Zdj.string({expr});"));
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => {
            out.line(&format!("Zdj.wstring({expr});"));
            Ok(())
        }
        TypeKind::Fixed { .. } => {
            out.line(&format!("Zdj.fixed({expr});"));
            Ok(())
        }
        TypeKind::Enumeration { .. } => {
            dump_enumeration(ast, type_, expr, out);
            Ok(())
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            dump_at(ast, target, expr, out, path)
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            if out.rec.contains(&type_) {
                return Err(recursive_type(path));
            }
            let first = out.temporary("first");
            out.open("{");
            out.line(&format!("boolean[] {first} = {{ true }};"));
            out.raw("{");
            for member in contract::members(ast, type_) {
                // The key is the IDL name and the read is the Java one.
                out.line(&format!("Zdj.key({first}, \"{}\");", member.name));
                dump_at(
                    ast,
                    member.type_,
                    &format!("{expr}.{}", java(&member.name)),
                    out,
                    &contract::join(path, &member.name),
                )?;
            }
            out.raw("}");
            out.close("}");
            Ok(())
        }
        TypeKind::Union { discriminator, .. } => {
            if out.rec.contains(&type_) {
                return Err(recursive_type(path));
            }
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the discriminator is unresolved"),
                })?;
            let first = out.temporary("first");
            out.open("{");
            out.line(&format!("boolean[] {first} = {{ true }};"));
            out.raw("{");
            out.line(&format!("Zdj.key({first}, \"_d\");"));
            dump_at(
                ast,
                discriminator,
                &format!("{expr}.discriminator()"),
                out,
                &format!("{path}._d"),
            )?;
            // What the `switch` selects on. §1.5.9 maps an enumeration to
            // a class whose ordinal is `value()`, and §1.5.2 maps
            // `boolean` to Java's own — neither is a numeric type a cast
            // reaches: `javac` refused both, "incompatible types: Colour
            // cannot be converted to int" on `union-discriminator-enum`
            // and "boolean cannot be converted to int" on
            // `union-discriminator-boolean`, measured in the jacorb
            // column. The labels come out of `eval_integral` (TRUE is 1,
            // FALSE is 0, an enumerator its ordinal), so the selector has
            // to reach the same integers.
            let resolved = contract::resolve_alias(ast, discriminator);
            let selector = match ast.type_(resolved).map(|node| node.kind.clone()) {
                Ok(TypeKind::Enumeration { .. }) => {
                    format!("{expr}.discriminator().value()")
                }
                Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                    format!("{expr}.discriminator() ? 1 : 0")
                }
                _ => format!("(int)({expr}.discriminator())"),
            };
            out.open(&format!("switch ({selector}) {{"));
            for id in ast.cases_of(type_) {
                let Ok(case) = ast.case(*id) else { continue };
                let Ok(member) = ast.member(case.member) else {
                    continue;
                };
                let Some(member_type) = member.type_.resolved() else {
                    continue;
                };
                for label in &case.labels {
                    match label.expr() {
                        Some(expression) => match ast.eval_integral(expression) {
                            Ok(value) => out.line(&format!("case {}:", value as i32)),
                            Err(error) => {
                                return Err(ProbeError::NoLayout {
                                    detail: format!(
                                        "{path}: a case label does not evaluate: {error}"
                                    ),
                                });
                            }
                        },
                        None => out.line("default:"),
                    }
                }
                out.depth += 1;
                out.line(&format!("Zdj.key({first}, \"{}\");", member.name));
                dump_at(
                    ast,
                    member_type,
                    &format!("{expr}.{}()", java(&member.name)),
                    out,
                    &contract::join(path, &member.name),
                )?;
                out.line("break;");
                out.depth = out.depth.saturating_sub(1);
            }
            out.close("}");
            out.raw("}");
            out.close("}");
            Ok(())
        }
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = contract::array_dimensions(ast, dims, path)?;
            dump_array(ast, element, expr, out, path, &counts)
        }
        TypeKind::Sequence { element, .. } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the sequence element is unresolved"),
            })?;
            let index = out.temporary("i");
            out.open("{");
            out.raw("[");
            out.open(&format!(
                "for (int {index} = 0; {index} < {expr}.length; {index}++) {{"
            ));
            out.line(&format!("if ({index} > 0) {{ Zdj.raw(\",\"); }}"));
            dump_at(
                ast,
                element,
                &format!("{expr}[{index}]"),
                out,
                &format!("{path}[]"),
            )?;
            out.close("}");
            out.raw("]");
            out.close("}");
            Ok(())
        }
        TypeKind::Bitmask { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a bitmask, which is XTypes 1.3 §7.3.1.2.1.4 and not CORBA IDL",
        }),
        TypeKind::Bitset { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a bitset, which is XTypes 1.3 §7.3.1.2.1.3 and not CORBA IDL",
        }),
        TypeKind::Map { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a map, which is IDL 4.2 §7.4.13.4.3 and outside JacORB 3.9's grammar — its idl says `Syntax error` at the keyword, measured on the test host",
        }),
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// The refusal for an aggregate on a cycle, in the words `corba_idl`
/// uses for the same type in the same column.
fn recursive_type(path: &str) -> ProbeError {
    ProbeError::Unsupported {
        path: path.to_string(),
        kind: "a recursive type, which CORBA CDR frames its own way — the same refusal \
               `corba_idl` makes, said here because the expansion of a cycle does not end",
    }
}

/// zerodds-lint: recursion-depth 8 — one frame per array dimension.
/// `counts` is the dimension list `contract::array_dimensions` built and
/// each frame drops its head, so the walk ends at the empty slice; the
/// innermost element goes to [`dump_at`], which is bounded separately.
fn dump_array(
    ast: &Ast,
    element: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
    counts: &[usize],
) -> Result<(), ProbeError> {
    let Some((&count, rest)) = counts.split_first() else {
        return dump_at(ast, element, expr, out, path);
    };
    out.raw("[");
    for index in 0..count {
        if index > 0 {
            out.raw(",");
        }
        dump_array(
            ast,
            element,
            &format!("{expr}[{index}]"),
            out,
            &format!("{path}[{index}]"),
            rest,
        )?;
    }
    out.raw("]");
    Ok(())
}

/// An enumeration, printed as the enumerator's name.
///
/// §1.5.9 makes the generated class carry `value()`, the ordinal, and
/// one constant per enumerator. The switch is over the ordinal so that a
/// value naming no enumerator reaches the default arm rather than
/// throwing out of `from_int`.
///
/// The `int` constant is `_` and then the enumerator's name, which is
/// §1.5.9's spelling and not JacORB's invention. Where the enumerator is
/// itself a Java keyword the two underscores stack — `__new` for `new`,
/// because the object constant is already `_new`. Composed rather than
/// measured: none of the corpus's enumerators is a Java keyword.
fn dump_enumeration(ast: &Ast, type_: Id<TypeNode>, expr: &str, out: &mut Dump) {
    let class = java_name(ast, type_);
    out.open(&format!("switch ({expr}.value()) {{"));
    for id in ast.enumerators_of(type_) {
        let Ok(node) = ast.enumerator(*id) else {
            continue;
        };
        out.line(&format!("case {class}._{}:", java(&node.name)));
        out.line(&format!("    Zdj.raw(\"\\\"{}\\\"\"); break;", node.name));
    }
    out.line(&format!("default: Zdj.enumUnknown({expr}.value()); break;"));
    out.close("}");
}

// ------------------------------------------------------------- helpers

/// The declared type and the `new` expression for an array of
/// `element`.
///
/// Java puts the sizes on the **innermost** dimensions and leaves the
/// element's own dimensions empty: an array of two `int[]` is
/// `new int[2][]` and not `new int[][2]`, which is a syntax error. The
/// element type's trailing brackets therefore have to move behind the
/// sizes rather than in front of them — `sequence-of-array` and the
/// three cases beside it were the ones that did not compile.
fn java_array(element: &str, depth: usize, sizes: &str) -> (String, String) {
    let base = element.trim_end_matches("[]");
    let suffix = &element[base.len()..];
    let declared = format!("{base}{}{suffix}", "[]".repeat(depth));
    (declared, format!("new {base}{sizes}{suffix}"))
}

/// The Java type §1.5.2 gives one primitive.
fn java_primitive(kind: PrimitiveKind, path: &str) -> Result<&'static str, ProbeError> {
    Ok(match kind {
        PrimitiveKind::Boolean => "boolean",
        // `int8` and `uint8` are spelled `octet` in the printed IDL, and
        // §1.5.2 makes an `octet` a signed `byte`.
        PrimitiveKind::Octet | PrimitiveKind::Int8 | PrimitiveKind::UInt8 => "byte",
        PrimitiveKind::Char | PrimitiveKind::WChar => "char",
        PrimitiveKind::Int16 | PrimitiveKind::UInt16 => "short",
        PrimitiveKind::Int32 | PrimitiveKind::UInt32 => "int",
        PrimitiveKind::Int64 | PrimitiveKind::UInt64 => "long",
        PrimitiveKind::Float32 => "float",
        PrimitiveKind::Float64 => "double",
        PrimitiveKind::Float128 => {
            return Err(ProbeError::Unsupported {
                path: path.to_string(),
                kind: "a long double, which the IDL-to-Java mapping §1.5.2 has no type for",
            });
        }
    })
}

/// The Java literal for one primitive.
///
/// Java has no unsigned integers, so an `unsigned short` is a `short`
/// and an `unsigned long` an `int` — §1.5.2 says so outright. The
/// literal is therefore the same bit pattern read signed, computed here
/// rather than left to a narrowing cast the compiler would refuse.
fn primitive_literal(kind: PrimitiveKind, value: &Value, path: &str) -> Result<String, ProbeError> {
    Ok(match kind {
        PrimitiveKind::Boolean => contract::boolean(value, path)?.to_string(),
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            let code = contract::character_for(kind, value, path)?;
            format!("(char)0x{:04x}", code as u32)
        }
        PrimitiveKind::Octet | PrimitiveKind::Int8 | PrimitiveKind::UInt8 => {
            let integer = contract::integer_for(kind, value, path)?;
            #[allow(clippy::cast_possible_truncation)]
            let octet = (integer as i64) as u8;
            format!("(byte)0x{octet:02x}")
        }
        PrimitiveKind::Int16 | PrimitiveKind::UInt16 => {
            let integer = contract::integer_for(kind, value, path)?;
            #[allow(clippy::cast_possible_truncation)]
            let bits = (integer as i64) as u16;
            format!("(short)0x{bits:04x}")
        }
        PrimitiveKind::Int32 | PrimitiveKind::UInt32 => {
            let integer = contract::integer_for(kind, value, path)?;
            #[allow(clippy::cast_possible_truncation)]
            let bits = (integer as i64) as u32;
            format!("0x{bits:08x}")
        }
        PrimitiveKind::Int64 | PrimitiveKind::UInt64 => {
            let integer = contract::integer_for(kind, value, path)?;
            #[allow(clippy::cast_possible_truncation)]
            let bits = integer as u64;
            format!("0x{bits:016x}L")
        }
        // `Float.intBitsToFloat` and not a decimal literal: the sample's
        // value is exact and a decimal that the Java compiler rounds
        // differently from `contract::float` would put different bytes
        // on the wire than every other probe.
        PrimitiveKind::Float32 => {
            #[allow(clippy::cast_possible_truncation)]
            let bits = (contract::float(value, path)? as f32).to_bits();
            format!("Float.intBitsToFloat(0x{bits:08x})")
        }
        PrimitiveKind::Float64 => {
            let bits = contract::float(value, path)?.to_bits();
            format!("Double.longBitsToDouble(0x{bits:016x}L)")
        }
        PrimitiveKind::Float128 => {
            return Err(ProbeError::Unsupported {
                path: path.to_string(),
                kind: "a long double, which the IDL-to-Java mapping §1.5.2 has no type for",
            });
        }
    })
}

/// The literal a union's discriminator takes in `__set_discriminator`.
fn discriminator_literal(
    ast: &Ast,
    discriminator: Id<TypeNode>,
    value: &Value,
    path: &str,
) -> Result<String, ProbeError> {
    let resolved = contract::resolve_alias(ast, discriminator);
    let node = ast.type_(resolved).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Enumeration { .. } => {
            let (name, _) = contract::enumerator(ast, resolved, value, path)?;
            Ok(format!("{}.{}", java_name(ast, resolved), java(&name)))
        }
        TypeKind::Primitive(kind) => primitive_literal(*kind, value, path),
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// The call that prints one primitive as canonical JSON.
///
/// Every unsigned type arrives in a signed Java one, so the sign has to
/// be undone before it is printed — an `unsigned long` of 0xDEADBEEF is
/// an `int` of −559038737 in Java and the canonical form wants
/// 3735928559.
fn primitive_print(kind: PrimitiveKind, expr: &str, path: &str) -> Result<String, ProbeError> {
    Ok(match kind {
        PrimitiveKind::Boolean => format!("Zdj.bool({expr});"),
        PrimitiveKind::Char => format!("Zdj.ch({expr});"),
        PrimitiveKind::WChar => format!("Zdj.wch({expr});"),
        PrimitiveKind::Int8 => format!("Zdj.i64((long)({expr}));"),
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => {
            format!("Zdj.u64((long)({expr}) & 0xFFL);")
        }
        PrimitiveKind::Int16 => format!("Zdj.i64((long)({expr}));"),
        PrimitiveKind::UInt16 => format!("Zdj.u64((long)({expr}) & 0xFFFFL);"),
        PrimitiveKind::Int32 => format!("Zdj.i64((long)({expr}));"),
        PrimitiveKind::UInt32 => format!("Zdj.u64((long)({expr}) & 0xFFFFFFFFL);"),
        PrimitiveKind::Int64 => format!("Zdj.i64String({expr});"),
        // No wider Java integer to widen into, so the unsigned decimal
        // is produced by `Long.toUnsignedString` inside the helper.
        PrimitiveKind::UInt64 => format!("Zdj.u64String({expr});"),
        PrimitiveKind::Float32 => format!("Zdj.f32({expr});"),
        PrimitiveKind::Float64 => format!("Zdj.f64({expr});"),
        PrimitiveKind::Float128 => {
            return Err(ProbeError::Unsupported {
                path: path.to_string(),
                kind: "a long double, which the IDL-to-Java mapping §1.5.2 has no type for",
            });
        }
    })
}

/// One case of a union, with what §1.5.11 needs to know about its
/// labels.
struct UnionCase {
    /// The case member's type.
    type_: Id<TypeNode>,
    /// How many `case <value>:` labels it carries.
    explicit_labels: usize,
    /// Whether it also carries `default:`.
    is_default: bool,
}

fn union_case(ast: &Ast, union: Id<TypeNode>, name: &str) -> Option<UnionCase> {
    for id in ast.cases_of(union) {
        let case = ast.case(*id).ok()?;
        let member = ast.member(case.member).ok()?;
        if member.name != name {
            continue;
        }
        return Some(UnionCase {
            type_: member.type_.resolved()?,
            explicit_labels: case
                .labels
                .iter()
                .filter(|label| label.expr().is_some())
                .count(),
            is_default: case.labels.iter().any(|label| label.expr().is_none()),
        });
    }
    None
}

fn union_member_names(ast: &Ast, union: Id<TypeNode>) -> Vec<String> {
    contract::union_cases(ast, union)
        .into_iter()
        .map(|case| case.name)
        .collect()
}

/// A Java string literal, with every non-ASCII character escaped.
///
/// `\uXXXX` and not raw UTF-8: `javac` reads the source in the platform
/// encoding unless told otherwise, and a probe whose string depended on
/// a locale would produce different bytes on two machines.
fn java_string(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if other.is_ascii_graphic() || other == ' ' => out.push(other),
            other => {
                let mut buffer = [0u16; 2];
                for unit in other.encode_utf16(&mut buffer) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
    }
    out.push('"');
    out
}

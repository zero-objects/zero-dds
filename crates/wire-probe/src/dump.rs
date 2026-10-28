// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value-printing code a foreign probe needs to answer with.
//!
//! `fill.rs` writes the statements that put a JSON sample into a
//! vendor's generated structure. This writes the statements that take
//! one back out: after the vendor's own deserialiser has filled the
//! structure, these print it as the canonical JSON of
//! `crate::canonical`, which is the form a pairwise run compares in.
//!
//! ```text
//! sample.ids._length = 3;                  // fill.rs
//! zdj_key(&first, "ids"); … sample.ids._buffer[i]   // here
//! ```
//!
//! ## Why this is not a self-oracle
//!
//! The same reason `fill.rs` is not. The statements read the *vendor's*
//! structure, which the *vendor's* deserialiser filled from the bytes. A
//! printer that read the wrong field shows up as a disagreement between
//! the implementations on that case, never as a false agreement.
//!
//! ## Why the JSON is built at run time and not baked in
//!
//! A sequence's length, a union's selected member and an `@optional`
//! member's presence are all decided by the bytes, not by the type. So
//! the emitted C carries the punctuation as statements — `zdj_key` puts
//! the comma in only when something has already been printed at that
//! level — rather than as one `printf` format string per aggregate.
//! `tools/wireprobe/dumpjson.h` holds the eleven helpers this uses.
//!
//! ## What has no C shape to print from
//!
//! The same three `fill.rs` refuses, for the same reason: XTypes 1.3
//! §7.5.1 defines no C binding for a bitset, a map or a fixed-point
//! type, and the IDL-to-C mapping it reuses has none of them either.
//! Guessing a field shape would compile against one vendor's header and
//! misread the other's.

use crate::contract;
use crate::encode::ProbeError;
use crate::fill::{Vendor, c_name, identifier};
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// The statements, ready to `#include` into a probe template.
#[derive(Debug, Default)]
struct Dump {
    body: String,
    depth: usize,
    next: usize,
    /// The aggregates on a cycle, each of which gets a function of its
    /// own — `zd_rec_dump_<position>` — instead of an inline expansion,
    /// which for a cycle has no end. See
    /// [`crate::recursion::recursive_aggregates`].
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
        let escaped = literal.replace('\\', "\\\\").replace('"', "\\\"");
        self.line(&format!("zdj_raw(\"{escaped}\");"));
    }

    /// A distinct name, so two nested loops do not shadow each other in
    /// a way that reads as working and iterates the wrong collection.
    ///
    /// `dump_cpp.rs` had this from the start and this file did not.
    /// `sequence<sequence<long>>` emitted `zd_i` at both levels, so the
    /// inner loop's declaration shadowed the outer one and the element
    /// expression — which still names the *outer* sequence — indexed it
    /// with the inner counter. `sequence-nested` is a corpus case; in C
    /// the out-of-range read is undefined behaviour rather than a
    /// diagnostic, which is why nothing had caught it. Found through the
    /// CORBA column, where omniORB's own sequence raised `BAD_PARAM`
    /// instead of reading past the end.
    fn temporary(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("zd_{stem}_{}", self.next)
    }
}

/// Emit the C statements that print `sample` as canonical JSON.
///
/// # Errors
/// [`ProbeError`], naming the path in the type.
pub fn dump(ast: &Ast, type_: Id<TypeNode>, vendor: Vendor) -> Result<String, ProbeError> {
    // Before `recursive_aggregates`, which follows a typedef to find
    // the aggregate below it and would itself not end on a chain that
    // returns to itself. See `contract::reject_alias_cycle`.
    contract::reject_alias_cycle(ast)?;
    let mut out = Dump {
        rec: crate::recursion::recursive_aggregates(ast, type_),
        ..Dump::default()
    };
    if !out.rec.is_empty() {
        // `dump.inc` lands inside `main`, where C allows no function
        // definition — except as a GNU nested function, which both
        // cell compilers accept (the cells build with gcc in its gnu
        // dialect). A mutual cycle needs the calls before the
        // definitions, which is what the `auto` forward declarations
        // are for; none of the functions has its address taken, so no
        // trampoline and no executable stack is involved. The bodies
        // are the same expansion the inline arms write, with every
        // aggregate on a cycle turned into a call.
        for position in 0..out.rec.len() {
            let name = c_name(ast, out.rec[position], vendor);
            out.line(&format!(
                "auto void zd_rec_dump_{position}(const {name} *zdv);"
            ));
        }
        for position in 0..out.rec.len() {
            let id = out.rec[position];
            let name = c_name(ast, id, vendor);
            out.open(&format!(
                "void zd_rec_dump_{position}(const {name} *zdv) {{"
            ));
            emit_rec_body(ast, id, vendor, &mut out, &format!("<{name}>"))?;
            out.close("}");
        }
    }
    emit(ast, type_, vendor, "sample", &mut out, "")?;
    out.line("zdj_raw(\"\\n\");");
    Ok(out.body)
}

/// The body of one cycle function: the aggregate's own expansion, with
/// the value behind the parameter.
fn emit_rec_body(
    ast: &Ast,
    type_: Id<TypeNode>,
    vendor: Vendor,
    out: &mut Dump,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            emit_aggregate(ast, type_, vendor, "(*zdv)", out, path)
        }
        TypeKind::Union { .. } => emit_union(ast, type_, vendor, "(*zdv)", out, path),
        _ => Err(ProbeError::NoLayout {
            detail: format!("{path}: a type on a cycle is not an aggregate"),
        }),
    }
}

/// zerodds-lint: recursion-depth 64 — the type graph's own nesting. There
/// is no sample here to end the walk, so the cycle is cut instead: an
/// aggregate named by `crate::recursion::recursive_aggregates` becomes a
/// call to the function emitted for it and is a leaf of this walk, which
/// therefore descends an acyclic graph. The typedef arm is finite because
/// `contract::reject_alias_cycle` at the entry point has refused a chain
/// that returns to itself. The corpus's deepest nesting is four.
#[allow(clippy::too_many_lines)]
fn emit(
    ast: &Ast,
    type_: Id<TypeNode>,
    vendor: Vendor,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        // The read direction of the same refusal `fill.rs` makes: no
        // generated structure in Cyclone, sixteen opaque octets in RTI.
        TypeKind::Primitive(PrimitiveKind::Float128) => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a long double, which neither C vendor gives this harness a value for",
        }),
        TypeKind::Primitive(kind) => {
            out.line(&primitive_call(*kind, expr));
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => {
            out.line(&format!("zdj_string({expr});"));
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => {
            // The element type is the vendor's, and §7.5.1.1.1 leaves the
            // C spelling of `Char16` to it, so the units are copied out
            // through `__typeof__` rather than named. The terminator is
            // the only thing marking the end: a `Char16 *` field carries
            // no length beside it.
            out.open("{");
            out.line("unsigned short zd_units[ZDJ_WSTRING_MAX];");
            out.line("size_t zd_count = 0;");
            out.line(&format!(
                "while ({expr} != NULL && {expr}[zd_count] != 0 && zd_count < ZDJ_WSTRING_MAX) {{"
            ));
            out.line(&format!(
                "    zd_units[zd_count] = (unsigned short){expr}[zd_count]; zd_count++;"
            ));
            out.line("}");
            out.line("zdj_wstring(zd_units, zd_count);");
            out.close("}");
            Ok(())
        }
        TypeKind::Enumeration { .. } => {
            emit_enumeration(ast, type_, vendor, expr, out);
            Ok(())
        }
        TypeKind::Bitmask { .. } => {
            emit_bitmask(ast, type_, vendor, expr, out);
            Ok(())
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            emit(ast, target, vendor, expr, out, path)
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } | TypeKind::Union { .. } => {
            // A type on a cycle is a call, never an inline expansion —
            // the expansion of a cycle does not end.
            if let Some(position) = out.rec.iter().position(|id| *id == type_) {
                out.line(&format!("zd_rec_dump_{position}(&({expr}));"));
                return Ok(());
            }
            if matches!(&node.kind, TypeKind::Union { .. }) {
                emit_union(ast, type_, vendor, expr, out, path)
            } else {
                emit_aggregate(ast, type_, vendor, expr, out, path)
            }
        }
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = contract::array_dimensions(ast, dims, path)?;
            emit_array(ast, element, vendor, expr, out, path, &counts)
        }
        TypeKind::Sequence { element, .. } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the sequence element is unresolved"),
            })?;
            emit_sequence(ast, element, vendor, expr, out, path)
        }
        TypeKind::Bitset { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a bitset, for which XTypes 1.3 §7.5.1 defines no C binding",
        }),
        TypeKind::Map { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a map, which the IDL-to-C mapping §7.5.1 reuses does not have",
        }),
        TypeKind::Fixed { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a fixed, which the IDL-to-C mapping §7.5.1 reuses does not have",
        }),
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// A structure or a valuetype, as a JSON object in declaration order.
///
/// `all_members_of`, so a base's members come out under their own names
/// and in front of the derived type's — XTypes 1.3 §7.4.1.1.5.1, and the
/// same walk the fill emitter had to be corrected to.
///
/// zerodds-lint: recursion-depth 64 — the type graph's own nesting, below
/// `emit`, which carries the argument: every member goes back through it,
/// and an aggregate on a cycle is a call there rather than an expansion
/// here.
fn emit_aggregate(
    ast: &Ast,
    type_: Id<TypeNode>,
    vendor: Vendor,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), ProbeError> {
    out.open("{");
    out.line("int zd_first = 1;");
    out.raw("{");
    for (member, member_expr, external) in c_members(ast, type_, expr, vendor) {
        let member_type = member.type_;
        let name = member.name.clone();
        let member_path = contract::join(path, &name);
        // XTypes 1.3 §7.5.1.2.5 maps an `@external` member to a pointer
        // as well, and both C generators do — `@external long a` is
        // `int32_t *a`. It is always present, so the key is always
        // printed; a null pointer is a sample the vendor left unset and
        // is said so rather than dereferenced.
        if external && !member.optional {
            out.line(&format!("zdj_key(&zd_first, \"{name}\");"));
            out.open(&format!("if ({member_expr} == NULL) {{"));
            out.raw("null");
            out.close("} else {");
            out.depth += 1;
            // Cyclone's @external string is the plain `char *` itself;
            // only rtiddsgen adds the second pointer (`DDS_Char **`),
            // so only there the value sits behind a dereference.
            let value_expr =
                if crate::fill::is_string_type(ast, member_type) && vendor == Vendor::Cyclone {
                    member_expr.clone()
                } else {
                    format!("(*{member_expr})")
                };
            emit(ast, member_type, vendor, &value_expr, out, &member_path)?;
            out.close("}");
            continue;
        }
        // XTypes 1.3 §7.5.1.2.4 maps an `@optional` member to a pointer
        // in C, so absence is a null pointer and the key is left out —
        // never written as `null`, which the canonical form refuses.
        if member.optional {
            out.open(&format!("if ({member_expr} != NULL) {{"));
            out.line(&format!("zdj_key(&zd_first, \"{name}\");"));
            // A string member is its own pointer — both generators
            // declare `char *text` for `@optional string`, NULL being
            // the absence — so it reads without a second dereference.
            // (An @external string under rtiddsgen is `DDS_Char **`
            // and takes the dereference; that path is the external arm
            // above, not this one.)
            let value_expr = if crate::fill::is_string_type(ast, member_type) {
                member_expr.clone()
            } else {
                format!("(*{member_expr})")
            };
            emit(ast, member_type, vendor, &value_expr, out, &member_path)?;
            out.close("}");
            continue;
        }
        out.line(&format!("zdj_key(&zd_first, \"{name}\");"));
        emit(ast, member_type, vendor, &member_expr, out, &member_path)?;
    }
    out.raw("}");
    out.close("}");
    Ok(())
}

/// Every member of an aggregate, with the C expression that reads it.
///
/// The wire order and the C shape part company here, and both matter. On
/// the wire a base's members lead and the run is flat — XTypes 1.3
/// §7.4.1.1.5.1, "as if all of the members had been defined, in the same
/// order, in the most-derived type". In C they are not flat: both
/// generators nest the base in a member called `parent`, so
/// `struct Sample : Base` reaches `base_a` as `sample.parent.base_a`.
/// Measured on 2026-08-09 against `idlc` and `rtiddsgen -language C`,
/// which spell it identically.
///
/// So the list is the wire order and the expressions are the C shape.
/// A printer that used `contract::members` directly gets the order right
/// and the expression wrong, and the probe does not compile — which is
/// where `fill.rs` stands and why the three `struct-inheritance` cases
/// have no C column in `vendor_bytes.md`.
///
/// zerodds-lint: recursion-depth 64 — one frame per base,
/// `zerodds_idl_lower::complete::MAX_INHERITANCE_DEPTH`, which is also
/// what `Ast::all_members_of` walks (`crates/ast/src/query.rs:147`). An
/// inheritance cycle is invalid IDL and the frontend reports it; a base
/// chain in a model that passed `model_from_idl` is finite. IDL 4.2 has no
/// forward declaration a typedef could use, and a base is named before it
/// is inherited from.
pub(crate) fn c_members(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    vendor: Vendor,
) -> Vec<(contract::Member, String, bool)> {
    let mut out = Vec::new();
    if let Ok(node) = ast.type_(type_) {
        let bases: Vec<Id<TypeNode>> = match &node.kind {
            TypeKind::Structure { base, .. } => base
                .as_ref()
                .and_then(zerodds_ast_core::Ref::resolved)
                .into_iter()
                .collect(),
            TypeKind::Valuetype { bases, .. } => bases
                .iter()
                .filter_map(zerodds_ast_core::Ref::resolved)
                .collect(),
            _ => Vec::new(),
        };
        for base in bases {
            out.extend(c_members(ast, base, &format!("{expr}.parent"), vendor));
        }
    }
    for id in ast.members_of(type_) {
        let Ok(member) = ast.member(*id) else {
            continue;
        };
        let Some(member_type) = member.type_.resolved() else {
            continue;
        };
        let name = member.name.clone();
        // The JSON key above keeps the IDL spelling; the field access
        // takes the vendor's, which for RTI is `_c_`-prefixed where the
        // name is a C keyword. `fill::identifier` carries the
        // measurement, and this is the reading half of it: a decode
        // probe over a keyword-named member did not compile either.
        let read = format!("{expr}.{}", identifier(vendor, &name));
        let external = member
            .facets
            .xtypes
            .as_ref()
            .is_some_and(|facet| facet.is_external);
        out.push((
            contract::Member {
                node: member.node_id,
                name,
                type_: member_type,
                optional: member.facets.is_optional(),
            },
            read,
            external,
        ));
    }
    out
}

/// A union, as `{"_d": …}` plus the member the discriminator selects.
///
/// The selection is a `switch` on the discriminator, with the labels the
/// IDL declares and a `default:` only where the union has one. That is
/// the same rule `decode.rs` follows, and a union whose discriminator
/// matches nothing prints the discriminator alone.
///
/// zerodds-lint: recursion-depth 64 — the type graph's own nesting, below
/// `emit`, which carries the argument: every case member and the
/// discriminator go back through it.
fn emit_union(
    ast: &Ast,
    type_: Id<TypeNode>,
    vendor: Vendor,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    let TypeKind::Union { discriminator, .. } = &node.kind else {
        return Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a union whose kind changed under the emitter",
        });
    };
    let discriminator = discriminator
        .resolved()
        .ok_or_else(|| ProbeError::NoLayout {
            detail: format!("{path}: the discriminator is unresolved"),
        })?;

    out.open("{");
    out.line("int zd_first = 1;");
    out.raw("{");
    out.line("zdj_key(&zd_first, \"_d\");");
    emit(
        ast,
        discriminator,
        vendor,
        &format!("{expr}._d"),
        out,
        &format!("{path}._d"),
    )?;
    // The width is the discriminator's, not always `long long`:
    // `union Choice switch (unsigned long long) { case 16045690984833335998: … }`
    // is a corpus case whose label does not fit a signed 64-bit
    // constant, and a `switch` cast to `long long` would compare it
    // wrongly even where the constant is accepted. `discriminator_c_width`
    // answers both halves at once.
    //
    // No [`zerodds_ast_layout::UnionLabelMatch`] here, and that is an
    // answer rather than an omission: what this file emits is a
    // **vendor's** probe, and a vendor's probe has to print what that
    // vendor's own codec put in the storage. A setting would make the
    // peer reproduce a setting instead of being measured. The zerodds C
    // column has its own emitter,
    // `tools/wireprobe/wireprobe-zerodds-c-gen/src/dump.rs`, and that
    // one does ask.
    let (width, suffix) = contract::discriminator_c_width(ast, discriminator);
    out.open(&format!("switch (({width})({expr}._d)) {{"));
    for id in ast.cases_of(type_) {
        let Ok(case) = ast.case(*id) else { continue };
        let Ok(member) = ast.member(case.member) else {
            continue;
        };
        let Some(member_type) = member.type_.resolved() else {
            continue;
        };
        let mut labels = Vec::with_capacity(case.labels.len());
        for label in &case.labels {
            match label.expr() {
                Some(expr) => match ast.eval_integral(expr) {
                    Ok(value) => labels.push(format!("case {value}{suffix}:")),
                    // A label that does not evaluate would silently drop
                    // its case out of the switch, and the member would
                    // never be printed for that discriminator.
                    Err(error) => {
                        return Err(ProbeError::NoLayout {
                            detail: format!("{path}: a case label does not evaluate: {error}"),
                        });
                    }
                },
                None => labels.push(String::from("default:")),
            }
        }
        for label in labels {
            out.line(&label);
        }
        out.depth += 1;
        out.line(&format!("zdj_key(&zd_first, \"{}\");", member.name));
        emit(
            ast,
            member_type,
            vendor,
            &format!("{expr}._u.{}", identifier(vendor, &member.name)),
            out,
            &format!("{path}.{}", member.name),
        )?;
        out.line("break;");
        out.depth = out.depth.saturating_sub(1);
    }
    out.close("}");
    out.raw("}");
    out.close("}");
    Ok(())
}

/// zerodds-lint: recursion-depth 8 — one frame per array dimension.
/// `counts` drops its head at each frame and the innermost element goes to
/// `emit`.
fn emit_array(
    ast: &Ast,
    element: Id<TypeNode>,
    vendor: Vendor,
    expr: &str,
    out: &mut Dump,
    path: &str,
    counts: &[usize],
) -> Result<(), ProbeError> {
    let Some((&count, rest)) = counts.split_first() else {
        return emit(ast, element, vendor, expr, out, path);
    };
    out.raw("[");
    for index in 0..count {
        if index > 0 {
            out.raw(",");
        }
        emit_array(
            ast,
            element,
            vendor,
            &format!("{expr}[{index}]"),
            out,
            &format!("{path}[{index}]"),
            rest,
        )?;
    }
    out.raw("]");
    Ok(())
}

/// A sequence, read out of whichever shape the vendor's code has.
///
/// The length is the bytes' and not the type's, so this is the one
/// construct whose JSON cannot be written straight through: the element
/// statements go inside a `for`.
///
/// zerodds-lint: recursion-depth 64 — the type graph's own nesting, below
/// `emit`, which carries the argument: the element goes back through it.
fn emit_sequence(
    ast: &Ast,
    element: Id<TypeNode>,
    vendor: Vendor,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), ProbeError> {
    // A `wstring` element needs no name here at all: the printer walks
    // the units off whatever `expr` turns out to be, through
    // `__typeof__` and a terminator, exactly as it does for a scalar
    // `wstring` field. `fill.rs` refused the write side and this
    // refused with it, so the two directions covered the same cases;
    // both are lifted together. AL-12.
    let count = out.temporary("n");
    let index = out.temporary("i");
    let (length, item) = match vendor {
        Vendor::Cyclone => (
            format!("(size_t){expr}._length"),
            format!("{expr}._buffer[{index}]"),
        ),
        Vendor::Rti => {
            let seq = rti_sequence_name(ast, element, vendor);
            (
                format!("(size_t){seq}_get_length(&{expr})"),
                format!("(*{seq}_get_reference(&{expr}, (int){index}))"),
            )
        }
    };
    out.open("{");
    out.line(&format!("size_t {count} = {length};"));
    out.raw("[");
    out.open(&format!(
        "for (size_t {index} = 0; {index} < {count}; {index}++) {{"
    ));
    out.line(&format!("if ({index} > 0) {{ zdj_raw(\",\"); }}"));
    emit(ast, element, vendor, &item, out, &format!("{path}[]"))?;
    out.close("}");
    out.raw("]");
    out.close("}");
    Ok(())
}

/// An enumeration, printed as the enumerator's name.
///
/// A `switch` over the declared enumerators and nothing else: the value
/// on the wire is the vendor's, and one that names no enumerator is a
/// finding. `zdj_enum_unknown` prints a string that no enumerator has,
/// so the comparison refuses it by name rather than printing a number
/// that would read as some other member's value.
fn emit_enumeration(ast: &Ast, type_: Id<TypeNode>, vendor: Vendor, expr: &str, out: &mut Dump) {
    // The same vendor split as the fill: Cyclone flattens the module
    // path onto the enumerator, rtiddsgen declares it bare.
    let prefix = match vendor {
        Vendor::Cyclone => module_prefix(ast, type_, vendor),
        Vendor::Rti => String::new(),
    };
    out.open(&format!("switch ((long long)({expr})) {{"));
    for id in ast.enumerators_of(type_) {
        let Ok(node) = ast.enumerator(*id) else {
            continue;
        };
        out.line(&format!(
            "case (long long){prefix}{}:",
            identifier(vendor, &node.name)
        ));
        out.line(&format!("    zdj_raw(\"\\\"{}\\\"\"); break;", node.name));
    }
    out.line(&format!(
        "default: zdj_enum_unknown((long long)({expr})); break;"
    ));
    out.close("}");
}

/// A bitmask, printed as the set bits' names in declaration order.
///
/// The flags are named rather than shifted, for the reason `fill.rs`
/// gives: §7.5.1.2.2 declares them as enumerators with `@value(1 <<
/// position)`, so the position-to-bit mapping stays on the vendor's side
/// of the line in both directions.
fn emit_bitmask(ast: &Ast, type_: Id<TypeNode>, vendor: Vendor, expr: &str, out: &mut Dump) {
    let prefix = module_prefix(ast, type_, vendor);
    out.open("{");
    out.line("int zd_first = 1;");
    out.raw("[");
    for id in ast.bits_of(type_) {
        let Ok(node) = ast.bit_value(*id) else {
            continue;
        };
        out.open(&format!(
            "if (((unsigned long long)({expr}) & (unsigned long long){prefix}{}) != 0) {{",
            identifier(vendor, &node.name)
        ));
        out.line("if (!zd_first) { zdj_raw(\",\"); }");
        out.line("zd_first = 0;");
        out.line(&format!("zdj_raw(\"\\\"{}\\\"\");", node.name));
        out.close("}");
    }
    out.raw("]");
    out.close("}");
}

/// The call that prints one primitive.
///
/// The two 64-bit widths print as strings, the two floating widths print
/// with enough digits to round-trip — `%.9g` and `%.17g` — and the
/// normaliser reduces both to the shortest form that reads back the same
/// value, so the C side never has to reproduce Rust's float formatting.
fn primitive_call(kind: PrimitiveKind, expr: &str) -> String {
    match kind {
        PrimitiveKind::Boolean => format!("zdj_bool({expr});"),
        PrimitiveKind::Octet | PrimitiveKind::UInt8 | PrimitiveKind::UInt16 => {
            format!("zdj_u64((unsigned long long)({expr}));")
        }
        PrimitiveKind::Int8 | PrimitiveKind::Int16 | PrimitiveKind::Int32 => {
            format!("zdj_i64((long long)({expr}));")
        }
        PrimitiveKind::UInt32 => format!("zdj_u64((unsigned long long)({expr}));"),
        PrimitiveKind::Char => format!("zdj_char((char)({expr}));"),
        PrimitiveKind::WChar => format!("zdj_wchar((unsigned long)({expr}));"),
        PrimitiveKind::Int64 => format!("zdj_i64_string((long long)({expr}));"),
        PrimitiveKind::UInt64 => format!("zdj_u64_string((unsigned long long)({expr}));"),
        PrimitiveKind::Float32 => format!("zdj_f32((float)({expr}));"),
        PrimitiveKind::Float64 | PrimitiveKind::Float128 => {
            format!("zdj_f64((double)({expr}));")
        }
    }
}

/// The `<T>Seq` prefix RTI generates for a sequence of this element.
///
/// A second copy of `fill.rs`'s rule rather than a call into it: the two
/// emitters are compared against each other through the vendor, and a
/// shared helper would make them agree about a mistake.
///
/// zerodds-lint: recursion-depth 64 — one frame per typedef hop,
/// `contract::MAX_ALIAS_HOPS`. The typedef arm is the only recursive one
/// and it stops at a collection rather than resolving past it; a chain
/// that returns to itself has been refused by
/// `contract::reject_alias_cycle` at the entry of every walk that reaches
/// here.
fn rti_sequence_name(ast: &Ast, element: Id<TypeNode>, vendor: Vendor) -> String {
    let Ok(node) = ast.type_(element) else {
        return String::from("DDS_LongSeq");
    };
    match &node.kind {
        // int8/uint8 keep their own sequence names (DDS_UInt8Seq /
        // DDS_Int8Seq) — the same measurement as the fill side.
        TypeKind::Primitive(PrimitiveKind::UInt8) => String::from("DDS_UInt8Seq"),
        TypeKind::Primitive(PrimitiveKind::Int8) => String::from("DDS_Int8Seq"),
        TypeKind::Primitive(kind) => format!("DDS_{}Seq", rti_primitive(*kind)),
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => String::from("DDS_WstringSeq"),
        TypeKind::String { .. } => String::from("DDS_StringSeq"),
        // A typedef of a collection keeps its own name (`TripleSeq`),
        // as on the fill side.
        TypeKind::Alias { aliased } => match aliased.resolved() {
            Some(target) => {
                let collection = matches!(
                    ast.type_(target).map(|node| node.kind.clone()),
                    Ok(TypeKind::Sequence { .. } | TypeKind::Array { .. })
                );
                if collection {
                    format!("{}Seq", c_name(ast, element, vendor))
                } else {
                    rti_sequence_name(ast, target, vendor)
                }
            }
            None => String::from("DDS_LongSeq"),
        },
        _ => format!("{}Seq", c_name(ast, element, vendor)),
    }
}

const fn rti_primitive(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "Boolean",
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => "Octet",
        PrimitiveKind::Int8 | PrimitiveKind::Char => "Char",
        PrimitiveKind::WChar => "Wchar",
        PrimitiveKind::Int16 => "Short",
        PrimitiveKind::UInt16 => "UnsignedShort",
        PrimitiveKind::Int32 => "Long",
        PrimitiveKind::UInt32 => "UnsignedLong",
        PrimitiveKind::Int64 => "LongLong",
        PrimitiveKind::UInt64 => "UnsignedLongLong",
        PrimitiveKind::Float32 => "Float",
        PrimitiveKind::Float64 | PrimitiveKind::Float128 => "Double",
    }
}

/// The flattened module path an enumerator carries, with a trailing
/// underscore when there is one.
fn module_prefix(ast: &Ast, type_: Id<TypeNode>, vendor: Vendor) -> String {
    let scoped = match ast.any_of_type(type_) {
        Ok(any) => ast.scoped_name_of(any),
        Err(_) => None,
    };
    let Some(scoped) = scoped else {
        return String::new();
    };
    let segments = scoped.segments();
    if segments.len() <= 1 {
        return String::new();
    }
    let mut prefix = segments[..segments.len() - 1]
        .iter()
        .map(|segment| identifier(vendor, segment).into_owned())
        .collect::<Vec<String>>()
        .join("_");
    prefix.push('_');
    prefix
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::encode::model_from_idl;

    fn emitted(idl: &str, type_name: &str, vendor: Vendor) -> String {
        let ast = model_from_idl(idl, "dump.idl").expect("the fixture parses");
        let type_ = ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
                type_name.to_string(),
            ]))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .expect("the type is in the model");
        dump(&ast, type_, vendor).expect("the type has a C shape")
    }

    #[test]
    fn a_structure_prints_its_members_in_declaration_order() {
        let c = emitted(
            "@final struct S { long a; double b; };",
            "S",
            Vendor::Cyclone,
        );
        let a = c.find("\\\"a\\\"").or_else(|| c.find("\"a\""));
        let b = c.find("\\\"b\\\"").or_else(|| c.find("\"b\""));
        assert!(a < b, "{c}");
        assert!(c.contains("zdj_i64((long long)(sample.a));"), "{c}");
        assert!(c.contains("zdj_f64((double)(sample.b));"), "{c}");
    }

    #[test]
    fn an_inherited_member_is_printed_and_not_skipped() {
        let c = emitted(
            "@final struct Base { long base_a; };
             @final struct S : Base { long derived_a; };",
            "S",
            Vendor::Cyclone,
        );
        assert!(c.contains("base_a"), "the base's member is missing: {c}");
        assert!(c.contains("derived_a"), "{c}");
    }

    #[test]
    fn a_sixty_four_bit_member_prints_as_a_string() {
        let c = emitted(
            "@final struct S { long long big; unsigned long long huge; };",
            "S",
            Vendor::Cyclone,
        );
        assert!(c.contains("zdj_i64_string"), "{c}");
        assert!(c.contains("zdj_u64_string"), "{c}");
    }

    #[test]
    fn the_two_vendors_reach_a_sequence_element_differently() {
        let idl = "@final struct S { sequence<long> ids; };";
        let cyclone = emitted(idl, "S", Vendor::Cyclone);
        let rti = emitted(idl, "S", Vendor::Rti);
        assert!(cyclone.contains("sample.ids._buffer[zd_i_2]"), "{cyclone}");
        assert!(rti.contains("DDS_LongSeq_get_reference"), "{rti}");
    }

    #[test]
    fn an_optional_member_is_left_out_rather_than_printed_as_null() {
        let c = emitted(
            "@mutable struct S { long a; @optional long b; };",
            "S",
            Vendor::Cyclone,
        );
        assert!(c.contains("if (sample.b != NULL)"), "{c}");
        assert!(
            !c.contains("null"),
            "an absent member is a missing key: {c}"
        );
    }

    #[test]
    fn a_union_switches_on_every_label_the_case_declares() {
        let c = emitted(
            "union Choice switch (long) {
                 case 1: case 2: long whole;
                 default: double fraction;
             };
             @final struct S { Choice picked; };",
            "S",
            Vendor::Cyclone,
        );
        assert!(c.contains("case 1LL:"), "{c}");
        assert!(c.contains("case 2LL:"), "{c}");
        assert!(c.contains("default:"), "{c}");
        assert!(c.contains("sample.picked._u.whole"), "{c}");
    }

    #[test]
    fn the_three_constructs_with_no_c_binding_are_refused_by_name() {
        let ast = model_from_idl(
            "@final struct WithMap { map<long, long> table; };
             @final struct WithFixed { fixed<7, 2> amount; };",
            "dump.idl",
        )
        .expect("the fixture parses");
        for name in ["WithMap", "WithFixed"] {
            let type_ = ast
                .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
                    name.to_string(),
                ]))
                .ok()
                .and_then(zerodds_ast::AnyId::as_type)
                .expect("the type is in the model");
            assert!(matches!(
                dump(&ast, type_, Vendor::Cyclone),
                Err(ProbeError::Unsupported { .. })
            ));
        }
    }
}

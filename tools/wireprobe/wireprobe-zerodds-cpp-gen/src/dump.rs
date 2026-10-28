// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The statements that print one filled C++ sample as canonical JSON.
//!
//! The counterpart of [`crate::fill`], which puts a JSON sample into the
//! generated C++. This takes one back out, after `runtime/cpp` and the
//! generated decoder have filled it from the bytes.
//!
//! ## The dialect is the shared one
//!
//! `crates/wire-probe/src/canonical.rs` defines what a value document
//! means and `crates/wire-probe/src/contract.rs` holds the rules all the
//! backends answer by. Two of them cannot be recovered by any reader and
//! so have to be right here:
//!
//! - a 64-bit integer is a **string**, because a JSON number is a double
//!   and loses the low bits past 2^53;
//! - an absent `@optional` member is a key that is **left out**, never a
//!   `null`, which the canonical form refuses.
//!
//! `tools/wireprobe/dumpjson.h` holds the helpers and is the same file
//! the C, Cyclone, RTI and Fast DDS probes print through, with
//! `dumpjson.hpp` adding the two a C++ binding needs. That sharing is
//! deliberate: the JSON dialect is a contract and not a wire rule, and
//! backends holding private copies of it is how they come to disagree
//! about what a sample is.
//!
//! ## Where this differs from the C column's dumper
//!
//! Only in the memory shapes, which are `adapter/cpp.rs`'s:
//!
//! | | C | C++ |
//! |---|---|---|
//! | `string`, `wstring`, `fixed` | `const char *` | `std::string` |
//! | `sequence<T>` | `_length` and `_buffer` | `std::vector<T>`, `.size()` |
//! | `map<K,V>` | a struct with `key` and `value` | `std::pair`, `.first` and `.second` |
//! | `@optional T` | a parallel `_has_` flag | `std::optional<T>` |
//! | a union's branch | `v._u.x` | `v.zd_x` |
//! | an enumerator | `a_b_red` | `::a::b::Colour::red` |
//! | `long double` | `zdj_f128_string` | `zdj_f128_string_cpp` |
//!
//! `zdj_wstring_cpp` is **not** among them, although `dumpjson.hpp`
//! carries it: it exists for a binding whose `wstring` is a
//! `std::wstring` of code points, and this column's is a `std::string`
//! of UTF-8 for the reason `adapter/cpp.rs` gives — the §7.4.1.1.2
//! conversion belongs inside `runtime/cpp`, where it is a second reading
//! of the clause.

use std::fmt::Write as _;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_rules::adapter::cpp::cpp_type_path;
use zerodds_lang_rules::emit::CodecTarget;
use zerodds_lang_rules::emit::cpp::Cpp;

/// The statements, ready to paste into the probe's `main`.
#[derive(Debug, Default)]
struct Dump {
    body: String,
    depth: usize,
    /// The types this descent is inside, innermost last.
    ///
    /// The printer walks the **type** and not the value, so a type that
    /// reaches itself has no bottom: `struct Node { sequence<Node> }` is
    /// finite in every sample and infinite as a shape. The C column's
    /// dumper carries the same guard, and its comment records what
    /// happened before it had one — the generator's own stack went, and
    /// took the whole gate with it rather than reporting one case.
    active: Vec<Id<TypeNode>>,
    /// The aggregates on a cycle, each of which gets a file-scope
    /// `static` function of its own — `zd_rec_dump_<n>` — instead of
    /// an inline expansion, which for a cycle has no end. The same
    /// shape the other emitters grew (`zerodds_wire_probe::recursion`).
    rec: Vec<Id<TypeNode>>,
    /// A counter, so two loops in one sample get distinct indices.
    next: usize,
    /// Whether a negative case label of a narrow signed discriminator
    /// selects its branch.
    ///
    /// It has to be here at all because the **selection** is the codec's.
    /// A generated decoder that falls through to `default:` and a dump
    /// that switches into `case -1:` print a member out of storage the
    /// decoder never wrote.
    union_label: UnionLabelMatch,
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

    fn temporary(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("zd_{stem}_{}", self.next)
    }

    /// The `zd_first` latch, declared only where something will read it.
    ///
    /// An aggregate with no members, a bitset with only anonymous
    /// padding and a bitmask with no bits each print an empty JSON value
    /// and never call `zdj_key`, and a declaration nothing reads is a
    /// `-Wunused-variable` error under the `-Werror` the probe is
    /// compiled with.
    ///
    /// The C column's dumper declares it unconditionally and does not
    /// meet the shape: its materialiser refuses `struct S {}` outright,
    /// because C23 §6.7.2.1p8 leaves a structure with no named member
    /// undefined. C++ declares one, so `empty-struct` reaches here.
    fn first_flag(&mut self, read: bool) {
        if read {
            self.line("int zd_first = 1;");
        }
    }
}

/// Emit the statements that print `zd_sample` as canonical JSON.
///
/// # Errors
/// A message naming the path in the type, in the same shape the fill
/// side reports one.
/// The two halves of the printer: file-scope functions for the types
/// on a cycle, and the statements for the read function's body.
#[derive(Debug, Default)]
pub struct DumpParts {
    /// `static void zd_rec_dump_<n>(const T &zdv)` definitions, for
    /// file scope ahead of the function holding [`Self::body`].
    pub functions: String,
    /// The statements that print `zd_sample` as canonical JSON.
    pub body: String,
}

pub fn dump(
    ast: &Ast,
    type_: Id<TypeNode>,
    union_label: UnionLabelMatch,
) -> Result<DumpParts, String> {
    // The probe reads the fields of the native structures: the wire column asks for `native`
    // (the default of the library is the DDS-PSM-Cxx mapping).
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let mut out = Dump {
        union_label,
        rec: zerodds_wire_probe::recursion::recursive_aggregates(ast, type_),
        ..Dump::default()
    };
    let functions = if out.rec.is_empty() {
        String::new()
    } else {
        // Forward declarations first: a mutual cycle calls before it
        // defines. File scope, because this generator owns the whole
        // translation unit.
        let spelled = |id| {
            cpp_type_path(ast, id)
                .segments()
                .iter()
                .map(|segment| segment.as_str().to_string())
                .collect::<Vec<_>>()
                .join("::")
        };
        for position in 0..out.rec.len() {
            let name = spelled(out.rec[position]);
            out.line(&format!(
                "static void zd_rec_dump_{position}(const {name} &zdv);"
            ));
        }
        for position in 0..out.rec.len() {
            let id = out.rec[position];
            let name = spelled(id);
            out.open(&format!(
                "static void zd_rec_dump_{position}(const {name} &zdv) {{"
            ));
            out.active.push(id);
            let outcome = emit_inner(ast, id, "zdv", &mut out, &format!("<{name}>"));
            out.active.pop();
            outcome?;
            out.close("}");
        }
        std::mem::take(&mut out.body)
    };
    emit(ast, type_, "zd_sample", &mut out, "sample")?;
    out.line("zdj_raw(\"\\n\");");
    Ok(DumpParts {
        functions,
        body: out.body,
    })
}

/// Emit the statements that print `expr` under `type_`.
///
/// zerodds-lint: recursion-depth = distinct types on the descent path,
/// via [`emit_inner`]. A type on a cycle is a call into its file-scope
/// function — `recursive_aggregates` names them and `Dump::rec` holds
/// them — so a cycle is never unrolled; a type the collector missed is
/// caught by `Dump::active` and refused by name rather than descended
/// into. That is the guard the CORBA C++ emitter did not have when
/// `A -> sequence<B> -> B -> sequence<A>` ran the generator's stack out.
fn emit(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let node = ast
        .type_(type_)
        .map_err(|error| format!("{path}: {error}"))?;
    // A type on a cycle is a call into its file-scope function, never
    // an inline expansion — the expansion of a cycle has no end.
    if let Some(position) = out.rec.iter().position(|id| *id == type_) {
        out.line(&format!("zd_rec_dump_{position}({expr});"));
        return Ok(());
    }
    if out.active.contains(&type_) {
        return Err(format!(
            "{path}: `{}` reaches itself outside the collected cycles — a defect \
             in the cycle collector, not in the IDL",
            node.name
        ));
    }
    out.active.push(type_);
    let outcome = emit_inner(ast, type_, expr, out, path);
    out.active.pop();
    outcome
}

/// The body of [`emit`], with `type_` already on the descent path.
///
/// zerodds-lint: recursion-depth = distinct types on the descent path
/// (every recursive call goes back through [`emit`], which pushes onto
/// `Dump::active` and refuses a repeat)
fn emit_inner(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let node = ast
        .type_(type_)
        .map_err(|error| format!("{path}: {error}"))?;
    match &node.kind {
        TypeKind::Primitive(kind) => {
            out.line(&primitive_call(*kind, expr));
            Ok(())
        }
        // Both string widths are a `std::string` holding UTF-8, and JSON
        // carries UTF-8, so the bytes go straight through. `.c_str()`
        // because the helper is the C one and takes a `const char *`.
        TypeKind::String { width, .. } => {
            let _: &CharWidth = width;
            out.line(&format!("zdj_string({expr}.c_str());"));
            Ok(())
        }
        // A `fixed<P,S>` is the decimal it was written as, and the
        // canonical form is a string for the reason the type exists: a
        // double cannot carry one.
        TypeKind::Fixed { .. } => {
            out.line(&format!("zdj_string({expr}.c_str());"));
            Ok(())
        }
        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the typedef target is unresolved"))?;
            emit(ast, target, expr, out, path)
        }
        TypeKind::Enumeration { .. } => {
            emit_enumeration(ast, type_, expr, out);
            Ok(())
        }
        TypeKind::Bitmask { .. } => {
            emit_bitmask(ast, type_, expr, out);
            Ok(())
        }
        TypeKind::Bitset { .. } => {
            // The whole list, base first: a derived `bitset` carries the
            // base's bitfields and the decoder fills them, so printing
            // only the derived type's own would return a value with
            // members missing and say nothing about it.
            let fields = ast.all_bitfields_of(type_);
            out.open("{");
            out.first_flag(
                fields
                    .iter()
                    .any(|field| ast.bitfield(*field).is_ok_and(|node| node.name.is_some())),
            );
            out.raw("{");
            for field in &fields {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous field is padding and carries no value.
                let Some(name) = field_node.name.as_ref() else {
                    continue;
                };
                let field_name = zerodds_lang_cst::cpp::escape_keyword(name);
                out.line(&format!("zdj_key(&zd_first, \"{name}\");"));
                out.line(&format!(
                    "zdj_u64(static_cast<unsigned long long>({expr}.{field_name}));"
                ));
            }
            out.raw("}");
            out.close("}");
            Ok(())
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            emit_aggregate(ast, type_, expr, out, path)
        }
        TypeKind::Union { .. } => emit_union(ast, type_, expr, out, path),
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{path}: the array element is unresolved"))?;
            let counts: Result<Vec<i128>, String> = dims
                .iter()
                .map(|dim| {
                    ast.eval_integral(*dim)
                        .map_err(|_| format!("{path}: an array length does not evaluate"))
                })
                .collect();
            emit_array(ast, element, expr, out, path, &counts?)
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{path}: the sequence element is unresolved"))?;
            emit_sequence(ast, element, expr, out, path)
        }
        TypeKind::Map { key, value, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| format!("{path}: the map key type is unresolved"))?;
            let value_type = value
                .resolved()
                .ok_or_else(|| format!("{path}: the map value type is unresolved"))?;
            emit_map(ast, key_type, value_type, expr, out, path)
        }
        other => Err(format!(
            "{path}: {} has no C++ shape to print from",
            other.as_str()
        )),
    }
}

/// A structure or a valuetype, as a JSON object in wire order.
///
/// `all_members_of`, so a base's members come out under their own names
/// and in front of the derived type's — §7.4.1.1.5.1, and the same walk
/// the fill side makes. The C++ shape is flat here, unlike the vendors':
/// `adapter/cpp.rs` puts a base's members straight into the derived
/// class rather than inheriting, and says why.
fn emit_aggregate(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let members = ast.all_members_of(type_);
    out.open("{");
    out.first_flag(!members.is_empty());
    out.raw("{");
    for member in members {
        let Ok(member_node) = ast.member(member) else {
            continue;
        };
        let member_type = member_node
            .type_
            .resolved()
            .ok_or_else(|| format!("{path}.{}: the member type is unresolved", member_node.name))?;
        let name = member_node.name.clone();
        let field = zerodds_lang_cst::cpp::escape_keyword(&name);
        let member_path = format!("{path}.{name}");
        // An `@external` member is a `std::shared_ptr<T>` — DDS-XTypes
        // 1.3 Table 21, spelled in `crates/lang-rules/src/adapter/cpp.rs`
        // — so the value is one dereference away and the pointer, not a
        // `std::optional`, is what carries an absence.
        let external = !matches!(
            zerodds_lang_rules::adapter::cpp::held(ast, member_node),
            zerodds_lang_rules::adapter::cpp::Held::Value
        );
        if member_node.facets.is_optional() {
            // An absent member is a key that is not printed. `null`
            // would read as "present and empty" to one backend and as
            // "absent" to another, and the canonical form refuses it.
            //
            // `.value()` and not `*`: the two are the same inside a
            // branch the presence test guarded, and the first says which
            // one it is.
            let (test, value) = (
                format!("{expr}.{field} != nullptr"),
                format!("(*{expr}.{field})"),
            );
            out.open(&format!("if ({test}) {{"));
            out.line(&format!("zdj_key(&zd_first, \"{name}\");"));
            emit(ast, member_type, &value, out, &member_path)?;
            out.close("}");
            continue;
        }
        out.line(&format!("zdj_key(&zd_first, \"{name}\");"));
        let value = if external {
            format!("(*{expr}.{field})")
        } else {
            format!("{expr}.{field}")
        };
        emit(ast, member_type, &value, out, &member_path)?;
    }
    out.raw("}");
    out.close("}");
    Ok(())
}

/// A union, as `{"_d": …}` plus the member the discriminator selects.
///
/// An if-chain over the labels and not a `switch`, for the reason the
/// codec emitter gives: a `case 16045690984833335998:` over an
/// `unsigned long long` does not fit a signed label, and the comparison
/// has to be made in the discriminator's own type.
fn emit_union(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let node = ast
        .type_(type_)
        .map_err(|error| format!("{path}: {error}"))?;
    let TypeKind::Union { discriminator, .. } = &node.kind else {
        return Err(format!("{path}: not a union"));
    };
    let discriminator = discriminator
        .resolved()
        .ok_or_else(|| format!("{path}: the discriminator is unresolved"))?;

    out.open("{");
    out.line("int zd_first = 1;");
    out.raw("{");
    out.line("zdj_key(&zd_first, \"_d\");");
    emit(
        ast,
        discriminator,
        &format!("{expr}.zd_d"),
        out,
        &format!("{path}._d"),
    )?;

    // The one crossing [`UnionLabelMatch`] has, asked of the same
    // discriminator the generated decoder asked it of.
    let suppress_negative = union_negative_matches_nothing(ast, type_, out.union_label);
    // The label cast to the discriminator's own type, which is what
    // `emit/decode.rs` writes at the site that *selects* the branch:
    // `cast(value_type(discriminator), label(value))`. Written the same
    // way here rather than nearly the same way, because the two have to
    // agree about which arm is live — a dumper that compared in a wider
    // type and a decoder that compared in the declared one would print a
    // member out of storage the decoder never wrote, which is the shape
    // this suite exists to find. It is also the only form that compiles
    // for a scoped enumeration, which does not convert to an integer.
    let discriminator_type = zerodds_lang_render::cpp::type_(&Cpp::value_type(ast, discriminator));
    let mut arms: Vec<(String, String, Id<TypeNode>, bool)> = Vec::new();
    let mut fallback: Option<(String, Id<TypeNode>, bool)> = None;
    for case in ast.cases_of(type_) {
        let Ok(case_node) = ast.case(*case) else {
            continue;
        };
        let Ok(member) = ast.member(case_node.member) else {
            continue;
        };
        let member_type = member
            .type_
            .resolved()
            .ok_or_else(|| format!("{path}.{}: the case type is unresolved", member.name))?;
        let mut condition = String::new();
        let mut is_default = false;
        for label in &case_node.labels {
            match label.expr() {
                None => is_default = true,
                Some(value_expr) => {
                    let value = ast.eval_integral(value_expr).map_err(|error| {
                        format!("{path}: a case label does not evaluate: {error}")
                    })?;
                    // A label the setting says selects nothing gets no
                    // test, so the chain falls to `default:` where the
                    // union has one and prints only `_d` where it has
                    // none — which is what the generated decoder left in
                    // the storage.
                    if suppress_negative && value < 0 {
                        continue;
                    }
                    if !condition.is_empty() {
                        condition.push_str(" || ");
                    }
                    let _ = write!(
                        condition,
                        "{expr}.zd_d == static_cast<{discriminator_type}>({})",
                        cpp_label(value)
                    );
                }
            }
        }
        let external = is_external(ast, member);
        if is_default {
            fallback = Some((member.name.clone(), member_type, external));
        }
        if !condition.is_empty() {
            arms.push((condition, member.name.clone(), member_type, external));
        }
    }

    let mut opened = 0usize;
    for (index, (condition, member_name, member_type, external)) in arms.iter().enumerate() {
        let head = if index == 0 {
            format!("if ({condition}) {{")
        } else {
            format!("}} else if ({condition}) {{")
        };
        if index == 0 {
            out.open(&head);
        } else {
            out.close(&head);
            out.depth += 1;
        }
        opened += 1;
        let field = format!("zd_{}", zerodds_lang_cst::cpp::escape_keyword(member_name));
        out.line(&format!("zdj_key(&zd_first, \"{member_name}\");"));
        emit(
            ast,
            *member_type,
            &branch_value(expr, &field, *external),
            out,
            &format!("{path}.{member_name}"),
        )?;
    }
    if let Some((member_name, member_type, external)) = fallback {
        if opened == 0 {
            out.open("if (true) {");
        } else {
            out.close("} else {");
            out.depth += 1;
        }
        opened += 1;
        let field = format!("zd_{}", zerodds_lang_cst::cpp::escape_keyword(&member_name));
        out.line(&format!("zdj_key(&zd_first, \"{member_name}\");"));
        emit(
            ast,
            member_type,
            &branch_value(expr, &field, external),
            out,
            &format!("{path}.{member_name}"),
        )?;
    }
    if opened > 0 {
        out.close("}");
    }

    out.raw("}");
    out.close("}");
    Ok(())
}

/// An array, one dimension at a time.
///
/// Nested `for` loops rather than the indices written out: a
/// `long[8][8]` unrolled is a hundred and twenty-eight statements that
/// say the same thing.
/// The index comes from the accumulator's counter and not from the
/// nesting level. `level` numbered the dimensions of **one** array
/// declaration, so `Row grid[2]` with `typedef long Row[3]` opened two
/// loops both called `zd_a0`: the inner one overwrote the outer one's
/// index and the printer read `grid[i][i]`, which for
/// `[[1,2,3],[4,5,6]]` prints `[[1,5,0],[1,5,0]]` and reads one element
/// past the array. The reader under it was correct the whole time —
/// this printed a correct decode as a wrong one, which is the direction
/// of error a test instrument must not have. The counter is the same
/// one the sequence printer has always used.
///
/// zerodds-lint: recursion-depth = array dimensions (one level per
/// entry of `counts`, which the split shortens on every call; the
/// element then goes through [`emit`])
fn emit_array(
    ast: &Ast,
    element: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
    counts: &[i128],
) -> Result<(), String> {
    let Some((&count, rest)) = counts.split_first() else {
        return emit(ast, element, expr, out, path);
    };
    let index = out.temporary("a");
    out.open("{");
    out.raw("[");
    out.open(&format!(
        "for (std::size_t {index} = 0; {index} < {count}u; {index}++) {{"
    ));
    out.line(&format!("if ({index} > 0) {{ zdj_raw(\",\"); }}"));
    emit_array(
        ast,
        element,
        &format!("{expr}[{index}]"),
        out,
        &format!("{path}[]"),
        rest,
    )?;
    out.close("}");
    out.raw("]");
    out.close("}");
    Ok(())
}

/// A sequence: the length is the bytes' and not the type's, so the
/// element statements go inside a `for`. A `std::vector` knows its own
/// size, so there is no `_length` beside it to disagree with.
fn emit_sequence(
    ast: &Ast,
    element: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let index = out.temporary("s");
    out.open("{");
    out.raw("[");
    out.open(&format!(
        "for (std::size_t {index} = 0; {index} < {expr}.size(); {index}++) {{"
    ));
    out.line(&format!("if ({index} > 0) {{ zdj_raw(\",\"); }}"));
    emit(
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

/// A map, as the array of two-element arrays the dialect calls for.
///
/// A JSON object keys on a string and `map<long, float>` does not, so
/// the pair form is what §7.4.1.1.4's scalar key restriction allows.
/// The entry is a `std::pair`, so its halves are `first` and `second`.
fn emit_map(
    ast: &Ast,
    key_type: Id<TypeNode>,
    value_type: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), String> {
    let index = out.temporary("m");
    out.open("{");
    out.raw("[");
    out.open(&format!(
        "for (std::size_t {index} = 0; {index} < {expr}.size(); {index}++) {{"
    ));
    out.line(&format!("if ({index} > 0) {{ zdj_raw(\",\"); }}"));
    out.raw("[");
    emit(
        ast,
        key_type,
        &format!("{expr}[{index}].first"),
        out,
        &format!("{path}[].key"),
    )?;
    out.raw(",");
    emit(
        ast,
        value_type,
        &format!("{expr}[{index}].second"),
        out,
        &format!("{path}[].value"),
    )?;
    out.raw("]");
    out.close("}");
    out.raw("]");
    out.close("}");
    Ok(())
}

/// An enumeration, printed as the enumerator's name.
///
/// An if-chain over the declared constants: two enumerators may carry
/// the same `@value`, which a `switch` cannot hold at all, and the first
/// match is what the Rust reader also returns. A value that names no
/// enumerator prints a string no enumerator has, so the comparison
/// refuses it by name — a bare number here would read as some other
/// member's value and pass.
///
/// The unknown arm casts, because the enumeration is scoped and does not
/// convert on its own. Its *underlying* type is what `adapter/cpp.rs`
/// chose from the bound and from the values, and a `long long` holds
/// every one of them.
fn emit_enumeration(ast: &Ast, type_: Id<TypeNode>, expr: &str, out: &mut Dump) {
    let unknown = format!("zdj_enum_unknown(static_cast<long long>({expr}));");
    let mut first = true;
    for id in ast.enumerators_of(type_) {
        let Ok(node) = ast.enumerator(*id) else {
            continue;
        };
        let constant =
            zerodds_lang_rules::emit::cpp::enumerator_constant_name(ast, type_, &node.name);
        let head = format!("if ({expr} == {constant}) {{");
        if first {
            out.open(&head);
            first = false;
        } else {
            out.close(&format!("}} else {head}"));
            out.depth += 1;
        }
        out.line(&format!("zdj_raw(\"\\\"{}\\\"\");", node.name));
    }
    if first {
        out.line(&unknown);
        return;
    }
    out.close("} else {");
    out.depth += 1;
    out.line(&unknown);
    out.close("}");
}

/// A bitmask, printed as the set bits' names in declaration order.
///
/// The flags are named rather than shifted: §7.5.1.2.2 declares them as
/// constants with the position already applied, so the
/// position-to-bit mapping stays in the materialiser where the codec
/// also reads it.
///
/// The constants are free `inline constexpr` variables beside the alias
/// — `adapter/cpp.rs` says why a bitmask is not a scoped enumeration —
/// so they sit in the *module's* namespace and not in the type's. That
/// is the one name here `Cpp::enumerator_constant` does not answer for.
fn emit_bitmask(ast: &Ast, type_: Id<TypeNode>, expr: &str, out: &mut Dump) {
    out.open("{");
    out.first_flag(!ast.bits_of(type_).is_empty());
    out.raw("[");
    for id in ast.bits_of(type_) {
        let Ok(node) = ast.bit_value(*id) else {
            continue;
        };
        let constant = bit_constant(ast, type_, &node.name);
        out.open(&format!(
            "if ((static_cast<unsigned long long>({expr}) & \
             static_cast<unsigned long long>({constant})) != 0) {{"
        ));
        out.line("if (!zd_first) { zdj_raw(\",\"); }");
        out.line("zd_first = 0;");
        out.line(&format!("zdj_raw(\"\\\"{}\\\"\");", node.name));
        out.close("}");
    }
    out.raw("]");
    out.close("}");
}

/// `::a::b::flag_a` — the bit constant beside the bitmask's alias.
///
/// The bitmask's own path with its last segment replaced: the alias is
/// `::a::b::Flags` and the constants are its siblings, which is what
/// `adapter/cpp.rs` emits — both are pushed under the type's `module`.
fn bit_constant(ast: &Ast, type_: Id<TypeNode>, name: &str) -> String {
    let path = cpp_type_path(ast, type_);
    let mut segments: Vec<String> = path
        .segments()
        .iter()
        .map(|segment| segment.as_str().to_string())
        .collect();
    segments.pop();
    segments.push(zerodds_lang_cst::cpp::escape_keyword(name));
    format!("::{}", segments.join("::"))
}

/// The call that prints one primitive.
fn primitive_call(kind: PrimitiveKind, expr: &str) -> String {
    match kind {
        PrimitiveKind::Boolean => format!("zdj_bool({expr});"),
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::UInt16
        | PrimitiveKind::UInt32 => {
            format!("zdj_u64(static_cast<unsigned long long>({expr}));")
        }
        PrimitiveKind::Int8 | PrimitiveKind::Int16 | PrimitiveKind::Int32 => {
            format!("zdj_i64(static_cast<long long>({expr}));")
        }
        PrimitiveKind::Char => format!("zdj_char(static_cast<char>({expr}));"),
        PrimitiveKind::WChar => format!("zdj_wchar(static_cast<unsigned long>({expr}));"),
        // A JSON number is a double, so both 64-bit widths go out as
        // decimal in a string. The one thing here no reader can undo.
        PrimitiveKind::Int64 => format!("zdj_i64_string(static_cast<long long>({expr}));"),
        PrimitiveKind::UInt64 => {
            format!("zdj_u64_string(static_cast<unsigned long long>({expr}));")
        }
        PrimitiveKind::Float32 => format!("zdj_f32(static_cast<float>({expr}));"),
        PrimitiveKind::Float64 => format!("zdj_f64(static_cast<double>({expr}));"),
        // A string holding every digit the value has, which is the
        // canonical form for a `long double`. Printing it as a `double`
        // would report agreement for a value this column never carried.
        PrimitiveKind::Float128 => format!("zdj_f128_string_cpp({expr});"),
    }
}

/// A union label as a C++ literal wide enough to hold itself.
///
/// Not written out here: [`Cpp::wide_int`] is the rule, and the
/// generated codec puts its answer inside the same `static_cast`. A
/// second copy of "when does a label need a suffix" is a second chance
/// to part over one — the C column keeps its own copy in `dump.rs` and
/// that is a difference this file does not have to inherit.
fn cpp_label(value: i128) -> String {
    zerodds_lang_render::cpp::expr(&Cpp::wide_int(value))
}

/// A union branch's value, dereferenced where the branch is
/// `@external` and so a `std::shared_ptr<T>`.
fn branch_value(expr: &str, field: &str, external: bool) -> String {
    if external {
        format!("(*{expr}.{field})")
    } else {
        format!("{expr}.{field}")
    }
}

/// Whether the C++ column holds a member behind `std::shared_ptr<T>`:
/// it is `@external`, or it closes a type cycle.
///
/// The same question `crates/lang-rules/src/adapter/cpp.rs` asks before
/// it declares the member ([`zerodds_lang_rules::adapter::held_externally`]),
/// so the probe and the declaration cannot hold one member two ways: such a
/// member is a
/// `std::shared_ptr<T>` and the value is one dereference away.
fn is_external(ast: &zerodds_ast::Ast, member: &zerodds_ast::MemberNode) -> bool {
    zerodds_lang_rules::adapter::held_externally(ast, member)
}

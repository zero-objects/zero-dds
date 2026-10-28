// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The fill and the dump for the OMG IDL-to-C++ mapping.
//!
//! Two of the three ORBs generate against it — omniORB 4.3.3 and TAO
//! 2.5.24 — and they generate the *same* API, because CORBA 3.3 part 3
//! §1.2 is a specification and not a suggestion. One backend serves
//! both; only the marshalling call differs, and that lives in the
//! templates (`tools/wireprobe/omniorb-template.cpp`,
//! `tao-template.cpp`), never here.
//!
//! ```text
//! sample.x = 1;                      // struct member, array or sequence element
//! sample.picked.whole(1);            // union case member — §1.2.9's modifier
//! sample.picked.inner().x = 1;       // …whose accessor returns a reference
//! ```
//!
//! ## Why this is not a self-oracle
//!
//! The same reason `fill.rs` is not. The statements place values into
//! the *ORB's* structures and read them back out of them; the bytes come
//! from the ORB's own `cdrMemoryStream` or `TAO_OutputCDR`. A statement
//! that placed a value wrongly shows up as a disagreement between the
//! probes on that case, never as a false agreement.
//!
//! ## Where the two mappings part company, and where they do not
//!
//! | construct | C++ mapping | why it matters here |
//! |---|---|---|
//! | `string` | `String_member` / `String_Manager`, both `operator const char*` | assigned through `CORBA::string_dup` in both |
//! | `wstring` | `WString_member` over `CORBA::WChar` | `CORBA::WChar` is **two** bytes in omniORB and **four** in TAO, so the units are copied rather than named |
//! | `sequence<T>` | `length(n)` and `operator[]` | §1.2.11, identical in both |
//! | `T x[N]` | a plain C array | §1.2.10 |
//! | `union` | modifier and accessor per case, `_d()` for the discriminator | §1.2.9 |
//! | `fixed<P,S>` | `CORBA::Fixed`, `Fixed(const char*)` and `to_string()` | §1.14. **TAO refuses the IDL** — "fixed data types are not supported" — so only omniORB reaches this arm |
//! | `octet` | `CORBA::Octet`, an unsigned char | which is also where `int8` and `uint8` land; see `corba_idl.rs` |
//!
//! ## Identifiers the C++ front ends rename
//!
//! An IDL member called `new` is legal IDL and not a C++ declaration,
//! so both front ends rename it — `_cxx_new`, measured 2026-08-11 from
//! omniidl's and `tao_idl`'s output for
//! `tests/interop-corpus/identifier-target-keywords`. An IDL member
//! called `_long` is the keyword escaped (IDL 4.2 §7.2.3.2) and both
//! front ends strip the escape before renaming what remains. Every
//! identifier this file pastes into an identifier position therefore
//! goes through `cpp_id` first, and only those: the JSON keys the
//! dump prints and the paths in a [`ProbeError`] carry the name the
//! IDL wrote, because the wire and the matrix are keyed on that one.

use crate::contract::{self, Consumed};
use crate::corba_idl::corba_scoped_name;
use crate::encode::ProbeError;
use crate::target_keywords::cxx;
use serde_json::Value;
use std::borrow::Cow;
use std::fmt::Write as _;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// The C++ spelling of one IDL identifier, the escape stripped first.
///
/// IDL 4.2 §7.2.3.2: a leading underscore escapes a keyword and is not
/// part of the identifier, so `_long` names `long`. Both front ends
/// strip the escape and then apply their keyword rename — measured
/// 2026-08-22 from `tao_idl` 2.5.24 over `identifier-escaped-keyword`:
/// the generated struct carries `_cxx_long`, `_cxx_double`,
/// `_cxx_switch` where the IDL wrote `_long`, `_double`, `_switch`,
/// which is [`cxx`] applied to the bare name. The JSON keys and the
/// paths keep the IDL spelling, same as [`cxx`] itself.
fn cpp_id(name: &str) -> Cow<'_, str> {
    match name.strip_prefix('_') {
        Some(bare) => Cow::Owned(cxx(bare).into_owned()),
        None => cxx(name),
    }
}

/// Where one value goes, in the two shapes the C++ mapping offers.
enum Place {
    /// A plain lvalue: a structure member, an array element, a sequence
    /// element, or a union case member reached through its accessor.
    Lvalue(String),
    /// A union case member of a type whose accessor returns by value.
    /// §1.2.9 gives it a modifier instead: `object.name(value)`.
    Modifier {
        /// The expression naming the union.
        object: String,
        /// The case member's name, which is also the modifier's.
        name: String,
    },
}

impl Place {
    /// The statement that writes `literal` here.
    fn assign(&self, literal: &str) -> String {
        match self {
            Self::Lvalue(expression) => format!("{expression} = {literal};"),
            Self::Modifier { object, name } => format!("{object}.{name}({literal});"),
        }
    }

    /// The expression that names what is here, for descending into it.
    ///
    /// Only reached for the kinds whose accessor returns a reference —
    /// a structure, a union, a sequence or an array. The scalar kinds
    /// take [`Self::assign`] and never come here.
    fn reference(&self) -> String {
        match self {
            Self::Lvalue(expression) => expression.clone(),
            Self::Modifier { object, name } => format!("{object}.{name}()"),
        }
    }
}

/// The statements, with the declarations that have to precede them.
#[derive(Debug, Default)]
struct Cpp {
    declarations: String,
    statements: String,
    next: usize,
    seen: Consumed,
}

impl Cpp {
    fn into_source(self) -> String {
        let mut out = self.declarations;
        out.push_str(&self.statements);
        out
    }

    fn temporary(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("zd_{stem}_{}", self.next)
    }
}

/// Emit the C++ statements that fill `sample` with `values`.
///
/// # Errors
/// [`ProbeError`], naming the path in the value.
pub fn fill_corba_cpp(
    ast: &Ast,
    type_: Id<TypeNode>,
    values: &Value,
) -> Result<String, ProbeError> {
    fill_corba_cpp_recording(ast, type_, values, &mut Consumed::new())
}

/// As [`fill_corba_cpp`], recording which scalars of the value were
/// read.
///
/// # Errors
/// As [`fill_corba_cpp`].
pub fn fill_corba_cpp_recording(
    ast: &Ast,
    type_: Id<TypeNode>,
    values: &Value,
    seen: &mut Consumed,
) -> Result<String, ProbeError> {
    // Before the first hop; see `contract::reject_alias_cycle`.
    contract::reject_alias_cycle(ast)?;
    let mut out = Cpp::default();
    let root = contract::root_path(ast, type_);
    let result = fill_at(
        ast,
        type_,
        values,
        &Place::Lvalue(String::from("sample")),
        &mut out,
        &root,
    );
    *seen = core::mem::take(&mut out.seen);
    result?;
    Ok(out.into_source())
}

/// Whether a type's union accessor returns a reference that can be
/// descended into, or a value that only a modifier can set.
///
/// CORBA 3.3 part 3 §1.2.9: a case member of a "variable-length or
/// constructed" type gets a reference accessor; a scalar gets a value
/// one. Getting this wrong does not compile, which is the reason it is
/// a function and not a comment.
fn accessor_is_a_reference(ast: &Ast, type_: Id<TypeNode>) -> bool {
    matches!(
        ast.type_(contract::resolve_alias(ast, type_))
            .map(|node| node.kind.clone()),
        Ok(TypeKind::Structure { .. }
            | TypeKind::Valuetype { .. }
            | TypeKind::Union { .. }
            | TypeKind::Sequence { .. }
            | TypeKind::Array { .. })
    )
}

/// A union case member that is an array, which §1.2.9 gives a slice
/// **pointer** accessor rather than a reference.
fn array_after_aliases(ast: &Ast, type_: Id<TypeNode>) -> Option<TypeKind> {
    let kind = ast
        .type_(contract::resolve_alias(ast, type_))
        .map(|node| node.kind.clone())
        .ok()?;
    matches!(kind, TypeKind::Array { .. }).then_some(kind)
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
    out: &mut Cpp,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Primitive(kind) => {
            let literal = primitive_literal(*kind, value, path)?;
            out.seen.scalar(path);
            let _ = writeln!(out.statements, "{}", place.assign(&literal));
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => {
            let text = contract::text(value, path)?;
            out.seen.scalar(path);
            let _ = writeln!(
                out.statements,
                "{}",
                place.assign(&format!("CORBA::string_dup({})", cpp_string(text)))
            );
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => {
            let text = contract::text(value, path)?;
            out.seen.scalar(path);
            // Not an `L"…"` literal. `wchar_t` is four bytes on Linux
            // and `CORBA::WChar` is two in omniORB, so a wide literal is
            // the wrong array against one of the two ORBs — and with a
            // cast in front of it, wrong without saying so. An array of
            // `CORBA::WChar` initialised from plain integers is right
            // against both, whatever the ORB made the type.
            //
            // The units are UTF-16, which is what §7.4.1.1.2 puts on the
            // wire. Where `CORBA::WChar` is four bytes the ORB is handed
            // the surrogates themselves; that is a measurement and not a
            // mistake — see `docs/corba-column.md`.
            let buffer = out.temporary("wstr");
            let mut units = String::new();
            for unit in text.encode_utf16() {
                let _ = write!(units, "0x{unit:04x}, ");
            }
            let _ = writeln!(
                out.declarations,
                "static CORBA::WChar {buffer}[] = {{ {units}0x0000 }};"
            );
            let _ = writeln!(
                out.statements,
                "{}",
                place.assign(&format!("CORBA::wstring_dup({buffer})"))
            );
            Ok(())
        }
        TypeKind::Fixed { .. } => {
            let text = contract::text(value, path)?;
            out.seen.scalar(path);
            // §1.14's `Fixed(const char *)`. The decimal goes in as the
            // sample wrote it and the ORB rounds it to the declared
            // scale, which is the ORB's rule to apply and not this
            // harness's to pre-apply.
            let _ = writeln!(
                out.statements,
                "{}",
                place.assign(&format!("CORBA::Fixed({})", cpp_string(text)))
            );
            Ok(())
        }
        TypeKind::Enumeration { .. } => {
            let (name, _) = contract::enumerator(ast, type_, value, path)?;
            out.seen.scalar(path);
            let _ = writeln!(
                out.statements,
                "{}",
                place.assign(&enumerator_name(ast, type_, &name))
            );
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
            let target = place.reference();
            for member in &members {
                let Some(member_value) = contract::member_value(object, member, path)? else {
                    continue;
                };
                fill_at(
                    ast,
                    member.type_,
                    member_value,
                    // The member as C++ has it on the left, the member as
                    // the IDL wrote it in the path on the right. They
                    // differ exactly when the name is a C++ keyword.
                    &Place::Lvalue(format!("{target}.{}", cpp_id(&member.name))),
                    out,
                    &contract::join(path, &member.name),
                )?;
            }
            Ok(())
        }
        TypeKind::Union { discriminator, .. } => {
            let object = contract::object(value, path)?;
            let discriminant = contract::discriminator(object, path)?;
            let target = place.reference();
            for (name, member_value) in object {
                if name == "_d" {
                    continue;
                }
                let Some(member_type) = union_member(ast, type_, name) else {
                    return Err(ProbeError::Unknown {
                        path: contract::join(path, name),
                        declared: union_member_names(ast, type_),
                    });
                };
                contract::reject_null(member_value, &contract::join(path, name))?;
                if let Some(TypeKind::Array { element, dims }) =
                    array_after_aliases(ast, member_type)
                {
                    // §1.2.9's array case member: the accessor returns a
                    // slice pointer, the modifier takes the array type
                    // itself, which decays to that same slice pointer —
                    // `void block(const _0RL_block _value)` in omniORB's
                    // gen.hh and `void block(_block)` in TAO's genC.h,
                    // both measured on `array-in-union-branch`. A *named*
                    // array of the slice type fits both, filled first and
                    // handed over whole; the modifier deep-copies. The
                    // accessor's pointee names the slice type without the
                    // emitter knowing the ORB's spelling of it, the same
                    // reason the reference branch below uses `decltype`;
                    // `remove_const` because omniORB's only accessor is
                    // the `const` one.
                    let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                        detail: format!("{path}: the array element is unresolved"),
                    })?;
                    let counts =
                        contract::array_dimensions(ast, &dims, &contract::join(path, name))?;
                    let temporary = out.temporary("case");
                    let accessor = cpp_id(name);
                    let _ = writeln!(
                        out.statements,
                        "std::remove_const<std::remove_pointer<decltype({target}.{accessor}())>::type>::type {temporary}[{}];",
                        counts[0]
                    );
                    fill_array(
                        ast,
                        element,
                        member_value,
                        &temporary,
                        out,
                        &contract::join(path, name),
                        &counts,
                    )?;
                    let _ = writeln!(out.statements, "{target}.{accessor}({temporary});");
                } else if accessor_is_a_reference(ast, member_type) {
                    // Built in a temporary and handed to the modifier,
                    // never through the accessor. §1.2.9's reference
                    // accessor is only defined once the branch is
                    // *active*, and nothing has activated it yet: TAO
                    // returns a reference to storage it has not
                    // constructed and the probe takes SIGSEGV —
                    // measured 2026-08-10 on `union-member-sequence`.
                    // omniORB happens to survive it, which is exactly
                    // why the shape had to be decided by the mapping
                    // and not by whether it ran.
                    //
                    // `decltype` names the member's type without the
                    // emitter knowing it, which matters because an
                    // anonymous `sequence<long>` inside a union is
                    // `Choice::_ids_seq` in omniORB and something else
                    // in TAO. `std::remove_reference` turns the
                    // accessor's reference back into the value type.
                    let temporary = out.temporary("case");
                    let accessor = cpp_id(name);
                    let _ = writeln!(
                        out.statements,
                        "std::remove_reference<decltype({target}.{accessor}())>::type {temporary};"
                    );
                    fill_at(
                        ast,
                        member_type,
                        member_value,
                        &Place::Lvalue(temporary.clone()),
                        out,
                        &contract::join(path, name),
                    )?;
                    let _ = writeln!(out.statements, "{target}.{accessor}({temporary});");
                } else {
                    fill_at(
                        ast,
                        member_type,
                        member_value,
                        &Place::Modifier {
                            object: target.clone(),
                            name: cpp_id(name).into_owned(),
                        },
                        out,
                        &contract::join(path, name),
                    )?;
                }
            }
            // After the member and never before it. §1.2.9's modifier
            // sets the discriminator to the *first* label of the case it
            // belongs to, so a case with two labels, or a `default:`
            // reached by a value of its own, would carry a
            // discriminator the sample did not ask for. Setting `_d`
            // last is what makes the two agree.
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the discriminator is unresolved"),
                })?;
            fill_at(
                ast,
                discriminator,
                discriminant,
                &Place::Modifier {
                    object: target,
                    name: String::from("_d"),
                },
                out,
                &contract::join(path, "_d"),
            )
        }
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = contract::array_dimensions(ast, dims, path)?;
            fill_array(ast, element, value, &place.reference(), out, path, &counts)
        }
        TypeKind::Sequence { element, bound } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the sequence element is unresolved"),
            })?;
            let items = contract::bounded_items(ast, value, *bound, "elements", path)?;
            let target = place.reference();
            let _ = writeln!(out.statements, "{target}.length({});", items.len());
            for (index, item) in items.iter().enumerate() {
                fill_at(
                    ast,
                    element,
                    item,
                    &Place::Lvalue(format!("{target}[{index}]")),
                    out,
                    &format!("{path}[{index}]"),
                )?;
            }
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
        TypeKind::Map {
            key,
            value: entry,
            bound,
        } => {
            // `map<K, V>` is IDL 4.2 and no CORBA construct, but TAO
            // 2.5.24 parses it and maps it to `std::map<K, V>` — its
            // generated header cites `be_visitor_map/map_ch.cpp` — so
            // an entry is made by subscripting. omniidl still refuses
            // the keyword ("Error in look-up of 'map': 'map' not
            // found", measured), so the omniorb column dies at omniidl
            // and never reaches this arm; the opendds and tao columns
            // both take it — `wireprobe-corba-idl` prints the map
            // rather than refusing it, and each front end answers for
            // itself.
            let key_type = key.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the map key type is unresolved"),
            })?;
            let value_type = entry.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the map value type is unresolved"),
            })?;
            // An array of pairs and not an object, the same shape every
            // other probe reads and writes: a JSON object keys on a
            // string and `map<long, long>` does not.
            let entries = contract::bounded_items(ast, value, *bound, "entries", path)?;
            let target = place.reference();
            for (index, pair) in entries.iter().enumerate() {
                let entry_path = format!("{path}[{index}]");
                let (entry_key, entry_value) = contract::map_entry(pair, &entry_path)?;
                let key_path = contract::join(&entry_path, "key");
                let key_literal = map_key_literal(ast, key_type, entry_key, &key_path, out)?;
                out.seen.scalar(&key_path);
                fill_at(
                    ast,
                    value_type,
                    entry_value,
                    &Place::Lvalue(format!("{target}[{key_literal}]")),
                    out,
                    &contract::join(&entry_path, "value"),
                )?;
            }
            Ok(())
        }
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// The C++ expression that names one map key.
///
/// A scalar key reuses the member literals. A string key is the C++
/// literal itself: `std::map`'s subscript builds the
/// `TAO::String_Manager` from it, and TAO's own `operator<`
/// (`tao/String_Manager_T.h:137`) orders by content, not by pointer. A
/// wide string key goes through the same unit-array declaration as the
/// wstring member fill, and for the same reason: a `L"…"` literal is
/// the wrong array against one of the two ORBs.
fn map_key_literal(
    ast: &Ast,
    key_type: Id<TypeNode>,
    value: &Value,
    path: &str,
    out: &mut Cpp,
) -> Result<String, ProbeError> {
    let resolved = contract::resolve_alias(ast, key_type);
    let node = ast.type_(resolved).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Primitive(kind) => primitive_literal(*kind, value, path),
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => {
            let text = contract::text(value, path)?;
            Ok(cpp_string(text))
        }
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => {
            let text = contract::text(value, path)?;
            let buffer = out.temporary("wkey");
            let mut units = String::new();
            for unit in text.encode_utf16() {
                let _ = write!(units, "0x{unit:04x}, ");
            }
            let _ = writeln!(
                out.declarations,
                "static CORBA::WChar {buffer}[] = {{ {units}0x0000 }};"
            );
            Ok(buffer)
        }
        TypeKind::Enumeration { .. } => {
            let (name, _) = contract::enumerator(ast, resolved, value, path)?;
            Ok(enumerator_name(ast, resolved, &name))
        }
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
    out: &mut Cpp,
    path: &str,
    counts: &[usize],
) -> Result<(), ProbeError> {
    let Some((&count, rest)) = counts.split_first() else {
        return fill_at(
            ast,
            element,
            value,
            &Place::Lvalue(lvalue.to_string()),
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

/// The statements, ready to `#include` into a decode template.
#[derive(Debug, Default)]
struct Dump {
    body: String,
    depth: usize,
    next: usize,
    /// The aggregates on a cycle, each of which gets a function of its
    /// own — `zd_rec::dump_<position>` — instead of an inline
    /// expansion, which for a cycle has no end. See
    /// [`recursive_aggregates`].
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

    /// A distinct name per loop, so a nested sequence does not shadow
    /// its own outer counter — see `dump.rs::Dump::temporary`.
    fn temporary(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("zd_{stem}_{}", self.next)
    }
}

/// Emit the C++ statements that print `sample` as canonical JSON.
///
/// # Errors
/// [`ProbeError`], naming the path in the type.
pub fn dump_corba_cpp(ast: &Ast, type_: Id<TypeNode>) -> Result<String, ProbeError> {
    // Before `recursive_aggregates`; see `contract::reject_alias_cycle`.
    contract::reject_alias_cycle(ast)?;
    let mut out = Dump {
        rec: crate::recursion::recursive_aggregates(ast, type_),
        ..Dump::default()
    };
    if !out.rec.is_empty() {
        // A local struct, because `dump.inc` lands inside `main` and a
        // plain function cannot be declared there; static members can,
        // and they see each other regardless of order, which is what a
        // mutual cycle needs. The bodies are the same expansion the
        // inline arms write, with every aggregate on a cycle turned
        // into a call.
        out.open("struct zd_rec {");
        for position in 0..out.rec.len() {
            let id = out.rec[position];
            let name = corba_cpp_type(ast, id)?;
            out.open(&format!(
                "static void dump_{position}(const {name} &zdv) {{"
            ));
            dump_aggregate(ast, id, "zdv", &mut out, &format!("<{name}>"))?;
            out.close("}");
        }
        out.close("};");
    }
    dump_at(ast, type_, "sample", &mut out, "")?;
    out.line("zdj_raw(\"\\n\");");
    Ok(out.body)
}

/// The C++ name of an aggregate, absolute.
fn corba_cpp_type(ast: &Ast, type_: Id<TypeNode>) -> Result<String, ProbeError> {
    let segments = corba_scoped_name(ast, type_).ok_or_else(|| ProbeError::NoLayout {
        detail: String::from("a type on a cycle has no scoped name"),
    })?;
    let mut out = String::new();
    for segment in &segments {
        out.push_str("::");
        out.push_str(&cpp_id(segment));
    }
    Ok(out)
}

/// zerodds-lint: recursion-depth 64 — the type graph's own nesting. There
/// is no sample here to end the walk, so the cycle is cut instead: an
/// aggregate named by `crate::recursion::recursive_aggregates` becomes a
/// call to the function emitted for it and is a leaf of this walk, which
/// therefore descends an acyclic graph. The typedef arm is finite because
/// `contract::reject_alias_cycle` at the entry point has refused a chain
/// that returns to itself. The corpus's deepest nesting is four.
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
            out.line(&format!("zdj_string((const char *)({expr}));"));
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => {
            out.line(&format!(
                "zdj_wstring_corba((const CORBA::WChar *)({expr}));"
            ));
            Ok(())
        }
        // `to_string` allocates through the ORB's own allocator, which
        // is why the helper frees it rather than the emitter leaking one
        // per member.
        TypeKind::Fixed { .. } => {
            out.line(&format!("zdj_fixed_corba({expr});"));
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
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } | TypeKind::Union { .. } => {
            // A type on a cycle is a call, never an inline expansion —
            // the expansion of a cycle does not end.
            if let Some(position) = out.rec.iter().position(|id| *id == type_) {
                out.line(&format!("zd_rec::dump_{position}({expr});"));
                return Ok(());
            }
            dump_aggregate(ast, type_, expr, out, path)
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
            let count = out.temporary("n");
            let index = out.temporary("i");
            out.open("{");
            out.line(&format!("CORBA::ULong {count} = {expr}.length();"));
            out.raw("[");
            out.open(&format!(
                "for (CORBA::ULong {index} = 0; {index} < {count}; {index}++) {{"
            ));
            out.line(&format!("if ({index} > 0) {{ zdj_raw(\",\"); }}"));
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
        TypeKind::Map {
            key, value: entry, ..
        } => {
            // The same `std::map` the fill subscripts; see the fill's
            // map arm for why only the opendds column reaches this.
            let key_type = key.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the map key type is unresolved"),
            })?;
            let value_type = entry.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the map value type is unresolved"),
            })?;
            let first = out.temporary("first");
            let pair = out.temporary("entry");
            // An array of pairs and not an object, the same shape every
            // other probe reads and writes: a JSON object keys on a
            // string and `map<long, long>` does not. The order is
            // `std::map`'s, which is also the order OpenDDS put the
            // entries on the wire.
            out.open("{");
            out.line(&format!("int {first} = 1;"));
            out.raw("[");
            out.open(&format!("for (const auto &{pair} : {expr}) {{"));
            out.line(&format!("if (!{first}) {{ zdj_raw(\",\"); }} {first} = 0;"));
            out.raw("[");
            dump_at(
                ast,
                key_type,
                &format!("{pair}.first"),
                out,
                &format!("{path}[].key"),
            )?;
            out.raw(",");
            dump_at(
                ast,
                value_type,
                &format!("{pair}.second"),
                out,
                &format!("{path}[].value"),
            )?;
            out.raw("]");
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
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// One aggregate's dump body — the expansion [`dump_at`] inlines for a
/// type off every cycle and `zd_rec`'s functions wrap for one on a
/// cycle.
///
/// zerodds-lint: recursion-depth 64 — the type graph's own nesting, below
/// `dump_at`, which carries the argument: every member goes back through
/// it, and an aggregate on a cycle is a call to `zd_rec::dump_<n>` there
/// rather than an expansion here.
#[allow(clippy::too_many_lines)]
fn dump_aggregate(
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
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            out.open("{");
            out.line("int zd_first = 1;");
            out.raw("{");
            for member in contract::members(ast, type_) {
                // The key is the IDL name and the read is the C++ one.
                // A renamed key would make this column disagree with
                // every other over a member name that is not on the wire.
                out.line(&format!("zdj_key(&zd_first, \"{}\");", member.name));
                dump_at(
                    ast,
                    member.type_,
                    &format!("{expr}.{}", cpp_id(&member.name)),
                    out,
                    &contract::join(path, &member.name),
                )?;
            }
            out.raw("}");
            out.close("}");
            Ok(())
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the discriminator is unresolved"),
                })?;
            out.open("{");
            out.line("int zd_first = 1;");
            out.raw("{");
            out.line("zdj_key(&zd_first, \"_d\");");
            dump_at(
                ast,
                discriminator,
                &format!("{expr}._d()"),
                out,
                &format!("{path}._d"),
            )?;
            // The width is the discriminator's, not always `long long`:
            // `union Choice switch (unsigned long long) { case 16045690984833335998: … }`
            // is a corpus case whose label does not fit a signed 64-bit
            // constant, and a `switch` cast to `long long` would compare it
            // wrongly even where the constant is accepted. `discriminator_c_width`
            // answers both halves at once.
            let (width, suffix) = contract::discriminator_c_width(ast, discriminator);
            out.open(&format!("switch (({width})({expr}._d())) {{"));
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
                            Ok(value) => out.line(&format!("case {value}{suffix}:")),
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
                out.line(&format!("zdj_key(&zd_first, \"{}\");", member.name));
                dump_at(
                    ast,
                    member_type,
                    &format!("{expr}.{}()", cpp_id(&member.name)),
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
        other => Err(ProbeError::NoLayout {
            detail: format!("{path}: not an aggregate: {}", other.as_str()),
        }),
    }
}

/// zerodds-lint: recursion-depth 8 — one frame per array dimension.
/// `counts` drops its head at each frame and the innermost element goes to
/// `dump_at`.
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
/// The same `switch` `dump.rs` writes and for the same reason: the value
/// is the ORB's, and one that names no enumerator is a finding rather
/// than a number that would read as some other member's value.
fn dump_enumeration(ast: &Ast, type_: Id<TypeNode>, expr: &str, out: &mut Dump) {
    out.open(&format!("switch ((long long)({expr})) {{"));
    for id in ast.enumerators_of(type_) {
        let Ok(node) = ast.enumerator(*id) else {
            continue;
        };
        let scoped = enumerator_name(ast, type_, &node.name);
        out.line(&format!("case (long long){scoped}:"));
        out.line(&format!("    zdj_raw(\"\\\"{}\\\"\"); break;", node.name));
    }
    out.line(&format!(
        "default: zdj_enum_unknown((long long)({expr})); break;"
    ));
    out.close("}");
}

// ------------------------------------------------------------- helpers

/// An enumerator, in the C++ scope the mapping puts it in.
///
/// CORBA 3.3 part 3 §1.2.5 puts an IDL enumeration's enumerators in the
/// **enclosing** scope, not inside a type of their own — so `RED`, and
/// `::common::RED` where the enumeration sits in a module. Absolute, so
/// a same-named identifier nearer the use site cannot win.
///
/// Every segment goes through [`cxx`], the enumerator included: measured
/// 2026-08-11, omniidl and `tao_idl` generate
/// `enum _cxx_class { _cxx_template, _cxx_virtual, _cxx_friend }` for
/// the corpus case `identifier-target-keywords`, so a namespace segment
/// or an enumerator that is a C++ keyword is renamed wherever it is
/// referred to and not only where it is declared. The enumeration's own
/// name is not in the result — §1.2.5 leaves it out of the scope — which
/// is why this walks `scope` and not the whole scoped name.
fn enumerator_name(ast: &Ast, type_: Id<TypeNode>, enumerator: &str) -> String {
    let Some(scoped) = corba_scoped_name(ast, type_) else {
        return format!("::{}", cpp_id(enumerator));
    };
    let Some((_, scope)) = scoped.split_last() else {
        return format!("::{}", cpp_id(enumerator));
    };
    let mut out = String::from("::");
    for segment in scope {
        out.push_str(&cpp_id(segment));
        out.push_str("::");
    }
    out.push_str(&cpp_id(enumerator));
    out
}

/// The C++ literal for one primitive.
///
/// Floating point goes out with seventeen significant digits, which is
/// what a `double` round-trips through.
fn primitive_literal(kind: PrimitiveKind, value: &Value, path: &str) -> Result<String, ProbeError> {
    Ok(match kind {
        PrimitiveKind::Boolean => {
            // `1` and `0` rather than `true` and `false`: omniORB's
            // `CORBA::Boolean` is an `unsigned char` and TAO's is a
            // `bool`, and the integers convert to both.
            if contract::boolean(value, path)? {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        // `char` is one octet, so a code point above U+00FF is a value
        // the type cannot hold; `character_for` refuses it here rather
        // than letting the compiler narrow it in silence.
        PrimitiveKind::Char => {
            let code = contract::character_for(kind, value, path)?;
            format!("(CORBA::Char)0x{:02x}", code as u32)
        }
        // A `CORBA::WChar` is two bytes in omniORB and four in TAO. The
        // integer converts to either; a `L'x'` literal would be four
        // bytes in both compilers and wrong against omniORB.
        PrimitiveKind::WChar => {
            let code = contract::character_for(kind, value, path)?;
            format!("(CORBA::WChar)0x{:04x}", code as u32)
        }
        // `int8` and `uint8` are spelled `octet` in the printed IDL —
        // Table 31 gives all three size 1 and alignment 1 — so the
        // literal is the octet the value occupies, computed here rather
        // than left to a narrowing conversion.
        PrimitiveKind::Octet | PrimitiveKind::UInt8 | PrimitiveKind::Int8 => {
            let integer = contract::integer_for(kind, value, path)?;
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            let octet = (integer as i64) as u8;
            format!("(CORBA::Octet)0x{octet:02x}")
        }
        PrimitiveKind::Float32 => format!("{:.17e}F", contract::float(value, path)?),
        PrimitiveKind::Float64 => format!("{:.17e}", contract::float(value, path)?),
        PrimitiveKind::Int64 => {
            let n = contract::integer_for(kind, value, path)?;
            // C++ has no literal for INT64_MIN either:
            // `-9223372036854775808LL` is a positive constant too large
            // for `long long`, parsed unsigned and then negated — g++
            // warns "integer constant is so large that it is unsigned",
            // measured on `integer-extremes` in the omniorb and tao
            // columns. The same subtraction `<cstdint>` uses spells the
            // value exactly; `fill.rs` already does this.
            if n == i128::from(i64::MIN) {
                "(-9223372036854775807LL - 1)".to_string()
            } else {
                format!("{n}LL")
            }
        }
        PrimitiveKind::UInt64 => format!("{}ULL", contract::integer_for(kind, value, path)?),
        PrimitiveKind::Float128 => {
            return Err(ProbeError::Unsupported {
                path: path.to_string(),
                kind: "a long double, whose C++ mapping is x87 extended and not §9.3.1.3's binary128",
            });
        }
        _ => format!("{}", contract::integer_for(kind, value, path)?),
    })
}

/// The call that prints one primitive as canonical JSON.
fn primitive_print(kind: PrimitiveKind, expr: &str, path: &str) -> Result<String, ProbeError> {
    Ok(match kind {
        PrimitiveKind::Boolean => format!("zdj_bool((int)({expr}));"),
        PrimitiveKind::Char => format!("zdj_char((char)({expr}));"),
        PrimitiveKind::WChar => format!("zdj_wchar((unsigned long)({expr}));"),
        // The octet came back as the ORB's `CORBA::Octet`, which is
        // unsigned. An `int8` member's value is the same octet read
        // signed, and printing it unsigned would hand back 251 where the
        // sample wrote −5.
        PrimitiveKind::Int8 => format!("zdj_i64((long long)(signed char)({expr}));"),
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => {
            format!("zdj_u64((unsigned long long)(unsigned char)({expr}));")
        }
        PrimitiveKind::Int16 | PrimitiveKind::Int32 => {
            format!("zdj_i64((long long)({expr}));")
        }
        PrimitiveKind::UInt16 | PrimitiveKind::UInt32 => {
            format!("zdj_u64((unsigned long long)({expr}));")
        }
        // A JSON number is a double, so the low bits of a 64-bit integer
        // do not survive one. The canonical form is a string.
        PrimitiveKind::Int64 => format!("zdj_i64_string((long long)({expr}));"),
        PrimitiveKind::UInt64 => format!("zdj_u64_string((unsigned long long)({expr}));"),
        PrimitiveKind::Float32 => format!("zdj_f32((float)({expr}));"),
        PrimitiveKind::Float64 => format!("zdj_f64((double)({expr}));"),
        PrimitiveKind::Float128 => {
            return Err(ProbeError::Unsupported {
                path: path.to_string(),
                kind: "a long double, whose C++ mapping is x87 extended and not §9.3.1.3's binary128",
            });
        }
    })
}

fn union_member(ast: &Ast, union: Id<TypeNode>, name: &str) -> Option<Id<TypeNode>> {
    contract::union_cases(ast, union)
        .into_iter()
        .find(|case| case.name == name)
        .map(|case| case.type_)
}

fn union_member_names(ast: &Ast, union: Id<TypeNode>) -> Vec<String> {
    contract::union_cases(ast, union)
        .into_iter()
        .map(|case| case.name)
        .collect()
}

/// A C++ string literal.
///
/// Every byte outside printable ASCII goes out as a three-digit octal
/// escape. Not hex: `\xHH` is greedy in C++ and `"\xc3a"` reads as one
/// character with an out-of-range value, so a UTF-8 string followed by a
/// hex digit would change meaning. Three-digit octal cannot run on.
fn cpp_string(text: &str) -> String {
    let mut out = String::from("\"");
    for byte in text.as_bytes() {
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7E => out.push(char::from(*byte)),
            other => {
                let _ = write!(out, "\\{other:03o}");
            }
        }
    }
    out.push('"');
    out
}

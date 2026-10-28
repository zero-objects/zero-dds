// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a model type is called in Rust, and how a value of it is
//! written and read.
//!
//! This replaces `zerodds_idl_rust::type_map`, which answered the same
//! two questions out of ten process-wide `thread_local!`s: the `any`
//! target, the set of names that are interfaces, the struct names, the
//! enum names, the typedef map, the const values, the enum literal
//! values, the union names, the struct definitions and the current
//! module path. Every one of them had to be set before a call and was
//! still set after it, so two generations in one process shared them
//! and the second one's answers depended on the first one's input.
//!
//! None of it is needed. All ten were caches of things the model
//! already holds, and the model arrives as an argument:
//!
//! * whether a name is an interface is [`TypeKind::ObjectRef`];
//! * whether it is a struct, a union or an enumeration is its
//!   [`TypeKind`];
//! * what a `typedef` names is [`TypeKind::Alias`];
//! * a constant's value is [`Ast::eval_integral`];
//! * the module path is [`Ast::scoped_name_of`].
//!
//! The `any` target was the one that was not a cache: a process-wide
//! `bool` deciding whether `any` maps to the DDS holder or the CORBA
//! one. It is not a process property. This crate generates CORBA, so
//! the answer is CORBA, written once, here.
//!
//! ## Which column this follows
//!
//! `zerodds-lang-rules`' materialiser is the column that emits the
//! **data** types — structs, unions, enumerations — and their `encode`
//! and `decode`. A user concatenates that output with this one, which
//! is what `tests/compile_check.rs` compiles, so the two have to name
//! the same type the same way. This follows
//! `zerodds_lang_rules::adapter::cst` for every kind that column also
//! emits.
//!
//! Three kinds it does not emit reach a name that resolves to nothing
//! there — `Any`, `ObjectRef` and `ValueBase`, written bare because
//! DDS has none of them and nothing in that column ever produces one.
//! Those three take the CORBA answers, which is
//! `zerodds-corba-codegen`'s Annex A.1 table.

use std::collections::BTreeMap;

use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, Ref};

use crate::error::{CorbaRustError, Result};

/// The Rust type a model type is written as.
///
/// # Errors
/// [`CorbaRustError::UnresolvedType`] for a reference the frontend left
/// unresolved, and [`CorbaRustError::Unsupported`] for a kind this
/// column has no name for.
pub fn rust_type_for(ast: &Ast, type_: &Ref<TypeNode>) -> Result<String> {
    let id = type_
        .resolved()
        .ok_or_else(|| CorbaRustError::UnresolvedType {
            scoped: String::from("<unresolved>"),
        })?;
    rust_type_of(ast, id, 0)
}

/// How deep an alias or an element chain is followed.
///
/// A model built by hand can hold a cycle; well-formed IDL cannot reach
/// this. Bounded rather than trusted, because a generator that
/// overflowed its stack on bad input is a generator a bad input file
/// crashes.
const MAX_DEPTH: u32 = 64;

/// The worker behind [`rust_type_for`], carrying the depth bound.
///
/// zerodds-lint: recursion-depth 64 (type nesting; bounded by `MAX_DEPTH`)
fn rust_type_of(ast: &Ast, id: Id<TypeNode>, depth: u32) -> Result<String> {
    if depth > MAX_DEPTH {
        return Err(CorbaRustError::Unsupported {
            what: "a type that refers to itself",
            at: 0,
        });
    }
    let node = ast.type_(id).map_err(|_| CorbaRustError::UnresolvedType {
        scoped: String::from("<stale handle>"),
    })?;
    Ok(match &node.kind {
        TypeKind::Primitive(PrimitiveKind::Float128) => String::from("zerodds_float128::Float128"),
        TypeKind::Primitive(kind) => String::from(rust_primitive(*kind)),
        // `string` is `String`, which is what the data column writes.
        // `wstring` is **not**: CORBA 3.3 part 2 §15.3.2.7 puts a
        // byte-order mark and a code-unit order on the wire, a
        // `String` has nowhere to carry either, and
        // `zerodds_corba_rt::WString` is the Annex A.1 row's answer
        // and does both. The data column writes `String` for both
        // widths because DDS has no such clause; the two are different
        // *positions* — a member there, a parameter here — so nothing
        // is declared twice and nothing collides.
        //
        // Measured consequence of getting it wrong: `wecho` against
        // omniORB, TAO and JacORB is a UTF-16 payload, and
        // `Writer::write_string` writes octets.
        TypeKind::String { width, .. } => match width {
            zerodds_ast::CharWidth::Char => String::from("String"),
            zerodds_ast::CharWidth::WChar => String::from("zerodds_corba_rt::WString"),
        },
        TypeKind::Sequence { element, .. } => {
            let element = resolved(element)?;
            format!(
                "::std::vec::Vec<{}>",
                rust_type_of(ast, element, depth + 1)?
            )
        }
        TypeKind::Array { element, dims } => {
            let element = resolved(element)?;
            let mut out = rust_type_of(ast, element, depth + 1)?;
            // Innermost dimension first, so `long a[2][3]` is
            // `[[i32; 3]; 2]` — the same order the data column writes.
            for dim in dims.iter().rev() {
                let length = ast
                    .eval_integral(*dim)
                    .map_err(|_| CorbaRustError::Unsupported {
                        what: "an array length that does not evaluate",
                        at: 0,
                    })?;
                out = format!("[{out}; {length}]");
            }
            out
        }
        TypeKind::Map { key, value, .. } => {
            let key = resolved(key)?;
            let value = resolved(value)?;
            format!(
                "::std::collections::BTreeMap<{}, {}>",
                rust_type_of(ast, key, depth + 1)?,
                rust_type_of(ast, value, depth + 1)?
            )
        }
        // A `typedef` is followed rather than named: the data column
        // emits no Rust type alias, so a name that stopped at the alias
        // would resolve to nothing.
        TypeKind::Alias { aliased } => {
            let aliased = resolved(aliased)?;
            rust_type_of(ast, aliased, depth + 1)?
        }
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. } => node.name.clone(),
        // A valuetype used as a type is the concrete state carrier this
        // crate emits beside its trait, not the trait: a trait is not a
        // type a parameter can take by value.
        TypeKind::Valuetype { .. } => format!("{}Value", node.name),
        TypeKind::ValueBox { boxed } => {
            let boxed = resolved(boxed)?;
            format!(
                "::std::boxed::Box<{}>",
                rust_type_of(ast, boxed, depth + 1)?
            )
        }
        TypeKind::Any => String::from("zerodds_corba_rt::CorbaAny"),
        TypeKind::ObjectRef { .. } => String::from("zerodds_corba_rust::ObjectReference"),
        // Refused rather than named. `zerodds-corba-codegen`'s Annex
        // A.1 row for both is a bare `Fixed` and a bare `ValueBase`,
        // and no crate in this tree declares either — a generated
        // signature carrying one does not compile, which is a worse
        // outcome than a generator that says which construct it has no
        // answer for.
        TypeKind::Fixed { .. } => {
            return Err(CorbaRustError::Unsupported {
                what: "fixed<P,S> — no holder type in this tree",
                at: 0,
            });
        }
        TypeKind::ValueBase => {
            return Err(CorbaRustError::Unsupported {
                what: "ValueBase as a data type",
                at: 0,
            });
        }
        TypeKind::Native => {
            return Err(CorbaRustError::Unsupported {
                what: "a native type used as a data type",
                at: 0,
            });
        }
        TypeKind::AnnotationType { .. } => {
            return Err(CorbaRustError::Unsupported {
                what: "an annotation type used as a data type",
                at: 0,
            });
        }
    })
}

/// The resolved handle of a reference, or the error that names it.
fn resolved(reference: &Ref<TypeNode>) -> Result<Id<TypeNode>> {
    reference
        .resolved()
        .ok_or_else(|| CorbaRustError::UnresolvedType {
            scoped: String::from("<unresolved>"),
        })
}

/// The Rust primitive an IDL primitive is written as.
///
/// The data column's table, followed rather than re-derived.
/// `long double` is not routed here; it takes
/// `zerodds_float128::Float128`.
const fn rust_primitive(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "bool",
        PrimitiveKind::Char | PrimitiveKind::WChar => "char",
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => "u8",
        PrimitiveKind::Int8 => "i8",
        PrimitiveKind::Int16 => "i16",
        PrimitiveKind::UInt16 => "u16",
        PrimitiveKind::Int32 => "i32",
        PrimitiveKind::UInt32 => "u32",
        PrimitiveKind::Int64 => "i64",
        PrimitiveKind::UInt64 => "u64",
        PrimitiveKind::Float32 => "f32",
        PrimitiveKind::Float64 | PrimitiveKind::Float128 => "f64",
    }
}

// ============================================================================
// Marshalling
// ============================================================================

/// What a write of one value looks like.
///
/// Split by whether the write can refuse, because `zerodds-wire`'s
/// writer is infallible for everything but a `char` it cannot carry and
/// a bound a value overruns. The previous generator put
/// `.map_err(|_| MARSHAL)?` on every write because `zerodds-cdr`'s
/// writer returned a `Result` for all of them; carrying that shape
/// forward would put an error path in generated code that no value can
/// take.
pub enum Write {
    /// A statement that cannot fail. No `?`, no `map_err`.
    Infallible(String),
    /// An expression of type `Result<(), zerodds_wire::WireError>`.
    Fallible(String),
}

impl Write {
    /// The statement form, with `?` and a mapped error where the write
    /// can refuse.
    #[must_use]
    pub fn statement(&self, on_error: &str) -> String {
        match self {
            Self::Infallible(stmt) => format!("{stmt};"),
            Self::Fallible(expr) => format!("{expr}.map_err(|_| {on_error})?;"),
        }
    }

    /// The statement form for a context that returns a value rather
    /// than propagating an error — a skeleton arm, which answers a
    /// refused write with an exception reply.
    #[must_use]
    pub fn guarded(&self, on_error: &str) -> String {
        match self {
            Self::Infallible(stmt) => format!("{stmt};"),
            Self::Fallible(expr) => format!("if {expr}.is_err() {{ return {on_error}; }}"),
        }
    }

    /// The form for a body that returns
    /// `Result<_, zerodds_wire::WireError>` itself, where an error
    /// propagates with `?` and no mapping.
    #[must_use]
    pub fn statement_propagating(&self) -> String {
        self.statement_inline()
    }

    /// The form that goes inside a closure body, where an error
    /// propagates with `?` and no mapping.
    fn statement_inline(&self) -> String {
        match self {
            Self::Infallible(stmt) => format!("{stmt};"),
            Self::Fallible(expr) => format!("{expr}?;"),
        }
    }
}

/// How `value` is written into the writer named `writer`.
///
/// # Errors
/// As [`rust_type_for`].
pub fn write_of(ast: &Ast, type_: &Ref<TypeNode>, writer: &str, value: &str) -> Result<Write> {
    let id = resolved(type_)?;
    write_at(ast, id, writer, value, 0)
}

/// zerodds-lint: recursion-depth 64 (type nesting; bounded by `MAX_DEPTH`)
fn write_at(ast: &Ast, id: Id<TypeNode>, w: &str, v: &str, depth: u32) -> Result<Write> {
    if depth > MAX_DEPTH {
        return Err(CorbaRustError::Unsupported {
            what: "a type that refers to itself",
            at: 0,
        });
    }
    let node = ast.type_(id).map_err(|_| CorbaRustError::UnresolvedType {
        scoped: String::from("<stale handle>"),
    })?;
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => match kind {
            // The two that can refuse: a code point wider than the
            // type carries. §7.2.2.2.1.2.2 for `char`, §15.3.1.6 for
            // `wchar`.
            PrimitiveKind::Char => Write::Fallible(format!("{w}.write_char({v})")),
            PrimitiveKind::WChar => Write::Fallible(format!("{w}.write_wchar_member({v})")),
            _ => Write::Infallible(format!("{w}.{}({v})", primitive_writer(*kind))),
        },
        TypeKind::String { width, .. } => match width {
            zerodds_ast::CharWidth::Char => Write::Infallible(format!("{w}.write_string(&{v})")),
            // §15.3.2.7's framing is the holder's, not the writer's.
            zerodds_ast::CharWidth::WChar => Write::Fallible(format!("{v}.encode(&mut {w})")),
        },
        TypeKind::Sequence { element, .. } => {
            let element = resolved(element)?;
            Write::Fallible(format!(
                "{w}.write_sequence(&{v}, {})",
                element_writer(ast, element, depth + 1)?
            ))
        }
        TypeKind::Array { element, .. } => {
            let element = resolved(element)?;
            Write::Fallible(format!(
                "{w}.write_array(&{v}, {})",
                element_writer(ast, element, depth + 1)?
            ))
        }
        TypeKind::Map { key, value, .. } => {
            // No map helper in `zerodds-wire`: a map's wire form is a
            // sequence of pairs, and which pair order and which
            // framing is the layout plan's answer, not the runtime's.
            // Written out so the two writes are visible.
            let key_id = resolved(key)?;
            let value_id = resolved(value)?;
            let key_write = write_at(ast, key_id, "(*__w)", "__k", depth + 1)?;
            let value_write = write_at(ast, value_id, "(*__w)", "__v", depth + 1)?;
            Write::Fallible(format!(
                "(|__w: &mut zerodds_wire::Writer| -> ::core::result::Result<(), zerodds_wire::WireError> {{ \
                 __w.write_u32({v}.len() as u32); \
                 for (__k, __v) in &{v} {{ {} {} }} \
                 ::core::result::Result::Ok(()) }})(&mut {w})",
                key_write.statement_inline(),
                value_write.statement_inline()
            ))
        }
        TypeKind::Alias { aliased } => {
            let aliased = resolved(aliased)?;
            write_at(ast, aliased, w, v, depth + 1)?
        }
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Any => Write::Fallible(format!("{v}.encode(&mut {w})")),
        // The one declared type whose `encode` cannot refuse.
        TypeKind::ObjectRef { .. } => Write::Infallible(format!("{v}.encode(&mut {w})")),
        TypeKind::ValueBox { boxed } => {
            let boxed = resolved(boxed)?;
            write_at(ast, boxed, w, &format!("(*{v})"), depth + 1)?
        }
        TypeKind::Fixed { .. }
        | TypeKind::ValueBase
        | TypeKind::Native
        | TypeKind::AnnotationType { .. } => {
            return Err(CorbaRustError::Unsupported {
                what: "a type with no CORBA CDR writer in this tree",
                at: 0,
            });
        }
    })
}

/// The element closure `write_sequence` and `write_array` take.
///
/// The closure's `__w` is already a `&mut Writer`, so the receiver is
/// `(*__w)` and not `__w`: `&mut __w` inside the body would be a second
/// mutable borrow of the borrow, which is what the first run of
/// `tests/compile_check.rs` reported for `sequence<Reading>`.
fn element_writer(ast: &Ast, element: Id<TypeNode>, depth: u32) -> Result<String> {
    let write = write_at(ast, element, "(*__w)", "(*__x)", depth)?;
    Ok(format!(
        "|__w: &mut zerodds_wire::Writer, __x: &_| {{ {} ::core::result::Result::Ok(()) }}",
        write.statement_inline()
    ))
}

/// An expression of type `Result<T, zerodds_wire::WireError>` reading a
/// value of `type_` from the reader named `reader`.
///
/// # Errors
/// As [`rust_type_for`].
pub fn read_of(ast: &Ast, type_: &Ref<TypeNode>, reader: &str) -> Result<String> {
    let id = resolved(type_)?;
    read_at(ast, id, reader, 0)
}

/// zerodds-lint: recursion-depth 64 (type nesting; bounded by `MAX_DEPTH`)
fn read_at(ast: &Ast, id: Id<TypeNode>, r: &str, depth: u32) -> Result<String> {
    if depth > MAX_DEPTH {
        return Err(CorbaRustError::Unsupported {
            what: "a type that refers to itself",
            at: 0,
        });
    }
    let node = ast.type_(id).map_err(|_| CorbaRustError::UnresolvedType {
        scoped: String::from("<stale handle>"),
    })?;
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => format!("{r}.{}()", primitive_reader(*kind)),
        TypeKind::String { width, .. } => match width {
            zerodds_ast::CharWidth::Char => format!("{r}.read_string()"),
            zerodds_ast::CharWidth::WChar => {
                format!("zerodds_corba_rt::WString::decode(&mut {r})")
            }
        },
        TypeKind::Sequence { element, .. } => {
            let element = resolved(element)?;
            format!(
                "{r}.read_sequence(|__r| {})",
                read_at(ast, element, "(*__r)", depth + 1)?
            )
        }
        TypeKind::Array { element, .. } => {
            let element = resolved(element)?;
            // `read_array_n` infers `N` from the destination, so the
            // call carries no turbofish and the length stays in the
            // type where the model put it.
            format!(
                "{r}.read_array_n(|__r| {})",
                read_at(ast, element, "(*__r)", depth + 1)?
            )
        }
        TypeKind::Map { key, value, .. } => {
            let key_id = resolved(key)?;
            let value_id = resolved(value)?;
            format!(
                "(|__r: &mut zerodds_wire::Reader<'_>| {{ \
                 let __n = __r.read_u32()? as usize; \
                 if __n > __r.remaining() {{ return ::core::result::Result::Err(zerodds_wire::WireError::ImpossibleLength {{ at: __r.position(), claimed: __n, available: __r.remaining() }}); }} \
                 let mut __m = ::std::collections::BTreeMap::new(); \
                 for _ in 0..__n {{ let __k = {}?; let __v = {}?; __m.insert(__k, __v); }} \
                 ::core::result::Result::Ok(__m) }})(&mut {r})",
                read_at(ast, key_id, "(*__r)", depth + 1)?,
                read_at(ast, value_id, "(*__r)", depth + 1)?
            )
        }
        TypeKind::Alias { aliased } => {
            let aliased = resolved(aliased)?;
            read_at(ast, aliased, r, depth + 1)?
        }
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. } => format!("{}::decode(&mut {r})", node.name),
        TypeKind::Valuetype { .. } => format!("{}Value::decode(&mut {r})", node.name),
        TypeKind::Any => format!("zerodds_corba_rt::CorbaAny::decode(&mut {r})"),
        TypeKind::ObjectRef { .. } => {
            format!("zerodds_corba_rust::ObjectReference::decode(&mut {r})")
        }
        TypeKind::ValueBox { boxed } => {
            let boxed = resolved(boxed)?;
            format!(
                "{}.map(::std::boxed::Box::new)",
                read_at(ast, boxed, r, depth + 1)?
            )
        }
        TypeKind::Fixed { .. }
        | TypeKind::ValueBase
        | TypeKind::Native
        | TypeKind::AnnotationType { .. } => {
            return Err(CorbaRustError::Unsupported {
                what: "a type with no CORBA CDR reader in this tree",
                at: 0,
            });
        }
    })
}

/// The writer method for a primitive that cannot refuse.
const fn primitive_writer(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "write_bool",
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => "write_u8",
        PrimitiveKind::Int8 => "write_i8",
        PrimitiveKind::Int16 => "write_i16",
        PrimitiveKind::UInt16 => "write_u16",
        PrimitiveKind::Int32 => "write_i32",
        PrimitiveKind::UInt32 => "write_u32",
        PrimitiveKind::Int64 => "write_i64",
        PrimitiveKind::UInt64 => "write_u64",
        PrimitiveKind::Float32 => "write_f32",
        PrimitiveKind::Float64 => "write_f64",
        PrimitiveKind::Float128 => "write_f128",
        // Routed to the fallible arm before reaching here.
        PrimitiveKind::Char => "write_char",
        PrimitiveKind::WChar => "write_wchar_member",
    }
}

/// The reader method for a primitive.
const fn primitive_reader(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "read_bool",
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => "read_u8",
        PrimitiveKind::Int8 => "read_i8",
        PrimitiveKind::Char => "read_char",
        PrimitiveKind::WChar => "read_wchar_member",
        PrimitiveKind::Int16 => "read_i16",
        PrimitiveKind::UInt16 => "read_u16",
        PrimitiveKind::Int32 => "read_i32",
        PrimitiveKind::UInt32 => "read_u32",
        PrimitiveKind::Int64 => "read_i64",
        PrimitiveKind::UInt64 => "read_u64",
        PrimitiveKind::Float32 => "read_f32",
        PrimitiveKind::Float64 => "read_f64",
        PrimitiveKind::Float128 => "read_f128",
    }
}

/// A value of `type_`, for an `out` parameter a skeleton has to hand
/// the servant before it is written.
///
/// **Not `Default::default()`**, which is what the previous generator
/// emitted. It worked because the previous data column derived
/// `Default` on everything it declared; `zerodds-lang-rules` derives
/// nothing — `adapter::cst` sets `derives: Vec::new()` on every struct
/// and enum, because a derive is a decision about the target language
/// and not about the wire — so `<Reading as Default>::default()` names
/// a trait the type does not implement, and every skeleton for an
/// operation with an `out` parameter of a declared type failed to
/// compile.
///
/// The value never reaches the wire: an `out` parameter is written by
/// the servant before the reply is marshalled, and nothing reads it in
/// between. So this is a *construction*, not Table 9 — the cheapest
/// value of the type there is, spelled out.
///
/// # Errors
/// As [`rust_type_for`].
pub fn zero_of(ast: &Ast, type_: &Ref<TypeNode>) -> Result<String> {
    let id = resolved(type_)?;
    zero_at(ast, id, 0)
}

/// zerodds-lint: recursion-depth 64 (type nesting; bounded by `MAX_DEPTH`)
fn zero_at(ast: &Ast, id: Id<TypeNode>, depth: u32) -> Result<String> {
    if depth > MAX_DEPTH {
        return Err(CorbaRustError::Unsupported {
            what: "a type that refers to itself",
            at: 0,
        });
    }
    let node = ast.type_(id).map_err(|_| CorbaRustError::UnresolvedType {
        scoped: String::from("<stale handle>"),
    })?;
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => match kind {
            PrimitiveKind::Boolean => String::from("false"),
            PrimitiveKind::Char | PrimitiveKind::WChar => String::from("'\\0'"),
            PrimitiveKind::Float32 | PrimitiveKind::Float64 => String::from("0.0"),
            PrimitiveKind::Float128 => String::from("zerodds_float128::Float128::from(0.0f64)"),
            _ => String::from("0"),
        },
        TypeKind::String { width, .. } => match width {
            zerodds_ast::CharWidth::Char => String::from("::std::string::String::new()"),
            zerodds_ast::CharWidth::WChar => {
                String::from("zerodds_corba_rt::WString(::std::string::String::new())")
            }
        },
        TypeKind::Sequence { .. } => String::from("::std::vec::Vec::new()"),
        TypeKind::Map { .. } => String::from("::std::collections::BTreeMap::new()"),
        TypeKind::Array { element, dims } => {
            let element = resolved(element)?;
            let mut out = zero_at(ast, element, depth + 1)?;
            // `from_fn` and not `[value; N]`: the repeat form needs the
            // element to be `Copy`, and a `String` element is not.
            for _ in dims {
                out = format!("::core::array::from_fn(|_| {out})");
            }
            out
        }
        TypeKind::Alias { aliased } => {
            let aliased = resolved(aliased)?;
            zero_at(ast, aliased, depth + 1)?
        }
        TypeKind::Structure { members, .. } => {
            let mut fields = Vec::new();
            for member in members {
                let member = ast
                    .member(*member)
                    .map_err(|_| CorbaRustError::StaleHandle { what: "a member" })?;
                let type_id = resolved(&member.type_)?;
                fields.push(format!(
                    "{}: {}",
                    member.name,
                    zero_at(ast, type_id, depth + 1)?
                ));
            }
            format!("{} {{ {} }}", node.name, fields.join(", "))
        }
        TypeKind::Enumeration { enumerators } => {
            let first = enumerators
                .first()
                .and_then(|id| ast.enumerator(*id).ok())
                .ok_or(CorbaRustError::Unsupported {
                    what: "an enumeration with no enumerators",
                    at: 0,
                })?;
            format!("{}::{}", node.name, first.name)
        }
        TypeKind::Union { cases, .. } => {
            // The union's own Rust form is the data column's, and its
            // first case is the one arm that always exists.
            let first = cases.first().and_then(|id| ast.case(*id).ok()).ok_or(
                CorbaRustError::Unsupported {
                    what: "a union with no cases",
                    at: 0,
                },
            )?;
            let member = ast
                .member(first.member)
                .map_err(|_| CorbaRustError::StaleHandle {
                    what: "a case member",
                })?;
            let type_id = resolved(&member.type_)?;
            format!(
                "{}::{}({})",
                node.name,
                member.name,
                zero_at(ast, type_id, depth + 1)?
            )
        }
        TypeKind::Valuetype { .. } | TypeKind::Bitmask { .. } | TypeKind::Bitset { .. } => {
            return Err(CorbaRustError::Unsupported {
                what: "an out parameter of this type has no constructible value",
                at: 0,
            });
        }
        TypeKind::ObjectRef { .. } => {
            String::from("zerodds_corba_rust::ObjectReference::default()")
        }
        TypeKind::Any => {
            String::from("zerodds_corba_rt::CorbaAny(zerodds_corba_rt::AnyValue::Null)")
        }
        TypeKind::ValueBox { boxed } => {
            let boxed = resolved(boxed)?;
            format!(
                "::std::boxed::Box::new({})",
                zero_at(ast, boxed, depth + 1)?
            )
        }
        TypeKind::Fixed { .. }
        | TypeKind::ValueBase
        | TypeKind::Native
        | TypeKind::AnnotationType { .. } => {
            return Err(CorbaRustError::Unsupported {
                what: "an out parameter of a type with no value in this tree",
                at: 0,
            });
        }
    })
}

/// The repository identifiers of every exception in the model, by
/// simple name.
///
/// The previous generator kept this in a `thread_local!` that a caller
/// had to set before generating and that stayed set afterwards. It is
/// a query over the model, so it is computed where it is used and
/// handed down as a value.
///
/// `zerodds_corba_codegen::repository_id_of` is what answers each one:
/// it reads the model's own identifier, which `zerodds-idl-lower` has
/// already computed with `#pragma prefix`, `#pragma version` and
/// `#pragma ID` applied. The previous generator re-derived it from the
/// declaration path and a `typeprefix` table it collected itself, and
/// saw none of the three pragmas.
#[must_use]
pub fn exception_repository_ids(ast: &Ast) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (id, node) in ast.exceptions() {
        let any = zerodds_ast::AnyId::Exception(id, node.node_id);
        if let Some(resolved) = zerodds_corba_codegen::repository_id_of(ast, any) {
            out.insert(node.name.clone(), resolved.id);
        }
    }
    out
}

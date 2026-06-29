// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL type → Rust type mapping.
//!
//! OMG IDL4 §7.4 + XTypes 1.3 §7.2.2 (type system). The mapping follows
//! Rust idiom: signed integers to `i*`, unsigned to `u*`, floats to
//! `f32`/`f64`, `string` to `String`, `sequence<T>` to `Vec<T>`,
//! `T[N]` to `[T; N]`.

use core::cell::{Cell, RefCell};

use zerodds_idl::ast::types::{
    ConstExpr, ConstrTypeDecl, Declarator, Definition, FloatingType, IntegerType, PrimitiveType,
    ScopedName, SequenceType, Specification, StringType, StructDcl, TypeDecl, TypeSpec,
};

use crate::error::{Result, RustGenError};

thread_local! {
    /// Codegen target for IDL `any`: `false` = DDS (`zerodds_dcps::DdsAny`,
    /// XCDR2 self-describing), `true` = CORBA (`zerodds_cdr::CorbaAny`,
    /// classic CDR TypeCode+Value). Set by the respective codegen entry
    /// (`generate_rust_module` depending on `cdr_only`;
    /// `generate_corba_rust_module`). Thread-local: each codegen run is
    /// isolated on its thread, so parallel runs (e.g. `cargo test`) do
    /// not clobber each other.
    static ANY_TARGET_CORBA: Cell<bool> = const { Cell::new(false) };
}

/// Sets the `any` codegen target (see [`ANY_TARGET_CORBA`]). Idempotent; each
/// codegen run sets it on entry, so there is no stale state between runs.
pub fn set_any_target_corba(corba: bool) {
    ANY_TARGET_CORBA.with(|c| c.set(corba));
}

/// `true` when the current codegen run targets CORBA/GIOP (classic CDR)
/// rather than the DDS XCDR2 PSM. Used to pick wire forms that differ
/// between the two PSMs (e.g. `wstring`: GIOP allows a BOM, XCDR2 does not).
#[must_use]
pub fn any_target_corba() -> bool {
    ANY_TARGET_CORBA.with(Cell::get)
}

thread_local! {
    /// Simple names of all interfaces declared in the current run. A
    /// scoped name whose last part appears here is an
    /// **interface reference** and maps (like `Object`) to
    /// `zerodds_corba_rust::ObjectReference` — an IOR on the wire
    /// (§15.3.3). Otherwise prevents e.g. `NamingContext new_context()`
    /// from referencing the trait itself (dyn-compatibility cycle).
    /// Thread-local (like ANY_TARGET_CORBA), so parallel codegen runs do
    /// not clobber each other; also avoids registry threading through ~25
    /// `rust_type_for` callers.
    static INTERFACE_REFS: RefCell<std::collections::BTreeSet<String>> =
        const { RefCell::new(std::collections::BTreeSet::new()) };
}

/// Registers the interface simple names of the current codegen run (replaces
/// the previous set). The CORBA codegen calls this on entry; the DDS codegen
/// leaves the set empty (there are no interface references there).
pub fn set_interface_refs<I: IntoIterator<Item = String>>(names: I) {
    INTERFACE_REFS.with(|r| {
        let mut g = r.borrow_mut();
        g.clear();
        g.extend(names);
    });
}

/// `true` if `name` is an interface registered during the run.
fn is_interface_ref(name: &str) -> bool {
    INTERFACE_REFS.with(|r| r.borrow().contains(name))
}

fn any_rust_type() -> &'static str {
    if ANY_TARGET_CORBA.with(Cell::get) {
        "zerodds_cdr::CorbaAny"
    } else {
        "zerodds_dcps::DdsAny"
    }
}

thread_local! {
    /// Simple names of struct types in the current run. A scoped field whose
    /// type resolves to one of these forwards `field_value()` (structs have the
    /// method). Enums and typedef-to-primitive do NOT — mis-forwarding them was
    /// the cause of "no method `field_value` found for f64 / enum" compile
    /// errors. Populated by [`register_types_for_field_value`].
    static STRUCT_NAMES: RefCell<std::collections::BTreeSet<String>> =
        const { RefCell::new(std::collections::BTreeSet::new()) };
    /// Simple names of enum types (terminal integer values in `field_value`).
    static ENUM_NAMES: RefCell<std::collections::BTreeSet<String>> =
        const { RefCell::new(std::collections::BTreeSet::new()) };
    /// Simple typedef name → aliased type, for chasing typedef chains to the
    /// effective leaf/struct/enum in `field_value` codegen.
    static TYPEDEF_MAP: RefCell<std::collections::BTreeMap<String, TypeSpec>> =
        const { RefCell::new(std::collections::BTreeMap::new()) };
    /// Simple const name → its value expression. The Rust backend does not emit
    /// IDL `const` declarations (they are language-specific), so a bound written
    /// as a named constant (`long v[N]`, §7.4.1.4.4.5) must be resolved to its
    /// integer value at codegen time. Populated by
    /// [`register_types_for_field_value`].
    static CONST_VALUES: RefCell<std::collections::BTreeMap<String, ConstExpr>> =
        const { RefCell::new(std::collections::BTreeMap::new()) };
    /// Enum literal simple-name → its discriminant value, matching the value
    /// `enum_emit` assigns (sequential `0..N-1`). Lets a `union switch(EnumT)`
    /// resolve its `case ENUM_LITERAL:` labels to the same integers the
    /// generated enum uses — so the wire discriminator round-trips.
    static ENUM_LITERAL_VALUES: RefCell<std::collections::BTreeMap<String, i128>> =
        const { RefCell::new(std::collections::BTreeMap::new()) };
    /// Simple names of union types in the current run. A union maps to a Rust
    /// `enum` (not `Copy`), so an `array-of-union` member cannot decode via the
    /// `impl CdrDecode for [T; N]` blanket impl (which requires `T: Default +
    /// Copy`). Used by [`scoped_resolves_to_non_copy`] to choose the manual
    /// element-wise array decoder. Populated by [`register_types_for_field_value`].
    static UNION_NAMES: RefCell<std::collections::BTreeSet<String>> =
        const { RefCell::new(std::collections::BTreeSet::new()) };
    /// Simple struct name → its full [`StructDef`], for two codegen decisions
    /// that need to look INTO a nested struct (not just know its name):
    ///   * `@mutable` member LC5-vs-LC4: a nested struct whose extensibility is
    ///     `@appendable`/`@mutable` emits a leading DHEADER (so its EMHEADER can
    ///     reuse it via LC5), whereas a `@final` nested struct has none (LC4).
    ///   * nested-struct `@key` expansion + key-holder max-size: recurse into
    ///     the nested struct's own `@key` members.
    /// Populated by [`register_types_for_field_value`].
    static STRUCT_DEFS: RefCell<std::collections::BTreeMap<String, zerodds_idl::ast::types::StructDef>> =
        const { RefCell::new(std::collections::BTreeMap::new()) };
}

/// Kind of a scoped field type, for `field_value` SQL-filter codegen.
pub enum FieldValueScopedKind {
    /// Nested struct — has `field_value()`; forward the dotted path.
    Struct,
    /// C-like enum — a terminal integer value.
    Enum,
    /// Typedef resolved to a primitive/string — a terminal value of that type.
    Leaf(TypeSpec),
    /// Unresolvable / not directly filterable — caller emits the `_` fallback.
    Unknown,
}

/// Records struct/enum/typedef names from `spec` so `field_value` codegen can
/// resolve scoped field types. Replaces the previous run's data (thread-local,
/// like the other codegen registries). Call once at codegen entry.
/// zerodds-lint: recursion-depth 16
pub fn register_types_for_field_value(spec: &Specification) {
    STRUCT_NAMES.with(|s| s.borrow_mut().clear());
    ENUM_NAMES.with(|s| s.borrow_mut().clear());
    TYPEDEF_MAP.with(|s| s.borrow_mut().clear());
    CONST_VALUES.with(|s| s.borrow_mut().clear());
    ENUM_LITERAL_VALUES.with(|s| s.borrow_mut().clear());
    UNION_NAMES.with(|s| s.borrow_mut().clear());
    STRUCT_DEFS.with(|s| s.borrow_mut().clear());
    /// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
    fn walk(defs: &[Definition]) {
        for def in defs {
            match def {
                Definition::Module(m) => walk(&m.definitions),
                Definition::Const(c) => {
                    CONST_VALUES.with(|m| {
                        m.borrow_mut().insert(c.name.text.clone(), c.value.clone());
                    });
                }
                Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                    STRUCT_NAMES.with(|set| {
                        set.borrow_mut().insert(s.name.text.clone());
                    });
                    STRUCT_DEFS.with(|m| {
                        m.borrow_mut().insert(s.name.text.clone(), s.clone());
                    });
                }
                Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                    ENUM_NAMES.with(|set| {
                        set.borrow_mut().insert(e.name.text.clone());
                    });
                    // Record each literal's value honoring explicit
                    // `@value(N)` (XTypes 1.3 §7.3.1.2.1.6), matching
                    // enum_emit exactly, so union case labels referencing
                    // them resolve to the same wire discriminator. Without
                    // `@value` the value is previous + 1, starting at 0.
                    let mut next: i128 = 0;
                    for en in &e.enumerators {
                        let value =
                            crate::annotations::enumerator_value(&en.annotations).unwrap_or(next);
                        ENUM_LITERAL_VALUES.with(|m| {
                            m.borrow_mut().insert(en.name.text.clone(), value);
                        });
                        next = value + 1;
                    }
                }
                Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(
                    zerodds_idl::ast::types::UnionDcl::Def(u),
                ))) => {
                    UNION_NAMES.with(|set| {
                        set.borrow_mut().insert(u.name.text.clone());
                    });
                }
                Definition::Type(TypeDecl::Typedef(td)) => {
                    for d in &td.declarators {
                        if let Declarator::Simple(n) = d {
                            TYPEDEF_MAP.with(|m| {
                                m.borrow_mut().insert(n.text.clone(), td.type_spec.clone());
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
    walk(&spec.definitions);
}

/// Resolves a scoped field name (chasing typedef chains) to its
/// [`FieldValueScopedKind`].
/// zerodds-lint: recursion-depth 16
#[must_use]
pub fn field_value_scoped_kind(scoped: &ScopedName) -> FieldValueScopedKind {
    let Some(mut name) = scoped.parts.last().map(|i| i.text.clone()) else {
        return FieldValueScopedKind::Unknown;
    };
    // Chase the typedef chain (bounded — legal IDL has no typedef cycles).
    for _ in 0..16 {
        let aliased = TYPEDEF_MAP.with(|m| m.borrow().get(&name).cloned());
        match aliased {
            Some(ts @ (TypeSpec::Primitive(_) | TypeSpec::String(_))) => {
                return FieldValueScopedKind::Leaf(ts);
            }
            Some(TypeSpec::Scoped(s)) => match s.parts.last() {
                Some(next) => name = next.text.clone(),
                None => return FieldValueScopedKind::Unknown,
            },
            // typedef of sequence/array/map/etc. — not a directly filterable leaf
            Some(_) => return FieldValueScopedKind::Unknown,
            // not a typedef — resolve as struct/enum below
            None => break,
        }
    }
    if STRUCT_NAMES.with(|s| s.borrow().contains(&name)) {
        FieldValueScopedKind::Struct
    } else if ENUM_NAMES.with(|s| s.borrow().contains(&name)) {
        FieldValueScopedKind::Enum
    } else {
        FieldValueScopedKind::Unknown
    }
}

/// Resolves a scoped name (chasing typedef chains, bounded) to the simple
/// name of the declared `struct` it ultimately refers to, if any. Returns
/// `None` if the scoped name is an enum/union/typedef-to-non-struct or
/// unresolved.
/// zerodds-lint: recursion-depth 16
fn resolve_scoped_to_struct_name(scoped: &ScopedName) -> Option<String> {
    let mut name = scoped.parts.last().map(|i| i.text.clone())?;
    for _ in 0..16 {
        if STRUCT_DEFS.with(|m| m.borrow().contains_key(&name)) {
            return Some(name);
        }
        // Chase a typedef alias to a struct (typedef MyAlias = SomeStruct;).
        match TYPEDEF_MAP.with(|m| m.borrow().get(&name).cloned()) {
            Some(TypeSpec::Scoped(s)) => name = s.parts.last()?.text.clone(),
            _ => return None,
        }
    }
    None
}

/// Looks up the registered [`StructDef`] for a scoped name (chasing typedef
/// aliases to a struct). Returns `None` for non-struct / unresolved scoped
/// names.
#[must_use]
pub fn struct_def_by_scoped(scoped: &ScopedName) -> Option<zerodds_idl::ast::types::StructDef> {
    let name = resolve_scoped_to_struct_name(scoped)?;
    STRUCT_DEFS.with(|m| m.borrow().get(&name).cloned())
}

/// `true` if a member of type `spec`, serialized under XCDR2, begins its body
/// with a 4-byte length word that an enclosing `@mutable` EMHEADER can REUSE
/// as the NEXTINT (length code LC5, XTypes 1.3 §7.4.3.4.2) — instead of
/// emitting a redundant separate NEXTINT (LC4). This holds when the body
/// starts with a DHEADER or a string/sequence byte-length, i.e.:
///   * a `string` / `wstring` (uint32 octet length prefix),
///   * a `sequence<E>` whose element `E` is **non-primitive** — under XCDR2 it
///     carries a DHEADER (= byte count of `[count + elements]`); a
///     `sequence<primitive>` does NOT (it starts with a bare element count
///     that is not a byte length), so LC4 stays,
///   * a `map<K,V>` — always non-primitive → DHEADER,
///   * a nested struct (or typedef-to-struct) whose extensibility is
///     `@appendable` / `@mutable` — those self-delimit with a leading DHEADER;
///     a `@final` nested struct does NOT (its body is tight-packed) → LC4.
///
/// FINDING T1b: ZeroDDS previously used LC4 for ALL such members, making it the
/// unique byte-size outlier vs CycloneDDS / RTI / FastDDS (which use LC5 for
/// the DHEADER-bearing members). This resolves the member's framing so the
/// EMHEADER matches the vendors byte-for-byte.
/// zerodds-lint: recursion-depth 16
#[must_use]
pub fn member_body_has_leading_dheader(spec: &TypeSpec) -> bool {
    match spec {
        TypeSpec::String(_) => true,
        TypeSpec::Map(_) => true,
        TypeSpec::Sequence(seq) => !typespec_is_cdr_primitive(&seq.elem),
        TypeSpec::Scoped(s) => {
            // A typedef-to-string/seq/map inherits the alias' framing.
            if let Some(inner) = resolve_typedef_to_spec(s) {
                return member_body_has_leading_dheader(&inner);
            }
            // A nested struct: leading DHEADER iff @appendable / @mutable.
            struct_def_by_scoped(s)
                .map(|sd| {
                    !matches!(
                        crate::annotations::struct_extensibility(&sd.annotations),
                        crate::annotations::StructExtensibility::Final
                    )
                })
                .unwrap_or(false)
        }
        _ => false,
    }
}

/// Resolves a scoped name (chasing typedef chains) to the effective
/// non-aliased `TypeSpec`, if the alias terminates in a concrete spec.
/// Returns `None` when the chain ends at a declared struct/union/enum (i.e.
/// the name is itself the terminal type) or cannot be resolved.
/// zerodds-lint: recursion-depth 16
fn resolve_typedef_to_spec(scoped: &ScopedName) -> Option<TypeSpec> {
    let mut name = scoped.parts.last().map(|i| i.text.clone())?;
    let mut last_spec: Option<TypeSpec> = None;
    for _ in 0..16 {
        let aliased = TYPEDEF_MAP.with(|m| m.borrow().get(&name).cloned());
        match aliased {
            Some(TypeSpec::Scoped(s)) => {
                last_spec = Some(TypeSpec::Scoped(s.clone()));
                match s.parts.last() {
                    Some(next) => name = next.text.clone(),
                    None => break,
                }
            }
            Some(other) => return Some(other),
            None => break,
        }
    }
    last_spec
}

/// `true` if `spec` is a CDR *primitive* element in the sense of
/// `zerodds_cdr::CdrEncode::IS_PRIMITIVE` (numeric / char / bool / octet) —
/// chasing typedef aliases to a primitive. Strings, sequences, structs,
/// unions, maps and `any` are NOT primitive. This MUST agree with the cdr
/// crate's `IS_PRIMITIVE` because it drives the array DHEADER decision
/// (`needs_collection_dheader`) the manual array decoder mirrors.
#[must_use]
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
pub fn typespec_is_cdr_primitive(spec: &TypeSpec) -> bool {
    match spec {
        TypeSpec::Primitive(_) => true,
        TypeSpec::Scoped(s) => {
            // An enum maps to a Rust enum whose CdrEncode is NOT primitive
            // (IS_PRIMITIVE defaults to false). A typedef-to-primitive IS.
            if let Some(inner) = resolve_typedef_to_spec(s) {
                typespec_is_cdr_primitive(&inner)
            } else {
                false
            }
        }
        _ => false,
    }
}

/// `true` if an array whose element `spec` is this type must be decoded
/// element-by-element rather than via the `impl CdrDecode for [T; N]`
/// blanket impl. That impl requires `T: Default + Copy`; any non-`Copy`
/// element type cannot use it — a generated struct or union (Rust `enum`,
/// `Default` but not `Copy`), a `String` / `WString` (`string names[3]`,
/// owns a heap buffer), and a `sequence<T>` (`Vec<T>`) or `map<K,V>`
/// (`BTreeMap`) array element all need the manual path.
/// Chases typedef chains. Primitives and C-like enums stay on the blanket
/// impl (they are `Copy`). The manual decoder mirrors the blanket impl's
/// XCDR2 DHEADER decision byte-for-byte, which is gated on
/// `T::IS_PRIMITIVE` — false for every type above — so it is correct for
/// all of them.
#[must_use]
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
pub fn array_elem_needs_manual_decode(spec: &TypeSpec) -> bool {
    match spec {
        TypeSpec::Scoped(s) => {
            // Follow typedef aliases first (a typedef to string/seq/struct
            // inherits the alias' Copy-ness).
            if let Some(inner) = resolve_typedef_to_spec(s) {
                return array_elem_needs_manual_decode(&inner);
            }
            let Some(name) = s.parts.last().map(|i| i.text.clone()) else {
                return false;
            };
            STRUCT_NAMES.with(|set| set.borrow().contains(&name))
                || UNION_NAMES.with(|set| set.borrow().contains(&name))
        }
        // Owned, non-`Copy` element types: String/WString, Vec (sequence),
        // BTreeMap (map). All have `IS_PRIMITIVE == false`, so the blanket
        // array impl would prepend a DHEADER under XCDR2 — which the manual
        // decoder also skips — but the blanket impl can't be instantiated
        // (no `Copy`), so we must emit element-wise decode.
        TypeSpec::String(_) | TypeSpec::Sequence(_) | TypeSpec::Map(_) => true,
        _ => false,
    }
}

/// Maps an IDL type spec to a Rust type expression (e.g. `i32`,
/// `Vec<f64>`, `[u8; 16]`).
///
/// `field_size_hint` is `None` for "a type without a fixed-size bound"
/// (Vec, String, ...) and `Some(bytes)` for fixed-size types — passed on
/// by the caller for the KEY_HOLDER_MAX_SIZE computation.
///
/// # Errors
/// `Unsupported` if the IDL construct lies outside the DDS DataType
/// scope (Fixed, Map, Any).
/// zerodds-lint: recursion-depth 8
/// IDL sequence nesting — 8 levels are enough for realistic use cases
/// (sequence<sequence<sequence<...>>>).
pub fn rust_type_for(spec: &TypeSpec) -> Result<String> {
    match spec {
        TypeSpec::Primitive(p) => Ok(rust_primitive(*p).to_string()),
        TypeSpec::Scoped(s) => Ok(rust_scoped(s)),
        TypeSpec::Sequence(seq) => rust_sequence(seq),
        TypeSpec::String(s) => Ok(rust_string(s)),
        TypeSpec::Fixed(f) => {
            let p = const_expr_as_usize(&f.digits).ok_or(RustGenError::InvalidAnnotation {
                name: "fixed-digits".to_string(),
                reason: "non-integer P",
            })?;
            let s = const_expr_as_usize(&f.scale).ok_or(RustGenError::InvalidAnnotation {
                name: "fixed-scale".to_string(),
                reason: "non-integer S",
            })?;
            Ok(format!("zerodds_cdr::fixed::Fixed<{p}, {s}>"))
        }
        TypeSpec::Map(m) => rust_map(m),
        // `any`: DDS → DdsAny (XCDR2), CORBA → CorbaAny (classic CDR).
        TypeSpec::Any => Ok(any_rust_type().to_string()),
    }
}

/// zerodds-lint: recursion-depth 8
/// IDL map nesting — 8 levels are enough for realistic use cases.
fn rust_map(m: &zerodds_idl::ast::types::MapType) -> Result<String> {
    let key = rust_type_for(m.key.as_ref())?;
    let value = rust_type_for(m.value.as_ref())?;
    Ok(format!("::std::collections::BTreeMap<{key}, {value}>"))
}

/// IDL-Primitive → Rust-Primitive.
#[must_use]
pub fn rust_primitive(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Integer(i) => rust_integer(i),
        PrimitiveType::Floating(f) => rust_floating(f),
        // IDL char = 8-bit (1 byte), wchar = UTF-16 code unit (2 byte LE) —
        // consistent with zerodds-xcdr2-rust-1.0 §123, the TypeIdentifier
        // (Char8/Char16) and idl-cpp/-java/-csharp/-c. Rust `char` (4 byte
        // Unicode scalar) is NOT a correct mapping — neither for classic
        // CORBA CDR (§9.3.1.5) nor for XCDR2 — and breaks wire interop.
        PrimitiveType::Char => "u8",
        // `wchar`: DDS XCDR2 = bare 2-byte UTF-16 unit (`u16`,
        // cross-vendor-validated); CORBA/GIOP 1.2 = a length-prefixed form
        // (1-octet length + UTF-16 unit big-endian, §15.3.1.6), modelled by
        // the `zerodds_cdr::WChar` newtype with its own CdrEncode/CdrDecode.
        // Gated on the CORBA codegen signal so the DDS path stays `u16`.
        PrimitiveType::WideChar => {
            if any_target_corba() {
                "zerodds_cdr::WChar"
            } else {
                "u16"
            }
        }
        PrimitiveType::Boolean => "bool",
        PrimitiveType::Octet => "u8",
    }
}

/// Wire size of an IDL primitive in bytes (0 if not fixed-size).
#[must_use]
pub fn primitive_wire_size(p: PrimitiveType) -> usize {
    match p {
        PrimitiveType::Integer(i) => integer_wire_size(i),
        PrimitiveType::Floating(FloatingType::Float) => 4,
        PrimitiveType::Floating(FloatingType::Double) => 8,
        PrimitiveType::Floating(FloatingType::LongDouble) => 16,
        PrimitiveType::Char => 1,
        PrimitiveType::WideChar => 2,
        PrimitiveType::Boolean => 1,
        PrimitiveType::Octet => 1,
    }
}

fn rust_integer(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "i16",
        IntegerType::Long | IntegerType::Int32 => "i32",
        IntegerType::LongLong | IntegerType::Int64 => "i64",
        IntegerType::UShort | IntegerType::UInt16 => "u16",
        IntegerType::ULong | IntegerType::UInt32 => "u32",
        IntegerType::ULongLong | IntegerType::UInt64 => "u64",
        IntegerType::Int8 => "i8",
        IntegerType::UInt8 => "u8",
    }
}

fn integer_wire_size(i: IntegerType) -> usize {
    match i {
        IntegerType::Int8 | IntegerType::UInt8 => 1,
        IntegerType::Short | IntegerType::Int16 | IntegerType::UShort | IntegerType::UInt16 => 2,
        IntegerType::Long | IntegerType::Int32 | IntegerType::ULong | IntegerType::UInt32 => 4,
        IntegerType::LongLong
        | IntegerType::Int64
        | IntegerType::ULongLong
        | IntegerType::UInt64 => 8,
    }
}

fn rust_floating(f: FloatingType) -> &'static str {
    match f {
        FloatingType::Float => "f32",
        FloatingType::Double => "f64",
        FloatingType::LongDouble => "f64", // Rust has no f128, fallback
    }
}

fn rust_scoped(s: &ScopedName) -> String {
    // `Object` is a reserved CORBA pseudo-type (no user-declared
    // definition) that the parser models as a scoped name (§7.4.6.3). In
    // the CORBA codegen it is the object reference; an IOR on the wire
    // (zerodds_corba_rust::ObjectReference impl CdrEncode as IOR §15.3.3).
    if s.parts.len() == 1 && s.parts[0].text == "Object" {
        return "zerodds_corba_rust::ObjectReference".to_string();
    }
    // A reference to a declared interface → object reference (IOR), not the
    // trait itself (otherwise a dyn cycle, e.g. `NamingContext new_context()`).
    if let Some(last) = s.parts.last() {
        if is_interface_ref(&last.text) {
            return "zerodds_corba_rust::ObjectReference".to_string();
        }
    }
    s.parts
        .iter()
        .map(|p| escape_keyword(&p.text))
        .collect::<Vec<_>>()
        .join("::")
}

/// Wraps an IDL identifier in Rust raw-identifier form `r#…` if it is a
/// Rust reserved keyword. Spec-ref: zerodds-idl-rust-1.0 §6.2.
#[must_use]
pub fn escape_keyword(ident: &str) -> String {
    if is_rust_keyword(ident) {
        format!("r#{ident}")
    } else {
        ident.to_string()
    }
}

/// List of Rust 2024 edition reserved keywords. Source:
/// `https://doc.rust-lang.org/reference/keywords.html` —
/// Strict-Keywords + reservierte Future-Keywords.
#[must_use]
pub fn is_rust_keyword(s: &str) -> bool {
    matches!(
        s,
        // Strict keywords
        "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern"
        | "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match"
        | "mod" | "move" | "mut" | "pub" | "ref" | "return" | "self" | "Self" | "static"
        | "struct" | "super" | "trait" | "true" | "type" | "unsafe" | "use" | "where"
        | "while"
        // 2018+ keywords
        | "async" | "await" | "dyn"
        // 2024+ keywords
        | "gen"
        // Reserved keywords
        | "abstract" | "become" | "box" | "do" | "final" | "macro" | "override"
        | "priv" | "typeof" | "unsized" | "virtual" | "yield" | "try"
        // Underscore is technically reserved
        | "_"
    )
}

/// zerodds-lint: recursion-depth 8
fn rust_sequence(seq: &SequenceType) -> Result<String> {
    let elem = rust_type_for(&seq.elem)?;
    Ok(format!("Vec<{elem}>"))
}

fn rust_string(s: &StringType) -> String {
    if s.wide {
        // wstring → distinct `WString` wrapper: GIOP-1.2 wire = uint32
        // length-in-octets + UTF-16 code units (message byte order), no
        // terminator. NOT `String` (UTF-8) — otherwise wstring would be
        // indistinguishable from string on the wire and incompatible with foreign ORBs.
        "zerodds_cdr::WString".to_string()
    } else {
        "String".to_string()
    }
}

/// Computes the maximum wire-size bound for an IDL type spec, used solely
/// for the `@key` `KEY_HOLDER_MAX_SIZE` computation (DDSI-RTPS §9.6.3.8:
/// a key whose MAX serialized size ≤ 16 takes the zero-pad branch, else
/// MD5). Returns `None` for dynamically-sized types (unbounded string,
/// sequence, scoped, fixed, map, any) → MD5.
///
/// A **bounded** `string<N>` / `wstring<N>` HAS a known max size and must
/// therefore use the pad branch when it fits — matching CycloneDDS / RTI /
/// FastDDS. Returning `None` for them (the old behaviour) forced the MD5
/// path and made keyed instances fail to correlate cross-vendor.
#[must_use]
pub fn wire_size_bound(spec: &TypeSpec) -> Option<usize> {
    match spec {
        TypeSpec::Primitive(p) => Some(primitive_wire_size(*p)),
        TypeSpec::String(s) => {
            // Bounded only; unbounded (bound == None) stays dynamic → MD5.
            // Narrow: uint32 length + N chars + NUL = 4 + N + 1.
            // Wide (wstring): uint32 length + N UTF-16 units (2 B) = 4 + 2N.
            let n = s.bound.as_ref().and_then(const_expr_as_usize)?;
            if s.wide {
                Some(4 + 2 * n)
            } else {
                Some(4 + n + 1)
            }
        }
        TypeSpec::Sequence(_) | TypeSpec::Scoped(_) => None,
        TypeSpec::Fixed(_) | TypeSpec::Map(_) | TypeSpec::Any => None,
    }
}

/// Evaluates a `ConstExpr` as `usize` (only for simple integer
/// literals — array sizes, sequence bounds, string bounds).
#[must_use]
pub fn const_expr_as_usize(expr: &ConstExpr) -> Option<usize> {
    let v = eval_const_i128(expr, 0)?;
    usize::try_from(v).ok()
}

/// Evaluates a constant expression to a signed integer, resolving named consts
/// and enum literals and folding arithmetic. Used for union `case` labels
/// (which may be negative and may reference enum literals or named consts).
#[must_use]
pub fn const_expr_as_i128(expr: &ConstExpr) -> Option<i128> {
    eval_const_i128(expr, 0)
}

/// Evaluates a constant expression to an integer, resolving named/scoped
/// constants through [`CONST_VALUES`] and folding arithmetic/bitwise operators
/// (§7.4.1.4.4 const_expr). `depth` bounds const-reference chains so a
/// (malformed) self-referential const cannot loop forever.
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn eval_const_i128(expr: &ConstExpr, depth: u32) -> Option<i128> {
    use zerodds_idl::ast::types::{BinaryOp, ConstExpr as CE, LiteralKind, UnaryOp};
    if depth > 32 {
        return None;
    }
    match expr {
        CE::Literal(lit) if lit.kind == LiteralKind::Integer => {
            parse_integer_literal(&lit.raw).map(|u| u as i128)
        }
        CE::Literal(lit) if lit.kind == LiteralKind::Boolean => {
            Some(i128::from(lit.raw == "TRUE" || lit.raw == "true"))
        }
        CE::Literal(_) => None,
        // A named constant or enum literal: resolve to its value and evaluate.
        CE::Scoped(s) => {
            let name = s.parts.last()?.text.clone();
            if let Some(v) = ENUM_LITERAL_VALUES.with(|m| m.borrow().get(&name).copied()) {
                return Some(v);
            }
            let value = CONST_VALUES.with(|m| m.borrow().get(&name).cloned())?;
            eval_const_i128(&value, depth + 1)
        }
        CE::Unary { op, operand, .. } => {
            let v = eval_const_i128(operand, depth + 1)?;
            Some(match op {
                UnaryOp::Plus => v,
                UnaryOp::Minus => v.checked_neg()?,
                UnaryOp::BitNot => !v,
            })
        }
        CE::Binary { op, lhs, rhs, .. } => {
            let a = eval_const_i128(lhs, depth + 1)?;
            let b = eval_const_i128(rhs, depth + 1)?;
            match op {
                BinaryOp::Or => Some(a | b),
                BinaryOp::Xor => Some(a ^ b),
                BinaryOp::And => Some(a & b),
                BinaryOp::Shl => u32::try_from(b).ok().map(|s| a << s),
                BinaryOp::Shr => u32::try_from(b).ok().map(|s| a >> s),
                BinaryOp::Add => a.checked_add(b),
                BinaryOp::Sub => a.checked_sub(b),
                BinaryOp::Mul => a.checked_mul(b),
                BinaryOp::Div => a.checked_div(b),
                BinaryOp::Mod => a.checked_rem(b),
            }
        }
    }
}

fn parse_integer_literal(raw: &str) -> Option<usize> {
    let trimmed = raw.trim_end_matches(['u', 'U', 'l', 'L']);
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        usize::from_str_radix(hex, 16).ok()
    } else if let Some(oct) = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
    {
        usize::from_str_radix(oct, 8).ok()
    } else if let Some(bin) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
    {
        usize::from_str_radix(bin, 2).ok()
    } else {
        trimmed.parse::<usize>().ok()
    }
}

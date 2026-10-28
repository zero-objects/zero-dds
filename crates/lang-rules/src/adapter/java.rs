// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::c`], which does the same for C,
//! and of [`crate::adapter::cst`], which does it for Rust. Reading two of
//! them side by side is the point: where they differ, the difference is
//! the language's, and it is written down here rather than spread through
//! a renderer.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the other two materialisers follow and for the same reason
//! (spec §2.3).
//!
//! ## The four things Java forces that C does not
//!
//! 1. **No unsigned integer.** Every Java integer is signed, so the width
//!    of the IDL type cannot decide the Java type. What decides it is what
//!    the field has to *hold*: `unsigned short` takes an `int` because
//!    65535 is not a `short`, and `unsigned long` takes a `long` because
//!    4294967295 is not an `int`. Only `unsigned long long` has no type
//!    wide enough, and it takes a `long` holding the **bit pattern**. The
//!    table below says which of the two every unsigned type is.
//! 2. **Arrays instead of declared collection types.** C has to declare a
//!    named struct for `sequence<long>`; Java writes `int[]`. So there is
//!    no structural-name machinery here and no forward declaration — a
//!    Java class may name a class declared after it — but
//!    [`ordered_types`] is still what the walk uses, so two runs over one
//!    model produce the same file.
//! 3. **One public top-level type per file.** Everything nests inside one
//!    outer class; see "Modules" below.
//! 4. **No typedef.** An IDL `typedef` has no Java declaration at all. It
//!    is not dropped: the member takes the target's Java type and the
//!    outer class carries a comment naming the alias and what it resolved
//!    to.
//!
//! ## The integer mapping
//!
//! | IDL | Java | what the field holds |
//! |---|---|---|
//! | `boolean` | `boolean` | |
//! | `char` | `char` | one code unit; the wire form is one octet |
//! | `wchar` | `char` | one UTF-16 code unit |
//! | `int8` | `byte` | a number, -128..127 |
//! | `octet` | `byte` | a bit pattern, which the codec writes as the number 0..255 |
//! | `uint8` | `int` | a **number**, 0..255 |
//! | `short` | `short` | a number |
//! | `unsigned short` | `int` | a **number**, 0..65535 |
//! | `long` | `int` | a number |
//! | `unsigned long` | `long` | a **number**, 0..4294967295 |
//! | `long long` | `long` | a number |
//! | `unsigned long long` | `long` | a **bit pattern**, not a number |
//! | `float` | `float` | |
//! | `double` | `double` | |
//! | `long double` | `org.zerodds.wire.Float128` | never a `double` |
//!
//! The unsigned types, stated once more because this is the decision
//! that reaches the wire: `uint8` and `unsigned short` are numbers in an
//! `int`; `unsigned long` is a number in a `long`; `unsigned long long`
//! is a pattern in a `long`, and `writeU64(-1L)` writes
//! `ffffffffffffffff`, which is 18446744073709551615. `octet` is the
//! exception, and follows the OMG mapping.
//!
//! This is not a choice made here alone. `runtime/java/.../Writer.java`
//! had to make it first, for its own arguments: `writeU8(int value)`
//! says "a *number* in [0, 255], not a pattern: the argument is an `int`
//! because Java's `byte` is signed, and 200 arriving as -56 would be a
//! caller who has already lost the value"; `writeU32(long value)` says
//! the same of 3735928559; `writeU64(long pattern)` says the opposite,
//! because there is no wider type. The generated code calls those
//! methods, so a field typed against a different rule would either not
//! compile or arrive already wrong.
//!
//! **`octet` is a `byte`.** CORBA 3.3 part 3 §1.5.2 maps `octet` to `byte`,
//! the JacORB column measured in `crates/wire-probe/src/corba_java.rs`
//! does the same, and DDS-XTypes 1.3 §7.5.1 reuses that mapping for the
//! Plain Language Binding. A signed `byte` holds 200 as -56; the codec
//! says so at both ends: `Writer.writeU8(byte)` writes the number the
//! eight bits are and the reader's `(byte) readU8()` keeps them, so the
//! wire form is unchanged. `uint8` has no OMG mapping and stays a number
//! in an `int`.
//!
//! ## The shapes
//!
//! | IDL | Java |
//! |---|---|
//! | `string`, `wstring` | `java.lang.String` |
//! | `fixed<P,S>` | `java.math.BigDecimal` |
//! | `sequence<T>`, `T[N]` | `T[]` |
//! | `T[N][M]` | `T[][]` |
//! | `map<K,V>` | `ZdMap_k_v`, a `java.util.LinkedHashMap` of the boxed types (`ZdEntry_k_v[]` for a key Java compares by identity) |
//! | `union` | a class with `_d` and one field per case |
//! | `enum` | a class with one instance per enumerator, `value()` and `from_int` (CORBA 3.3 part 3 §1.5.9) |
//! | `bitmask` | the unsigned holder of its bound, beside constants |
//! | `bitset` | a class of one field per bitfield |
//! | `@optional T m` | the boxed `T`, `null` when absent |
//! | `@external T m` | the boxed `T` for a primitive, the reference otherwise |
//! | `typedef` | nothing; a comment, and the target's type at each use |
//!
//! ### `fixed<P,S>` is a `BigDecimal`
//!
//! §1.5.6 of the OMG mapping says `java.math.BigDecimal`, and the JacORB
//! column follows it. So does this one. The digits a fixed-point value
//! carries on the wire come from the type (`fixed<P,S>`), and a
//! `BigDecimal` keeps its scale, so `new BigDecimal("0.00")` writes the
//! digits `0.00` has; `Writer.writeFixed` takes the decimal text, which
//! the codec gets from `toPlainString()`, and `Reader.readFixed` hands
//! back text the codec turns into a `BigDecimal`. What a `BigDecimal` does
//! not keep is leading zeroes (`precision()` of `0.00` is 1), so the
//! `FixedDigitCount::ValueStringDigits` axis, which counts the digits of
//! the text a caller wrote, sees the normalised text.
//!
//! ### The collections
//!
//! A `sequence<T>` and an IDL array are both a Java `T[]`, which is what
//! §1.5.7 does and what the JacORB column records ("a Java array — so a
//! sequence is allocated, not lengthened"). A multi-dimensional array is
//! a nested Java array, one `[]` per dimension.
//!
//! A `map<K,V>` is a `java.util.Map` (XTypes 1.3 §7.5.1.3.5): a generated
//! class extending `LinkedHashMap` of the boxed key and value types, so
//! that the order the entries were put in is the order they go on the wire
//! in; `MapEntryOrder` sorts them when asked to. The codec reads and
//! writes the entries in the generated entry class, in order
//! (`ZdMap….entries()` and `ZdMap….of(entries)`), and the class carries
//! the operations of Table 47 that Java does not have. A key a
//! `java.util.Map` would compare by identity (a structure, a sequence)
//! keeps the array of entries.
//!
//! ### `@optional` is a boxed type and `null`
//!
//! One field, not a field beside a `boolean`. A `boolean _has_m` and an
//! `m` are two things that can disagree, and nothing in the type system
//! stops them; `null` cannot disagree with itself. `@optional long m`
//! therefore takes `java.lang.Long`, because a primitive `long` has no
//! value that means absent — which is the case that decides this, and the
//! reason C needs the parallel flag it has.
//!
//! ### An enumeration is a class of instances
//!
//! CORBA 3.3 part 3 §1.5.9, which DDS-XTypes 1.3 §7.5.1 reuses: a final
//! class with one `public static final` instance per enumerator, the
//! `int` constants `_NAME`, `value()` and `from_int`. A member of the
//! type is an instance of the class and the codec writes `value()`.
//!
//! The JacORB column records the same shape. `from_int` refuses a value
//! no enumerator has; the codec never meets that, because it reads a
//! value through the chain of enumerators and applies the position's
//! try-construct behaviour to one it does not know (Table 18), and
//! assigns the instance of the enumerator it found.
//!
//! ### Modules are nested classes, in one file
//!
//! Not packages. Two facts decide it. Java requires the public top-level
//! type of a file to be named after the file, so real packages mean many
//! files; and this function returns `Vec<java::TypeDecl>` with no file
//! boundary in it, so a package-per-module mapping could not be expressed
//! by the signature at all — the package would be lost between here and
//! the renderer, and `module a { struct P; }` and `module b { struct P; }`
//! would collide in one scope. That collision is the defect
//! [`MaterialiseError::NoModule`] was added for in the Rust column.
//!
//! The consumer decides it the same way: `tools/wireprobe` compiles one
//! probe per corpus case, and `runtime/java/run-tests.sh` invokes
//! `javac --release 8 -Xlint:all,-options -Werror` over a directory of
//! sources. One generated file with one outer class is one `javac`
//! argument; it also keeps `-Xlint:auxiliaryclass` quiet, which a file of
//! several top-level classes would not.
//!
//! So everything nests inside [`OUTER`], each IDL module becomes a
//! `public static final class` of that name, and every reference to a
//! generated type is written from [`OUTER`] down — `ZdTypes.a.P`. The
//! qualification is not decoration: JLS §6.5.2 resolves the first
//! segment of a qualified name to a *variable* before a type, so a field
//! named `common` in a class that also names the type `common.util.Point`
//! would be a compile error that only some corpora would show.
//!
//! ### Identifiers
//!
//! Through [`zerodds_lang_cst::java::escape_keyword`], which appends an
//! underscore: the module `package` becomes the class `package_` and the
//! enumeration `class` becomes `class_`. `crates/wire-probe/src/
//! target_keywords.rs` prefixes one instead — `_package`, `_class` —
//! because that is what JacORB was measured to emit over
//! `tests/interop-corpus/identifier-target-keywords`. Both avoid the
//! keyword and neither reaches the wire; this crate does not depend on
//! `zerodds-wire-probe`, and the one CST spelling keeps the four
//! generated columns' identifiers consistent with each other rather than
//! with one ORB. Only a Java keyword moves, so the enumerators
//! `template`, `virtual` and `friend` keep their spelling here as they do
//! there.
//!
//! ## What is refused
//!
//! `any`, `ValueBase` and an object reference have no Java data form
//! here and are named as [`MaterialiseError::NoLanguageForm`] rather than
//! left out of the class. A generator that leaves a member out emits a
//! type missing a field and reports success, which is the defect class
//! this whole file is arranged against.

use crate::adapter::c::ordered_types;
use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, CharWidth, ModuleNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::java;

/// The class every generated declaration nests inside.
///
/// One public top-level type per file is a Java rule, and this is that
/// type. A generated IDL type named the same is refused rather than
/// nested inside it: JLS §8.1 forbids a class sharing the simple name of
/// an enclosing one, and the file would not compile.
pub const OUTER: &str = "ZdTypes";

/// The runtime class an IDL `long double` takes.
///
/// `org.zerodds.wire.Float128` and never a `double`: Java has no
/// 128-bit floating type, and a `double` member would put eight bytes on
/// the wire where the type says sixteen. It is the one primitive the OMG
/// mapping has no answer for — §1.5.2 maps the others and stops — which
/// is why `crates/wire-probe/src/corba_java.rs` refuses a case carrying
/// one rather than approximating it.
const FLOAT128: &[&str] = &["org", "zerodds", "wire", "Float128"];

/// `java.lang.String`, which four IDL types share.
const STRING: &[&str] = &["java", "lang", "String"];

/// `fixed<P,S>` is a `java.math.BigDecimal` (CORBA 3.3 part 3 §1.5.6).
const BIG_DECIMAL: &[&str] = &["java", "math", "BigDecimal"];

/// The interface an unbounded sequence is under [`JavaMapping::Psm`].
const COLLECTION: &[&str] = &["java", "util", "Collection"];

/// What a new unbounded sequence is under [`JavaMapping::Psm`].
pub const ARRAY_LIST: &[&str] = &["java", "util", "ArrayList"];

/// The prefix a generated map-entry class carries.
const ENTRY_PREFIX: &str = "ZdEntry_";
/// The prefix a generated map class carries.
pub const MAP_PREFIX: &str = "ZdMap";

/// The top-level names a generated declaration may not take.
///
/// Every reference this materialiser writes is qualified from [`OUTER`],
/// `java.lang` or `org.zerodds.wire`, and JLS §6.5.2 resolves the first
/// segment against the enclosing scopes first. A nested class named
/// `java` would therefore capture `java.lang.String`.
const RESERVED_TOP_LEVEL: &[&str] = &[OUTER, "java", "org"];

/// The shape generated Java types take.
///
/// The default of `idlc generate --lang java` is [`JavaMapping::Psm`], Clause 9
/// of DDS-Java-PSM 1.0, "Improved Plain Language Binding for Java"; the
/// IDL-to-Java mapping of the OMG ([`JavaMapping::Omg`], public fields and
/// `T[]` for every sequence) is the named option `--java-mapping omg`.
/// [`JavaMapping::Psm`] changes two things against it.
///
/// 1. §9.1.1: "DDS aggregation types shall be mapped to a final Java
///    class. Contained attributes shall be encapsulated. Java Bean style
///    accessors shall be provided." A structure, a union (its
///    discriminator is the property `discriminator`), a bit set and an
///    exception become a `public static final class` with private state,
///    a no-argument constructor and `getX()`/`setX(v)` for each member; a
///    `boolean` member also has `isX()`. The entry class of a `map` is
///    not an IDL aggregate and stays a two-field holder.
/// 2. §9.1.2: "Unbounded DDS sequences are mapped to `Collection<E>`
///    interface. [...] Bounded sequences and arrays are mapped to Java
///    arrays." An unbounded `sequence<T>` is a
///    `java.util.Collection<E>` with `E` the boxed type of `T`.
///
/// Nothing else moves: the integer table, `string`, `fixed`, enumerations,
/// bitmasks, `map` and the optional member are as they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JavaMapping {
    /// IDL-to-Java: public fields, `T[]` for every sequence.
    Omg,
    /// DDS-Java-PSM 1.0 Clause 9.
    #[default]
    Psm,
}

impl JavaMapping {
    /// The name the command line gives it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Omg => "omg",
            Self::Psm => "psm",
        }
    }

    /// The mapping a command-line name selects.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "omg" => Some(Self::Omg),
            "psm" => Some(Self::Psm),
            _ => None,
        }
    }
}

std::thread_local! {
    static MAPPING: std::cell::Cell<JavaMapping> =
        const { std::cell::Cell::new(JavaMapping::Psm) };
}

/// The mapping in force on this thread.
///
/// The materialiser, the codec target and the defaulters are free
/// functions and associated functions with no state to carry a choice in
/// (the same code serves a dozen other columns), so the choice is the
/// thread's for the duration of [`with_mapping`] and [`JavaMapping::Psm`],
/// the default of `idlc`, outside it: a caller that needs the IDL-to-Java
/// shape asks for it with `with_mapping(JavaMapping::Omg, ...)`.
/// Generation is synchronous on one thread; two threads
/// generating with different mappings do not see each other.
#[must_use]
pub fn mapping() -> JavaMapping {
    MAPPING.with(std::cell::Cell::get)
}

/// Run `body` with `mapping` in force, and restore what was before — also
/// when `body` panics.
pub fn with_mapping<R>(mapping: JavaMapping, body: impl FnOnce() -> R) -> R {
    struct Restore(JavaMapping);
    impl Drop for Restore {
        fn drop(&mut self) {
            MAPPING.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(MAPPING.with(|cell| cell.replace(mapping)));
    body()
}

/// `getX` for the member `field`, which is the escaped Java name.
///
/// The first letter is capitalised and nothing else changes, so the
/// accessor of `a_b` is `getA_b`. A union's discriminator `_d` is the
/// property `discriminator`. `getClass` is `Object`'s final method, so a
/// member `Class` takes `getClass_`.
#[must_use]
pub fn getter_name(field: &str) -> String {
    format!("get{}", property(field))
}

/// `setX` for the member `field`; see [`getter_name`].
#[must_use]
pub fn setter_name(field: &str) -> String {
    format!("set{}", property(field))
}

/// `isX` for a `boolean` member `field`; see [`getter_name`].
#[must_use]
pub fn boolean_getter_name(field: &str) -> String {
    format!("is{}", property(field))
}

fn property(field: &str) -> String {
    if field == "_d" {
        return "Discriminator".to_string();
    }
    let mut chars = field.chars();
    let mut out = String::new();
    if let Some(first) = chars.next() {
        out.extend(first.to_uppercase());
    }
    out.push_str(chars.as_str());
    if out == "Class" {
        out.push('_');
    }
    out
}

/// The runtime class the Psm mapping's codec calls for the one thing it
/// cannot decide by type: indexing, counting, allocating and filling a
/// sequence that may be an array or a `Collection`. Overload resolution
/// does it, and the class is `org.zerodds.wire.Sequences`.
pub const SEQUENCES: &str = "Sequences";

fn is_sequences(receiver: &java::Expr) -> bool {
    matches!(receiver, java::Expr::Path(path)
        if matches!(path.segments(), [only] if only.as_str() == SEQUENCES))
}

fn sequences_call(name: &str, args: Vec<java::Expr>) -> java::Expr {
    java::Expr::MethodCall {
        receiver: Some(Box::new(java::Expr::Path(Path::new(&[SEQUENCES])))),
        name: Ident::new(name),
        args,
    }
}

/// `Sequences.view(v)`: marks the expression the codec indexes into, so
/// that [`index`] can tell a sequence from a nested array.
#[must_use]
pub fn sequence_view(value: java::Expr) -> java::Expr {
    sequences_call("view", vec![value])
}

/// `a[i]` for an array, and `Sequences.at(v, i)` for the sequence the
/// codec marked with [`sequence_view`].
#[must_use]
pub fn index(value: java::Expr, at: java::Expr) -> java::Expr {
    if let java::Expr::MethodCall {
        receiver: Some(receiver),
        name,
        args,
    } = &value
        && name.as_str() == "view"
        && is_sequences(receiver)
        && let [inner] = args.as_slice()
    {
        return sequences_call("at", vec![inner.clone(), at]);
    }
    java::Expr::Index {
        value: Box::new(value),
        index: Box::new(at),
    }
}

/// `Sequences.count(v)`: the length of an array or a `Collection`.
#[must_use]
pub fn sequences_count(value: java::Expr) -> java::Expr {
    sequences_call("count", vec![value])
}

/// `Sequences.first(v, n)`: the first `n` elements, in a container of the
/// kind `v` is.
#[must_use]
pub fn sequences_first(value: java::Expr, count: java::Expr) -> java::Expr {
    sequences_call("first", vec![value, count])
}

/// `Sequences.alloc(v, n)`: a new container of the kind `v` is, with room
/// for `n` elements.
#[must_use]
pub fn sequences_alloc(value: java::Expr, count: java::Expr) -> java::Expr {
    sequences_call("alloc", vec![value, count])
}

/// The place an element of a sequence is written into: a pseudo
/// l-value that [`assign`] turns into `Sequences.put(v, i, value)`.
#[must_use]
pub fn sequence_slot(sequence: java::Expr, at: java::Expr) -> java::Expr {
    sequences_call("slot", vec![sequence, at])
}

/// `base.getX()`: the read of a member under [`JavaMapping::Psm`], and
/// the pseudo l-value that [`assign`] turns into `base.setX(value)`.
#[must_use]
pub fn accessor(base: java::Expr, field: &str) -> java::Expr {
    java::Expr::MethodCall {
        receiver: Some(Box::new(base)),
        name: Ident::new(&getter_name(field)),
        args: Vec::new(),
    }
}

/// `place = value;`, in the form the mapping in force can write.
///
/// Under [`JavaMapping::Omg`] this is the assignment. Under
/// [`JavaMapping::Psm`] a `place` made by [`accessor`] is the setter call
/// and a `place` made by [`sequence_slot`] is `Sequences.put`; every
/// other place (a local, an element of an array, the field of a map
/// entry) is assigned as before.
#[must_use]
pub fn assign(place: java::Expr, value: java::Expr) -> java::Statement {
    if mapping() == JavaMapping::Psm {
        if let java::Expr::MethodCall {
            receiver: Some(receiver),
            name,
            args,
        } = &place
        {
            if args.is_empty() && name.as_str().starts_with("get") {
                return java::Statement::Expr(java::Expr::MethodCall {
                    receiver: Some(receiver.clone()),
                    name: Ident::new(&format!("set{}", &name.as_str()["get".len()..])),
                    args: vec![value],
                });
            }
            if name.as_str() == "slot"
                && is_sequences(receiver)
                && let [sequence, at] = args.as_slice()
            {
                return java::Statement::Expr(sequences_call(
                    "put",
                    vec![sequence.clone(), at.clone(), value],
                ));
            }
        }
    }
    java::Statement::Expr(java::Expr::Binary {
        op: "=",
        lhs: Box::new(place),
        rhs: Box::new(value),
    })
}

/// A statement the walk built as `place = value` or as the bare
/// expression `place = value`, rewritten by [`assign`]; anything else is
/// returned as it is.
#[must_use]
pub fn assignment(statement: java::Statement) -> java::Statement {
    match statement {
        java::Statement::Expr(java::Expr::Binary { op: "=", lhs, rhs }) => assign(*lhs, *rhs),
        other => other,
    }
}

/// Build the Java declarations a rule application produced.
///
/// One element: the outer class, with everything nested inside it. See
/// the module documentation for why it is one and not many.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no Java shape, or a generated name would collide
/// with the class it nests in — never a silently dropped declaration.
pub fn java_items(ast: &Ast, applied: &Applied) -> Result<Vec<java::TypeDecl>, MaterialiseError> {
    // Structural declarations — the map entry classes — sit at the outer
    // level and not in a module: two identical maps declared in two
    // modules are one Java class, exactly as two identical anonymous
    // collections are one C type.
    let mut structural: Vec<java::Member> = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    let mut items: Vec<Pending> = Vec::new();

    // The same walk the C column uses. Java needs no forward
    // declarations, so the order is not load-bearing for the compiler;
    // it is load-bearing for the output, which must not move between two
    // runs over one model.
    for id in ordered_types(ast) {
        let start = items.len();
        declare(ast, applied, id, &mut items, &mut structural, &mut emitted)?;
        annotate_declared(ast, id, start, &mut items)?;
        weave_verbatim(ast, id, start, &mut items);
    }

    // DDS-XTypes 1.3 §7.5.1.2.6.1: "Each user-defined IDL annotation type
    // shall be represented by a corresponding Java annotation type."
    for (module, member) in annotation_types(ast)? {
        items.push(Pending { module, member });
    }

    // Constants and exceptions are not types, so they are walked
    // separately — the model keeps them in arenas of their own, and both
    // sibling materialisers do the same.
    for (id, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "JavaConst").is_none() {
            continue;
        }
        // `eval_const` and not `eval_integral`: `const string MODULE_ID`
        // evaluates perfectly well and is not a number, and asking the
        // integral question about it answered `Unevaluated` and dropped
        // the field.
        let value = ast
            .eval_const(node.expr)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "constant's value".to_string(),
                at: node.name.clone(),
            })?;
        let type_ = match node.type_.resolved() {
            Some(target) => java_member_type(ast, target, &node.name)?,
            None => java::Type::Primitive(java::Primitive::Int),
        };
        let value = match value {
            // CORBA 3.3 part 3 §1.5.2 (and IDL4-Java): `octet` is a Java
            // `byte`, which holds the octet's eight bits. An octet above
            // 127 is that bit pattern: the hex literal of the IDL value,
            // cast — `(byte) 0xF1` for `0xF1`. Refusing it left no Java form
            // for the octet constants of XTypes 1.3 Annex B (`EK_MINIMAL`).
            zerodds_ast::ConstValue::Integer(number)
                if (128..=255).contains(&number)
                    && node
                        .type_
                        .resolved()
                        .is_some_and(|target| is_octet(ast, target, 0)) =>
            {
                java::Expr::Cast {
                    type_: Box::new(java::Type::Primitive(java::Primitive::Byte)),
                    value: Box::new(java::Expr::Literal(Literal::Int(format!("0x{number:02X}")))),
                }
            }
            value => constant_expr(&type_, &value, &node.name)?,
        };
        items.push(Pending {
            module: node.module,
            member: java::Member::Field(java::Field {
                doc: Doc::none(),
                annotations: user_annotations(ast, zerodds_ast::AnyId::Const(id, node.node_id))?,
                modifiers: vec![
                    java::Modifier::Public,
                    java::Modifier::Static,
                    java::Modifier::Final,
                ],
                type_,
                name: Ident::new(&java::escape_keyword(&node.name)),
                value: Some(value),
            }),
        });
    }

    for (id, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "JavaError").is_none() {
            continue;
        }
        let _ = id;
        let mut members = Vec::new();
        for member in &node.members {
            let Ok(member_node) = ast.member(*member) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "exception member the model listed".to_string(),
                    at: node.name.clone(),
                });
            };
            let Some(target) = member_node.type_.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "exception member type".to_string(),
                    at: member_node.name.clone(),
                });
            };
            members.push(field(
                java_member_type(ast, target, &member_node.name)?,
                &member_node.name,
                Doc::none(),
            ));
        }
        items.push(Pending {
            module: node.module,
            member: data_class(
                &java::escape_keyword(&node.name),
                members,
                Doc::lines(&[
                    "An exception, as the structure its members make.",
                    "",
                    "A data holder and not a java.lang.Throwable: a Throwable is",
                    "Serializable, which under -Xlint:all -Werror demands a",
                    "serialVersionUID, and nothing about the wire form decides what",
                    "that number should be.",
                ]),
                None,
                true,
            )?,
        });
    }

    let mut root = Scope::default();
    for item in items {
        insert(ast, &mut root, item)?;
    }
    let mut members = structural;
    members.extend(scope_members(ast, root, 0)?);
    check_names(&members, &[OUTER.to_string()], 0)?;

    // `@verbatim(placement = BEGIN_FILE)` opens the generated file and
    // `END_FILE` closes it — OMG IDL 4.2 §8.3.5.1 — outside even the
    // outer class, because the file and not the class is what the
    // placement names.
    let (begin_file, end_file) = verbatim::file_slots(ast, verbatim::aliases::JAVA);
    let mut decls: Vec<java::TypeDecl> = begin_file
        .into_iter()
        .map(java::TypeDecl::Verbatim)
        .collect();
    decls.push(java::TypeDecl::Class(java::Class {
        doc: Doc::lines(&[
            "The Java shape of one IDL file.",
            "",
            "Every declaration nests here: Java allows one public top-level type",
            "per file, and one file is what a generated probe compiles.",
        ]),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public, java::Modifier::Final],
        name: Ident::new(OUTER),
        generics: Vec::new(),
        extends: None,
        implements: Vec::new(),
        members,
    }));
    decls.extend(end_file.into_iter().map(java::TypeDecl::Verbatim));
    Ok(decls)
}

// -- user-defined annotation types (DDS-XTypes 1.3 §7.5.1.2.6.1) --------------

/// The suffix of the Java annotation type that groups several instances of
/// one annotation on one element.
const GROUP_SUFFIX: &str = "Group";

/// The Java type of an annotation element, which Java restricts to
/// primitives, `String` and arrays of them.
fn element_type(
    ast: &Ast,
    member: &zerodds_ast::AnnotationMemberNode,
    owner: &str,
) -> Result<java::Type, MaterialiseError> {
    let at = format!("@{owner}.{}", member.name);
    let Some(target) = member.type_.resolved() else {
        return Err(MaterialiseError::Unevaluated {
            construct: "annotation member type".to_string(),
            at,
        });
    };
    let type_ = java_member_type(ast, target, &at)?;
    let allowed = match &type_ {
        java::Type::Primitive(_) => true,
        java::Type::Named { path, .. } => {
            let segments: Vec<&str> = path.segments().iter().map(Ident::as_str).collect();
            segments == STRING
        }
        _ => false,
    };
    if !allowed {
        return Err(MaterialiseError::NoLanguageForm {
            construct: format!(
                "an annotation member of the type {}, which a Java annotation element cannot have",
                zerodds_lang_render::java::type_(&type_)
            ),
            language: "Java",
            at,
        });
    }
    Ok(type_)
}

/// The path of the Java annotation type that stands for `def`, written
/// from the outer class down.
fn annotation_path(
    ast: &Ast,
    def: Id<zerodds_ast::AnnotationDefNode>,
    group: bool,
) -> Result<Path, MaterialiseError> {
    let node = ast
        .annotation_def(def)
        .map_err(|_| MaterialiseError::Unevaluated {
            construct: "annotation declaration".to_string(),
            at: OUTER.to_string(),
        })?;
    let mut segments = vec![OUTER.to_string()];
    for (_, name) in module_chain(ast, node.module)? {
        segments.push(name);
    }
    let mut name = java::escape_keyword(&node.name);
    if group {
        name.push_str(GROUP_SUFFIX);
    }
    segments.push(name);
    let refs: Vec<&str> = segments.iter().map(String::as_str).collect();
    Ok(Path::new(&refs))
}

/// `public @interface <TypeName> { … }` and `public @interface
/// <TypeName>Group { <TypeName>[] value(); }` for every annotation the
/// model declares, each with the module it was declared in.
///
/// The elements are the declaration's members in order; "If a default
/// value is specified for a given member, it shall be reflected in the
/// Java definition. Otherwise, the Java definition shall have no default
/// value."
///
/// # Errors
/// [`MaterialiseError`] for a member whose type a Java annotation element
/// cannot have, or whose default does not evaluate.
fn annotation_types(ast: &Ast) -> Result<Vec<(Id<ModuleNode>, java::Member)>, MaterialiseError> {
    let mut out = Vec::new();
    for (_, def) in ast.annotation_defs() {
        let in_builtin = ast
            .module(def.module)
            .is_ok_and(|module| module.name == zerodds_ast::node::annotation::BUILTIN_MODULE_NAME);
        if in_builtin {
            continue;
        }
        let name = java::escape_keyword(&def.name);
        let mut elements = Vec::new();
        for member_id in &def.members {
            let member =
                ast.annotation_member(*member_id)
                    .map_err(|_| MaterialiseError::Unevaluated {
                        construct: "annotation member".to_string(),
                        at: def.name.clone(),
                    })?;
            let type_ = element_type(ast, member, &def.name)?;
            let default = match member.default {
                Some(expr) => {
                    let at = format!("@{}.{}", def.name, member.name);
                    let value =
                        ast.eval_const(expr)
                            .map_err(|_| MaterialiseError::Unevaluated {
                                construct: "annotation member default".to_string(),
                                at: at.clone(),
                            })?;
                    Some(constant_expr(&type_, &value, &at)?)
                }
                None => None,
            };
            elements.push(java::AnnotationElement {
                doc: Doc::none(),
                type_,
                name: Ident::new(&java::escape_keyword(&member.name)),
                default,
            });
        }
        out.push((
            def.module,
            java::Member::Nested(Box::new(java::TypeDecl::AnnotationType(
                java::AnnotationType {
                    doc: Doc::line("A user-defined annotation type (XTypes 1.3 7.5.1.2.6.1)."),
                    annotations: Vec::new(),
                    modifiers: vec![java::Modifier::Public],
                    name: Ident::new(&name),
                    elements,
                },
            ))),
        ));
        out.push((
            def.module,
            java::Member::Nested(Box::new(java::TypeDecl::AnnotationType(
                java::AnnotationType {
                    doc: Doc::line(
                        "Several instances of the annotation on one element (XTypes 1.3 \
                         7.5.1.2.6.1).",
                    ),
                    annotations: Vec::new(),
                    modifiers: vec![java::Modifier::Public],
                    name: Ident::new(&format!("{name}{GROUP_SUFFIX}")),
                    elements: vec![java::AnnotationElement {
                        doc: Doc::none(),
                        type_: java::Type::Array(Box::new(java::Type::named(Path::single(&name)))),
                        name: Ident::new("value"),
                        default: None,
                    }],
                },
            ))),
        ));
    }
    Ok(out)
}

/// Whether `def` is declared in the module of the built-in annotations.
fn is_builtin_annotation(ast: &Ast, def: Id<zerodds_ast::AnnotationDefNode>) -> bool {
    ast.annotation_def(def)
        .ok()
        .and_then(|node| ast.module(node.module).ok())
        .is_some_and(|module| module.name == zerodds_ast::node::annotation::BUILTIN_MODULE_NAME)
}

/// One applied annotation as the Java annotation of the same name.
fn applied_annotation(
    ast: &Ast,
    applied: &zerodds_ast::AppliedAnnotation,
    def: Id<zerodds_ast::AnnotationDefNode>,
) -> Result<java::Annotation, MaterialiseError> {
    let node = ast
        .annotation_def(def)
        .map_err(|_| MaterialiseError::Unevaluated {
            construct: "annotation declaration".to_string(),
            at: applied.name.clone(),
        })?;
    let mut args = Vec::new();
    for argument in &applied.arguments {
        let member = argument
            .name
            .as_deref()
            .and_then(|written| {
                node.members.iter().find_map(|id| {
                    ast.annotation_member(*id)
                        .ok()
                        .filter(|m| m.name.eq_ignore_ascii_case(written))
                })
            })
            .or_else(|| {
                node.members.iter().find_map(|id| {
                    ast.annotation_member(*id)
                        .ok()
                        .filter(|m| m.is_value_member)
                })
            })
            .ok_or_else(|| MaterialiseError::Unevaluated {
                construct: "annotation argument naming no member".to_string(),
                at: applied.name.clone(),
            })?;
        let at = format!("@{}.{}", node.name, member.name);
        let type_ = element_type(ast, member, &node.name)?;
        let value = ast
            .eval_const(argument.value)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "annotation argument".to_string(),
                at: at.clone(),
            })?;
        args.push(java::AnnotationArg::Pair {
            name: Ident::new(&java::escape_keyword(&member.name)),
            value: constant_expr(&type_, &value, &at)?,
        });
    }
    Ok(java::Annotation {
        path: annotation_path(ast, def, false)?,
        args,
    })
}

/// The Java annotations an IDL element carries.
///
/// "For each IDL element to which a single instance user-defined
/// annotation is applied, the corresponding Java element shall be
/// annotated with the Java annotation of the same name. For each IDL
/// element to which multiple instances of the annotation are applied, the
/// corresponding Java element shall be annotated with the generated
/// annotation bearing the 'Group' suffix; each application of the
/// user-defined annotation shall correspond to a member of the array in
/// the group." An annotation the model has no declaration for is not
/// represented: there is no Java type to name.
fn user_annotations(
    ast: &Ast,
    any: zerodds_ast::AnyId,
) -> Result<Vec<java::Annotation>, MaterialiseError> {
    let facets = ast
        .facets(any)
        .ok_or_else(|| MaterialiseError::Unevaluated {
            construct: "the annotations of an element the model listed".to_string(),
            at: OUTER.to_string(),
        })?;
    let mut seen: Vec<Id<zerodds_ast::AnnotationDefNode>> = Vec::new();
    let mut out = Vec::new();
    for applied in &facets.annotations {
        let Some(def) = applied.definition.resolved() else {
            continue;
        };
        // A built-in annotation (the specifications' own, such as the
        // DDS-RPC ones the model keeps whole) is not a user-defined
        // annotation type and has no Java declaration here.
        if is_builtin_annotation(ast, def) {
            continue;
        }
        if seen.contains(&def) {
            continue;
        }
        seen.push(def);
        let same: Vec<&zerodds_ast::AppliedAnnotation> = facets
            .annotations
            .iter()
            .filter(|other| other.definition.resolved() == Some(def))
            .collect();
        if let [only] = same.as_slice() {
            out.push(applied_annotation(ast, only, def)?);
            continue;
        }
        let mut instances = Vec::new();
        for each in same {
            instances.push(java::Expr::Annotation(applied_annotation(ast, each, def)?));
        }
        out.push(java::Annotation {
            path: annotation_path(ast, def, true)?,
            args: vec![java::AnnotationArg::Value(java::Expr::ArrayInit(instances))],
        });
    }
    Ok(out)
}

/// Put the user-defined annotations of a type, its members, its
/// enumerators and its bits on the Java elements `declare` made for them.
fn annotate_declared(
    ast: &Ast,
    id: Id<TypeNode>,
    start: usize,
    items: &mut [Pending],
) -> Result<(), MaterialiseError> {
    let lost = |construct: &str, at: &str| MaterialiseError::Unevaluated {
        construct: construct.to_string(),
        at: at.to_string(),
    };
    let node = ast
        .type_(id)
        .map_err(|_| lost("a type the model listed", OUTER))?;
    let type_name = java::escape_keyword(&node.name);
    let type_annotations = user_annotations(ast, zerodds_ast::AnyId::Type(id, node.node_id))?;
    // Field annotations by Java field name.
    let mut by_field: Vec<(String, Vec<java::Annotation>)> = Vec::new();
    let mut push_field = |name: String, annotations: Vec<java::Annotation>| {
        if !annotations.is_empty() {
            by_field.push((name, annotations));
        }
    };
    match &node.kind {
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            for member in ast.all_members_of(id) {
                let member_node = ast
                    .member(member)
                    .map_err(|_| lost("a member the model listed", &node.name))?;
                push_field(
                    java::escape_keyword(&member_node.name),
                    user_annotations(ast, zerodds_ast::AnyId::Member(member, member_node.node_id))?,
                );
            }
        }
        TypeKind::Union { .. } => {
            for case in ast.cases_of(id) {
                let case_node = ast
                    .case(*case)
                    .map_err(|_| lost("a union case the model listed", &node.name))?;
                let member_node = ast
                    .member(case_node.member)
                    .map_err(|_| lost("the member of a union case", &node.name))?;
                push_field(
                    java::escape_keyword(&member_node.name),
                    user_annotations(
                        ast,
                        zerodds_ast::AnyId::Member(case_node.member, member_node.node_id),
                    )?,
                );
            }
        }
        TypeKind::Enumeration { enumerators } => {
            for enumerator in enumerators {
                let enum_node = ast
                    .enumerator(*enumerator)
                    .map_err(|_| lost("an enumerator the model listed", &node.name))?;
                push_field(
                    format!("_{}", enum_node.name),
                    user_annotations(
                        ast,
                        zerodds_ast::AnyId::Enumerator(*enumerator, enum_node.node_id),
                    )?,
                );
            }
        }
        TypeKind::Bitmask { bits } => {
            for bit in bits {
                let bit_node = ast
                    .bit_value(*bit)
                    .map_err(|_| lost("a bitmask bit the model listed", &node.name))?;
                push_field(
                    java::escape_keyword(&bit_node.name),
                    user_annotations(ast, zerodds_ast::AnyId::BitValue(*bit, bit_node.node_id))?,
                );
            }
        }
        TypeKind::Bitset { .. } => {
            for bitfield in &ast.all_bitfields_of(id) {
                let field_node = ast
                    .bitfield(*bitfield)
                    .map_err(|_| lost("a bitset field the model listed", &node.name))?;
                if let Some(name) = field_node.name.as_ref() {
                    push_field(
                        java::escape_keyword(name),
                        user_annotations(
                            ast,
                            zerodds_ast::AnyId::Bitfield(*bitfield, field_node.node_id),
                        )?,
                    );
                }
            }
        }
        // Collections and aliases declare no Java element of their own.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. }
        | TypeKind::Map { .. }
        | TypeKind::Alias { .. }
        | TypeKind::ValueBox { .. }
        | TypeKind::Native
        | TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => {}
    }
    for pending in items.iter_mut().skip(start) {
        let java::Member::Nested(decl) = &mut pending.member else {
            continue;
        };
        let java::TypeDecl::Class(class) = decl.as_mut() else {
            continue;
        };
        if class.name.as_str() != type_name {
            continue;
        }
        class.annotations.extend(type_annotations.iter().cloned());
        for member in &mut class.members {
            let java::Member::Field(field) = member else {
                continue;
            };
            for (name, annotations) in &by_field {
                if field.name.as_str() == name {
                    field.annotations.extend(annotations.iter().cloned());
                }
            }
        }
    }
    Ok(())
}

/// Weave one type's `@verbatim` text around and into what `declare`
/// just pushed. OMG IDL 4.2 §8.3.5.1; the slot semantics are
/// [`crate::verbatim`]'s.
///
/// `start` is where the type's declarations begin in `items`.
/// `BEGIN_DECLARATION` and `END_DECLARATION` land inside the class body
/// when the type materialised as one; an enum, a constant or a type
/// that declared nothing has no body this tree can hold raw text in, so
/// the text lands adjacent to the declaration instead — the nearest
/// position the tree can express, never a dropped directive.
fn weave_verbatim(ast: &Ast, id: Id<TypeNode>, start: usize, items: &mut Vec<Pending>) {
    let slots = verbatim::declaration_slots(ast, id, verbatim::aliases::JAVA);
    if slots.before.is_empty()
        && slots.begin.is_empty()
        && slots.end.is_empty()
        && slots.after.is_empty()
    {
        return;
    }
    let Ok(node) = ast.type_(id) else {
        return;
    };
    let name = java::escape_keyword(&node.name);
    let mut declared = items.split_off(start);
    let mut begin = slots.begin;
    let mut end = slots.end;
    let body = declared
        .iter_mut()
        .find_map(|pending| match &mut pending.member {
            java::Member::Nested(decl) => match decl.as_mut() {
                java::TypeDecl::Class(class) if class.name.as_str() == name => Some(class),
                _ => None,
            },
            _ => None,
        });
    if let Some(class) = body {
        for text in begin.drain(..).rev() {
            class.members.insert(0, java::Member::Verbatim(text));
        }
        class
            .members
            .extend(end.drain(..).map(java::Member::Verbatim));
    }
    for text in slots.before.into_iter().chain(begin) {
        items.push(Pending {
            module: node.module,
            member: java::Member::Verbatim(text),
        });
    }
    items.append(&mut declared);
    for text in end.into_iter().chain(slots.after) {
        items.push(Pending {
            module: node.module,
            member: java::Member::Verbatim(text),
        });
    }
}

/// One declaration, kept with the module that declared it.
struct Pending {
    /// The IDL module it was declared in.
    module: Id<ModuleNode>,
    /// What it materialised as.
    member: java::Member,
}

/// One nesting level of the output: a module's declarations and the
/// modules declared inside it.
///
/// A tree and not a list of chains. Grouping by module id alone would
/// emit `module common { struct Robot; }` and
/// `module common { module util { … } }` as two classes called `common`
/// in one scope, which is the `tests/interop-corpus/module-issue41-shape`
/// arrangement.
#[derive(Default)]
struct Scope {
    /// What this module declares directly, in declaration order.
    members: Vec<java::Member>,
    /// The modules declared inside it, in first-seen order.
    children: Vec<(Id<ModuleNode>, String, Scope)>,
}

/// How deep a module chain is followed before the model is taken to be
/// malformed. IDL nests nowhere near this.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a type's Java spelling is followed.
///
/// A `sequence<sequence<…>>` nests once per level; a chain longer than
/// this is a cycle the resolver did not catch, and recursing into it
/// would end the process rather than report anything.
const MAX_TYPE_DEPTH: usize = 64;

/// Put one declaration in the scope its module names.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module in the chain, or the chain runs past [`MAX_MODULE_DEPTH`].
fn insert(ast: &Ast, root: &mut Scope, item: Pending) -> Result<(), MaterialiseError> {
    let chain = module_chain(ast, item.module)?;
    let mut scope = root;
    for (id, name) in chain {
        let at = match scope.children.iter().position(|(seen, _, _)| *seen == id) {
            Some(at) => at,
            None => {
                scope.children.push((id, name, Scope::default()));
                scope.children.len() - 1
            }
        };
        scope = &mut scope
            .children
            .get_mut(at)
            .ok_or_else(|| MaterialiseError::NoModule {
                at: module_handle(item.module),
            })?
            .2;
    }
    scope.members.push(item.member);
    Ok(())
}

/// A scope's members, with each module inside it as a nested class.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the nesting runs past
/// [`MAX_MODULE_DEPTH`], which `insert` has already bounded.
///
/// zerodds-lint: recursion-depth 64 (module nesting; bounded by `MAX_MODULE_DEPTH`)
fn scope_members(
    ast: &Ast,
    scope: Scope,
    depth: usize,
) -> Result<Vec<java::Member>, MaterialiseError> {
    if depth > MAX_MODULE_DEPTH {
        return Err(MaterialiseError::NoModule {
            at: format!("the nesting runs past {MAX_MODULE_DEPTH}"),
        });
    }
    let mut out = scope.members;
    for (module, name, child) in scope.children {
        let mut class = nested_class(
            &name,
            scope_members(ast, child, depth + 1)?,
            Doc::line("An IDL module, as the class its declarations nest in."),
        );
        let node = ast.module(module).map_err(|_| MaterialiseError::NoModule {
            at: module_handle(module),
        })?;
        let annotations = user_annotations(ast, zerodds_ast::AnyId::Module(module, node.node_id))?;
        if let java::Member::Nested(decl) = &mut class
            && let java::TypeDecl::Class(inner) = decl.as_mut()
        {
            inner.annotations = annotations;
        }
        out.push(class);
    }
    Ok(out)
}

/// The modules a declaration nests in, outermost first, with the class
/// name each takes.
///
/// The root module is the anonymous global scope and is not a class.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module, or the parent chain runs past [`MAX_MODULE_DEPTH`]. Returning
/// the declaration unnested instead would put it in another scope and say
/// nothing, which is the same class of substitution as writing the wrong
/// octet.
fn module_chain(
    ast: &Ast,
    module: Id<ModuleNode>,
) -> Result<Vec<(Id<ModuleNode>, String)>, MaterialiseError> {
    let mut out = Vec::new();
    let mut current = Some(module);
    let mut depth = 0;
    while let Some(id) = current {
        if depth > MAX_MODULE_DEPTH {
            return Err(MaterialiseError::NoModule {
                at: format!(
                    "{} — the parent chain runs past {MAX_MODULE_DEPTH}",
                    module_handle(module)
                ),
            });
        }
        let node = ast.module(id).map_err(|_| MaterialiseError::NoModule {
            at: module_handle(id),
        })?;
        // The root has no name and no parent, and produces no class.
        if node.parent.is_none() {
            break;
        }
        out.push((id, java::escape_keyword(&node.name)));
        current = node.parent;
        depth += 1;
    }
    out.reverse();
    Ok(out)
}

/// The module handle, named without the node the model would not answer
/// for. Slot and generation are what the arena was asked for.
fn module_handle(module: Id<ModuleNode>) -> String {
    format!("#{}@{}", module.slot(), module.generation())
}

/// Refuse a nested type whose name a Java compiler would reject.
///
/// Two collisions, both of which produce a file no compiler reads:
/// a class sharing the simple name of one it nests in (JLS §8.1), and
/// two declarations of one name in one scope. A materialiser that let
/// either through would report success for output that does not build.
///
/// zerodds-lint: recursion-depth 64 (module nesting; bounded by `MAX_MODULE_DEPTH`)
fn check_names(
    members: &[java::Member],
    enclosing: &[String],
    depth: usize,
) -> Result<(), MaterialiseError> {
    if depth > MAX_MODULE_DEPTH {
        return Err(MaterialiseError::NoModule {
            at: format!("the nesting runs past {MAX_MODULE_DEPTH}"),
        });
    }
    let mut seen: Vec<String> = Vec::new();
    for member in members {
        let java::Member::Nested(decl) = member else {
            continue;
        };
        let name = decl_name(decl).to_string();
        // `@verbatim` text has no name and takes part in no collision.
        if name.is_empty() {
            continue;
        }
        if enclosing.contains(&name) {
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "a declaration named `{name}` inside a class of that name \
                     (JLS 8.1: a nested class may not share the simple name of an \
                     enclosing one)"
                ),
                language: "Java",
                at: name,
            });
        }
        if depth == 0 && RESERVED_TOP_LEVEL.contains(&name.as_str()) {
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "a declaration named `{name}`, which would capture the first \
                     segment of every qualified reference this file writes \
                     (JLS 6.5.2)"
                ),
                language: "Java",
                at: name,
            });
        }
        if seen.contains(&name) {
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!("a second declaration named `{name}` in one scope"),
                language: "Java",
                at: name,
            });
        }
        let mut inner: Vec<String> = enclosing.to_vec();
        inner.push(name.clone());
        seen.push(name);
        if let java::TypeDecl::Class(class) = decl.as_ref() {
            check_names(&class.members, &inner, depth + 1)?;
        }
    }
    Ok(())
}

/// The name a type declaration carries.
fn decl_name(decl: &java::TypeDecl) -> &str {
    match decl {
        java::TypeDecl::Class(node) => node.name.as_str(),
        java::TypeDecl::Interface(node) => node.name.as_str(),
        java::TypeDecl::Enum(node) => node.name.as_str(),
        java::TypeDecl::Record(node) => node.name.as_str(),
        java::TypeDecl::AnnotationType(node) => node.name.as_str(),
        // Raw text declares nothing; the empty name is the caller's
        // signal to pass it over.
        java::TypeDecl::Verbatim(_) => "",
    }
}

/// One type's Java declarations, appended where they belong.
#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    items: &mut Vec<Pending>,
    structural: &mut Vec<java::Member>,
    emitted: &mut Vec<String>,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };
    match &node.kind {
        // A primitive, a string and a `fixed` are spellings Java already
        // has; a sequence is `T[]`. None of them declares anything.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. } => {}

        // Neither does an array — but every dimension is evaluated
        // anyway. Java's array type does not state a length, so nothing
        // here would substitute a zero; what a dimension that does not
        // evaluate does mean is that no caller can allocate the array,
        // and a file that compiles while the length is unknown is
        // exactly the silent success this project refuses. The C column
        // fails on the same input, and the two columns' failure sets
        // stay comparable.
        TypeKind::Array { dims, .. } => {
            for dim in dims {
                ast.eval_integral(*dim)
                    .map_err(|_| MaterialiseError::Unevaluated {
                        construct: "array length".to_string(),
                        at: named(&node.name, node.node_id),
                    })?;
            }
        }

        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: named(&node.name, node.node_id),
                });
            };
            let name = map_entry_name(ast, id)?;
            let at = named(&node.name, node.node_id);
            if !emitted.contains(&name) {
                emitted.push(name.clone());
                structural.push(nested_class(
                    &name,
                    vec![
                        field(java_member_type(ast, key, &at)?, "key", Doc::none()),
                        field(java_member_type(ast, value, &at)?, "value", Doc::none()),
                    ],
                    Doc::line(
                        "One entry of a map. XTypes 1.3 7.4.1.1.4 defines a map as a \
                         sequence of these, and the rules write the two halves with no \
                         framing between them. The codec reads and writes the entries \
                         of a map in this form, in the order of the map.",
                    ),
                ));
            }
            if map_key_supported(ast, key) {
                let class = map_class_name(ast, id)?;
                if !emitted.contains(&class) {
                    emitted.push(class.clone());
                    structural.push(map_class(ast, id, key, value, &class, &name, &at)?);
                }
            }
        }

        // A typedef has no Java form. The comment is what keeps it from
        // being a silent omission: the name, and what it resolved to.
        TypeKind::Alias { aliased } => {
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "JavaAlias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let spelling =
                zerodds_lang_render::java::type_(&java_member_type(ast, target, &node.name)?);
            items.push(Pending {
                module: node.module,
                member: java::Member::Comment(Doc::line(&format!(
                    "typedef {} {}; — Java has no alias, so every use takes {spelling}.",
                    spelling, node.name
                ))),
            });
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "JavaEnum").is_none() {
                return Ok(());
            }
            let mut listed: Vec<(String, i128)> = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "enumerator the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, enum_node.node_id, "JavaEnumerator").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                let value = in_range(
                    value,
                    i128::from(i32::MIN),
                    i128::from(u32::MAX),
                    "int",
                    &enum_node.name,
                )?;
                // A value above `int`'s range is written as the pattern
                // it will occupy, because XTypes 1.3 7.4.3.5.3 makes an
                // enumerated literal 32 bits wide and a `@bit_bound(32)`
                // enumeration may use all of them. `in_range` above has
                // already refused anything outside those 32 bits.
                let literal = match i32::try_from(value) {
                    Ok(fits) => i128::from(fits),
                    Err(_) => value - (i128::from(u32::MAX) + 1),
                };
                listed.push((java::escape_keyword(&enum_node.name), literal));
            }
            let class_name = java::escape_keyword(&node.name);
            items.push(Pending {
                module: node.module,
                member: nested_class(
                    &class_name,
                    vec![java::Member::Verbatim(enumeration_class_body(
                        &class_name,
                        &listed,
                    ))],
                    Doc::line(
                        "An enumeration, as CORBA 3.3 part 3 §1.5.9 maps it: one instance \
                         per enumerator, the int constants `_NAME`, `value()` and \
                         `from_int`.",
                    ),
                ),
            });
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "JavaBitmask").is_none() {
                return Ok(());
            }
            let bound = ast.effective_bit_bound(id);
            let holder = unsigned_holder(bound);
            let wide = holder == java::Type::Primitive(java::Primitive::Long);
            let mut members = Vec::new();
            let mut flags: Vec<(String, java::Expr)> = Vec::new();
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "bitmask bit the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, bit_node.node_id, "JavaBitConst").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                // `1L` and not `1` where the holder is a `long`: `1 << 40`
                // is an int shift in Java and evaluates to 256, silently.
                let flag_value = java::Expr::Binary {
                    op: "<<",
                    lhs: Box::new(java::Expr::Literal(Literal::Int(
                        if wide { "1L" } else { "1" }.to_string(),
                    ))),
                    rhs: Box::new(java::Expr::Literal(Literal::int(i128::from(position)))),
                };
                flags.push((bit_node.name.clone(), flag_value.clone()));
                members.push(constant(holder.clone(), &bit_node.name, flag_value));
            }
            // XTypes 1.3 §7.5.1.2.2: "the language binding shall be as if
            // there was an enumeration definition like the following:
            // @bit_bound(n) enum <TypeName>Bits { @value(1 << p) FLAG, … }".
            // Java binds an enumeration as the type-safe class of
            // §7.5.1.2.1.3.
            items.push(Pending {
                module: node.module,
                member: type_safe_enumeration(
                    &format!("{}Bits", java::escape_keyword(&node.name)),
                    holder.clone(),
                    flags,
                    Doc::line(
                        "The enumeration of the flags of a bitmask (XTypes 1.3 \
                         7.5.1.2.2): `_<label>` is the flag's value, 1 << position.",
                    ),
                ),
            });
            items.push(Pending {
                module: node.module,
                member: nested_class(
                    &java::escape_keyword(&node.name),
                    members,
                    Doc::line(
                        "A bitmask, as the constants its bits name. A member of this \
                         type takes the unsigned holder Table 33 of XTypes 1.3 \
                         7.4.1.1.3.2 gives for its bound, mapped through the unsigned \
                         rule this file states.",
                    ),
                ),
            });
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "JavaBitset").is_none() {
                return Ok(());
            }
            let mut members = Vec::new();
            // The base's bitfields are fields of the derived class too:
            // an inherited bitset is one flat run.
            for bitfield in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*bitfield) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "bitset field the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                // An anonymous field is padding: it moves the next field
                // along and carries no value of its own.
                let Some(field_name) = field_node.name.as_ref() else {
                    continue;
                };
                // The width's holder, not the declared one: `bitfield<1,
                // boolean>` names a range and not a wire form, and the C
                // column takes the width for the same reason. The
                // packing into the anchor is the codec's.
                members.push(field(
                    unsigned_holder(field_node.bits),
                    field_name,
                    Doc::none(),
                ));
            }
            items.push(Pending {
                module: node.module,
                member: data_class(
                    &java::escape_keyword(&node.name),
                    members,
                    Doc::line(
                        "A bitset. The fields are separate integers here; the packing \
                         into the anchor is the codec's.",
                    ),
                    None,
                    true,
                )?,
            });
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "JavaValue"
            } else {
                "JavaStruct"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            // XTypes 1.3 §7.5.1.4.1.2: "The Java class corresponding to
            // the subtype shall extend the Java class corresponding to the
            // supertype." The class declares its own members; the plan
            // still puts a base's members on the wire first (§7.4.1.1.5.1),
            // and the codec reaches them by name as inherited fields.
            //
            // Every member is emitted, with no per-member rule lookup:
            // a member whose lookup missed would vanish from the class
            // and the file would still compile. The C column takes the
            // same view.
            let base = match &node.kind {
                TypeKind::Structure {
                    base: Some(base), ..
                } => base.resolved(),
                _ => None,
            };
            let own: Vec<_> = ast.members_of(id).to_vec();
            let mut members = Vec::new();
            let mut spelled = Vec::new();
            for member in ast.all_members_of(id) {
                let Ok(member_node) = ast.member(member) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                let Some(target) = member_node.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member type".to_string(),
                        at: member_node.name.clone(),
                    });
                };
                let type_ = java_member_type(ast, target, &member_node.name)?;
                spelled.push((
                    java::escape_keyword(&member_node.name),
                    member_node.name.clone(),
                ));
                if base.is_some() && !own.contains(&member) {
                    continue;
                }
                if member_node.facets.is_optional() {
                    members.push(field(
                        boxed(type_),
                        &member_node.name,
                        Doc::line(
                            "An @optional member: null when absent. In a @mutable type \
                             an absent member is not written at all; outside one its \
                             absence is written — a zero is_present byte under XCDR2 \
                             (rule 20) or a parameter header declaring a length of zero \
                             under XCDR1 (7.4.1.2).",
                        ),
                    ));
                } else if super::annotated_external(member_node) {
                    // XTypes 1.3 §7.5.1.2.3.4: "External members shall be
                    // represented using object references [...] For IDL
                    // types that map to Java primitive types, those Java
                    // primitive types shall be replaced by the
                    // corresponding object box types from the java.lang
                    // package."
                    members.push(field(
                        boxed(type_),
                        &member_node.name,
                        Doc::line(
                            "An @external member: an object reference, so a primitive \
                             is held as its java.lang box (XTypes 1.3 7.5.1.2.3.4).",
                        ),
                    ));
                } else {
                    members.push(field(type_, &member_node.name, Doc::none()));
                }
            }
            // The escape is the only layer that holds both spellings at
            // once. Left to the target: javac reports a duplicate field, three layers from the IDL.
            no_spelling_collision(
                spelled
                    .iter()
                    .map(|(field, declared)| (field.as_str(), declared.as_str())),
                "Java",
                &node.name,
            )?;
            let extends = match base {
                Some(base) => Some(java::Type::named(qualified(ast, base)?)),
                None => None,
            };
            items.push(Pending {
                module: node.module,
                member: data_class(
                    &java::escape_keyword(&node.name),
                    members,
                    Doc::none(),
                    extends,
                    !has_derived(ast, id),
                )?,
            });
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "JavaUnion").is_none() {
                return Ok(());
            }
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: node.name.clone(),
                });
            };
            // A field per case and not one per type: two cases may carry
            // the same type, and a Java class has no untagged union to
            // overlay them with. Which field is live is what `_d` says.
            let mut members = vec![java::Member::Field(java::Field {
                doc: Doc::line("The discriminator. Which case field is live is what this says."),
                annotations: Vec::new(),
                modifiers: vec![java::Modifier::Public],
                type_: java_member_type(ast, disc, &node.name)?,
                name: Ident::new("_d"),
                value: None,
            })];
            let mut accessed: Vec<UnionAccess> = Vec::new();
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, case_node.node_id, "JavaCase").is_none() {
                    continue;
                }
                let Ok(member) = ast.member(case_node.member) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member behind a union case the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                let Some(target) = member.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case type".to_string(),
                        at: member.name.clone(),
                    });
                };
                let branch = java_member_type(ast, target, &member.name)?;
                // An `@external` branch is an object reference, so a primitive is boxed
                // (XTypes 1.3 §7.5.1.2.3.4).
                let case_type = if super::annotated_external(member) {
                    boxed(branch)
                } else {
                    branch
                };
                let mut labels = Vec::new();
                for candidate in &case_node.labels {
                    if let zerodds_ast::CaseLabel::Value(expr) = candidate {
                        let value = ast.eval_integral(*expr).map_err(|_| {
                            MaterialiseError::Unevaluated {
                                construct: "union case label".to_string(),
                                at: node.name.clone(),
                            }
                        })?;
                        labels.push(java_label_text(ast, disc, value));
                    }
                }
                accessed.push(UnionAccess {
                    name: java::escape_keyword(&member.name),
                    type_: zerodds_lang_render::java::type_(&case_type),
                    labels,
                });
                members.push(field(case_type, &member.name, Doc::none()));
            }
            // IDL to Java 1.3 §1.5.10 (XTypes 1.3 §7.5.1.4): `discriminator()` and, per case,
            // an accessor and a modifier. The fields stay public because the codec writes them.
            if mapping() != JavaMapping::Psm {
                let disc_type =
                    zerodds_lang_render::java::type_(&java_member_type(ast, disc, &node.name)?);
                members.push(java::Member::Verbatim(union_accessors(
                    &disc_type, &accessed,
                )));
            }
            items.push(Pending {
                module: node.module,
                member: data_class(
                    &java::escape_keyword(&node.name),
                    members,
                    Doc::line("A union: the discriminator, and one field per case."),
                    None,
                    true,
                )?,
            });
        }

        // A native type is opaque and has no Java declaration either —
        // the mapping is the facet's spelling, and a comment records it.
        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "JavaNative").is_none() {
                return Ok(());
            }
            let spelling = zerodds_lang_render::java::type_(&native_type(ast, id));
            items.push(Pending {
                module: node.module,
                member: java::Member::Comment(Doc::line(&format!(
                    "native {}; — every use takes {spelling}.",
                    node.name
                ))),
            });
        }

        // A value box is a reference in Java already: the boxed type,
        // null when absent. Nothing to declare.
        TypeKind::ValueBox { .. } => {}

        // An object reference belongs to the RPC arc and an annotation
        // type has no runtime form at all — `zerodds_ast_layout` refuses
        // to plan either — so a Java declaration would be a shape nothing
        // could serialise. `any` and `ValueBase` are in the model because
        // Building Block Any is one of IDL 4.2's fifteen, and what they
        // hold is decided at run time. All four are named here rather
        // than passed over, and `java_member_type` refuses them by name
        // where a member would carry one.
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// The Java type of a value in a position that has to hold one, refusing
/// the constructs Java cannot hold.
///
/// `any` is the first of those. What it holds is decided at run time —
/// there is no Java type for it and no wire form to generate a codec
/// against. Emitting `Object a;` into the class and reporting success
/// produced a field no encoder can write, from a generator that said
/// nothing was wrong.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase`, an object
/// reference and an annotation type;
/// [`MaterialiseError::Unevaluated`] for a type the model will not answer
/// for.
pub fn java_member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<java::Type, MaterialiseError> {
    type_at(ast, target, at, 0)
}

/// Whether `id` is `octet`, through any typedefs.
///
/// zerodds-lint: recursion-depth 64 (typedef chains; bounded by
/// `MAX_TYPE_DEPTH`, past which the answer is no)
fn is_octet(ast: &Ast, id: Id<TypeNode>, depth: usize) -> bool {
    if depth > MAX_TYPE_DEPTH {
        return false;
    }
    match ast.type_(id).map(|node| &node.kind) {
        Ok(TypeKind::Primitive(PrimitiveKind::Octet)) => true,
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .is_some_and(|target| is_octet(ast, target, depth + 1)),
        _ => false,
    }
}

/// [`java_member_type`], carrying how deep the spelling has nested.
///
/// zerodds-lint: recursion-depth 64 (collection and typedef nesting;
/// bounded by `MAX_TYPE_DEPTH`, which returns
/// `MaterialiseError::Unevaluated` naming the spelling that nested)
fn type_at(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<java::Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose Java spelling nests past {MAX_TYPE_DEPTH}"),
            at: at.to_string(),
        });
    }
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: at.to_string(),
    })?;
    let refused = match &node.kind {
        TypeKind::Any => Some("any"),
        TypeKind::ValueBase => Some("ValueBase"),
        // An object reference is a handle into the RPC arc and
        // `coverage.rs` says so; there is no Java data type for it.
        TypeKind::ObjectRef { .. } => Some("Object"),
        // An annotation type has no runtime form, so nothing declares one
        // and a reference to it would name a class that is not there.
        TypeKind::AnnotationType { .. } => Some("annotation type"),
        _ => None,
    };
    if let Some(construct) = refused {
        return Err(MaterialiseError::NoLanguageForm {
            construct: construct.to_string(),
            language: "Java",
            at: at.to_string(),
        });
    }
    Ok(match &node.kind {
        // Matched ahead of the arm below, which would reach
        // `java_primitive`'s `double` — eight bytes where the member is
        // sixteen.
        TypeKind::Primitive(PrimitiveKind::Float128) => java::Type::named(Path::new(FLOAT128)),
        TypeKind::Primitive(kind) => java::Type::Primitive(java_primitive(*kind)),
        // A `wstring` is a String like a `string`: Java strings are
        // UTF-16 already, and the surrogate rule of XTypes 1.3
        // 7.4.1.1.2 is the runtime's, in `Writer.writeWstring`.
        TypeKind::String { .. } => java::Type::named(Path::new(STRING)),
        TypeKind::Fixed { .. } => java::Type::named(Path::new(BIG_DECIMAL)),
        TypeKind::Sequence { element, bound } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: at.to_string(),
                });
            };
            let element = type_at(ast, element, at, depth + 1)?;
            // DDS-Java-PSM 1.0 §9.1.2: an unbounded sequence is a
            // `Collection<E>`, a bounded one a Java array.
            if mapping() == JavaMapping::Psm && matches!(bound, zerodds_ast::Bound::Unbounded) {
                java::Type::generic(Path::new(COLLECTION), vec![boxed(element)])
            } else {
                java::Type::Array(Box::new(element))
            }
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: at.to_string(),
                });
            };
            let mut out = type_at(ast, element, at, depth + 1)?;
            // One `[]` per dimension. Java's array type carries no
            // length, so `long a[2][3]` and `long a[3][2]` are the same
            // Java type; the lengths are the plan's, and `declare`
            // evaluates them there.
            for _ in dims {
                out = java::Type::Array(Box::new(out));
            }
            out
        }
        // XTypes 1.3 §7.5.1.3.5: "an implementation of the standard
        // java.util.Map interface". The class is declared beside the entry
        // class (see [`map_class`]); a key Java cannot compare as an object
        // keeps the array of entries, because a `java.util.Map` keyed by it
        // would compare by identity.
        TypeKind::Map { key, .. }
            if key
                .resolved()
                .is_some_and(|key| map_key_supported(ast, key)) =>
        {
            java::Type::named(Path::new(&[OUTER, &map_class_name(ast, id)?]))
        }
        TypeKind::Map { .. } => java::Type::Array(Box::new(java::Type::named(Path::new(&[
            OUTER,
            &map_entry_name(ast, id)?,
        ])))),
        // A typedef is transparent: the use takes the target's type.
        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: at.to_string(),
                });
            };
            type_at(ast, target, at, depth + 1)?
        }
        // CORBA 3.3 part 3 §1.5.9: an enumeration is a class with one
        // instance per enumerator.
        TypeKind::Enumeration { .. } => java::Type::named(qualified(ast, id)?),
        TypeKind::Bitmask { .. } => unsigned_holder(ast.effective_bit_bound(id)),
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. } => java::Type::named(qualified(ast, id)?),
        TypeKind::Native => native_type(ast, id),
        // A boxed value is a reference, absent when null — which is what
        // a Java reference already is. A primitive takes its wrapper.
        TypeKind::ValueBox { boxed: inner } => {
            let Some(target) = inner.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "value box target".to_string(),
                    at: at.to_string(),
                });
            };
            boxed(type_at(ast, target, at, depth + 1)?)
        }
        TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => {
            // Refused above; the arm keeps the match total, so a kind
            // added to the model without a Java answer is a compile
            // error and not a field that silently takes another's type.
            return Err(MaterialiseError::NoLanguageForm {
                construct: node.kind.as_str().to_string(),
                language: "Java",
                at: at.to_string(),
            });
        }
    })
}

/// The Java primitive an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without a Java spelling
/// is a compile error and not a member that silently takes another's.
///
/// `long double` is **not** routed here: it takes
/// `org.zerodds.wire.Float128`, which is a real binary128. The
/// `Float128` arm below exists so the match is total and answers for a
/// caller that wants the nearest built-in; `type_at` matches it before
/// it reaches this function. A caller that does not is the defect the
/// three-column gate found in the C column: `double` is eight bytes
/// where the member is sixteen.
///
/// `char` and `wchar` are both a Java `char`, as §1.5.2 has them. A Java
/// `char` is an unsigned 16-bit code unit, so it holds an IDL `char`'s
/// 0..255 and a `wchar`'s BMP code point alike; `Writer.writeChar`
/// refuses a code point above 255 rather than truncating it.
#[must_use]
pub const fn java_primitive(kind: PrimitiveKind) -> java::Primitive {
    match kind {
        PrimitiveKind::Boolean => java::Primitive::Boolean,
        PrimitiveKind::Char | PrimitiveKind::WChar => java::Primitive::Char,
        PrimitiveKind::Int8 => java::Primitive::Byte,
        // CORBA 3.3 part 3 §1.5.2 (the [JAVA-MAP] XTypes 1.3 §7.5.1 reuses): `octet` is `byte`.
        PrimitiveKind::Octet => java::Primitive::Byte,
        // A number in [0, 255], which a signed `byte` does not hold.
        PrimitiveKind::UInt8 => java::Primitive::Int,
        PrimitiveKind::Int16 => java::Primitive::Short,
        // A number in [0, 65535].
        PrimitiveKind::UInt16 => java::Primitive::Int,
        PrimitiveKind::Int32 => java::Primitive::Int,
        // A number in [0, 4294967295].
        PrimitiveKind::UInt32 => java::Primitive::Long,
        PrimitiveKind::Int64 => java::Primitive::Long,
        // The bit pattern: Java has no integer wider than `long`.
        PrimitiveKind::UInt64 => java::Primitive::Long,
        PrimitiveKind::Float32 => java::Primitive::Float,
        PrimitiveKind::Float64 | PrimitiveKind::Float128 => java::Primitive::Double,
    }
}

/// The Java type an unsigned value of `bits` bits is held in.
///
/// The same rule as the primitive table, applied to a width the source
/// gave as a number: up to 16 bits is a number in an `int`, 17 to 32 a
/// number in a `long`, and 33 to 64 a pattern in a `long`. A bitmask of
/// bound 32 therefore takes a `long` and not an `int` — an `int` would
/// hold all 32 bits, but widening it for `writeBitmask(long, int)` would
/// sign-extend bit 31 into the top half.
#[must_use]
pub fn unsigned_holder(bits: u16) -> java::Type {
    if bits <= 16 {
        java::Type::Primitive(java::Primitive::Int)
    } else {
        java::Type::Primitive(java::Primitive::Long)
    }
}

/// A type that can hold `null`, for an `@optional` member.
///
/// A primitive takes its wrapper class, fully qualified: a generated
/// class named `Integer` would otherwise capture the simple name.
/// Everything else is a reference already.
fn boxed(type_: java::Type) -> java::Type {
    match type_ {
        java::Type::Primitive(primitive) => {
            java::Type::named(Path::new(&["java", "lang", primitive.boxed()]))
        }
        other => other,
    }
}

/// What a `native` declaration maps to.
///
/// The facet if a `#pragma` or an annotation gave one, and
/// `java.lang.Object` otherwise — never a guess at what the name might
/// mean.
fn native_type(ast: &Ast, id: Id<TypeNode>) -> java::Type {
    let mapping = ast
        .any_of_type(id)
        .ok()
        .and_then(|any| ast.facets(any))
        .and_then(|facets| facets.corba.as_ref())
        .and_then(|corba| corba.native_mapping.clone());
    match mapping {
        Some(spelling) => java::Type::named(Path::new(&spelling.split('.').collect::<Vec<&str>>())),
        None => java::Type::named(Path::new(&["java", "lang", "Object"])),
    }
}

/// A named type's path, written from [`OUTER`] down.
///
/// `ZdTypes.a.b.Point` and not `a.b.Point`: JLS §6.5.2 resolves the first
/// segment of a qualified name to a variable in scope before a type, so a
/// class carrying a field named `a` would turn `a.b.Point` into a compile
/// error — and only in the corpora that happen to have such a field.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model will not answer for a
/// module the type nests in.
fn qualified(ast: &Ast, id: Id<TypeNode>) -> Result<Path, MaterialiseError> {
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: OUTER.to_string(),
    })?;
    let mut segments = vec![OUTER.to_string()];
    for (_, name) in module_chain(ast, node.module)? {
        segments.push(name);
    }
    segments.push(java::escape_keyword(&node.name));
    Ok(Path::new(
        &segments.iter().map(String::as_str).collect::<Vec<&str>>(),
    ))
}

/// The map entry class a `map<K,V>` needs a name for.
///
/// # Errors
/// As [`type_tag`].
pub fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> Result<String, MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "map".to_string(),
            at: OUTER.to_string(),
        });
    };
    let TypeKind::Map { key, value, .. } = &node.kind else {
        return Err(MaterialiseError::Unevaluated {
            construct: "map".to_string(),
            at: named(&node.name, node.node_id),
        });
    };
    let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "map key or value".to_string(),
            at: named(&node.name, node.node_id),
        });
    };
    Ok(format!(
        "{ENTRY_PREFIX}{}_{}",
        type_tag(ast, key, 0)?,
        type_tag(ast, value, 0)?
    ))
}
/// The entry class of the map class `class` (`ZdMap<bound>_<key>_<value>`, the
/// bound being absent or `_b<digits>` or `_bx`): `ZdEntry_<key>_<value>`.
///
/// The inverse of how [`map_class_name`] and [`map_entry_name`] build the
/// two names, for a caller that has the class and not the model. A key type
/// named like a bound (`b5`) cannot be told apart from one; the bound is
/// taken off only when a key and a value remain.
#[must_use]
pub fn entry_class_of_map_class(class: &str) -> Option<String> {
    let rest = class.strip_prefix(MAP_PREFIX)?;
    let rest = rest.strip_prefix('_').map_or(rest, |tail| {
        // `_b<digits>_` or `_bx_` in front of the key and the value
        let after = tail.strip_prefix('b').and_then(|bound| {
            let end = bound.find('_')?;
            let (digits, remainder) = bound.split_at(end);
            let is_bound =
                digits == "x" || (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()));
            (is_bound && remainder[1..].contains('_')).then(|| &remainder[1..])
        });
        after.unwrap_or(tail)
    });
    // `rest` is `<key>_<value>` (the leading `_` was taken above)
    rest.contains('_').then(|| format!("{ENTRY_PREFIX}{rest}"))
}

/// The class of a `map<K,V>`: `ZdMap<bound>_<key>_<value>`.
///
/// # Errors
/// As [`type_tag`].
pub fn map_class_name(ast: &Ast, id: Id<TypeNode>) -> Result<String, MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "map".to_string(),
            at: OUTER.to_string(),
        });
    };
    let TypeKind::Map { key, value, bound } = &node.kind else {
        return Err(MaterialiseError::Unevaluated {
            construct: "map".to_string(),
            at: named(&node.name, node.node_id),
        });
    };
    let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "map key or value".to_string(),
            at: named(&node.name, node.node_id),
        });
    };
    Ok(format!(
        "{MAP_PREFIX}{}_{}_{}",
        bound_tag(ast, bound),
        type_tag(ast, key, 0)?,
        type_tag(ast, value, 0)?
    ))
}

/// Whether the key type of a map is one a `java.util.Map` can be keyed by:
/// a primitive, a string, an enumeration or a bitmask, whose Java objects
/// compare by value, and a structure, a union or a bit set, whose generated class
/// has `equals` and `hashCode` over its members (see
/// `equals_and_hash_code`).
#[must_use]
pub fn map_key_supported(ast: &Ast, key: Id<TypeNode>) -> bool {
    let mut current = key;
    for _ in 0..MAX_TYPE_DEPTH {
        let Ok(node) = ast.type_(current) else {
            return false;
        };
        match &node.kind {
            TypeKind::Primitive(PrimitiveKind::Float128) => return false,
            TypeKind::Primitive(_)
            | TypeKind::String { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Bitmask { .. }
            | TypeKind::Structure { .. }
            | TypeKind::Union { .. }
            | TypeKind::Bitset { .. } => return true,
            TypeKind::Alias { aliased } => match aliased.resolved() {
                Some(next) => current = next,
                None => return false,
            },
            _ => return false,
        }
    }
    false
}

/// The class of a map, which "shall be represented in Java by an
/// implementation of the standard java.util.Map interface" (XTypes 1.3
/// §7.5.1.3.5): a `LinkedHashMap` of the boxed key and value types, so
/// that the order the entries were put in is the order they go on the
/// wire in (`MapEntryOrder` sorts them when asked to).
///
/// What Table 47 asks beyond `java.util.Map` and Java does not have is
/// here: the capacity (`getMaxSize`, `setMaxSize`), `copyFrom` with
/// `autogrow`, and `insert` with `replace`, which answer with the return
/// codes of DDS 1.4. The rest is native: `new` and `initialize` are the
/// constructor, `delete` and `finalize` the garbage collector (or
/// `clear`), `get_size` is `size`, `clear` is `clear`, `erase` is
/// `remove`, `find_element` is `get`, and `get_first`, `get_next`,
/// `find_entry` and `get_pair` are the iterator of `entrySet` and
/// `Map.Entry`. `entries` and `of` are the codec's: it reads and writes
/// the entry class, in order.
fn map_class(
    ast: &Ast,
    id: Id<TypeNode>,
    key: Id<TypeNode>,
    value: Id<TypeNode>,
    class: &str,
    entry: &str,
    at: &str,
) -> Result<java::Member, MaterialiseError> {
    let bound = match ast.type_(id).map(|node| &node.kind) {
        Ok(TypeKind::Map { bound, .. }) => match bound.expr() {
            None => 0,
            Some(expr) => ast
                .eval_integral(expr)
                .ok()
                .and_then(|limit| u32::try_from(limit).ok())
                .ok_or_else(|| MaterialiseError::Unevaluated {
                    construct: "map bound".to_string(),
                    at: at.to_string(),
                })?,
        },
        _ => 0,
    };
    let key_type = zerodds_lang_render::java::type_(&boxed(java_member_type(ast, key, at)?));
    let value_type = zerodds_lang_render::java::type_(&boxed(java_member_type(ast, value, at)?));
    let entry = format!("{OUTER}.{entry}");
    let text = format!(
        "    private static final long serialVersionUID = 1L;
    /** `map<K, V, N>`: N, or 0 for a map with no bound. */
    private static final int ZD_BOUND = {bound};
    /** DDS 1.4 2.2.2.1.1, ReturnCode_t. */
    public static final int RETCODE_OK = 0;
    /** DDS 1.4 2.2.2.1.1, ReturnCode_t. */
    public static final int RETCODE_ERROR = 1;
    /** DDS 1.4 2.2.2.1.1, ReturnCode_t. */
    public static final int RETCODE_BAD_PARAMETER = 3;
    /** DDS 1.4 2.2.2.1.1, ReturnCode_t. */
    public static final int RETCODE_PRECONDITION_NOT_MET = 4;
    private int zdMaximum;

    /** Table 47 initialize: the size and the capacity are 0. */
    public {class}() {{
        super();
    }}

    /**
     * Table 47 get_max_size: how many entries the map may contain. A map
     * the codec filled holds the entries it read, so it is never below
     * the size.
     */
    public int getMaxSize() {{
        return Math.max(zdMaximum, size());
    }}

    /**
     * Table 47 set_max_size: BAD_PARAMETER if the new maximum is below
     * the size or above the bound.
     */
    public int setMaxSize(int maxSize) {{
        if (maxSize < size() || (ZD_BOUND != 0 && maxSize > ZD_BOUND)) {{
            return RETCODE_BAD_PARAMETER;
        }}
        zdMaximum = maxSize;
        return RETCODE_OK;
    }}

    /**
     * Table 47 insert (for {{@code replace}} false) and insert_or_assign (for
     * true): an existing key is BAD_PARAMETER or has its element replaced, a
     * new one past the capacity is PRECONDITION_NOT_MET. A null key or
     * element is BAD_PARAMETER.
     */
    public int insert({key_type} key, {value_type} element, boolean replace) {{
        if (key == null || element == null) {{
            return RETCODE_BAD_PARAMETER;
        }}
        if (containsKey(key)) {{
            if (!replace) {{
                return RETCODE_BAD_PARAMETER;
            }}
            put(key, element);
            return RETCODE_OK;
        }}
        if (size() >= getMaxSize()) {{
            return RETCODE_PRECONDITION_NOT_MET;
        }}
        put(key, element);
        return RETCODE_OK;
    }}

    /**
     * Table 47 copy: overwrite this map with {{@code other}}. Too large for
     * the capacity, {{@code autogrow}} decides: the capacity grows, or the
     * answer is PRECONDITION_NOT_MET and this map stays as it was.
     */
    public int copyFrom({class} other, boolean autogrow) {{
        if (other == null) {{
            return RETCODE_BAD_PARAMETER;
        }}
        if (other == this) {{
            return RETCODE_OK;
        }}
        if (other.size() > getMaxSize()) {{
            if (!autogrow) {{
                return RETCODE_PRECONDITION_NOT_MET;
            }}
            int grown = setMaxSize(other.size());
            if (grown != RETCODE_OK) {{
                return grown;
            }}
        }}
        clear();
        putAll(other);
        return RETCODE_OK;
    }}

    /** The entries, in the order of the map: what the codec writes. */
    public {entry}[] entries() {{
        {entry}[] out = new {entry}[size()];
        int i = 0;
        for (java.util.Map.Entry<{key_type}, {value_type}> e : entrySet()) {{
            out[i] = new {entry}();
            out[i].key = e.getKey();
            out[i].value = e.getValue();
            i = i + 1;
        }}
        return out;
    }}

    /**
     * The map of entries, in their order: what the codec reads. A key
     * twice is no map and is refused.
     */
    public static {class} of({entry}[] entries) {{
        {class} map = new {class}();
        for ({entry} e : entries) {{
            if (map.containsKey(e.key)) {{
                throw new java.lang.IllegalArgumentException(\"the map has the key \" + e.key + \" twice\");
            }}
            map.put(e.key, e.value);
        }}
        map.zdMaximum = map.size();
        return map;
    }}

    /** No entries: where a reader starts. */
    public static {entry}[] noEntries() {{
        return new {entry}[0];
    }}
"
    );
    Ok(java::Member::Nested(Box::new(java::TypeDecl::Class(
        java::Class {
            doc: Doc::lines(&[
                "A map (XTypes 1.3 7.5.1.3.5): a java.util.Map in the order the entries",
                "were put, with the operations of Table 47 that Java does not have.",
            ]),
            annotations: Vec::new(),
            modifiers: vec![
                java::Modifier::Public,
                java::Modifier::Static,
                java::Modifier::Final,
            ],
            name: Ident::new(class),
            generics: Vec::new(),
            extends: Some(java::Type::generic(
                Path::new(&["java", "util", "LinkedHashMap"]),
                vec![
                    boxed(java_member_type(ast, key, at)?),
                    boxed(java_member_type(ast, value, at)?),
                ],
            )),
            implements: Vec::new(),
            members: vec![java::Member::Verbatim(text)],
        },
    ))))
}

/// Where a declaration sits, as Java has to write it: `ZdTypes.a.b.Name`.
///
/// [`type_tag`] joins the same segments with `_` and is a *name* — what
/// a generated codec method is called after. This is the *path*, which
/// is what reaches the class: every declaration nests inside `ZdTypes`
/// because Java allows one public top-level type per file, and a caller
/// that spelled `ZdTypes.a_b_Name` would name nothing.
///
/// # Errors
/// When the module chain nests past the cap.
pub fn declared_path(ast: &Ast, id: Id<TypeNode>) -> Result<String, MaterialiseError> {
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: OUTER.to_string(),
    })?;
    let mut segments = vec!["ZdTypes".to_string()];
    for (_, name) in module_chain(ast, node.module)? {
        segments.push(name);
    }
    segments.push(java::escape_keyword(&node.name));
    Ok(segments.join("."))
}

/// The bound a `string`, a `sequence` or a `map` carries, as the suffix
/// that keeps two of them apart.
///
/// The tag also names the routine a hoisted collection's codec is
/// emitted under, and one body is emitted per name. With the bound left
/// out, `sequence<long>` and `sequence<long, 4>` are one name and
/// whichever was declared first decides: measured on
/// `@final struct S { sequence<long> b; sequence<long, 4> a; };`, the
/// generated Java held no `zd_over` check at all, so `a` accepted a
/// stream of any length; with the members the other way round both got
/// the bounded body, so `b` refused — and truncated — a length the IDL
/// puts no limit on. `string<4>` beside `string` and `wstring<4>`
/// beside `wstring` collapse the same way, onto the trimming call.
/// Either way the reader answered with a value the IDL does not
/// describe and said nothing, which is why the bound is part of the
/// name. The same suffix the Zig column carries.
///
/// An `array` needs nothing here: its `dims` are already in the tag,
/// under the `Array` arm below.
fn bound_tag(ast: &Ast, bound: &zerodds_ast::Bound) -> String {
    match bound.expr() {
        None => String::new(),
        // `_b` and not a bare `_n`: `seq_long_4` is what the `Array`
        // arm answers for an array of four `long`, and the two are
        // different wire forms.
        Some(expr) => match ast.eval_integral(expr) {
            Ok(limit) => format!("_b{limit}"),
            // As the array's `_x`: a bound that does not evaluate is
            // still not the unbounded form, and the materialiser
            // reports the expression that would not evaluate.
            Err(_) => "_bx".to_string(),
        },
    }
}

/// The name component an entry class takes from one of its halves.
///
/// The IDL kind and not the Java spelling, and this is not cosmetic:
/// `octet` and `unsigned short` are both a Java `int`, and they are one
/// and two bytes on the wire. A structural name built from the Java type
/// would give `map<octet,long>` and `map<unsigned short,long>` one entry
/// class, and a codec written against it would put one of the two out at
/// the other's width with nothing to say it had. An alias is followed
/// through, because a typedef is transparent in Java and the two names
/// would otherwise describe the same class twice.
///
/// # Errors
/// [`MaterialiseError`] when a module or an array dimension will not
/// answer.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
pub fn type_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Result<String, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose name nests past {MAX_TYPE_DEPTH}"),
            at: OUTER.to_string(),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok("unknown".to_string());
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => kind.as_str().replace(' ', "_"),
        TypeKind::String {
            width: CharWidth::Char,
            bound,
        } => format!("string{}", bound_tag(ast, bound)),
        TypeKind::String {
            width: CharWidth::WChar,
            bound,
        } => format!("wstring{}", bound_tag(ast, bound)),
        TypeKind::Fixed { digits, scale } => format!("fixed_{digits}_{scale}"),
        TypeKind::Sequence { element, bound } => match element.resolved() {
            Some(element) => format!(
                "seq{}_{}",
                bound_tag(ast, bound),
                type_tag(ast, element, depth + 1)?
            ),
            None => format!("seq_unknown{}", bound_tag(ast, bound)),
        },
        TypeKind::Array { element, dims } => {
            let mut name = match element.resolved() {
                Some(element) => format!("arr_{}", type_tag(ast, element, depth + 1)?),
                None => "arr_unknown".to_string(),
            };
            for dim in dims {
                match ast.eval_integral(*dim) {
                    Ok(length) => name.push_str(&format!("_{length}")),
                    // A dimension that does not evaluate gets no number
                    // rather than a zero: the name has to differ from a
                    // `[0]` array's, and `declare` reports the failure by
                    // name.
                    Err(_) => name.push_str("_x"),
                }
            }
            name
        }
        // The entry class's own name carries no bound — two maps that
        // differ only in bound hold the same pairs — but the tag does,
        // for the routine-name reason in [`bound_tag`].
        TypeKind::Map { key, value, bound } => {
            let key = match key.resolved() {
                Some(key) => type_tag(ast, key, depth + 1)?,
                None => "unknown".to_string(),
            };
            let value = match value.resolved() {
                Some(value) => type_tag(ast, value, depth + 1)?,
                None => "unknown".to_string(),
            };
            format!("map{}_{key}_{value}", bound_tag(ast, bound))
        }
        TypeKind::Alias { aliased } => match aliased.resolved() {
            Some(target) => type_tag(ast, target, depth + 1)?,
            None => "unknown".to_string(),
        },
        TypeKind::ValueBox { boxed: inner } => match inner.resolved() {
            Some(target) => format!("box_{}", type_tag(ast, target, depth + 1)?),
            None => "box_unknown".to_string(),
        },
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Native => {
            let mut segments = Vec::new();
            for (_, name) in module_chain(ast, node.module)? {
                segments.push(name);
            }
            segments.push(java::escape_keyword(&node.name));
            segments.join("_")
        }
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    })
}

/// `public static final class <name> { … }`, as a member of its scope.
fn nested_class(name: &str, members: Vec<java::Member>, doc: Doc) -> java::Member {
    nested_class_extending(name, members, doc, None, true)
}

/// [`nested_class`] of a class that extends another, and that is not
/// `final` where another extends it (a class with a subtype cannot be).
fn nested_class_extending(
    name: &str,
    members: Vec<java::Member>,
    doc: Doc,
    extends: Option<java::Type>,
    final_: bool,
) -> java::Member {
    let mut modifiers = vec![java::Modifier::Public, java::Modifier::Static];
    if final_ {
        modifiers.push(java::Modifier::Final);
    }
    java::Member::Nested(Box::new(java::TypeDecl::Class(java::Class {
        doc,
        annotations: Vec::new(),
        modifiers,
        name: Ident::new(name),
        generics: Vec::new(),
        extends,
        implements: Vec::new(),
        members,
    })))
}

/// Whether another structure extends this one: the class is not `final` then.
pub(crate) fn has_derived(ast: &Ast, id: Id<TypeNode>) -> bool {
    ast.types().any(|(_, node)| {
        matches!(
            &node.kind,
            TypeKind::Structure { base: Some(base), .. } if base.resolved() == Some(id)
        )
    })
}

/// A class of data members: an IDL structure, union, bit set or exception.
///
/// [`nested_class`] under [`JavaMapping::Omg`]; under [`JavaMapping::Psm`]
/// the class of DDS-Java-PSM 1.0 §9.1.1, see [`encapsulate`].
fn data_class(
    name: &str,
    members: Vec<java::Member>,
    doc: Doc,
    extends: Option<java::Type>,
    final_: bool,
) -> Result<java::Member, MaterialiseError> {
    let value_methods = equals_and_hash_code(name, &members, extends.is_some());
    let mut members = if mapping() == JavaMapping::Psm {
        encapsulate(name, members)?
    } else {
        members
    };
    members.push(java::Member::Verbatim(value_methods));
    Ok(nested_class_extending(name, members, doc, extends, final_))
}

/// `equals` and `hashCode` of a class of data members: two values are equal
/// when every member is, an array by its elements. XTypes 1.3 §7.5.1.3.5
/// asks a `java.util.Map` for every key type of IDL, and a structure or a
/// union is a key by its members; a `HashMap` finds a key by these two
/// methods. A derived class adds its base's.
fn equals_and_hash_code(name: &str, members: &[java::Member], derived: bool) -> String {
    let fields: Vec<&str> = members
        .iter()
        .filter_map(|member| match member {
            java::Member::Field(data) if !data.modifiers.contains(&java::Modifier::Static) => {
                Some(data.name.as_str())
            }
            _ => None,
        })
        .collect();
    let mut equal = String::new();
    if derived {
        equal.push_str("super.equals(other)");
    }
    for field in &fields {
        if !equal.is_empty() {
            equal.push_str("\n            && ");
        }
        equal.push_str(&format!(
            "java.util.Objects.deepEquals({field}, that.{field})"
        ));
    }
    if equal.is_empty() {
        equal.push_str("true");
    }
    let mut hashed: Vec<String> = Vec::new();
    if derived {
        hashed.push("super.hashCode()".to_string());
    }
    hashed.extend(fields.iter().map(|field| (*field).to_string()));
    format!(
        "    /** Equal when every member is equal; an array by its elements. */
    @Override
    public boolean equals(Object other) {{
        if (this == other) {{
            return true;
        }}
        if (other == null || getClass() != other.getClass()) {{
            return false;
        }}
        {name} that = ({name}) other;
        return {equal};
    }}

    /** Consistent with {{@link #equals(Object)}}. */
    @Override
    public int hashCode() {{
        return java.util.Arrays.deepHashCode(new Object[] {{ {} }});
    }}
",
        hashed.join(", ")
    )
}

/// DDS-Java-PSM 1.0 §9.1.1: "Contained attributes shall be encapsulated.
/// Java Bean style accessors shall be provided. [...] The representation
/// of internal state shall be private."
///
/// Every instance field becomes private. The class gets an explicit
/// no-argument constructor (§8.6 asks for one of a type a Service
/// instantiates reflectively; this is the one the codec calls) and
/// `getX()`/`setX(v)` for each field; a `boolean` field has `isX()` as
/// well, the "special mapping rule for boolean properties" §9.1.1 allows,
/// and `getX()` too, which is what the codec calls because it does not
/// know the type.
///
/// # Errors
/// [`MaterialiseError::SpellingCollision`] when two members make one
/// accessor name, or a member makes the name of another's accessor.
fn encapsulate(
    class_name: &str,
    members: Vec<java::Member>,
) -> Result<Vec<java::Member>, MaterialiseError> {
    let mut fields = Vec::new();
    let mut accessors = Vec::new();
    let mut others = Vec::new();
    let mut names: Vec<(String, String)> = Vec::new();
    for member in members {
        let java::Member::Field(mut data) = member else {
            others.push(member);
            continue;
        };
        if data.modifiers.contains(&java::Modifier::Static) {
            fields.push(java::Member::Field(data));
            continue;
        }
        let field_name = data.name.as_str().to_string();
        let type_ = data.type_.clone();
        data.modifiers = vec![java::Modifier::Private];
        fields.push(java::Member::Field(data));

        let this = |name: &str| java::Expr::Field {
            value: Box::new(java::Expr::Path(Path::new(&["this"]))),
            name: Ident::new(name),
        };
        let getter = |name: String| {
            java::Member::Method(java::Method {
                doc: Doc::none(),
                annotations: Vec::new(),
                modifiers: vec![java::Modifier::Public],
                generics: Vec::new(),
                returns: type_.clone(),
                name: Ident::new(&name),
                params: Vec::new(),
                throws: Vec::new(),
                body: Some(java::Block {
                    statements: vec![java::Statement::Return(Some(this(&field_name)))],
                }),
            })
        };
        names.push((getter_name(&field_name), field_name.clone()));
        accessors.push(getter(getter_name(&field_name)));
        if type_ == java::Type::Primitive(java::Primitive::Boolean) {
            names.push((boolean_getter_name(&field_name), field_name.clone()));
            accessors.push(getter(boolean_getter_name(&field_name)));
        }
        names.push((setter_name(&field_name), field_name.clone()));
        accessors.push(java::Member::Method(java::Method {
            doc: Doc::none(),
            annotations: Vec::new(),
            modifiers: vec![java::Modifier::Public],
            generics: Vec::new(),
            returns: java::Type::Void,
            name: Ident::new(&setter_name(&field_name)),
            params: vec![java::Param {
                annotations: Vec::new(),
                final_: false,
                type_: type_.clone(),
                name: Ident::new("v"),
                varargs: false,
            }],
            throws: Vec::new(),
            body: Some(java::Block {
                statements: vec![java::Statement::Expr(java::Expr::Binary {
                    op: "=",
                    lhs: Box::new(this(&field_name)),
                    rhs: Box::new(java::Expr::Path(Path::new(&["v"]))),
                })],
            }),
        }));
    }
    no_spelling_collision(
        names
            .iter()
            .map(|(spelled, declared)| (spelled.as_str(), declared.as_str())),
        "Java accessor",
        class_name,
    )?;
    let mut out = others;
    out.extend(fields);
    out.push(java::Member::Constructor(java::Constructor {
        doc: Doc::none(),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public],
        name: Ident::new(class_name),
        params: Vec::new(),
        throws: Vec::new(),
        body: java::Block {
            statements: Vec::new(),
        },
    }));
    out.extend(accessors);
    Ok(out)
}

/// `public <type> <name>;`
///
/// Public and not accessor-wrapped: §1.5.10 of the OMG mapping makes a
/// struct's members public fields, the JacORB column's fill statements
/// assign to them, and a generated codec that had to call a getter would
/// be reading a decision nothing on the wire made.
fn field(type_: java::Type, name: &str, doc: Doc) -> java::Member {
    java::Member::Field(java::Field {
        doc,
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public],
        type_,
        name: Ident::new(&java::escape_keyword(name)),
        value: None,
    })
}

/// The pre-Java-5 "type-safe enumeration" class of DDS-XTypes 1.3
/// §7.5.1.2.1.3, with the shape [JAVA-MAP] gives it.
///
/// `public static final int _<label> = <value>;` per literal, and an
/// object `public static final <Class> <label>` per literal built from
/// it. `value()` answers the number and `from_int` the object, and
/// refuses a number the enumeration does not list.
///
/// `value_type` is `int` for an enumeration; a bitmask's flag
/// enumeration takes the holder of its bound, which for 17 to 64 bits is
/// a `long`.
fn type_safe_enumeration(
    class: &str,
    value_type: java::Type,
    literals: Vec<(String, java::Expr)>,
    doc: Doc,
) -> java::Member {
    let own = || java::Type::named(Path::single(class));
    let path = |name: &str| java::Expr::Path(Path::single(name));
    let mut members = Vec::new();
    for (label, value) in &literals {
        members.push(constant(
            value_type.clone(),
            &format!("_{label}"),
            value.clone(),
        ));
    }
    for (label, _) in &literals {
        members.push(constant(
            own(),
            label,
            java::Expr::New {
                type_: Box::new(own()),
                args: vec![path(&format!("_{label}"))],
            },
        ));
    }
    members.push(java::Member::Field(java::Field {
        doc: Doc::none(),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Private, java::Modifier::Final],
        type_: value_type.clone(),
        name: Ident::new("__value"),
        value: None,
    }));
    members.push(java::Member::Constructor(java::Constructor {
        doc: Doc::none(),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Private],
        name: Ident::new(class),
        params: vec![java::Param {
            annotations: Vec::new(),
            final_: false,
            type_: value_type.clone(),
            name: Ident::new("v"),
            varargs: false,
        }],
        throws: Vec::new(),
        body: java::Block {
            statements: vec![java::Statement::Expr(java::Expr::Binary {
                op: "=",
                lhs: Box::new(java::Expr::Field {
                    value: Box::new(java::Expr::This),
                    name: Ident::new("__value"),
                }),
                rhs: Box::new(path("v")),
            })],
        },
    }));
    members.push(java::Member::Method(java::Method {
        doc: Doc::line("The value the IDL gives the literal."),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public],
        generics: Vec::new(),
        returns: value_type.clone(),
        name: Ident::new("value"),
        params: Vec::new(),
        throws: Vec::new(),
        body: Some(java::Block {
            statements: vec![java::Statement::Return(Some(path("__value")))],
        }),
    }));
    let mut lookup: Vec<java::Statement> = Vec::new();
    for (label, _) in &literals {
        lookup.push(java::Statement::If {
            condition: java::Expr::Binary {
                op: "==",
                lhs: Box::new(path("v")),
                rhs: Box::new(path(&format!("_{label}"))),
            },
            then: java::Block {
                statements: vec![java::Statement::Return(Some(path(&java::escape_keyword(
                    label,
                ))))],
            },
            otherwise: None,
        });
    }
    lookup.push(java::Statement::Throw(java::Expr::New {
        type_: Box::new(java::Type::named(Path::new(&[
            "java",
            "lang",
            "IllegalArgumentException",
        ]))),
        args: vec![java::Expr::Binary {
            op: "+",
            lhs: Box::new(java::Expr::Literal(Literal::Str(format!(
                "not a value of {class}: "
            )))),
            rhs: Box::new(path("v")),
        }],
    }));
    members.push(java::Member::Method(java::Method {
        doc: Doc::line("The literal that has the value `v`."),
        annotations: Vec::new(),
        modifiers: vec![java::Modifier::Public, java::Modifier::Static],
        generics: Vec::new(),
        returns: own(),
        name: Ident::new("from_int"),
        params: vec![java::Param {
            annotations: Vec::new(),
            final_: false,
            type_: value_type,
            name: Ident::new("v"),
            varargs: false,
        }],
        throws: Vec::new(),
        body: Some(java::Block { statements: lookup }),
    }));
    nested_class(class, members, doc)
}

/// `public static final <type> <name> = <value>;`
fn constant(type_: java::Type, name: &str, value: java::Expr) -> java::Member {
    java::Member::Field(java::Field {
        doc: Doc::none(),
        annotations: Vec::new(),
        modifiers: vec![
            java::Modifier::Public,
            java::Modifier::Static,
            java::Modifier::Final,
        ],
        type_,
        name: Ident::new(&java::escape_keyword(name)),
        value: Some(value),
    })
}

/// The body of the class an enumeration is (CORBA 3.3 part 3 §1.5.9): one instance per
/// enumerator named by it, the `int` constants `_NAME`, `value()` and `from_int`, which
/// answers the instance of a value and refuses one no enumerator has. The codec reads a
/// value through the enumerator chain, so `from_int` is the application's door and the
/// codec never meets that refusal.
fn enumeration_class_body(class: &str, enumerators: &[(String, i128)]) -> String {
    let mut text = format!(
        "    private final int zdValue;\n\n    private {class}(int value) {{\n        zdValue = value;\n    }}\n\n"
    );
    for (name, value) in enumerators {
        text.push_str(&format!(
            "    /** The value of `{name}`. */\n    public static final int _{name} = {value};\n    /** The instance of `{name}`. */\n    public static final {class} {name} = new {class}(_{name});\n\n"
        ));
    }
    text.push_str("    /** The value of this enumerator. */\n    public int value() {\n        return zdValue;\n    }\n\n");
    text.push_str(&format!(
        "    /** The enumerator with this value (the first one, where two share it). */\n    public static {class} from_int(int value) {{\n"
    ));
    for (name, _) in enumerators {
        text.push_str(&format!(
            "        if (value == _{name}) {{\n            return {name};\n        }}\n"
        ));
    }
    text.push_str(&format!(
        "        throw new IllegalArgumentException(\"no enumerator of {class} has the value \" + value);\n    }}\n"
    ));
    text
}

/// One case of a union, for [`union_accessors`].
struct UnionAccess {
    name: String,
    type_: String,
    /// The Java text of each label of the case; none for a `default:` case.
    labels: Vec<String>,
}

/// The Java text of a union label, in the type of the discriminator.
fn java_label_text(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    let mut current = discriminator;
    for _ in 0..MAX_TYPE_DEPTH {
        match ast.type_(current).map(|node| &node.kind) {
            Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(next) => current = next,
                None => break,
            },
            Ok(TypeKind::Enumeration { .. }) => {
                let owner = declared_path(ast, current)
                    .unwrap_or_else(|_| format!("UnresolvedEnum{}", current.slot()));
                // A value above `int` is the bit pattern it occupies.
                let narrowed = i32::try_from(value).unwrap_or_else(|_| {
                    u32::try_from(value & 0xFFFF_FFFF)
                        .map_or(0, |bits| i32::from_ne_bytes(bits.to_ne_bytes()))
                });
                return format!("{owner}.from_int({narrowed})");
            }
            Ok(TypeKind::Primitive(PrimitiveKind::Boolean)) => {
                return if value == 0 { "false" } else { "true" }.to_string();
            }
            Ok(TypeKind::Primitive(kind)) => {
                return match java_primitive(*kind) {
                    java::Primitive::Byte => format!("(byte) {value}"),
                    java::Primitive::Short => format!("(short) {value}"),
                    java::Primitive::Char => format!("(char) {value}"),
                    // An `unsigned long long` label past `Long.MAX_VALUE` is the signed
                    // decimal with the same sixty-four bits, as in `CodecTarget::wide_int`.
                    java::Primitive::Long => {
                        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                        let pattern = if value > i128::from(i64::MAX) {
                            value as u64 as i64
                        } else {
                            value as i64
                        };
                        format!("{pattern}L")
                    }
                    _ => format!("{value}"),
                };
            }
            _ => break,
        }
    }
    format!("{value}")
}

/// `discriminator()`, the accessor and the modifiers of each case (IDL to Java 1.3 §1.5.10).
///
/// An accessor refuses a case the discriminator does not select; the modifier of a case with one
/// label selects it, and every case has the modifier that names the discriminator, which is
/// refused when it selects another case.
fn union_accessors(discriminator: &str, cases: &[UnionAccess]) -> String {
    let all: Vec<&str> = cases
        .iter()
        .flat_map(|case| case.labels.iter().map(String::as_str))
        .collect();
    let any_label = |labels: &[&str]| -> String {
        if labels.is_empty() {
            "false".to_string()
        } else {
            labels
                .iter()
                .map(|label| format!("_d == {label}"))
                .collect::<Vec<_>>()
                .join(" || ")
        }
    };
    let mut text = format!(
        "    /** The discriminator. */\n    public {discriminator} discriminator() {{\n        return _d;\n    }}\n\n"
    );
    for case in cases {
        let own: Vec<&str> = case.labels.iter().map(String::as_str).collect();
        let selected = if own.is_empty() {
            format!("!({})", any_label(&all))
        } else {
            any_label(&own)
        };
        let (name, ty) = (&case.name, &case.type_);
        text.push_str(&format!(
            "    /** The value of `{name}`; refused when the discriminator selects another case. */\n    public {ty} {name}() {{\n        if (!({selected})) {{\n            throw new IllegalStateException(\"the discriminator does not select `{name}`\");\n        }}\n        return {name};\n    }}\n\n"
        ));
        if let [only] = own.as_slice() {
            text.push_str(&format!(
                "    /** Sets `{name}` and selects it. */\n    public void {name}({ty} value) {{\n        this.{name} = value;\n        this._d = {only};\n    }}\n\n"
            ));
        }
        text.push_str(&format!(
            "    /** Sets `{name}` under the discriminator `disc`, which has to select it. */\n    public void {name}({discriminator} disc, {ty} value) {{\n        {discriminator} previous = _d;\n        _d = disc;\n        if (!({selected})) {{\n            _d = previous;\n            throw new IllegalArgumentException(\"the discriminator does not select `{name}`\");\n        }}\n        this.{name} = value;\n    }}\n\n"
        ));
    }
    text
}

/// The initialiser a constant of `type_` takes for `value`.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] when the value does not fit the
/// type Java gives it — never a truncation;
/// [`MaterialiseError::Unevaluated`] when the constant is not an integer,
/// which is what `Ast::eval_integral` answers for. A `const string` is
/// the case: the C column reports the same, and neither substitutes a
/// The Java expression a constant's value takes.
///
/// The declared type already carries the four string-shaped forms:
/// `java_member_type` answers `java.lang.String` for a `string` and a
/// `wstring`, so unlike C++ nothing here has to substitute a type. What
/// it does have to do is pick the right literal for each, and refuse the
/// one Java has no literal for.
///
/// A `wstring` becomes a `String` because a Java `String` *is* UTF-16 —
/// the code points go in directly and the renderer escapes what has to
/// be escaped. A `fixed` becomes `new java.math.BigDecimal("<decimal>")`,
/// the type the OMG mapping gives it.
///
/// `long double` is refused by name. Java has no 128-bit floating type,
/// `org.zerodds.wire.Float128` is a class with no literal, and a
/// `double` would put a different number in the file than the IDL wrote.
///
/// # Errors
/// [`MaterialiseError`] naming the constant and what will not hold it.
fn constant_expr(
    type_: &java::Type,
    value: &zerodds_ast::ConstValue,
    at: &str,
) -> Result<java::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: String| MaterialiseError::NoLanguageForm {
        construct,
        language: "Java",
        at: at.to_string(),
    };
    match value {
        ConstValue::Integer(number) => literal_of(type_, *number, at),
        ConstValue::Boolean(flag) => Ok(java::Expr::Literal(Literal::Bool(*flag))),
        ConstValue::Char(character) => Ok(java::Expr::Literal(Literal::Char(*character))),
        // A Java `char` is one UTF-16 code unit, so a code point above
        // the Basic Multilingual Plane does not fit one and is refused
        // rather than truncated into a lone surrogate.
        ConstValue::WChar(point) => match constant::wchar_char(*point)
            .filter(|character| u32::from(*character) <= u32::from(u16::MAX))
        {
            Some(character) => Ok(java::Expr::Literal(Literal::Char(character))),
            None => Err(refuse(format!(
                "the code point U+{point:04X}, which a Java `char` does not hold"
            ))),
        },
        ConstValue::Float { mantissa, exponent } => {
            if !matches!(
                type_,
                java::Type::Primitive(java::Primitive::Float | java::Primitive::Double)
            ) {
                return Err(refuse(String::from(
                    "a `long double` constant, which has no Java literal",
                )));
            }
            let mut text = constant::float_text(mantissa, exponent.as_deref());
            // Without the suffix a decimal literal is a `double`, and
            // `float f = 1.5;` is a compile error rather than a
            // narrowing Java performs for you.
            if matches!(type_, java::Type::Primitive(java::Primitive::Float)) {
                text.push('f');
            }
            Ok(java::Expr::Literal(Literal::Float(text)))
        }
        ConstValue::Fixed { digits, scale } => Ok(java::Expr::New {
            type_: Box::new(java::Type::named(Path::new(BIG_DECIMAL))),
            args: vec![java::Expr::Literal(Literal::Str(constant::fixed_decimal(
                digits, *scale,
            )))],
        }),
        ConstValue::String(text) => Ok(java::Expr::Literal(Literal::Str(text.clone()))),
        ConstValue::WString(points) => match constant::wstring_utf8(points) {
            Some(text) => Ok(java::Expr::Literal(Literal::Str(text))),
            None => Err(refuse(String::from(
                "a `wstring` constant holding a lone surrogate",
            ))),
        },
        // `ConstValue` is `#[non_exhaustive]`, so a form added to the
        // model reaches here. Refused by name rather than given a
        // plausible default: a constant emitted as the wrong literal is
        // a field that disagrees with the IDL and compiles.
        _ => Err(refuse(String::from(
            "a constant form this binding has no literal for",
        ))),
    }
}

/// value.
fn literal_of(type_: &java::Type, value: i128, at: &str) -> Result<java::Expr, MaterialiseError> {
    let java::Type::Primitive(primitive) = type_ else {
        return Err(MaterialiseError::Unevaluated {
            construct: "constant's value, which is not an integer".to_string(),
            at: at.to_string(),
        });
    };
    Ok(match primitive {
        java::Primitive::Boolean => java::Expr::Literal(Literal::Bool(value != 0)),
        // A narrowing assignment of a constant expression is legal Java,
        // but the cast is written anyway: it is what says the narrowing
        // was intended and what keeps the line legible.
        java::Primitive::Char => cast(
            *primitive,
            in_range(value, 0, i128::from(u16::MAX), "char", at)?,
        ),
        java::Primitive::Byte => cast(*primitive, in_range(value, -128, 127, "byte", at)?),
        java::Primitive::Short => cast(
            *primitive,
            in_range(
                value,
                i128::from(i16::MIN),
                i128::from(i16::MAX),
                "short",
                at,
            )?,
        ),
        java::Primitive::Int => java::Expr::Literal(Literal::int(in_range(
            value,
            i128::from(i32::MIN),
            i128::from(i32::MAX),
            "int",
            at,
        )?)),
        java::Primitive::Long => {
            let value = in_range(
                value,
                i128::from(i64::MIN),
                i128::from(i64::MAX),
                "long",
                at,
            )?;
            // The `L`: `4294967295` without it is an int literal Java
            // refuses, and a `long` field is exactly where an
            // `unsigned long` value lands.
            java::Expr::Literal(Literal::Int(format!("{value}L")))
        }
        java::Primitive::Float | java::Primitive::Double => {
            java::Expr::Literal(Literal::int(value))
        }
    })
}

/// `(<primitive>) <value>`
fn cast(primitive: java::Primitive, value: i128) -> java::Expr {
    java::Expr::Cast {
        type_: Box::new(java::Type::Primitive(primitive)),
        value: Box::new(java::Expr::Literal(Literal::int(value))),
    }
}

/// `value`, or the failure that says it does not fit.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`], naming the value and the type
/// that will not hold it. Truncating instead would put a number nobody
/// wrote into the output.
fn in_range(
    value: i128,
    low: i128,
    high: i128,
    type_: &str,
    at: &str,
) -> Result<i128, MaterialiseError> {
    if value < low || value > high {
        return Err(MaterialiseError::NoLanguageForm {
            construct: format!("the value {value}, which a Java `{type_}` does not hold"),
            language: "Java",
            at: at.to_string(),
        });
    }
    Ok(value)
}

/// A model node's name, or its identity when the node is anonymous.
///
/// An array type declared inline has no name of its own, and an error
/// that said only "the array length does not evaluate" would leave the
/// reader to find which array.
fn named(name: &str, node: NodeId) -> String {
    if name.is_empty() {
        format!("{node:?}")
    } else {
        name.to_string()
    }
}

/// Whether the rules created a node of `type_name` for `node`.
///
/// The same two-hop walk the other two materialisers do: a correspondence
/// is an ordinary node with a link to the anchor and a link to what was
/// created.
fn ghost_created_for(applied: &Applied, node: NodeId, type_name: &str) -> Option<GhostId> {
    let anchor = applied
        .origin
        .iter()
        .find(|(_, model)| **model == node)
        .map(|(ghost, _)| *ghost)?;
    let typ = applied.graph.types.lookup(type_name)?;
    applied
        .graph
        .nodes_of_type(typ)
        .map(|created| created.id)
        .find(|created| {
            applied.graph.connected(&anchor, created)
                || applied
                    .graph
                    .parts(&anchor)
                    .any(|part| applied.graph.connected(&part.other, created))
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::rule;
    use crate::{ModelGraph, apply};
    use std::path::{Path as FsPath, PathBuf};

    #[test]
    fn the_entry_class_is_found_from_the_map_class() {
        assert_eq!(
            entry_class_of_map_class("ZdMap_long_string").as_deref(),
            Some("ZdEntry_long_string")
        );
        assert_eq!(
            entry_class_of_map_class("ZdMap_b8_long_string").as_deref(),
            Some("ZdEntry_long_string")
        );
        assert_eq!(
            entry_class_of_map_class("ZdMap_bx_unsigned_short_string").as_deref(),
            Some("ZdEntry_unsigned_short_string")
        );
        assert_eq!(entry_class_of_map_class("Other_long_string"), None);
        for idl in [
            "@final struct P { map<long, string> m; };",
            "@final struct P { map<long, string, 8> m; };",
        ] {
            let ast = {
                let parsed = zerodds_idl_parse::parse(idl);
                let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "t.idl");
                let mut ast = lowered.ast;
                ast.resolve_all_by_name().expect("resolve");
                ast
            };
            let map = ast
                .types()
                .find(|(_, node)| matches!(node.kind, TypeKind::Map { .. }))
                .map(|(id, _)| id)
                .expect("a map type");
            assert_eq!(
                entry_class_of_map_class(&map_class_name(&ast, map).expect("class")),
                Some(map_entry_name(&ast, map).expect("entry")),
                "{idl}"
            );
        }
    }

    /// IDL in, one Java file out. The tests of this module state the
    /// properties of the IDL-to-Java mapping (public fields), so they ask
    /// for it by name; the default is Clause 9 (`tests/java_psm_mapping.rs`).
    fn render(idl: &str, path: &str) -> Result<String, String> {
        with_mapping(JavaMapping::Omg, || render_in_force(idl, path))
    }

    fn render_in_force(idl: &str, path: &str) -> Result<String, String> {
        let parsed = zerodds_idl_parse::parse(idl);
        if parsed.has_errors() {
            return Err(format!("parse: {:?}", parsed.diagnostics()));
        }
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), path);
        if !lowered.diagnostics.is_empty() {
            return Err(format!("lower: {:?}", lowered.diagnostics));
        }
        let mut ast = lowered.ast;
        ast.resolve_all_by_name()
            .map_err(|error| format!("resolve: {error}"))?;
        let applied = apply(&ModelGraph::of(&ast), &rule::java::catalogue())
            .map_err(|error| format!("apply: {error}"))?;
        let types = java_items(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
        Ok(zerodds_lang_render::java::file(&java::File {
            doc: Doc::lines(&[
                "Generated by zerodds-lang-rules. Do not edit.",
                "",
                "The Java shape of one IDL file. The wire rules are not here — they",
                "are in runtime/java, which implements them from DDS-XTypes 1.3.",
            ]),
            package: None,
            imports: Vec::new(),
            types,
        }))
    }

    /// The same, for a source with no file behind it.
    fn declarations(idl: &str) -> String {
        match render(idl, "test.idl") {
            Ok(text) => text,
            Err(why) => panic!("{why}"),
        }
    }

    /// Every corpus case, by name.
    fn corpus() -> Vec<(String, PathBuf)> {
        let root = FsPath::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop-corpus");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&root).expect("the corpus directory") {
            let entry = entry.expect("a corpus entry");
            let idl = entry.path().join("type.idl");
            if idl.is_file() {
                out.push((entry.file_name().to_string_lossy().into_owned(), idl));
            }
        }
        out.sort();
        out
    }

    /// The whole corpus, through every layer, with nothing skipped.
    ///
    /// A rendered file is not a compiled one. `ZD_JAVA_OUT` names a
    /// directory to write the sources into, and `javac` over that
    /// directory is the second half of the check — a file that does not
    /// compile is a materialiser defect, not a corpus defect.
    #[test]
    fn every_corpus_case_renders_or_names_its_failure() {
        let cases = corpus();
        assert!(
            cases.len() > 300,
            "the corpus is not where it was: {} cases",
            cases.len()
        );
        let out = std::env::var("ZD_JAVA_OUT").ok();
        let mut failures = Vec::new();
        for (name, path) in cases {
            let source = std::fs::read_to_string(&path).expect("a corpus source");
            match render(&source, &path.display().to_string()) {
                Ok(text) => {
                    if let Some(dir) = &out {
                        let case = FsPath::new(dir).join(&name);
                        std::fs::create_dir_all(&case).expect("the output directory");
                        std::fs::write(case.join(format!("{OUTER}.java")), text)
                            .expect("the output file");
                    }
                }
                Err(why) => failures.push(format!("{name}: {why}")),
            }
        }
        // `resolve-module-named-as-type` is
        // `module common { struct common { … }; }`, which JLS 8.1
        // forbids — a nested class may not share the simple name of an
        // enclosing one. It no longer reaches this adapter at all: the
        // front end refuses the file under IDL 4.2 §7.5.2, which is
        // where `crate::adapter::REFUSED_BY_THE_FRONT_END` states the
        // reason and where the check that it is still refused lives.
        let failures = crate::adapter::without_the_front_end_refusals(failures);
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn every_idl_primitive_reaches_the_java_type_that_holds_it() {
        let generated = declarations(
            "@final struct P {
               boolean a; octet b; char c; wchar d;
               int8 e; uint8 f; short g; unsigned short h;
               long i; unsigned long j; long long k; unsigned long long l;
               float m; double n; long double o;
             };",
        );
        for expected in [
            "public boolean a;",
            "public byte b;",
            "public char c;",
            "public char d;",
            "public byte e;",
            "public int f;",
            "public short g;",
            "public int h;",
            "public int i;",
            "public long j;",
            "public long k;",
            "public long l;",
            "public float m;",
            "public double n;",
            "public org.zerodds.wire.Float128 o;",
        ] {
            assert!(generated.contains(expected), "{expected}\n{generated}");
        }
    }

    #[test]
    fn a_sequence_and_an_array_are_both_java_arrays() {
        let generated = declarations(
            "@final struct P { sequence<long> s; long a[3]; long g[2][3]; sequence<string> t; };",
        );
        assert!(generated.contains("public int[] s;"), "{generated}");
        assert!(generated.contains("public int[] a;"), "{generated}");
        assert!(generated.contains("public int[][] g;"), "{generated}");
        assert!(
            generated.contains("public java.lang.String[] t;"),
            "{generated}"
        );
    }

    #[test]
    fn a_map_is_a_java_util_map_over_a_generated_entry_class() {
        let generated = declarations("@final struct P { map<long, string> m; };");
        // The tag is the IDL kind's spelling and not the Java type's:
        // `long` is an `int` here, and the tag is the IDL kind's spelling.
        assert!(
            generated.contains("class ZdEntry_long_string {"),
            "{generated}"
        );
        assert!(generated.contains("public int key;"), "{generated}");
        assert!(
            generated.contains("public java.lang.String value;"),
            "{generated}"
        );
        // XTypes 1.3 §7.5.1.3.5: "an implementation of the standard
        // java.util.Map interface", with the boxed key and value types.
        assert!(
            generated.contains(
                "class ZdMap_long_string extends java.util.LinkedHashMap<java.lang.Integer, java.lang.String> {"
            ),
            "{generated}"
        );
        assert!(
            generated.contains("public ZdTypes.ZdMap_long_string m;"),
            "{generated}"
        );
        // The operations of Table 47 Java does not have.
        for operation in [
            "public int getMaxSize()",
            "public int setMaxSize(int maxSize)",
            "public int insert(java.lang.Integer key, java.lang.String element, boolean replace)",
            "public int copyFrom(ZdMap_long_string other, boolean autogrow)",
        ] {
            assert!(generated.contains(operation), "{operation}:\n{generated}");
        }
    }

    #[test]
    fn a_map_keyed_by_a_structure_is_a_java_util_map_over_a_class_with_equals() {
        let generated = declarations("struct K { long a; }; @final struct P { map<K, long> m; };");
        assert!(
            generated.contains("class ZdMap_K_long extends"),
            "{generated}"
        );
        assert!(
            generated.contains("public boolean equals(Object other)")
                && generated.contains("public int hashCode()"),
            "{generated}"
        );
        assert!(!generated.contains("ZdEntry_K_long[] m;"), "{generated}");
    }

    #[test]
    fn a_map_keyed_by_a_sequence_keeps_its_entries() {
        let generated = declarations("@final struct P { map<sequence<long>, long> m; };");
        assert!(generated.contains("[] m;"), "{generated}");
        assert!(!generated.contains("ZdMap_"), "{generated}");
    }

    #[test]
    fn a_bounded_map_has_a_class_of_its_own_with_the_bound() {
        let generated =
            declarations("@final struct P { map<long, long, 3> a; map<long, long> b; };");
        assert!(
            generated.contains("class ZdMap_b3_long_long extends"),
            "{generated}"
        );
        assert!(
            generated.contains("class ZdMap_long_long extends"),
            "{generated}"
        );
        assert!(
            generated.contains("private static final int ZD_BOUND = 3;"),
            "{generated}"
        );
        // One entry class for both: they hold the same pairs.
        assert_eq!(generated.matches("class ZdEntry_long_long {").count(), 1);
    }

    #[test]
    fn two_maps_of_one_shape_are_one_entry_class() {
        let generated = declarations("@final struct P { map<long, long> a; map<long, long> b; };");
        assert_eq!(generated.matches("class ZdEntry_long_long {").count(), 1);
    }

    #[test]
    fn two_maps_of_one_java_shape_and_two_idl_shapes_are_two_entry_classes() {
        // `uint8` and `unsigned short` are both a Java `int` and one and
        // two bytes on the wire. One entry class for both would put one
        // of them out at the other's width.
        let generated =
            declarations("@final struct P { map<uint8, long> a; map<unsigned short, long> b; };");
        assert!(
            generated.contains("class ZdEntry_uint8_long {"),
            "{generated}"
        );
        assert!(
            generated.contains("class ZdEntry_unsigned_short_long {"),
            "{generated}"
        );
    }

    #[test]
    fn an_optional_member_is_boxed_and_a_primitive_one_too() {
        let generated = declarations(
            "@mutable struct P {
               @optional long a; @optional string b; @optional long long c;
             };",
        );
        // An IDL `long` is 32 bits, so the wrapper is `Integer`.
        assert!(
            generated.contains("public java.lang.Integer a;"),
            "{generated}"
        );
        assert!(
            generated.contains("public java.lang.String b;"),
            "{generated}"
        );
        assert!(
            generated.contains("public java.lang.Long c;"),
            "{generated}"
        );
    }

    #[test]
    fn a_user_annotation_is_an_interface_with_its_defaults_and_a_group() {
        // §7.5.1.2.6.1: `public @interface <TypeName>` with the members of
        // the IDL annotation, a default only where the IDL gives one, and
        // `public @interface <TypeName>Group { <TypeName>[] value(); }`.
        let generated = declarations(
            "@annotation Note { string text default \"none\"; long level; };\n\
             @final struct P { long a; };",
        );
        assert!(
            generated.contains("public @interface Note {"),
            "{generated}"
        );
        assert!(
            generated.contains("java.lang.String text() default \"none\";"),
            "{generated}"
        );
        assert!(
            generated.contains("int level();"),
            "no default: {generated}"
        );
        assert!(
            generated.contains("public @interface NoteGroup {"),
            "{generated}"
        );
        assert!(generated.contains("Note[] value();"), "{generated}");
    }

    #[test]
    fn an_applied_annotation_sits_on_the_java_element_and_several_go_through_the_group() {
        let generated = declarations(
            "@annotation Note { string text default \"none\"; long level; };\n\
             @Note(level = 3) @final struct P { @Note(text = \"a\", level = 1) \
             @Note(level = 2) long a; long b; };",
        );
        assert!(
            generated.contains("@ZdTypes.Note(level = 3)"),
            "{generated}"
        );
        assert!(
            generated.contains(
                "@ZdTypes.NoteGroup({@ZdTypes.Note(text = \"a\", level = 1), \
                 @ZdTypes.Note(level = 2)})"
            ),
            "{generated}"
        );
    }

    #[test]
    fn an_annotation_on_a_module_and_on_a_constant_reaches_the_java_element() {
        let generated = declarations(
            "@annotation Note { long level; };\n\
             @Note(level = 5) module M { @Note(level = 6) const long LIMIT = 3; \
             @final struct P { long a; }; };",
        );
        assert!(
            generated.contains("@ZdTypes.Note(level = 5)"),
            "{generated}"
        );
        assert!(
            generated.contains("@ZdTypes.Note(level = 6)"),
            "{generated}"
        );
    }

    #[test]
    fn an_external_member_is_an_object_reference_and_a_primitive_one_is_boxed() {
        let generated = declarations(
            "@final struct P { @external short a; @external long b; @external string c; \
             @external sequence<long> d; @external boolean e; };",
        );
        assert!(
            generated.contains("public java.lang.Short a;"),
            "{generated}"
        );
        assert!(
            generated.contains("public java.lang.Integer b;"),
            "{generated}"
        );
        assert!(
            generated.contains("public java.lang.String c;"),
            "{generated}"
        );
        assert!(generated.contains("public int[] d;"), "{generated}");
        assert!(
            generated.contains("public java.lang.Boolean e;"),
            "{generated}"
        );
    }

    #[test]
    fn a_union_is_a_class_with_a_discriminator_and_one_field_per_case() {
        // Two cases of one type: a field each, which is what a class can
        // hold and an overlay could not.
        let generated = declarations(
            "union Choice switch (long) {
               case 1: long whole;
               case 2: long other;
               default: double fraction;
             };
             @final struct P { Choice c; };",
        );
        assert!(generated.contains("public int _d;"), "{generated}");
        assert!(generated.contains("public int whole;"), "{generated}");
        assert!(generated.contains("public int other;"), "{generated}");
        assert!(generated.contains("public double fraction;"), "{generated}");
        assert!(
            generated.contains("public ZdTypes.Choice c;"),
            "{generated}"
        );
    }

    #[test]
    fn an_enumeration_is_a_class_of_instances_and_a_member_of_it_is_one() {
        let generated = declarations(
            "enum Colour { RED, GREEN, BLUE };
             @final struct P { Colour c; };",
        );
        assert!(
            generated.contains("public static final int _RED = 0;"),
            "{generated}"
        );
        assert!(
            generated.contains("public static final Colour RED = new Colour(_RED);"),
            "{generated}"
        );
        assert!(
            generated.contains("public static final int _BLUE = 2;"),
            "{generated}"
        );
        assert!(generated.contains("public int value()"), "{generated}");
        assert!(
            generated.contains("public static Colour from_int(int value)"),
            "{generated}"
        );
        assert!(
            generated.contains("public ZdTypes.Colour c;"),
            "{generated}"
        );
    }

    #[test]
    fn a_construct_with_no_java_data_form_is_named_and_not_left_out() {
        // The member is not dropped and no `Object` is substituted for
        // it: a class missing a field that reports success is the defect
        // this materialiser is arranged against.
        for (idl, construct) in [
            ("@final struct P { any a; long tail; };", "any"),
            ("@final struct P { Object o; long tail; };", "Object"),
        ] {
            let failure = render(idl, "test.idl").expect_err(construct);
            assert!(failure.contains(construct), "{failure}");
            assert!(failure.contains("Java"), "{failure}");
        }
    }

    #[test]
    fn an_explicit_enumerator_value_is_written_out() {
        let generated = declarations("enum E { @value(-1) LOW, @value(70000) HIGH };");
        assert!(
            generated.contains("public static final int _LOW = -1;"),
            "{generated}"
        );
        assert!(
            generated.contains("public static final int _HIGH = 70000;"),
            "{generated}"
        );
    }

    #[test]
    fn a_bitmask_takes_the_holder_its_bound_calls_for() {
        let narrow =
            declarations("@bit_bound(8) bitmask Small { @position(0) A, @position(7) B };");
        assert!(
            narrow.contains("public static final int B = 1 << 7;"),
            "{narrow}"
        );
        let wide = declarations("@bit_bound(64) bitmask Big { @position(0) A, @position(40) B };");
        assert!(
            wide.contains("public static final long B = 1L << 40;"),
            "{wide}"
        );
    }

    #[test]
    fn a_bitset_is_a_class_of_one_field_per_bitfield() {
        let generated = declarations(
            "bitset Packed { bitfield<3> low; bitfield<5> high; };
             @final struct P { Packed value; };",
        );
        assert!(generated.contains("public int low;"), "{generated}");
        assert!(generated.contains("public int high;"), "{generated}");
        assert!(
            generated.contains("public ZdTypes.Packed value;"),
            "{generated}"
        );
    }

    #[test]
    fn a_fixed_is_a_bigdecimal() {
        let generated = declarations(
            "typedef fixed<9,3> Money;
             @final struct P { Money amount; };",
        );
        assert!(
            generated.contains("public java.math.BigDecimal amount;"),
            "{generated}"
        );
    }

    #[test]
    fn a_typedef_declares_nothing_and_says_so() {
        let generated = declarations(
            "typedef sequence<long> Row;
             @final struct P { Row r; };",
        );
        assert!(generated.contains("public int[] r;"), "{generated}");
        assert!(generated.contains("typedef int[] Row;"), "{generated}");
    }

    #[test]
    fn a_module_is_a_nested_class_and_two_modules_do_not_collide() {
        let generated = declarations(
            "module a { @final struct P { long x; }; };
             module b { @final struct P { double y; }; };
             @final struct Q { a::P one; b::P two; };",
        );
        assert!(generated.contains("class a {"), "{generated}");
        assert!(generated.contains("class b {"), "{generated}");
        assert!(generated.contains("public ZdTypes.a.P one;"), "{generated}");
        assert!(generated.contains("public ZdTypes.b.P two;"), "{generated}");
    }

    #[test]
    fn a_module_reopened_beside_a_nested_one_is_one_class() {
        // `module common { struct Robot; module util { … } }` — the
        // arrangement that produced two classes of one name when the
        // grouping was by module id rather than by path.
        let generated = declarations(
            "module common {
               module util { @final struct Location { double x; }; };
               @final struct Robot { ::common::util::Location where_; };
             };",
        );
        assert_eq!(
            generated.matches("class common {").count(),
            1,
            "{generated}"
        );
        assert!(
            generated.contains("public ZdTypes.common.util.Location where_;"),
            "{generated}"
        );
    }

    #[test]
    fn an_identifier_that_is_a_java_keyword_is_escaped() {
        let generated = declarations(
            "module package {
               enum class { template, virtual, friend };
               @final struct type { long new; };
             };",
        );
        assert!(generated.contains("class package_ {"), "{generated}");
        assert!(generated.contains("class class_ {"), "{generated}");
        assert!(generated.contains("class type {"), "{generated}");
        assert!(generated.contains("public int new_;"), "{generated}");
        // Only a Java keyword moves: these three are C++ keywords and
        // keep their spelling, as the JacORB column records.
        assert!(
            generated.contains("public static final int _template = 0;"),
            "{generated}"
        );
    }

    #[test]
    fn a_name_that_would_capture_the_qualified_references_is_refused() {
        let failure = render("module java { @final struct P { long x; }; };", "test.idl")
            .expect_err("a module named java is refused");
        assert!(failure.contains("java"), "{failure}");
    }

    #[test]
    fn a_declaration_named_after_the_outer_class_is_refused() {
        let failure = render("@final struct ZdTypes { long x; };", "test.idl")
            .expect_err("a type named after the outer class is refused");
        assert!(failure.contains("ZdTypes"), "{failure}");
    }

    #[test]
    fn an_unevaluated_array_length_is_named_and_not_substituted() {
        // The C column fails on the same input; the two columns' failure
        // sets stay comparable.
        let failure = render("@final struct P { long a[UNKNOWN]; };", "test.idl")
            .expect_err("an array whose length does not evaluate is refused");
        assert!(!failure.is_empty(), "{failure}");
    }
}

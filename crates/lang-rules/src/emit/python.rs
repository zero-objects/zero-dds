// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Python backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as `runtime/c`,
//! `runtime/java`, `runtime/typescript` and the rest do, and none of
//! them calls another. Where two columns disagree on an octet, one has
//! misread the specification — which is the only reason to have a sixth.
//!
//! # The four decisions this column turns on
//!
//! **An `int` is unbounded, so a width is never a cast.** Every other
//! backend spends part of itself on narrowing: `(int)` in Java,
//! `static_cast` in C++, `zdNumber`/`zdBigInt` in TypeScript, because a
//! value that arrived wider than its holder is a value already lost.
//! Python has one integer type and it holds every IDL width exactly, so
//! [`CodecTarget::cast`] is the identity on every integer and the width
//! lives where it belongs — in the codec's *name*, which
//! `adapter::python::type_tag` builds from the IDL kind and never from
//! the Python spelling. A tag built from the Python type would give
//! `octet`, `unsigned short` and `unsigned long long` one codec.
//!
//! **An assignment is a statement.** `a = b` is not an expression in
//! Python: `(a = b)` is a `SyntaxError`. The shared walk builds one
//! through `T::perform(T::binary("=", …))` at every assignment site but
//! two, so [`Python::perform`] recognises that shape and answers with
//! [`py::Statement::Set`]. Nothing else in the tree can hold an `=`.
//!
//! **A block is an indent.** There is no delimiter, so an empty suite is
//! not untidy output — it is a `SyntaxError`, and there is no program the
//! walk could have meant. The four constructors that take a body
//! ([`Python::when`], [`Python::count_up`], [`Python::until`],
//! [`Python::routine`]) pad an empty one with `pass` here, where they can
//! say which body was empty; the renderer's own guard is the backstop.
//!
//! **A `long double` is a `Float128` and never a `float`.** A Python
//! `float` is a binary64 with 53 significand bits and a `long double` is
//! a binary128 with 113. `runtime/python`'s `Float128` carries the
//! sixteen octets, and this column reaches for it exactly as the
//! TypeScript one reaches for `ZdFloat128`.
//!
//! # The one place this file and the materialiser disagree
//!
//! `adapter/python.rs` declares a `long double` member as
//! `_Float128 = typing.Annotated[str, _IdlFloat(bits=128)]` — the
//! **decimal text** — and argues for it: a materialised module imports
//! nothing but `dataclasses`, `enum` and `typing`, so a runtime class
//! there would make every generated file depend on a layer the
//! materialiser is not part of. [`Python::scalar`] answers `Float128`.
//!
//! The runtime settles which of the two a *codec* may use, and it is not
//! this file's preference: `Writer.write_f128` takes `Float128 | float`
//! and `Reader.read_f128` answers a `Float128`. Neither takes a `str`,
//! so a codec built on the materialiser's spelling would hand a decimal
//! string to a method that has no branch for one.
//!
//! Nothing enforces the annotation at run time, so the two disagree on
//! paper and not on the wire — `primitive-longdouble` and
//! `sequence-double-and-longdouble` are byte-identical in both
//! directions under all ten encoding and byte-order pairs. What it does
//! mean is that a generated module carrying both would not pass a strict
//! type check, and the answer is one of two decisions neither of which
//! is a codec's to take: the materialiser reaches for the runtime class
//! and gives up its independence from this layer, or the codec converts
//! at the member boundary — for which the shared walk offers no seam,
//! because `ScalarWrite::F128` hands the member straight to
//! `write_f128` with no cast in between.
//!
//! # What used to sit beside this file
//!
//! `crates/lang-python/src/emit.rs` was a second Python emitter, written
//! against text rather than against a tree, driven by `zerodds-idl-python`
//! and `tools/wireprobe/wireprobe-zerodds-python`. It carried its own
//! copy of the walk — which members go out, in what order, under which
//! header — and that copy was the thing this file exists to stop
//! needing: `tools/wireprobe/wireprobe-zerodds-python-gen` and
//! `tools/idlc/src/python_support.rs` are built on this file now, the
//! same as the C#, Java, C, C++ and TypeScript columns are built on
//! their own, and `crates/lang-python` is gone.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::python::{declared_path, type_tag};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, MemberNode, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::python::{self as py, escape_name};

/// The Python backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Python;

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

/// The type name [`CodecTarget::named`] is handed for C's signed `char`.
///
/// `emit/encode.rs`'s `map_key_less` writes `T::named("signed char")`
/// where a `map`'s key is a `char` and the entry order is
/// `MapKeyOrdering::SignedInteger`. That is a C spelling in the walk and
/// not a type any other column has; this column recognises the string
/// and answers with the conversion rather than with a name Python has
/// never heard of. See [`Python::cast`].
const SIGNED_CHAR: &str = "signed char";

impl Python {
    /// A dotted name as a path.
    fn path(name: &str) -> Path {
        Path::new(&name.split('.').collect::<Vec<_>>())
    }

    /// A name in `runtime/python`'s `zerodds_wire`, as the generated
    /// module writes it.
    ///
    /// The simple name and not `zerodds_wire.X`: a generated module
    /// imports what it uses by name, the way
    /// `tools/wireprobe/wireprobe-zerodds-python-gen` writes the import
    /// line, so a name the runtime does not export is an `ImportError`
    /// naming it at the top of the file rather than a `NameError` at the
    /// first sample.
    fn runtime_name(name: &str) -> Path {
        Path::single(name)
    }

    /// A body, with `pass` where the walk produced none.
    ///
    /// A Python suite cannot be empty. Every caller of this is a
    /// constructor that is about to indent, so the padding is applied
    /// where the body's owner is still in hand.
    fn suite(mut body: Vec<py::Statement>) -> Vec<py::Statement> {
        if body.is_empty() {
            body.push(py::Statement::Pass);
        }
        body
    }

    /// A reader's body, with the sample it answers with around it.
    ///
    /// `v = zd_def_T()` … `return v`. Two things are settled here and
    /// neither is a spelling:
    ///
    /// - **Where the sample comes from.** Table 9's value, through the
    ///   generated defaulter, and not `T()` — a generated dataclass has
    ///   required fields and constructing one without them is a
    ///   `TypeError`, which is exactly what the materialiser meant by
    ///   leaving them required. Every position the stream does not carry
    ///   is then already what DDS-XTypes 1.3 Table 9 says it is, without
    ///   the walk having to reach each one.
    /// - **What the name is.** The defaulter and the decoder are keyed
    ///   the same way — `emit::decode::decoder_name` and
    ///   [`crate::default_python::defaulter_name`] differ in the three
    ///   letters after `zd_` — so the one is the other with the verb
    ///   changed. A name that does not resolve is a `NameError` naming
    ///   the function.
    fn reading_body(name: &str, body: Vec<py::Statement>) -> Vec<py::Statement> {
        let defaulter = crate::default_python::defaulter_from_decoder(name);
        let mut statements = vec![py::Statement::Set {
            place: py::Expr::name("v"),
            value: Self::call(&defaulter, Vec::new()),
        }];
        statements.extend(body);
        statements.push(py::Statement::Return(Some(py::Expr::name("v"))));
        statements
    }

    /// A runtime call whose Python parameters are not in the walk's
    /// order, written with its arguments named.
    ///
    /// Three of them, and all three are the bit-bounded kinds:
    /// `runtime/python`'s `write_enum(value, bit_bound)`,
    /// `write_bitmask(value, bit_bound)` and `write_bitset(value,
    /// anchor)` take the value first, and the walk hands the bound
    /// first — `call::<T>("write_enum", vec![int(bits), value])`.
    ///
    /// Named rather than transposed silently, because both arguments are
    /// integers: a positional swap is a `@bit_bound(8)` enumerator
    /// written as the value 8 with the enumerator as the bound, which
    /// raises nothing at the call and produces a plausible octet. With
    /// the names written out, a signature that moves is a `TypeError` at
    /// the call naming the parameter.
    ///
    /// The read side needs none of this: `read_enum(bit_bound)`,
    /// `read_bitmask(bit_bound)` and `read_bitset(anchor)` each take one
    /// argument, and every other operation on both sides already agrees
    /// with the walk's order.
    fn keywords_for(method: &str) -> Option<[&'static str; 2]> {
        match method {
            "write_enum" | "write_bitmask" => Some(["bit_bound", "value"]),
            "write_bitset" => Some(["anchor", "value"]),
            _ => None,
        }
    }
}

impl CodecTarget for Python {
    type Expr = py::Expr;
    type Stmt = py::Statement;
    /// A suite, which in Python is a list of statements and nothing
    /// else: there is no brace to hold and the indent belongs to the
    /// renderer.
    type Block = Vec<py::Statement>;
    type Type = py::Type;
    type Decl = py::Statement;

    // -- types --------------------------------------------------------

    /// One `int` for every integer width, because Python has one.
    ///
    /// Not a simplification. A Python `int` is unbounded, so `octet` and
    /// `unsigned long long` are held exactly by the same type and
    /// neither loses a bit at the assignment. What that costs is that
    /// the type no longer says the width — which is why
    /// `adapter::python::type_tag` names a codec after the IDL kind and
    /// why `adapter/python.rs` writes every member's width into
    /// `typing.Annotated` metadata rather than into the annotation.
    ///
    /// A `char` and a `wchar` are a `str` of one, which is the only
    /// character type Python has, and what `runtime/python`'s
    /// `write_char` and `write_wchar` take.
    fn scalar(kind: Scalar) -> Self::Type {
        match kind {
            Scalar::Bool => py::Type::name("bool"),
            Scalar::Char => py::Type::name("str"),
            Scalar::Int8
            | Scalar::UInt8
            | Scalar::Int16
            | Scalar::UInt16
            | Scalar::Int32
            | Scalar::UInt32
            | Scalar::Int64
            | Scalar::UInt64
            | Scalar::Index
            | Scalar::Size => py::Type::name("int"),
            Scalar::Float | Scalar::Double => py::Type::name("float"),
            Scalar::LongDouble => py::Type::name("Float128"),
        }
    }

    fn named(name: &str) -> Self::Type {
        py::Type::Name(Self::path(name))
    }

    /// `None`, which is what a Python function that answers with nothing
    /// is annotated with.
    fn nothing() -> Self::Type {
        py::Type::name("None")
    }

    /// The runtime's handles, by their exported names.
    ///
    /// Each is a `dataclass` or an `Enum` in `runtime/python`, and a
    /// generated module imports it by name.
    fn handle(kind: Handle) -> Self::Type {
        py::Type::name(match kind {
            Handle::DHeader => "DHeader",
            Handle::EmHeader => "EmHeader",
            Handle::Pid => "Pid",
            Handle::TryConstruct => "TryConstruct",
            Handle::ReaderMark => "ReaderMark",
            Handle::PidRead => "PidRead",
            Handle::EmHeaderRead => "EmHeaderRead",
        })
    }

    /// A `str` of one character is what a `char` is here.
    ///
    /// `runtime/python`'s `write_char` takes the character and refuses a
    /// code point above U+00FF rather than truncating it — which is a
    /// decision about the wire and is the runtime's, not this file's.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    /// The runtime's policy values, in the spelling `runtime/python`
    /// accepts.
    ///
    /// **Strings and integers, not an enumeration per axis.** That is
    /// the runtime's shape and not this file's preference:
    /// `Writer.write_wstring` matches `policy` against
    /// `_WSTRING_POLICIES` and `Writer.write_wchar` takes `width` as the
    /// octet count. The spellings are `WstringLengthPolicy::as_str` and
    /// its siblings in `crates/ast-layout/src/key.rs`, which is where
    /// the measurement lives, and they are **not** the TypeScript
    /// column's: `runtime/typescript` writes `octets-terminated-2` where
    /// this one writes `octets/terminated/2`, and a value copied across
    /// would be refused by name at the first sample.
    ///
    /// `crates/lang-rules/tests/python_names_its_runtime.rs` reads both
    /// files and compares them, because nothing between this match and
    /// the interpreter looks at the string.
    ///
    /// One arm per constant and no catch-all: a policy this column has
    /// not implemented has to be a compile error here rather than a
    /// value that looks plausible.
    fn constant(which: Constant) -> Self::Expr {
        let text = |value: &str| py::Expr::Literal(Literal::Str(value.to_string()));
        match which {
            // The octet count itself. `write_wchar(width=…)` takes the
            // number, so there is no name to write.
            Constant::WcharWidth2 => py::Expr::int(2),
            Constant::WcharWidth4 => py::Expr::int(4),
            Constant::WcharFixed => text("fixed"),
            Constant::WcharGiopLengthPrefixed => text("giop-length-prefixed"),
            Constant::WstringUnitsStream => text("stream"),
            Constant::WstringUnitsBigEndianAlways => text("big-endian-always"),
            Constant::WstringUnitsByteOrderMark => text("byte-order-mark"),
            Constant::WstringUnitsHostUnswapped => text("host-unswapped"),
            Constant::WstringSpecification => text("specification"),
            Constant::WstringCharacterCount => text("character-count"),
            Constant::WstringCharacterCountTerminated => text("character-count-terminated"),
            Constant::WstringSpecificationWideUnits => text("specification-wide-units"),
            Constant::WstringRosTypeSupport => text("ros-type-support"),
            // The three cells nobody has named, spelled by their
            // coordinates. `runtime/python` reaches every cell of
            // C1 x C2 x C3 by `counts/terminator/width` as well as by
            // name, and these are the coordinate forms.
            Constant::WstringOctetsTerminated2 => text("octets/terminated/2"),
            Constant::WstringOctetsTerminated4 => text("octets/terminated/4"),
            Constant::WstringCharactersTerminated2 => text("characters/terminated/2"),
            Constant::FixedDeclared => text("declared"),
            Constant::FixedValuePrecision => text("value-precision"),
            Constant::FixedValueStringDigits => text("value-string-digits"),
            Constant::ParameterMember => py::Expr::Path(Self::path("ParameterRole.MEMBER")),
            Constant::ParameterOptional => py::Expr::Path(Self::path("ParameterRole.OPTIONAL")),
            Constant::TryConstructDiscard => py::Expr::Path(Self::path("TryConstruct.DISCARD")),
            Constant::TryConstructTrim => py::Expr::Path(Self::path("TryConstruct.TRIM")),
            Constant::TryConstructUseDefault => {
                py::Expr::Path(Self::path("TryConstruct.USE_DEFAULT"))
            }
            Constant::StatusOk => py::Expr::Path(Self::path("Status.OK")),
            Constant::StatusDiscarded => py::Expr::Path(Self::path("Status.DISCARDED")),
            Constant::StatusImpossibleLength => {
                py::Expr::Path(Self::path("Status.IMPOSSIBLE_LENGTH"))
            }
            Constant::StatusOutOfRange => py::Expr::Path(Self::path("Status.OUT_OF_RANGE")),
            Constant::StatusTruncated => py::Expr::Path(Self::path("Status.TRUNCATED")),
        }
    }

    /// `int`, at every bit bound.
    ///
    /// The one place this column's unbounded integer is simply the right
    /// answer: a `@bit_bound(64)` set and a `@bit_bound(3)` set are
    /// assembled in the same type and neither can overflow it. The other
    /// columns pick the smallest holder that fits and one of them —
    /// Java — has to go up a signedness to do it.
    fn unsigned_holder(_bits: u16) -> Self::Type {
        py::Type::name("int")
    }

    /// The Python type of a value of `id`.
    ///
    /// Not [`Self::type_name`], which is the codec's tag — `p_long`,
    /// `seq_long` — and names a function, not a type.
    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        value_type_of(ast, id, 0)
    }

    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        type_tag(ast, id, 0).unwrap_or_else(|_| format!("unresolved_type_{}", id.slot()))
    }

    /// A `map`'s entry type, which in this column is not a type.
    ///
    /// §7.4.1.1.4 defines a `map` as a sequence of entries, and
    /// `adapter/python.rs` writes that as `list[tuple[K, V]]` with no
    /// entry class at all — "the one place this column declares less
    /// than the Java one". So there is no name to answer with, and the
    /// walk needs one for exactly two things: the `element` argument of
    /// [`Self::make_room`] and the type of [`Self::storage`]'s
    /// `zd_scratch`. Both are answered here from the shape rather than
    /// from a declaration, and this name is what says which shape it is.
    ///
    /// A `list` and not a `tuple`, because the reader writes each half
    /// into the slot separately — `v[i][0] = …` — and a tuple cannot be
    /// written into at all. [`Self::make_room`] turns the run back into
    /// tuples once both halves are there, so what a decoder answers with
    /// is what `from_json` built and what the encoder reads.
    fn map_entry_name(_ast: &Ast, _id: Id<TypeNode>) -> String {
        MAP_ENTRY.to_string()
    }

    /// Nothing. `adapter/python.rs` nests an IDL module in a class and
    /// every reference is written from the module root, which
    /// [`Self::enumerator_constant`] and `value_type_of` do through
    /// `declared_path`. A prefix would be a second, shorter answer to
    /// the same question.
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    fn escape(name: &str) -> String {
        escape_name(name)
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some(format!("{}.", escape_name("parent")).repeat(depth))
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        py::Expr::Path(Self::path(text))
    }

    fn int(value: i128) -> Self::Expr {
        py::Expr::int(value)
    }

    /// The number.
    ///
    /// The trait's default is C's `UINT64_C(..)`, which is a macro no
    /// Python file declares, and the whole reason for a widened literal
    /// is a language whose integer constants have a type. Python's do
    /// not: `18446744073709551615` is an `int` and so is
    /// `-9223372036854775808`.
    fn wide_int(value: i128) -> Self::Expr {
        py::Expr::int(value)
    }

    /// As [`Self::wide_int`], and the value rather than the
    /// two's-complement pattern: a Python `int` is unbounded, so a
    /// negative enumerator is written as the negative number it is.
    fn wide_unsigned(value: u64) -> Self::Expr {
        py::Expr::int(i128::from(value))
    }

    /// As [`Self::wide_unsigned`].
    fn wide_signed(value: i128) -> Self::Expr {
        py::Expr::int(value)
    }

    /// The number, with no suffix.
    ///
    /// The trait's default is C's `123u`. `2u` is not a Python
    /// expression at all — it is a `SyntaxError`, "invalid decimal
    /// literal" — and every array bound and every string bound the
    /// decode walk writes goes through here.
    fn unsigned_int(value: i128) -> Self::Expr {
        py::Expr::int(value)
    }

    /// A literal, with the two raw tokens the walk escapes through
    /// [`Literal::Int`] rewritten.
    ///
    /// `emit/decode.rs`'s `literal::<T>("false")` carries a *keyword* in
    /// an integer literal, because that variant is the tree's raw-token
    /// escape and `false` is what C, C++, C#, Java and TypeScript all
    /// spell a false boolean with. Python spells it `False`, and a file
    /// carrying `zd_seen0 = false` is a `NameError` at the first
    /// `@mutable` member — or worse, a name something else in scope
    /// bound. Twelve sites in the decode walk reach this.
    fn literal(value: Literal) -> Self::Expr {
        let Literal::Int(text) = &value else {
            return py::Expr::Literal(value);
        };
        match text.as_str() {
            "true" => py::Expr::Literal(Literal::Bool(true)),
            "false" => py::Expr::Literal(Literal::Bool(false)),
            _ => py::Expr::Literal(value),
        }
    }

    /// A call to a module-level function.
    ///
    /// `strcmp` is the one name the walk hands here that is not a
    /// function this column generates, and it is C's. See
    /// `strcmp_sign` for what it becomes and why.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if callee == "strcmp" {
            return strcmp_sign(args);
        }
        py::Expr::Call {
            func: Self::path(callee),
            args: args.into_iter().map(py::Arg::Positional).collect(),
        }
    }

    /// `w.write_u32(x)` — a method on the writer or the reader.
    ///
    /// The runtime's methods are named in the wire's convention already,
    /// which is `snake_case`, so unlike the Java, C# and TypeScript
    /// backends there is nothing to convert: `write_u32` is
    /// `write_u32`. What there *is* to do is name the arguments of the
    /// three operations whose Python signature is not in the walk's
    /// order — see `Python::keywords_for`.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        // `read_pid()` is the walk's name for "read a header in the
        // MEMBER role", and in `runtime/python` that name belongs to the
        // operation `crates/lang-python`'s decoder uses — which reads
        // the header and opens no value block, because that column opens
        // one itself with `begin_parameter_value`. The walk has no such
        // pair: it reads a header and later calls `finish_pid_read`, so
        // the block has to be opened by the read. `read_pid_as` is the
        // entry point that does, and §7.4.1.2 makes the difference a
        // wire question — a member's internal alignment is counted from
        // the first octet after its header — so it is not one this
        // backend may answer by leaving the call as it stands.
        if method == "read_pid" && args.is_empty() {
            return Self::runtime(
                receiver,
                "read_pid_as",
                vec![Self::constant(Constant::ParameterMember)],
            );
        }
        let func = Path::new(&[receiver, method]);
        let Some(names) = Python::keywords_for(method) else {
            return py::Expr::Call {
                func,
                args: args.into_iter().map(py::Arg::Positional).collect(),
            };
        };
        // Only where the walk handed exactly the two the table names.
        // A call that grew a third argument falls back to positional
        // and becomes a `TypeError` naming the parameter, which is
        // louder than a keyword quietly applied to the wrong slot.
        if args.len() != names.len() {
            return py::Expr::Call {
                func,
                args: args.into_iter().map(py::Arg::Positional).collect(),
            };
        }
        py::Expr::Call {
            func,
            args: args
                .into_iter()
                .zip(names)
                .map(|(value, name)| {
                    py::Arg::Keyword(py::Keyword {
                        name: Ident::new(name),
                        value,
                    })
                })
                .collect(),
        }
    }

    /// `runtime/python` carries the three HDDS axes:
    /// `begin_packed_element` / `finish_packed_element` on the writer and
    /// the reader, the DHEADER pair for a sequence element's own length,
    /// and `write_wstring_utf8_octets` / `read_wstring_utf8_octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    /// A free function the generated module imports from the runtime.
    ///
    /// Two of the three the walk asks for are module-level functions in
    /// `zerodds_wire` — `no_dheader`, `utf8_octets`, `utf16_units`. The
    /// third is not: `pid_is_user_member(header)` is
    /// `PidRead.is_user_member()` here, a method on the header the
    /// reader handed back, which is the same difference as C's free
    /// function against Java's static and is decided here rather than in
    /// the walk.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if method == "pid_is_user_member" {
            if let [py::Expr::Path(header)] = args.as_slice() {
                let mut segments: Vec<String> = header
                    .segments()
                    .iter()
                    .map(|segment| segment.as_str().to_string())
                    .collect();
                segments.push("is_user_member".to_string());
                return py::Expr::Call {
                    func: Path::new(&segments.iter().map(String::as_str).collect::<Vec<&str>>()),
                    args: Vec::new(),
                };
            }
        }
        py::Expr::Call {
            func: Self::runtime_name(method),
            args: args.into_iter().map(py::Arg::Positional).collect(),
        }
    }

    fn member(base: &str, field: &str) -> Self::Expr {
        py::Expr::Attribute {
            value: Box::new(py::Expr::name(base)),
            name: Ident::new(field),
        }
    }

    fn whole(base: &str) -> Self::Expr {
        py::Expr::name(base)
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        py::Expr::Attribute {
            value: Box::new(value),
            name: Ident::new(field),
        }
    }

    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        py::Expr::Index {
            value: Box::new(value),
            index: Box::new(at),
        }
    }

    /// `len(v)`, or the bound where the materialiser fixed one.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => py::Expr::int(i128::from(bound)),
            None => py::Expr::Call {
                func: Path::single("len"),
                args: vec![py::Arg::Positional(py::Expr::name(base))],
            },
        }
    }

    fn elements(base: &str) -> Self::Expr {
        py::Expr::name(base)
    }

    /// `v.field` — a union's selected case member.
    ///
    /// `adapter/python.rs` declares a union as one dataclass with `_d`
    /// and one `typing.Optional` field per case, so a case member is a
    /// plain attribute. This is the Java shape and not the TypeScript
    /// one, which needs the value narrowed to an arm before the property
    /// exists at all.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `entry[0]` and `entry[1]`.
    ///
    /// A `map`'s entry is a two-element sequence here and not a class
    /// with named halves: `adapter/python.rs` writes
    /// `list[tuple[K, V]]`, and §7.4.1.1.4's "sequence of entries" is
    /// what the wire form is. The index and not a name because there is
    /// no name to use.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        py::Expr::Index {
            value: Box::new(entry),
            index: Box::new(py::Expr::int(i128::from(!key))),
        }
    }

    fn absent() -> Self::Expr {
        py::Expr::Literal(Literal::Null)
    }

    /// Nothing.
    ///
    /// The guard this asks for is C's: `zdw_read_string_alloc` answers
    /// `NULL` where the reader's arena is exhausted, and every use of
    /// what it produced is tested against that. A Python `str` owns its
    /// own storage and there is no such state, so the guard is not
    /// written at all rather than written against a value that is always
    /// there.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    /// The member itself. `typing.Optional[T]` is the value or `None`,
    /// with no wrapper to unwrap the way C#'s `Nullable<T>` has.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `v.field is not None`.
    ///
    /// `is not` and not `!=`. `!=` calls `__eq__`, which a generated
    /// dataclass defines over its fields, so `v.field != None` is a
    /// structural comparison against `None` rather than an identity
    /// test — and for a dataclass whose `__eq__` returns
    /// `NotImplemented` against a non-dataclass, Python falls back to
    /// identity and the two agree only by accident. PEP 8 says the same
    /// thing: "Comparisons to singletons like None should always be done
    /// with `is` or `is not`."
    fn present(base: &str, field: &str) -> Self::Expr {
        py::Expr::Binary {
            op: "is not",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::absent()),
        }
    }

    /// Nothing on the way in, `v.field = None` on the way out.
    ///
    /// Writing the member is what makes it present — the field's
    /// declared default is `None` and the reader starts from Table 9's
    /// sample — so the present case has no statement to add.
    /// `None` in every case field: `adapter/python.rs` declares them
    /// `typing.Optional[...] = None`, and the class promises that at most
    /// one is live.
    fn clear_union_cases(base: &str, fields: &[String]) -> Vec<Self::Stmt> {
        fields
            .iter()
            .map(|field| py::Statement::Set {
                place: Self::member(base, field),
                value: Self::absent(),
            })
            .collect()
    }

    /// The default's rows as lists, outermost first: `v = list(v)`, then
    /// `v[zd_i0] = list(v[zd_i0])` inside a loop per further dimension.
    ///
    /// `adapter/python.rs` declares an array member `tuple[T, …]` and
    /// `default_python.rs` answers a tuple to match, which the reader
    /// cannot assign an element into; [`Self::close_array`] turns every
    /// row back once the elements are in.
    fn open_array(base: &str, lengths: &[i128]) -> Vec<Self::Stmt> {
        array_rows(base, lengths, 0, false)
    }

    /// Every row a tuple again, innermost first, so what the reader
    /// answers with is the `tuple[T, …]` the member declares. A list in
    /// its place compares unequal to the tuple a caller wrote —
    /// `(1, 2) == [1, 2]` is `False` — and a sample read back from the
    /// wire was not `==` to the one written (measured:
    /// `idl-conformance/python`, `combo::Telemetry.window`).
    fn close_array(base: &str, lengths: &[i128]) -> Vec<Self::Stmt> {
        array_rows(base, lengths, 0, true)
    }

    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![py::Statement::Set {
            place: Self::member(base, field),
            value: Self::absent(),
        }]
    }

    /// The member, with nothing to do first.
    ///
    /// `adapter/python.rs`'s optional member is a `typing.Optional[T]`
    /// field that already exists on the dataclass, so unlike C++'s
    /// `std::optional` there is nothing to engage before there is
    /// somewhere to write.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::member(base, field))
    }

    /// The operators, in Python's spelling.
    ///
    /// `&&` and `||` are `and` and `or`, which are not the same
    /// operators as `&` and `|`: the bitwise pair does not short-circuit
    /// and returns an integer. The walk uses both — `status_is_ok() &&
    /// reader_position() < zd_end` needs the short circuit, and a
    /// bitmask's undeclared-bit test needs the bitwise `&` — and mapping
    /// them onto one would break whichever it was not.
    ///
    /// `=` is left as it is and never reaches a rendered expression:
    /// [`Self::perform`] is the only thing that receives one and turns
    /// it into a statement.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        py::Expr::Binary {
            op: match op {
                "&&" => "and",
                "||" => "or",
                other => other,
            },
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    /// `not a` and `~a`.
    ///
    /// `not` carries its trailing space because `notx` is an identifier;
    /// `~` takes none. The tree records which, so the renderer does not
    /// have to guess.
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        py::Expr::Unary {
            op: match op {
                "!" => "not ",
                other => other,
            },
            value: Box::new(value),
        }
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        py::Expr::Paren(Box::new(value))
    }

    /// The identity on every integer, and a rewrite on three things that
    /// are not integers.
    ///
    /// **Why there is no narrowing.** A Python `int` holds every IDL
    /// width exactly, so `(long) x` and `static_cast<int32_t>(x)` have no
    /// counterpart: there is nothing the value could have lost. The
    /// runtime range-checks what it writes — `Writer._check` — so a value
    /// outside the type raises at the octet rather than being folded
    /// silently, which is the opposite trade from a C cast and is the
    /// one this column wants.
    ///
    /// **Three rewrites, all at a union's discriminator.** The walk
    /// writes `cast(value_type(discriminator), wide_int(label))` for
    /// every case label, and the label arrives as an integer:
    ///
    /// - a `char` or `wchar` discriminator holds a `str`, and
    ///   `v._d == 97` is `False` for every sample. The label is the code
    ///   point and the member is the character, so the literal becomes
    ///   one.
    /// - a `boolean` discriminator holds a `bool`. `v._d == 1` happens
    ///   to be true in Python — `True == 1` — but `False` is what the
    ///   sample carries and `True`/`False` is what says so.
    /// - `signed char` is not a Python type at all; see `SIGNED_CHAR`
    ///   and `strcmp_sign`.
    ///
    /// An enumeration discriminator needs none of it: `adapter/python.rs`
    /// declares an `enum.IntEnum`, whose members *are* `int`s, so the
    /// label compares equal to the enumerator without either side moving.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if type_ == py::Type::name(SIGNED_CHAR) {
            return signed_char(value);
        }
        let py::Expr::Literal(Literal::Int(text)) = &value else {
            return value;
        };
        let digits = text.strip_prefix('-').unwrap_or(text);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return value;
        }
        if type_ == py::Type::name("str") {
            // `chr(<code>)` and not a character literal: `Literal::Char`
            // carries a Rust `char`, which is a Unicode scalar value and
            // rejects `0xD800..=0xDFFF`, while a Python `str` holds any
            // code point and §7.4.1.1.1 writes a `wchar` as one UTF-16
            // code unit — a lone surrogate among them.
            //
            // Going through `char::from_u32` left the *integer* standing
            // where a one-character `str` belongs, and Python compares
            // an `int` to a `str` as unequal without a word: a `wchar`
            // discriminator whose case label is a surrogate would select
            // no branch, every time, silently. The frontend admits that
            // label, because 0xD800 is inside a `wchar`'s range.
            return text
                .parse::<u32>()
                .ok()
                .filter(|code| *code <= u32::from(u16::MAX))
                .map_or_else(
                    || value.clone(),
                    |code| match char::from_u32(code) {
                        // The character itself where Rust can spell it,
                        // so `case 'a'` stays `"a"` in the generated
                        // file and reads as the label it came from.
                        Some(point) => py::Expr::Literal(Literal::Char(point)),
                        // `chr(..)` for the surrogates, which are code
                        // units a `wchar` carries and Rust's `char` will
                        // not hold.
                        None => py::Expr::Call {
                            func: Path::single("chr"),
                            args: vec![py::Arg::Positional(py::Expr::int(i128::from(code)))],
                        },
                    },
                );
        }
        if type_ == py::Type::name("bool") {
            return py::Expr::Literal(Literal::Bool(text != "0"));
        }
        value
    }

    /// The value, unchanged.
    ///
    /// [`Handoff`] abstracts C's `&v->x` against `v.x`. Python passes
    /// every object by reference and takes no address, so both forms are
    /// the value.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    // -- statements ----------------------------------------------------

    /// `name: T = value`, or `name = value` where there is nothing to
    /// initialise it with.
    ///
    /// The annotation is written because it costs nothing and says the
    /// width the Python type does not: a reader of the generated file
    /// sees `zd_n: int = r.read_u32()` and knows what the local is for.
    /// An uninitialised local is a **binding with no value**, which
    /// Python does have — `zd_h: EmHeaderRead` declares the annotation
    /// and binds nothing — and that is exactly what the walk means when
    /// it declares `zd_h` and assigns it inside the loop.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        py::Statement::Annotated(py::Annotated {
            doc: Doc::none(),
            name: Ident::new(name),
            type_,
            value,
        })
    }

    /// An expression evaluated for its effect — or, where the walk built
    /// an assignment, the assignment.
    ///
    /// `a = b` is not an expression in Python and `(a = b)` is a
    /// `SyntaxError`. The shared walk's `assign` helper and its
    /// `increment` both build `T::binary("=", …)` and hand it here, and
    /// this is the only place either can arrive: nothing else in
    /// `emit/encode.rs` or `emit/decode.rs` puts an `=` anywhere but
    /// directly inside a `T::perform`. So the shape is recognised here
    /// and nowhere else, and an `=` that escaped into a nested
    /// expression would be a `SyntaxError` in the generated file naming
    /// the line — which is louder than a walrus quietly standing in for
    /// it.
    fn perform(value: Self::Expr) -> Self::Stmt {
        if let py::Expr::Binary { op: "=", lhs, rhs } = value {
            return py::Statement::Set {
                place: *lhs,
                value: *rhs,
            };
        }
        py::Statement::Expr(value)
    }

    /// `return v`.
    ///
    /// Every `T::give_back` the decode walk builds is `give_back(None)`,
    /// which is C's shape: a `void` decoder that wrote through a
    /// pointer. This column's decoders answer with the sample — see
    /// [`Self::routine`] and [`Self::read_call`] — so a bare `return`
    /// would answer `None` and the caller would assign it. On a refusal
    /// the partially filled `v` goes back, exactly as the TypeScript
    /// column does, and the caller reads `reader_status()` to find out
    /// what it is.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        py::Statement::Return(Some(value.unwrap_or_else(|| py::Expr::name("v"))))
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        py::Statement::If {
            condition,
            then: Self::suite(then),
            // An `else:` the walk built empty is dropped rather than
            // padded: `else: pass` says nothing and reads as though a
            // branch had been forgotten.
            otherwise: otherwise.filter(|body| !body.is_empty()),
        }
    }

    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        py::Statement::For {
            counter: Ident::new(counter),
            bound,
            body: Self::suite(body),
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        py::Statement::While {
            condition,
            body: Self::suite(body),
        }
    }

    fn leave() -> Self::Stmt {
        py::Statement::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        py::Statement::Comment(doc)
    }

    /// The call, and nothing around it.
    ///
    /// The trait's default is `(void) expr;`, which is C's way of saying
    /// an ignored return was meant. Python has no such diagnostic and
    /// `(None) expr` is not a statement at all, so the call stands
    /// alone.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        py::Statement::Expr(value)
    }

    /// Nothing.
    ///
    /// `-Wunused-parameter -Werror` is what this exists for and Python
    /// has no counterpart; `zd_tc` on a line of its own is a legal
    /// statement that does nothing, which is worse than no statement.
    fn mark_used(_name: &str) -> Vec<Self::Stmt> {
        Vec::new()
    }

    /// `place = zd_dec_T(r, zd_tc)`.
    ///
    /// The trait's default hands the destination *to* the callee, which
    /// is C's shape: its decoder takes a `T *` and fills it. A Python
    /// function has no out-parameter — rebinding a parameter rebinds the
    /// local and nothing else — so this column's decoders answer with
    /// the value and the destination is assigned at the call site.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        py::Statement::Set {
            place,
            value: Self::call(callee, vec![py::Expr::name("r"), behaviour]),
        }
    }

    /// `v = zd_dec_T(r, zd_tc)` — a `typedef`, handed straight on.
    fn read_forward(callee: &str) -> Self::Stmt {
        Self::read_call(
            callee,
            py::Expr::name("v"),
            Handoff::Value,
            py::Expr::name("zd_tc"),
        )
    }

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        statements
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block
    }

    /// Room for `count` values, and the fill inside it.
    ///
    /// Two shapes, and the difference is `assigned_whole` — which is the
    /// difference between a slot that *is* storage and a slot that has
    /// to be one before anything can be written into it.
    ///
    /// **A sequence** (`assigned_whole`): `v = [None] * zd_n`, because
    /// the element decoder assigns the slot outright. `[None] * n` and
    /// not `[[..]] * n`: `None` is immutable and repeating it aliases
    /// nothing, which is not true of the map's shape below.
    ///
    /// **A `map`** (not `assigned_whole`): each entry is written half at
    /// a time — `v[i][0] = …`, `v[i][1] = …` — so each slot has to be a
    /// *distinct* mutable pair. `[[None, None]] * n` would be one list
    /// repeated `n` times and every entry would be the last one read;
    /// the loop that appends is what makes them distinct. Once both
    /// halves are there the run is turned back into tuples, so what the
    /// decoder answers with is what `adapter/python.rs` declared —
    /// `list[tuple[K, V]]` — and what `from_json` built.
    fn make_room(
        base: &str,
        _element: Self::Type,
        count: Self::Expr,
        assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let slot = |index: py::Expr| py::Expr::Index {
            value: Box::new(py::Expr::name(base)),
            index: Box::new(index),
        };
        let mut out = Vec::new();
        if assigned_whole {
            out.push(py::Statement::Set {
                place: py::Expr::name(base),
                value: py::Expr::Binary {
                    op: "*",
                    lhs: Box::new(py::Expr::List(vec![py::Expr::Literal(Literal::Null)])),
                    rhs: Box::new(count.clone()),
                },
            });
            out.extend(fill);
            return out;
        }
        out.push(py::Statement::Set {
            place: py::Expr::name(base),
            value: py::Expr::List(Vec::new()),
        });
        out.push(py::Statement::For {
            counter: Ident::new(ENTRY_INDEX),
            bound: count.clone(),
            body: vec![py::Statement::Expr(py::Expr::Call {
                func: Path::new(&[base, "append"]),
                args: vec![py::Arg::Positional(py::Expr::List(vec![
                    py::Expr::Literal(Literal::Null),
                    py::Expr::Literal(Literal::Null),
                ]))],
            })],
        });
        out.extend(fill);
        out.push(py::Statement::For {
            counter: Ident::new(ENTRY_INDEX),
            bound: count,
            body: vec![py::Statement::Set {
                place: slot(py::Expr::name(ENTRY_INDEX)),
                value: py::Expr::Call {
                    func: Path::single("tuple"),
                    args: vec![py::Arg::Positional(slot(py::Expr::name(ENTRY_INDEX)))],
                },
            }],
        });
        out
    }

    /// `v = v[:count]`.
    ///
    /// §7.3.1.2.1.12.3's TRIM for a bounded collection, applied after
    /// every element has been read: stopping at the bound would leave
    /// the rest of the sequence in the stream and put every following
    /// member at the wrong offset.
    ///
    /// A slice and not `del v[count:]`, which does the same thing to a
    /// `list` and not to a `tuple` — and the walk reaches this for a
    /// bounded sequence, whose Python holder `adapter/python.rs` is free
    /// to change.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![py::Statement::Set {
            place: py::Expr::name(base),
            value: py::Expr::Index {
                value: Box::new(py::Expr::name(base)),
                index: Box::new(py::Expr::Slice {
                    start: None,
                    end: Some(Box::new(count)),
                }),
            },
        }]
    }

    /// The subscript, with nothing before or after it.
    ///
    /// The seam exists for `std::vector<bool>`, whose `operator[]`
    /// answers a proxy. A Python `list` element is an ordinary
    /// reference at every element type.
    fn sequence_element(
        base: &str,
        index: Self::Expr,
        _element: Self::Type,
    ) -> (Vec<Self::Stmt>, Self::Expr, Vec<Self::Stmt>) {
        (
            Vec::new(),
            Self::index(Self::elements(base), index),
            Vec::new(),
        )
    }

    /// A local the body writes *into*.
    ///
    /// One caller: `zd_scratch`, the entry of an uncounted `map`, which
    /// the reader fills a half at a time. There is only one type this is
    /// ever asked for — see [`Self::map_entry_name`] — and it is a
    /// two-slot list, so this answers with one rather than with a
    /// constructor call to a class this column does not declare.
    fn storage(type_: Self::Type, name: &str) -> Self::Stmt {
        if type_ == py::Type::name(MAP_ENTRY) {
            return py::Statement::Set {
                place: py::Expr::name(name),
                value: py::Expr::List(vec![
                    py::Expr::Literal(Literal::Null),
                    py::Expr::Literal(Literal::Null),
                ]),
            };
        }
        Self::declare(type_, name, None)
    }

    // -- assembly ------------------------------------------------------

    /// `def zd_enc_T(w, v) -> None:` and `def zd_dec_T(r, zd_tc) -> T:`.
    ///
    /// A forward declaration is answered with nothing: Python resolves a
    /// global at the call and not at the `def`, so a recursive or
    /// mutually recursive type needs no declaration ahead of its body.
    /// That is what `body: None` asks for and `None` is the honest
    /// answer — the caller emits nothing.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let body = spec.body?;
        let mut params = vec![py::Param {
            name: Ident::new(if spec.writing { "w" } else { "r" }),
            type_: Some(py::Type::name(if spec.writing {
                "Writer"
            } else {
                "Reader"
            })),
            default: None,
        }];
        if spec.writing {
            params.push(py::Param {
                name: Ident::new("v"),
                type_: Some(spec.sample.clone()),
                default: None,
            });
        }
        params.extend(spec.extra.into_iter().map(|(type_, name)| py::Param {
            name: Ident::new(&name),
            type_: Some(type_),
            default: None,
        }));
        let (returns, body) = if spec.writing {
            (Self::nothing(), body)
        } else {
            (spec.sample, Self::reading_body(&spec.name, body))
        };
        Some(py::Statement::Function(py::Function {
            doc: spec.doc,
            decorators: Vec::new(),
            name: Ident::new(&spec.name),
            params,
            returns: Some(returns),
            body: Self::suite(body),
        }))
    }

    fn initialized_reader(spec: Routine<Self>, initializer: Self::Stmt) -> Option<Self::Decl> {
        if spec.writing {
            return None;
        }
        let mut body = vec![initializer];
        body.extend(spec.body?);
        body.push(py::Statement::Return(Some(py::Expr::name("v"))));
        let mut params = vec![py::Param {
            name: Ident::new("r"),
            type_: Some(py::Type::name("Reader")),
            default: None,
        }];
        params.extend(spec.extra.into_iter().map(|(type_, name)| py::Param {
            name: Ident::new(&name),
            type_: Some(type_),
            default: None,
        }));
        Some(py::Statement::Function(py::Function {
            doc: spec.doc,
            decorators: Vec::new(),
            name: Ident::new(&spec.name),
            params,
            returns: Some(spec.sample),
            body: Self::suite(body),
        }))
    }

    /// A call to a generated defaulter and not a literal in place.
    ///
    /// A literal cannot carry an aggregate, and one written at every
    /// position a member can go missing would put the same answer in the
    /// generated file as many times as the type is reached — where two
    /// copies can differ. [`crate::default_python`] carries the whole
    /// table and the argument.
    ///
    /// # Errors
    /// Never; the signature matches the emitters around it.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        Ok(crate::default_python::default_into(ast, id, place))
    }

    /// # Errors
    /// As [`crate::default_python::member_default`]: a `@default` this
    /// column cannot spell as a Python expression is refused rather than
    /// silently replaced by Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_python::member_default(ast, node, type_, place)
    }

    /// `a.b.Colour.RED`, written from the module root.
    ///
    /// The trait's default answers the bare enumerator name, which is
    /// right for a target that flattens its modules into a prefix and
    /// wrong here: `adapter/python.rs` nests each IDL module in a class
    /// and an `enum.IntEnum` inside it, and a generated codec sits at the
    /// module root — where `RED` resolves to nothing, or to whatever
    /// else happened to be bound.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        let Ok(path) = declared_path(ast, id) else {
            return Self::name(&escape_name(enumerator));
        };
        let mut segments: Vec<String> = path
            .segments()
            .iter()
            .map(|segment| segment.as_str().to_string())
            .collect();
        segments.push(escape_name(enumerator));
        py::Expr::Path(Path::new(
            &segments.iter().map(String::as_str).collect::<Vec<&str>>(),
        ))
    }
}

/// The counter the two entry loops of [`Python::make_room`] run on.
///
/// Its own name and not the walk's `zd_i`: the fill the walk handed over
/// declares `zd_i` itself, and a loop of this file's that reused it
/// would leave `zd_i` bound to the wrong number after the fill — Python
/// has no block scope, so a `for` counter outlives its loop.
const ENTRY_INDEX: &str = "zd_e";

/// What [`CodecTarget::map_entry_name`] answers with.
///
/// Not a type this column declares. It is a marker the two seams that
/// receive it — [`Python::make_room`] and [`Python::storage`] — match
/// on, because a `map` entry has no Python class and both of them need
/// to know that the thing they are making room for is a two-slot list.
const MAP_ENTRY: &str = "_zd_map_entry";

/// A character's code point read as C's signed `char`.
///
/// `emit/encode.rs`'s `map_key_less` writes
/// `T::cast(T::named("signed char"), key)` where a `map`'s key is an IDL
/// `char` and the entry order is `MapKeyOrdering::SignedInteger` — which
/// `map-key-char` in the corpus reaches. The comparison "says the
/// signedness out loud rather than taking the C type's", because an IDL
/// `char` is a C `char` and its signedness is implementation-defined:
/// the corpus was recorded on a host where it is signed, so `U+00FF`
/// sorts *below* `U+0000` and a column comparing code points would order
/// the entries differently.
///
/// `((ord(k) + 128) % 256) - 128` is the whole of it: the two's
/// complement reading of one octet. `ord("\x00")` gives 0, `ord("\x7f")`
/// 127, `ord("\x80")` −128 and `ord("\xff")` −1.
///
/// Arithmetic and not a helper in `runtime/python`. A conditional
/// expression would read more plainly and the tree has no node for one;
/// a helper would put a *language mapping* question — how wide and how
/// signed C spells a character — into a runtime whose subject is §7.4,
/// where a second column could then reach it and inherit this column's
/// host.
fn signed_char(value: py::Expr) -> py::Expr {
    let point = py::Expr::Call {
        func: Path::single("ord"),
        args: vec![py::Arg::Positional(value)],
    };
    let wrapped = py::Expr::Binary {
        op: "%",
        lhs: Box::new(py::Expr::Binary {
            op: "+",
            lhs: Box::new(point),
            rhs: Box::new(py::Expr::int(128)),
        }),
        rhs: Box::new(py::Expr::int(256)),
    };
    py::Expr::Binary {
        op: "-",
        lhs: Box::new(wrapped),
        rhs: Box::new(py::Expr::int(128)),
    }
}

/// `strcmp(a, b)` as its sign, computed in Python.
///
/// `emit/encode.rs`'s `map_key_less` compares two string keys with
/// `strcmp(..) < 0` under `MapKeyOrdering::CodeUnits`. C11 §7.24.4 says
/// `strcmp` compares as `unsigned char`; Python's `str` comparison is by
/// code point, and UTF-8 is order preserving, so the two orders are the
/// same order and `(a > b) - (a < b)` is the same sign.
///
/// Written as an expression rather than added to `runtime/python`
/// deliberately: this is not a wire question and nothing about it
/// belongs in a runtime that implements §7.4. A helper there would be a
/// second place a column's key order could be decided.
fn strcmp_sign(args: Vec<py::Expr>) -> py::Expr {
    let [left, right] = <[py::Expr; 2]>::try_from(args).unwrap_or_else(|args| {
        // Unreachable for the one call site, which passes exactly two.
        // A different arity would be a walk that changed under this
        // file, and answering `0` would say the two keys are equal —
        // which reorders a `map` silently. `len(..)` of the arguments is
        // not something Python can be asked here, so the shape that
        // survives is a call to a name no module binds: a `NameError`
        // that says `strcmp` at the line it happened.
        [
            py::Expr::Call {
                func: Path::single("strcmp"),
                args: args.into_iter().map(py::Arg::Positional).collect(),
            },
            py::Expr::int(0),
        ]
    });
    let side = |op: &'static str| {
        py::Expr::Paren(Box::new(py::Expr::Binary {
            op,
            lhs: Box::new(left.clone()),
            rhs: Box::new(right.clone()),
        }))
    };
    py::Expr::Paren(Box::new(py::Expr::Binary {
        op: "-",
        lhs: Box::new(side(">")),
        rhs: Box::new(side("<")),
    }))
}

/// The Python type of a value, structurally.
///
/// The counterpart of `adapter::python::python_member_type`, which
/// spells a *member's* declared type and carries the width in
/// `typing.Annotated` metadata a codec has no use for. Both read the
/// same model and agree on every shape.
///
/// Every named answer is the path from the **module root** and not the
/// declaration's own name. `adapter/python.rs` nests an IDL module in a
/// class and a generated codec sits at the root, so `Point` there names
/// nothing; `module_prefix` answers the empty string because a target
/// with scopes has no prefix to add, and that is not the same as having
/// nothing to qualify.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
fn value_type_of(ast: &Ast, id: Id<TypeNode>, depth: usize) -> py::Type {
    use zerodds_ast::{PrimitiveKind, TypeKind};
    // `object` and not `typing.Any`: a value of `Any` may be used as
    // anything without a checker saying so, and a shape this column
    // could not spell is exactly the case that should not pass quietly.
    let unknown = || py::Type::name("object");
    if depth > MAX_TYPE_DEPTH {
        return unknown();
    }
    let Ok(node) = ast.type_(id) else {
        return unknown();
    };
    let declared = || declared_path(ast, id).map_or_else(|_| unknown(), py::Type::Name);
    let inner = |half: Option<Id<TypeNode>>| {
        half.map_or_else(unknown, |target| value_type_of(ast, target, depth + 1))
    };
    match &node.kind {
        TypeKind::Primitive(kind) => match kind {
            PrimitiveKind::Boolean => Python::scalar(Scalar::Bool),
            PrimitiveKind::Char | PrimitiveKind::WChar => Python::scalar(Scalar::Char),
            PrimitiveKind::Float32 | PrimitiveKind::Float64 => Python::scalar(Scalar::Double),
            PrimitiveKind::Float128 => Python::scalar(Scalar::LongDouble),
            _ => Python::scalar(Scalar::Int32),
        },
        // A `fixed<P,S>` is a decimal *string* and not a `float` or a
        // `decimal.Decimal`: `fixed-scales` declares `fixed<31,10>` and
        // its value has thirty significant digits, which neither holds,
        // and `FixedDigitCount::ValuePrecision` measures how many digits
        // a peer writes — a holder that normalises leading zeros away
        // has already answered that.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => py::Type::name("str"),
        // An enumeration is an `enum.IntEnum` and a bitmask an
        // `enum.IntFlag`; the declared name and not `int`, because only
        // one of the two says which enumeration.
        TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Native => declared(),
        TypeKind::Sequence { element, .. } => {
            py::Type::subscript(Path::single("list"), vec![inner(element.resolved())])
        }
        // The `tuple[T, …]` the materialiser writes for a member, one
        // argument per element and one `tuple[..]` per dimension:
        // `long a[2][3]` is `tuple[tuple[int, int, int], tuple[int, int,
        // int]]`. The reader builds the rows as lists between
        // `open_array` and `close_array` and answers with tuples.
        TypeKind::Array { element, dims } => {
            let mut out = inner(element.resolved());
            for dim in dims.iter().rev() {
                let Some(length) = ast
                    .eval_integral(*dim)
                    .ok()
                    .and_then(|length| usize::try_from(length).ok())
                else {
                    return unknown();
                };
                out = py::Type::subscript(Path::single("tuple"), vec![out; length]);
            }
            out
        }
        // §7.4.1.1.4: a sequence of entries, and the entry is a pair.
        TypeKind::Map { key, value, .. } => py::Type::subscript(
            Path::single("list"),
            vec![py::Type::subscript(
                Path::single("tuple"),
                vec![inner(key.resolved()), inner(value.resolved())],
            )],
        ),
        // A `typedef` has no Python declaration at all —
        // `adapter/python.rs` writes a comment where the other columns
        // write an alias, because a Python alias is evaluated eagerly
        // and could not be ordered against the classes it names — so a
        // codec resolves through it.
        TypeKind::Alias { aliased } => inner(aliased.resolved()),
        // A boxed value is a reference, absent when it is not there.
        TypeKind::ValueBox { boxed } => py::Type::Optional(Box::new(inner(boxed.resolved()))),
        // No Python data form at all; `adapter/python.rs` refuses a
        // member carrying one by name.
        TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => unknown(),
    }
}

/// `v`, `v[zd_i0]`, `v[zd_i0][zd_i1]`, … — an array's row `depth`
/// indices in, spelled with the counters `array_body` loops over.
fn array_row(base: &str, depth: usize) -> py::Expr {
    let mut out = py::Expr::name(base);
    for level in 0..depth {
        out = py::Expr::Index {
            value: Box::new(out),
            index: Box::new(py::Expr::name(&format!("zd_i{level}"))),
        };
    }
    out
}

/// Every row of an array from `depth` inward passed through `list`,
/// top-down so a row is a list before its own rows are reached, or —
/// `into_tuple` — through `tuple`, bottom-up so a row's rows are tuples
/// before it is.
///
/// Built from the innermost row outward, each level wrapping the loop
/// over the level inside it. IDL puts no bound on the number of an
/// array's dimensions, so a version that recursed once per dimension
/// could be driven as deep as the file asked.
fn array_rows(base: &str, lengths: &[i128], depth: usize, into_tuple: bool) -> Vec<py::Statement> {
    let func = if into_tuple { "tuple" } else { "list" };
    let convert = |level: usize| {
        let row = array_row(base, level);
        py::Statement::Set {
            place: row.clone(),
            value: py::Expr::Call {
                func: Path::single(func),
                args: vec![py::Arg::Positional(row)],
            },
        }
    };
    let innermost = lengths.len().saturating_sub(1).max(depth);
    let mut rows = vec![convert(innermost)];
    for level in (depth..innermost).rev() {
        let inward = py::Statement::For {
            counter: Ident::new(&format!("zd_i{level}")),
            bound: py::Expr::int(lengths[level]),
            body: Python::suite(rows),
        };
        rows = if into_tuple {
            vec![inward, convert(level)]
        } else {
            vec![convert(level), inward]
        };
    }
    rows
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A `rosidl` interface package for one type, printed from the model.
//!
//! ROS 2 rides on DDS but does not use the DDS type system. `rosidl` has
//! its own IDL subset, its own type model and its own per-middleware
//! serialisers: `rosidl_typesupport_fastrtps_c` generates C++ that calls
//! Fast CDR directly rather than calling the code `fastddsgen` would
//! have written, and `rmw_cyclonedds_cpp` walks the introspection type
//! support with a serialiser of its own rather than Cyclone's `idlc`
//! output. Two more encoders over the same wire, and between them the
//! largest population of DDS traffic there is.
//!
//! ## `.idl` and not `.msg`
//!
//! Both are `rosidl` inputs. `.idl` is the one used here, for three
//! reasons, in order of weight.
//!
//! 1. `.msg` is not a second input at all. `rosidl_adapter`'s `msg2idl`
//!    converts it to `.idl` before any generator runs, so `.msg` is a
//!    strictly earlier stage of the same pipeline and going through it
//!    can only lose.
//! 2. `.msg` reaches less of the corpus. It has no `wchar`, no bounded
//!    `wstring`, no `typedef`, no `long double`, no module nesting and
//!    no annotation syntax whatsoever. Every one of those is a corpus
//!    construct that `.idl` carries into the generator.
//! 3. The corpus is written in IDL. Printing IDL from the model is a
//!    restriction of the same language; printing `.msg` would be a
//!    translation through a second dialect, and a difference in the
//!    bytes could then be the translation's rather than the
//!    serialiser's.
//!
//! ## Printed from the model, not filtered as text
//!
//! The same reasoning `corba_idl` records. A textual filter passes what
//! it does not recognise, so an unknown construct would reach the
//! generator, the generator would fail, and the case would be written
//! down as "`rosidl` cannot take this construct" when the truth was that
//! the filter could not. Printing from the model inverts it: every
//! construct is reached by an arm of one `match` and anything without a
//! `rosidl` form is refused **by name**, with the path.
//!
//! ## The shape `rosidl` demands
//!
//! Measured on the Linux test host against `rosidl` 3.1.6 (ROS 2 Humble,
//! Fast DDS 2.6.10, Cyclone 0.10.5):
//!
//! * one message type per file, at `<package>/msg/<Name>.idl`;
//! * the type declared as `module <package> { module msg { struct
//!   <Name> { … }; }; };` and nothing else — a third module level is an
//!   `AssertionError` out of `rosidl_parser`;
//! * a dependency referred to by its absolute scoped name and pulled in
//!   with `#include "<package>/msg/<Dep>.idl"`. A relative include
//!   (`"msg/Dep.idl"`) parses and then fails to resolve the name.
//!
//! So a corpus type that names other types becomes a package of files,
//! not one file, and the corpus's own module path has to be flattened
//! into the single `Name` `rosidl` allows. [`ros_message_name`] does the
//! flattening and [`RosPackage`] refuses a collision rather than letting
//! two corpus types land in one file.
//!
//! ## What is dropped, and why dropping it is byte-neutral
//!
//! `rosidl_typesupport_fastrtps_c` emits **plain CDR, encoding version
//! 1, always**. Measured: the generated `_cdr_serialize` for the same
//! struct under `@final`, `@appendable` and `@mutable` is byte-identical
//! source (one MD5 over the three), and there is no DHEADER, no
//! EMHEADER and no sentinel anywhere in the template. Against that:
//!
//! | annotation | why it can go |
//! |---|---|
//! | `@final` | rule (17) is members in declaration order with no framing, which is what the generated code writes |
//! | `@appendable` | rule (29): under encoding version 1 an appendable type is not delimited, so its bytes are the final form's — inventory row H2, three vendors, 18 bytes both |
//! | `@key` | §7.6.8 puts nothing in the payload. The KeyHash itself is a different matter and [`ros_key_refusal`] states it |
//! | `@id`, `@hashid`, `@must_understand` | all three appear on the wire only inside a `@mutable` type's member headers, and `@mutable` is refused below |
//! | `@external` | §7.3.1.2.1.10 gives it no wire effect |
//! | `@default`, `@try_construct` | the receiver's answer to a missing or unfitting value; neither changes what a writer emits |
//! | `@nested`, `@verbatim`, `@unit`, `@range`, `@min`, `@max` | no wire effect in §7.4 at all |
//!
//! Dropping `@default` also steps around a defect rather than hiding
//! one: passed through, `@default(9)` on a scalar member makes
//! `rosidl_generator_c` 3.1.6 abort inside `msg__functions.c.em` with
//! `TypeError: 'int' object is not subscriptable`. That is recorded in
//! `docs/ros-column.md`; it is not the reason the annotation is
//! dropped, and the annotation would be dropped without it.
//!
//! Everything else is refused rather than dropped.

use crate::contract;
use crate::encode::ProbeError;
use crate::target_keywords;
use std::borrow::Cow;

use std::fmt::Write as _;
use zerodds_ast::{Ast, Bound, CharWidth, Extensibility, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// The interface package name every generated file sits under.
///
/// One package for the whole corpus: the package name is part of every
/// generated C identifier (`zdros__msg__Sample`), so a per-case package
/// would make the identifiers differ between cases for no reason.
pub const PACKAGE: &str = "zdros";

/// Refuse a construct that has no `rosidl` form.
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

/// One generated `.idl` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosFile {
    /// The `Name` in `<package>/msg/<Name>.idl`, and the struct's name.
    pub name: String,
    /// The file's whole contents.
    pub text: String,
}

/// The files one corpus type becomes, in dependency order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosPackage {
    /// Dependencies first, the root last.
    pub files: Vec<RosFile>,
    /// The root type's `Name`.
    pub root: String,
}

impl RosPackage {
    /// The C identifier the root type takes in the generated code.
    ///
    /// `rosidl_generator_c` joins the package, the `msg` sub-namespace
    /// and the message name with **double** underscores.
    #[must_use]
    pub fn root_c_name(&self) -> String {
        format!("{PACKAGE}__msg__{}", self.root)
    }
}

/// The `rosidl` interface package for `root`.
///
/// # Errors
/// [`ProbeError::Unsupported`], naming the construct that has no
/// `rosidl` form and the path it sits at.
pub fn ros_package(ast: &Ast, root: Id<TypeNode>) -> Result<RosPackage, ProbeError> {
    // Before the first hop; see `contract::reject_alias_cycle`. The
    // `Printer` refuses a cycle through an aggregate on its own — that
    // is what `open` is — and a typedef chain that returns to itself
    // passes it, because a typedef is not one of the types it opens.
    contract::reject_alias_cycle(ast)?;
    let path = contract::root_path(ast, root);
    let mut printer = Printer::default();
    // The root has to be a message in its own right: `rosidl` has no
    // file shape for a bare sequence or a bare primitive at top level.
    match ast.type_(root).map(|node| node.kind.clone()) {
        Ok(TypeKind::Structure { .. }) => {}
        Ok(TypeKind::Alias { .. }) => {
            return Err(refuse(
                &path,
                "a typedef as the top-level type, where a rosidl file holds one struct",
            ));
        }
        Ok(_) => {
            return Err(refuse(
                &path,
                "a top-level type that is not a struct, and a rosidl message is always a struct",
            ));
        }
        Err(error) => {
            return Err(ProbeError::NoLayout {
                detail: format!("{error}"),
            });
        }
    }
    printer.declare(ast, root, &path)?;
    let name = ros_message_name(ast, root).ok_or_else(|| {
        refuse(
            &path,
            "a top-level type with no name to give the rosidl file",
        )
    })?;
    Ok(RosPackage {
        files: printer.files,
        root: name,
    })
}

/// The single `Name` a corpus type takes inside the `rosidl` package.
///
/// `rosidl` allows exactly one module level under the package (`msg`),
/// so `::common::util::Location` cannot keep its path. Every segment is
/// capitalised and the path is concatenated: `CommonUtilLocation`. The
/// flattening is injective on the corpus — [`ros_package`] refuses a
/// collision rather than silently putting two types in one file — and
/// it never produces an underscore, which matters because
/// `rosidl_cmake` derives the *file* name by converting the message
/// name from camel case and would round-trip an underscore wrongly.
#[must_use]
pub fn ros_message_name(ast: &Ast, type_: Id<TypeNode>) -> Option<String> {
    let scoped = ast
        .any_of_type(type_)
        .ok()
        .and_then(|any| ast.scoped_name_of(any))?;
    let mut out = String::new();
    for segment in scoped.segments() {
        // An underscore in the source identifier is a word break, not a
        // character to carry: `my_type` becomes `MyType`, which is the
        // same convention `rosidl_adapter` applies to a `.msg` name.
        for word in segment.split('_').filter(|word| !word.is_empty()) {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.push_str(chars.as_str());
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Why a `--emit keyhash` cannot be answered by a ROS type support.
///
/// `rosidl` has no `@key`. The concept is absent from its type model,
/// absent from the generated struct and absent from the type support's
/// four callbacks, so there is nothing on the ROS side to ask. Returning
/// the ZeroDDS KeyHash here would be this harness answering for ROS.
#[must_use]
pub const fn ros_key_refusal() -> &'static str {
    "a KeyHash, which rosidl has no @key to compute one from"
}

/// The first construct Fast CDR 1.0.24 cannot write in a byte order that
/// is not the host's, or `None` if the type reaches none.
///
/// ## Why this exists
///
/// ROS 2 Humble pins Fast CDR 1.0.24, and `rmw_fastrtps` always
/// serialises in the host's own order — `Cdr::DEFAULT_ENDIAN`. Nothing
/// in ROS ever asks that library for the other order, and two things
/// break when a probe does. Both were reproduced with Fast CDR alone,
/// no `rosidl` in the program:
///
/// * `serializeArray` over a **multi-byte numeric** element writes the
///   *address* of the array instead of its contents. `long arr[3] = {1,
///   2, 3}` under `BIG_ENDIANNESS` came out `5f623270 00007ffc
///   5f623270` — the stack address of `arr`, twice — and the bytes
///   change between runs, because the address does.
/// * `serializeArray` over `wchar_t`, which is how a `wstring`'s code
///   units are written, does not swap at all: the length word goes out
///   big-endian and the units stay in the host's order.
///
/// One-byte elements are correct, because there is nothing to swap, and
/// so is every scalar.
///
/// **The two are not the same kind of thing, and only one of them may
/// not be recorded.** The unswapped units are wrong for the order that
/// was asked for and are the *same* wrong octets for the same sample on
/// every run — a measurement of the library, and one this project
/// reproduces with `--wstring-units host-unswapped`. The address is not
/// a function of the sample at all: it changes with the run's
/// address-space layout, so no setting reproduces it and no record may
/// hold it. [`ByteOrderHazard`] is that distinction, and the caller
/// refuses on one kind and records the other.
///
/// Measured on the Linux test host, Fast CDR 1.0.24, `int32_t
/// data[3] = {-1234567, 2147483647, -1}` through `serializeArray` under
/// `BIG_ENDIANNESS`, three runs:
///
/// ```text
/// &data=0x7fff704f0f34   payload 00000003 704f0f34 00007fff 704f11f8
/// &data=0x7ffe15c7ce14   payload 00000003 15c7ce14 00007ffe 15c7d0d8
/// setarch -R, twice      payload 00000003 ffffe9b4 00007fff ffffec78  (both runs)
/// ```
///
/// The octets are the address of the *pointer argument*, read as two
/// `int32` and emitted big-endian, and then whatever follows it. Under
/// `LITTLE_ENDIANNESS` the same call writes the sample.
#[must_use]
pub fn non_native_byte_order_hazard(
    ast: &Ast,
    type_: Id<TypeNode>,
) -> Option<(ByteOrderHazard, String)> {
    let path = contract::root_path(ast, type_);
    let mut open = Vec::new();
    hazard_at(ast, type_, &path, &mut open)
}

/// Which of the two Fast CDR 1.0.24 does, and therefore whether the
/// octets may be written down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrderHazard {
    /// `serializeArray` over a multi-byte numeric element writes the
    /// array's address. **Not a function of the sample**, so the row is
    /// refused rather than recorded: an octet no re-run reproduces
    /// verifies green forever once it is committed, and six of them
    /// did.
    NotAFunctionOfTheSample,
    /// `serializeArray` over `wchar_t` does not swap. Wrong for the
    /// order that was asked for, identical on every run for the same
    /// sample, and reproduced by a setting this project ships — so it
    /// is recorded, like any other divergence.
    Deterministic,
}

impl ByteOrderHazard {
    /// The word the `--print-be-hazard` line carries, so that the shell
    /// decides on a name rather than on a substring of the sentence.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotAFunctionOfTheSample => "not-a-function-of-the-sample",
            Self::Deterministic => "deterministic",
        }
    }
}

/// zerodds-lint: recursion-depth = the number of types in the model.
/// `open` holds every type this descent has already entered and none is
/// entered twice, so the depth cannot pass the count of distinct types the
/// model declares. That guard is what makes this walk safe on its own:
/// [`non_native_byte_order_hazard`] returns an `Option` and has no refusal
/// to hand back, so it carries no entry gate.
fn hazard_at(
    ast: &Ast,
    type_: Id<TypeNode>,
    path: &str,
    open: &mut Vec<Id<TypeNode>>,
) -> Option<(ByteOrderHazard, String)> {
    if open.contains(&type_) {
        return None;
    }
    open.push(type_);
    let found = hazard_now(ast, type_, path, open);
    open.pop();
    found
}

/// A node the walk cannot read, or an element it cannot resolve.
///
/// Reported as a hazard and not as its absence. This function decides
/// whether a big-endian run is allowed to produce bytes at all, so an
/// answer it cannot justify has to be the one that refuses. Reaching it
/// means the model changed under the walk; `ros_package` has already
/// resolved the same type by the time this runs.
fn unreadable(path: &str) -> Option<(ByteOrderHazard, String)> {
    Some((
        ByteOrderHazard::NotAFunctionOfTheSample,
        format!(
            "{path}: a type this walk could not read, so whether Fast CDR 1.0.24          can write it in a non-native byte order is unknown"
        ),
    ))
}

/// zerodds-lint: recursion-depth = the number of types in the model. Every
/// recursive arm goes through `hazard_at`, whose `open` set enters no type
/// twice.
fn hazard_now(
    ast: &Ast,
    type_: Id<TypeNode>,
    path: &str,
    open: &mut Vec<Id<TypeNode>>,
) -> Option<(ByteOrderHazard, String)> {
    let Ok(node) = ast.type_(type_) else {
        return unreadable(path);
    };
    let kind = node.kind.clone();
    match kind {
        // A `wstring` is `serializeArray` over `wchar_t` whether it sits
        // in a collection or on its own.
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => Some((
            ByteOrderHazard::Deterministic,
            format!("{path}: a wstring, whose code units Fast CDR 1.0.24 does not swap"),
        )),
        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return unreadable(path);
            };
            hazard_at(ast, target, path, open)
        }
        TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => {
            let Some(element) = element.resolved() else {
                return unreadable(path);
            };
            if is_multibyte_primitive(ast, element) {
                return Some((
                    ByteOrderHazard::NotAFunctionOfTheSample,
                    format!(
                        "{path}: a collection of a multi-byte primitive, which Fast CDR 1.0.24 \
                         serialises as the array's address in a non-native byte order"
                    ),
                ));
            }
            hazard_at(ast, element, &format!("{path}[]"), open)
        }
        TypeKind::Structure { .. } => {
            for member in contract::members(ast, type_) {
                let found = hazard_at(ast, member.type_, &contract::join(path, &member.name), open);
                if found.is_some() {
                    return found;
                }
            }
            None
        }
        // A primitive, a narrow string, or a construct `ros_package`
        // has already refused — it runs first and this walk only ever
        // sees a type it accepted. A narrow string is `serializeArray`
        // over `char`, one byte, nothing to swap.
        TypeKind::Primitive(_) | TypeKind::String { .. } => None,
        _ => unreadable(path),
    }
}

/// Whether `type_` is a primitive wider than one byte once typedefs are
/// seen through. A one-byte element has nothing to swap and comes out
/// right in either order.
///
/// The chain is walked by `contract::resolve_alias`, which is iterative
/// and stops at `contract::MAX_ALIAS_HOPS`, rather than by recursing on
/// the typedef arm as the three helpers below still do. They are only
/// ever reached under `ros_package`, which refuses a typedef cycle
/// before the first hop; this one is also reached under
/// [`non_native_byte_order_hazard`], which is a `#[must_use] -> Option`
/// with no refusal to return and no gate in front of it.
fn is_multibyte_primitive(ast: &Ast, type_: Id<TypeNode>) -> bool {
    matches!(
        ast.type_(contract::resolve_alias(ast, type_))
            .map(|node| node.kind.clone()),
        Ok(TypeKind::Primitive(kind)) if !matches!(
            kind,
            PrimitiveKind::Boolean
                | PrimitiveKind::Octet
                | PrimitiveKind::UInt8
                | PrimitiveKind::Int8
                | PrimitiveKind::Char
        )
    )
}

#[derive(Default)]
struct Printer {
    /// Types already emitted, so a diamond becomes one file.
    done: Vec<(Id<TypeNode>, String)>,
    /// Types on the current path, so a cycle is refused.
    open: Vec<Id<TypeNode>>,
    /// Dependencies first, the root last.
    files: Vec<RosFile>,
}

impl Printer {
    /// Emit `type_` and everything it depends on, once.
    fn declare(&mut self, ast: &Ast, type_: Id<TypeNode>, path: &str) -> Result<(), ProbeError> {
        if self.done.iter().any(|(known, _)| *known == type_) {
            return Ok(());
        }
        if self.open.contains(&type_) {
            // `rosidl` does accept a struct that names itself — measured,
            // `sequence<zdros::msg::Sink>` inside `Sink` generates and
            // compiles. It is refused anyway: §7.4.1.1 has no rule for a
            // recursive type, so the reference implementations this
            // column is compared against produce nothing to compare to.
            return Err(refuse(
                path,
                "a recursive type, for which §7.4.1.1 gives no encoding to compare against",
            ));
        }
        self.open.push(type_);
        let result = self.declare_now(ast, type_, path);
        self.open.pop();
        result
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
        let TypeKind::Structure { base, members } = &kind else {
            // Everything that is not a struct is spelled where it is
            // used, or refused there. `spell` is the single gate.
            return Ok(());
        };
        if base.is_some() {
            // Measured, `rosidl_parser` 3.1.6: `struct T : B` is
            // `UnexpectedCharacters: No terminal matches ':'`. IDL 4.2
            // inheritance is not in the grammar at all.
            return Err(refuse(
                path,
                "a struct with a base, which rosidl's IDL grammar has no syntax for",
            ));
        }
        if members.is_empty() {
            // Measured: an empty `struct T { };` is a parse error. ROS
            // spells an empty message with a placeholder member, and
            // inventing one would put a byte on the wire the corpus case
            // does not have.
            return Err(refuse(
                path,
                "a struct with no members, which rosidl's IDL grammar refuses",
            ));
        }
        match ast.effective_extensibility(type_) {
            Extensibility::Final | Extensibility::Appendable => {}
            Extensibility::Mutable => {
                // Dropping this one is not byte-neutral and that is the
                // whole reason it is refused. `rosidl` parses `@mutable`
                // and then generates the same serialiser it generates
                // for `@final` — measured, one MD5 over the three
                // extensibilities — so a `@mutable` case handed to this
                // column would come back with the *final* bytes under a
                // mutable heading: no DHEADER, no EMHEADERs, no member
                // ids, no must-understand flags. That is a different
                // case, not this one.
                return Err(refuse(
                    path,
                    "a @mutable type, whose member headers rosidl parses and then does not write",
                ));
            }
        }

        let name = ros_message_name(ast, type_)
            .ok_or_else(|| refuse(path, "a struct with no name to give the rosidl file"))?;
        if let Some((_, other)) = self
            .done
            .iter()
            .find(|(_, known)| *known == name)
            .map(|(id, known)| (*id, known.clone()))
        {
            // Two corpus types flattening to one `rosidl` name would
            // overwrite one file with the other and the run would
            // measure the wrong type in silence.
            let _ = other;
            return Err(refuse(
                path,
                "a type whose flattened rosidl name collides with another type's in the same case",
            ));
        }

        // Dependencies first: `rosidl` resolves an included name at
        // parse time, so a file has to exist before the file that
        // includes it is generated.
        let mut body = String::new();
        let mut includes: Vec<String> = Vec::new();
        for id in members.clone() {
            let member = ast.member(id).map_err(|error| ProbeError::NoLayout {
                detail: format!("{error}"),
            })?;
            let member_path = contract::join(path, &member.name);
            if member.facets.is_optional() {
                // §7.4.1.2.6: an optional member is carried by a
                // presence marker — a length-code-0 EMHEADER in a
                // mutable type, a boolean flag otherwise. `rosidl` has
                // no optionality, so the member would go out
                // unconditionally with no marker in front of it.
                return Err(refuse(
                    &member_path,
                    "an @optional member, for which rosidl has no presence marker",
                ));
            }
            if is_non_serialized(&member.facets) {
                return Err(refuse(
                    &member_path,
                    "a @non_serialized member, which rosidl cannot leave off the wire",
                ));
            }
            if matches!(target_keywords::cxx(&member.name), Cow::Owned(_)) {
                // Measured, `rosidl_generator_c` 3.1.6: `msg__struct.h.em`
                // writes the member name verbatim — `long class;` becomes
                // `int32_t class;` in `sample__struct.h` — and no resource
                // template renames a keyword. gcc takes the spellings C
                // does not reserve, but `rosidl_typesupport_fastrtps_c`'s
                // own generated `*__type_support_c.cpp` includes the same
                // header as C++ and stops at the keyword before any probe
                // code is reached. The C++ list plus the C list, the same
                // pair `target_keywords::cxx` checks, because the header
                // is compiled as both.
                return Err(refuse(
                    &member_path,
                    "a member named by a C or C++ keyword, which rosidl_generator_c spells \
                     verbatim in a header its fastrtps type support compiles as C++",
                ));
            }
            let member_type = member
                .type_
                .resolved()
                .ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{member_path}: the member type is unresolved"),
                })?;
            self.declare_dependency(ast, member_type, &member_path, &mut includes)?;
            let _ = writeln!(
                body,
                "      {}",
                member_declaration(ast, member_type, &member.name, &member_path)?
            );
        }

        let mut text = String::new();
        for include in includes {
            let _ = writeln!(text, "#include \"{PACKAGE}/msg/{include}.idl\"");
        }
        let _ = writeln!(text, "module {PACKAGE} {{");
        let _ = writeln!(text, "  module msg {{");
        let _ = writeln!(text, "    struct {name} {{");
        text.push_str(&body);
        let _ = writeln!(text, "    }};");
        let _ = writeln!(text, "  }};");
        let _ = writeln!(text, "}};");

        self.done.push((type_, name.clone()));
        self.files.push(RosFile { name, text });
        Ok(())
    }

    /// Declare whatever named structs `type_` reaches, recording the
    /// includes the current file needs.
    fn declare_dependency(
        &mut self,
        ast: &Ast,
        type_: Id<TypeNode>,
        path: &str,
        includes: &mut Vec<String>,
    ) -> Result<(), ProbeError> {
        let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
            detail: format!("{error}"),
        })?;
        match &node.kind {
            TypeKind::Structure { .. } => {
                self.declare(ast, type_, path)?;
                if let Some(name) = ros_message_name(ast, type_)
                    && !includes.contains(&name)
                {
                    includes.push(name);
                }
                Ok(())
            }
            // A typedef is spelled through: `rosidl` accepts `typedef`
            // only inside the file that uses it, and a typedef of a
            // struct would need the struct's include anyway. Rule (7)
            // makes it transparent on the wire (inventory F1), so
            // spelling the target costs no byte.
            TypeKind::Alias { aliased } => {
                let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the typedef target is unresolved"),
                })?;
                self.declare_dependency(ast, target, path, includes)
            }
            TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => {
                let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                    detail: format!("{path}: the element type is unresolved"),
                })?;
                self.declare_dependency(ast, element, path, includes)
            }
            _ => Ok(()),
        }
    }
}

/// One member, as it is written inside the struct.
fn member_declaration(
    ast: &Ast,
    type_: Id<TypeNode>,
    name: &str,
    path: &str,
) -> Result<String, ProbeError> {
    // An array puts its dimensions after the name, so it cannot go
    // through `spell`.
    if let Some((element, counts)) = array_shape(ast, type_, path)? {
        if counts.len() > 1 {
            // Measured, `rosidl_parser` 3.1.6: "Unsupported
            // multidimensional array".
            return Err(refuse(
                path,
                "a multi-dimensional array, which rosidl's IDL parser refuses by name",
            ));
        }
        // An array the model gives no dimensions is not an array, and
        // `name[0]` is not C. Refused rather than defaulted: a zero
        // would compile in some dialects and carry no elements.
        let Some(&count) = counts.first() else {
            return Err(refuse(
                path,
                "an array whose dimensions the model does not give",
            ));
        };
        // An array element that is itself a collection has no rosidl
        // spelling: measured, `sequence<int32> s[2];` is an
        // `AssertionError` and `typedef int32 T[3]; T s[2];` aborts the
        // generator.
        if is_collection(ast, element) {
            return Err(refuse(
                path,
                "an array whose element is itself a collection, which rosidl has no shape for",
            ));
        }
        if is_wide_char(ast, element) {
            return Err(wide_char_collection(path, "an array"));
        }
        return Ok(format!("{} {name}[{count}];", spell(ast, element, path)?));
    }
    Ok(format!("{} {name};", spell(ast, type_, path)?))
}

/// An array's element type and its dimension counts.
type ArrayShape = (Id<TypeNode>, Vec<usize>);

/// The `(element, dimensions)` of `type_` if it is an array once
/// typedefs are seen through, otherwise `None`.
///
/// zerodds-lint: recursion-depth 64 — one frame per typedef hop,
/// `contract::MAX_ALIAS_HOPS`. The typedef arm is the only recursive one,
/// and a chain that returns to itself has been refused by
/// `contract::reject_alias_cycle` at the entry of `ros_package`, which is
/// the only walk that reaches here.
fn array_shape(
    ast: &Ast,
    type_: Id<TypeNode>,
    path: &str,
) -> Result<Option<ArrayShape>, ProbeError> {
    match ast.type_(type_).map(|node| node.kind.clone()) {
        Ok(TypeKind::Array { element, dims }) => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = contract::array_dimensions(ast, &dims, path)?;
            Ok(Some((element, counts)))
        }
        Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
            Some(target) => array_shape(ast, target, path),
            None => Err(ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            }),
        },
        _ => Ok(None),
    }
}

/// Whether `type_` is a sequence or an array once typedefs are seen
/// through.
///
/// zerodds-lint: recursion-depth 64 — one frame per typedef hop,
/// `contract::MAX_ALIAS_HOPS`. The typedef arm is the only recursive one,
/// and a chain that returns to itself has been refused by
/// `contract::reject_alias_cycle` at the entry of `ros_package`, which is
/// the only walk that reaches here.
fn is_collection(ast: &Ast, type_: Id<TypeNode>) -> bool {
    match ast.type_(type_).map(|node| node.kind.clone()) {
        Ok(TypeKind::Sequence { .. } | TypeKind::Array { .. }) => true,
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .is_some_and(|target| is_collection(ast, target)),
        _ => false,
    }
}

/// Whether `type_` is a `wchar` once typedefs are seen through.
///
/// zerodds-lint: recursion-depth 64 — one frame per typedef hop,
/// `contract::MAX_ALIAS_HOPS`. The typedef arm is the only recursive one,
/// and a chain that returns to itself has been refused by
/// `contract::reject_alias_cycle` at the entry of `ros_package`, which is
/// the only walk that reaches here.
fn is_wide_char(ast: &Ast, type_: Id<TypeNode>) -> bool {
    match ast.type_(type_).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(PrimitiveKind::WChar)) => true,
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .is_some_and(|target| is_wide_char(ast, target)),
        _ => false,
    }
}

/// A collection of `wchar`, which `rosidl` generates and cannot compile.
///
/// Measured, `rosidl_typesupport_fastrtps_c` 2.2.2: for
/// `sequence<wchar>` and for `wchar[N]` the emitted `_cdr_serialize`
/// contains
///
/// ```text
/// if (!callbacks->cdr_serialize(static_cast<wchar_t *>(&array_ptr[i]), cdr))
/// ```
///
/// `callbacks` is declared nowhere in that function — the template's
/// nested-message branch is taken for a `wchar` element — and the cast
/// is from `uint16_t *` to `wchar_t *`, which C++ refuses as well. The
/// file does not compile, so there is no serialiser for the case and no
/// bytes to compare. A scalar `wchar` takes a different branch and does
/// build.
///
/// Refused here, by name, rather than left to surface as a compiler log:
/// a compile failure in the run reads as "this harness is broken", and
/// this one is a property of `rosidl`.
fn wide_char_collection(path: &str, what: &'static str) -> ProbeError {
    let _ = what;
    // A defect and not a missing form: `rosidl`'s IDL takes the
    // construct without a word and its own generator is what fails.
    // Recorded as a missing form it read as "ROS has no collection of
    // wchar", which is the opposite of the measurement above.
    ProbeError::GeneratorDefect {
        path: path.to_string(),
        kind: "a collection of wchar, for which rosidl_typesupport_fastrtps \
               generates C++ that does not compile",
    }
}

/// How a type is written where it is used.
///
/// zerodds-lint: recursion-depth 64 — the type graph's own nesting, and in
/// practice two. A struct is a leaf: it is spelled by name and never
/// descended into, which is what `Printer::declare` walks instead, with an
/// `open` set that refuses a cycle. A sequence descends once and no
/// further, because a sequence whose element is itself a collection is
/// refused above the recursive call. What is left is the typedef arm, and
/// a chain that returns to itself has been refused by
/// `contract::reject_alias_cycle` at the entry of `ros_package`.
fn spell(ast: &Ast, type_: Id<TypeNode>, path: &str) -> Result<String, ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Structure { .. } => {
            // Absolute, because a relative name inside `module zdros {
            // module msg { … } }` resolves against the corpus's own
            // module path, which the flattening has thrown away.
            let name = ros_message_name(ast, type_)
                .ok_or_else(|| refuse(path, "a struct with no name to reference"))?;
            Ok(format!("{PACKAGE}::msg::{name}"))
        }
        TypeKind::Primitive(kind) => primitive(*kind, path).map(ToString::to_string),
        TypeKind::String { width, bound } => {
            let keyword = match width {
                CharWidth::Char => "string",
                CharWidth::WChar => "wstring",
            };
            match bound_value(ast, *bound, path)? {
                Some(n) => Ok(format!("{keyword}<{n}>")),
                None => Ok(keyword.to_string()),
            }
        }
        TypeKind::Sequence { element, bound } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the sequence element is unresolved"),
            })?;
            // Measured: `sequence<sequence<int32> >` is an
            // `AssertionError` out of `rosidl_parser`, and routing it
            // through a `typedef` aborts `rosidl_generator_c` instead.
            // Neither is a shape, so both are refused here by name.
            if is_collection(ast, element) {
                return Err(refuse(
                    path,
                    "a sequence whose element is itself a collection, which rosidl has no shape for",
                ));
            }
            if is_wide_char(ast, element) {
                return Err(wide_char_collection(path, "a sequence"));
            }
            let spelled = spell(ast, element, path)?;
            match bound_value(ast, *bound, path)? {
                // Measured: a bound of zero is a parse error, and it is
                // also the one bound that would make every sample of
                // the case unrepresentable.
                Some(0) => Err(refuse(
                    path,
                    "a sequence bounded at zero, which rosidl's IDL parser refuses",
                )),
                Some(n) => Ok(format!("sequence<{spelled}, {n}>")),
                None => Ok(format!("sequence<{spelled}>")),
            }
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            spell(ast, target, path)
        }
        // An array reached here is an array inside something that has no
        // place to put the dimensions. `member_declaration` handles the
        // member case; everything else is a refusal.
        TypeKind::Array { .. } => Err(refuse(
            path,
            "an array in a position where rosidl has nowhere to write its dimensions",
        )),
        TypeKind::Enumeration { .. } => Err(refuse(
            path,
            "an enumeration, which rosidl's type model does not have — its IDL parses `enum` and then reports the name as unknown",
        )),
        TypeKind::Union { .. } => Err(refuse(
            path,
            "a union, which rosidl's IDL grammar has no `union` keyword for",
        )),
        TypeKind::Map { .. } => Err(refuse(
            path,
            "a map, which rosidl's IDL grammar has no `map` keyword for",
        )),
        TypeKind::Bitmask { .. } => Err(refuse(
            path,
            "a bitmask, which rosidl's IDL grammar has no `bitmask` keyword for",
        )),
        TypeKind::Bitset { .. } => Err(refuse(
            path,
            "a bitset, which rosidl's IDL grammar has no `bitset` keyword for",
        )),
        TypeKind::Fixed { .. } => Err(refuse(
            path,
            "a fixed, which rosidl's IDL grammar has no `fixed` keyword for",
        )),
        other => Err(unfilled(path, other.as_str())),
    }
}

fn bound_value(ast: &Ast, bound: Bound, path: &str) -> Result<Option<i128>, ProbeError> {
    match bound.expr() {
        None => Ok(None),
        Some(expr) => ast
            .eval_integral(expr)
            .map(Some)
            .map_err(|_| refuse(path, "a bound that does not evaluate to a count")),
    }
}

/// Whether a member carries `@non_serialized`.
fn is_non_serialized(facets: &zerodds_ast::Facets) -> bool {
    facets
        .xtypes
        .as_ref()
        .is_some_and(|xtypes| xtypes.is_non_serialized)
}

/// One primitive, in `rosidl` IDL's spelling.
///
/// `rosidl`'s grammar takes both the IDL 4.2 names and the extended
/// integer names, and `rosidl_generator_c` maps them as measured on the
/// host: `char` to `signed char`, `wchar` to `uint16_t`, `octet` and
/// `uint8` both to `uint8_t`, `long double` to `long double`.
fn primitive(kind: PrimitiveKind, path: &str) -> Result<&'static str, ProbeError> {
    Ok(match kind {
        PrimitiveKind::Boolean => "boolean",
        PrimitiveKind::Octet => "octet",
        PrimitiveKind::UInt8 => "uint8",
        PrimitiveKind::Int8 => "int8",
        PrimitiveKind::Char => "char",
        PrimitiveKind::WChar => "wchar",
        PrimitiveKind::Int16 => "int16",
        PrimitiveKind::UInt16 => "uint16",
        PrimitiveKind::Int32 => "int32",
        PrimitiveKind::UInt32 => "uint32",
        PrimitiveKind::Int64 => "int64",
        PrimitiveKind::UInt64 => "uint64",
        PrimitiveKind::Float32 => "float",
        PrimitiveKind::Float64 => "double",
        // `rosidl` parses `long double` and `rosidl_generator_c` gives
        // the member the C compiler's `long double`, which on x86-64
        // Linux is the x87 80-bit format padded to sixteen bytes and not
        // the IEEE binary128 Table 31 names. There is no expression that
        // puts a binary128 sample into that holder unchanged, so a
        // measurement taken through it would be a measurement of a
        // different value. This is the refusal `fill_cpp` already makes
        // for Fast DDS — "whose Fast DDS bytes differ between x86-64 and
        // aarch64" — reached by the same road.
        PrimitiveKind::Float128 => {
            return Err(refuse(
                path,
                "a long double, whose rosidl holder is the C compiler's and not the binary128 Table 31 names",
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::{
        ByteOrderHazard, PACKAGE, non_native_byte_order_hazard, ros_message_name, ros_package,
    };
    use crate::encode::{ProbeError, model_from_idl};
    use zerodds_ast::{Ast, TypeNode};
    use zerodds_ast_core::Id;

    fn model(idl: &str, type_name: &str) -> (Ast, Id<TypeNode>) {
        let ast = model_from_idl(idl, "ros.idl").expect("the fixture parses");
        let type_ = ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
                type_name.to_string(),
            ]))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .expect("the type is in the model");
        (ast, type_)
    }

    fn refusal(idl: &str, type_name: &str) -> String {
        let (ast, type_) = model(idl, type_name);
        match ros_package(&ast, type_) {
            Ok(_) => panic!("expected a refusal"),
            Err(error) => format!("{error}"),
        }
    }

    #[test]
    fn a_plain_struct_becomes_one_file_in_the_msg_namespace() {
        let (ast, type_) = model("@final struct Sample { long a; double b; };", "Sample");
        let package = ros_package(&ast, type_).expect("a plain struct is expressible");
        assert_eq!(package.root, "Sample");
        assert_eq!(package.files.len(), 1);
        assert_eq!(package.root_c_name(), format!("{PACKAGE}__msg__Sample"));
        assert!(package.files[0].text.contains("module zdros {"));
        assert!(package.files[0].text.contains("  module msg {"));
        assert!(package.files[0].text.contains("int32 a;"));
        assert!(package.files[0].text.contains("double b;"));
    }

    #[test]
    fn a_nested_struct_becomes_a_second_file_and_an_include() {
        let idl = "@final struct Cell { long x; };\n\
                   @final struct Sample { Cell c; };";
        let (ast, type_) = model(idl, "Sample");
        let package = ros_package(&ast, type_).expect("a nested struct is expressible");
        assert_eq!(package.files.len(), 2);
        // Dependency first: `rosidl` resolves an included name at parse
        // time, so the file order is the generation order.
        assert_eq!(package.files[0].name, "Cell");
        assert_eq!(package.files[1].name, "Sample");
        assert!(
            package.files[1]
                .text
                .contains("#include \"zdros/msg/Cell.idl\"")
        );
        assert!(package.files[1].text.contains("zdros::msg::Cell c;"));
    }

    #[test]
    fn a_module_path_is_flattened_onto_the_message_name() {
        let idl = "module common { module util { @final struct Location { long x; }; }; };\n\
                   @final struct Sample { common::util::Location where; };";
        let (ast, type_) = model(idl, "Sample");
        let package = ros_package(&ast, type_).expect("a module path flattens");
        assert!(package.files.iter().any(|f| f.name == "CommonUtilLocation"));
        assert!(
            package.files[1]
                .text
                .contains("zdros::msg::CommonUtilLocation")
        );
    }

    #[test]
    fn an_underscored_identifier_becomes_camel_case_and_not_an_underscore() {
        let idl = "@final struct my_sample { long a; };";
        let (ast, type_) = model(idl, "my_sample");
        assert_eq!(ros_message_name(&ast, type_).as_deref(), Some("MySample"));
    }

    #[test]
    fn the_five_constructs_rosidl_has_no_keyword_for_are_refused_by_name() {
        assert!(
            refusal(
                "union U switch (long) { case 1: long w; };\n@final struct Sample { U choice; };",
                "Sample"
            )
            .contains("union")
        );
        assert!(refusal("@final struct Sample { map<long, long> m; };", "Sample").contains("map"));
        assert!(
            refusal(
                "@bit_bound(8) bitmask Flags { @position(0) F0 };\n@final struct Sample { Flags f; };",
                "Sample"
            )
            .contains("bitmask")
        );
        assert!(
            refusal(
                "bitset Bits { bitfield<3> a; };\n@final struct Sample { Bits b; };",
                "Sample"
            )
            .contains("bitset")
        );
        assert!(refusal("@final struct Sample { fixed<5,2> f; };", "Sample").contains("fixed"));
    }

    #[test]
    fn an_enumeration_is_refused_because_rosidl_has_no_enum_in_its_type_model() {
        let refusal = refusal(
            "enum Colour { RED, GREEN };\n@final struct Sample { Colour c; };",
            "Sample",
        );
        assert!(refusal.contains("enumeration"), "{refusal}");
    }

    #[test]
    fn a_mutable_type_is_refused_rather_than_written_as_a_final_one() {
        let refusal = refusal("@mutable struct Sample { @id(1) long a; };", "Sample");
        assert!(refusal.contains("@mutable"), "{refusal}");
    }

    #[test]
    fn an_appendable_type_is_expressible_because_xcdr1_does_not_delimit_it() {
        let (ast, type_) = model("@appendable struct Sample { long a; };", "Sample");
        ros_package(&ast, type_).expect("H2: appendable is final's bytes under XCDR1");
    }

    #[test]
    fn an_optional_member_is_refused_for_want_of_a_presence_marker() {
        let refusal = refusal(
            "@final struct Sample { long a; @optional octet b; };",
            "Sample",
        );
        assert!(refusal.contains("@optional"), "{refusal}");
    }

    #[test]
    fn a_member_named_by_a_cxx_keyword_is_refused_before_the_generated_header_fails_to_compile() {
        // `class` is legal IDL (CORBA 3.3 part 1 §7.2.3 reserves neither
        // it nor `operator`), and `rosidl_generator_c` emits
        // `int32_t class;`, which the C++ half of its own type support
        // cannot compile.
        let refusal = refusal(
            "@final struct Sample { long class; long plain; };",
            "Sample",
        );
        assert!(refusal.contains("Sample.class"), "{refusal}");
        assert!(refusal.contains("C or C++ keyword"), "{refusal}");
    }

    #[test]
    fn a_member_named_by_a_c_only_keyword_is_refused_too() {
        let refusal = refusal("@final struct Sample { long restrict; };", "Sample");
        assert!(refusal.contains("Sample.restrict"), "{refusal}");
    }

    #[test]
    fn a_key_is_dropped_from_the_idl_and_the_keyhash_is_refused_instead() {
        let (ast, type_) = model("@final struct Sample { @key long a; long b; };", "Sample");
        let package = ros_package(&ast, type_).expect("@key has no payload effect");
        assert!(!package.files[0].text.contains("@key"));
    }

    #[test]
    fn a_multi_dimensional_array_is_refused_by_the_name_rosidl_refuses_it_under() {
        let refusal = refusal("@final struct Sample { long grid[2][3]; };", "Sample");
        assert!(refusal.contains("multi-dimensional"), "{refusal}");
    }

    #[test]
    fn a_collection_inside_a_collection_is_refused_in_both_orders() {
        assert!(
            refusal(
                "@final struct Sample { sequence<sequence<long> > s; };",
                "Sample"
            )
            .contains("sequence whose element")
        );
        assert!(
            refusal("@final struct Sample { sequence<long> s[2]; };", "Sample")
                .contains("array whose element")
        );
    }

    #[test]
    fn a_typedef_is_spelled_through_because_rule_7_makes_it_transparent() {
        let idl = "typedef long MyLong;\n@final struct Sample { MyLong a; };";
        let (ast, type_) = model(idl, "Sample");
        let package = ros_package(&ast, type_).expect("a typedef spells through");
        assert!(package.files[0].text.contains("int32 a;"));
    }

    #[test]
    fn a_bound_is_carried_into_the_rosidl_idl_and_not_dropped() {
        let idl = "@final struct Sample { string<5> s; sequence<long, 4> q; };";
        let (ast, type_) = model(idl, "Sample");
        let package = ros_package(&ast, type_).expect("bounds are rosidl syntax");
        assert!(package.files[0].text.contains("string<5> s;"));
        assert!(package.files[0].text.contains("sequence<int32, 4> q;"));
    }

    #[test]
    fn a_collection_of_wchar_is_refused_because_the_generated_cpp_does_not_compile() {
        assert!(
            refusal("@final struct Sample { wchar w[2]; };", "Sample")
                .contains("collection of wchar")
        );
        assert!(
            refusal("@final struct Sample { sequence<wchar> w; };", "Sample")
                .contains("collection of wchar")
        );
        // The scalar takes a different branch of the same template and
        // does build, so it stays measurable.
        let (ast, type_) = model("@final struct Sample { wchar w; };", "Sample");
        ros_package(&ast, type_).expect("a scalar wchar builds");
    }

    #[test]
    fn a_long_double_is_refused_because_the_holder_is_the_compilers() {
        assert!(
            refusal("@final struct Sample { long double d; };", "Sample").contains("long double")
        );
    }

    #[test]
    fn the_byte_order_hazard_names_a_multi_byte_collection_and_not_a_one_byte_one() {
        let (ast, type_) = model("@final struct Sample { sequence<long> q; };", "Sample");
        let (kind, hazard) =
            non_native_byte_order_hazard(&ast, type_).expect("int32 is multi-byte");
        assert!(hazard.contains("multi-byte primitive"), "{hazard}");
        // The kind, and not only the sentence: this is the one that may
        // not be written down, because the octets are the array's
        // address and change with the run.
        assert_eq!(kind, ByteOrderHazard::NotAFunctionOfTheSample);

        let (ast, type_) = model("@final struct Sample { sequence<octet> q; };", "Sample");
        assert!(
            non_native_byte_order_hazard(&ast, type_).is_none(),
            "an octet has nothing to swap"
        );

        let (ast, type_) = model("@final struct Sample { long a; double b; };", "Sample");
        assert!(
            non_native_byte_order_hazard(&ast, type_).is_none(),
            "a scalar is swapped correctly"
        );
    }

    #[test]
    fn the_byte_order_hazard_names_a_wstring_wherever_it_sits() {
        for idl in [
            "@final struct Sample { wstring w; };",
            "@final struct Sample { sequence<wstring> w; };",
            "@final struct Cell { wstring w; };\n@final struct Sample { Cell c; };",
        ] {
            let (ast, type_) = model(idl, "Sample");
            let (kind, hazard) = non_native_byte_order_hazard(&ast, type_)
                .unwrap_or_else(|| panic!("a wstring is a hazard: {idl}"));
            assert!(hazard.contains("wstring"), "{hazard}");
            // Deterministic, so the row is recorded rather than
            // refused: the units are wrong for the order asked for and
            // are the same wrong units every run, which is what
            // `--wstring-units host-unswapped` reproduces.
            assert_eq!(kind, ByteOrderHazard::Deterministic, "{idl}");
        }
    }

    #[test]
    fn the_byte_order_hazard_walk_terminates_on_a_recursive_type() {
        // The hazard walk runs before `ros_package` has had a chance to
        // refuse the recursion, so it has to stop by itself.
        let (ast, type_) = model("@final struct Sample { sequence<Sample> kids; };", "Sample");
        let _ = non_native_byte_order_hazard(&ast, type_);
    }

    #[test]
    fn a_recursive_type_is_refused_rather_than_recursed_into() {
        let idl = "@final struct Sample { sequence<Sample> kids; };";
        let (ast, type_) = model(idl, "Sample");
        let error = ros_package(&ast, type_);
        assert!(matches!(error, Err(ProbeError::Unsupported { .. })));
    }
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The `toAmqpValue<T>` and `toJsonString<T>` helpers, per IDL type.
//!
//! One walk over the [`TypePlan`], three targets. The walk decides what
//! the generated program does — which runtime call each field takes,
//! where a list opens, how a `@mutable` body's keys are written, which
//! branch of a union reaches the wire. None of that is C++. What is C++
//! is the shape the decision is written in: `v_.x` against `v_.getX()`,
//! `std::vector<Bytes>` against `List<byte[]>`, a `+=` against an
//! `append`.
//!
//! `Op` is that seam. The walk emits into it once and each column
//! renders it into its own CST, which `zerodds-lang-render` writes out.
//! A fourth column costs a renderer rather than a second copy of §7.
//!
//! ## What the emitted code assumes about the type it is given
//!
//! **Public members, accessed by their IDL names.** IDL4 to C++ 1.0
//! §8.1.1 makes `struct_mapping = STRUCT_WITH_PUBLIC_MEMBERS` the
//! default, and the Java and TypeScript mappings put the state on public
//! fields too. [`carriers_for`] writes a declaration in exactly that
//! shape, so a generated file compiles on its own; a binding that
//! already declares the type takes the helpers alone.
//!
//! **An `@optional` member is the language's own absent form** —
//! `std::optional<T>`, a boxed Java reference that may be `null`, a
//! TypeScript `T | undefined`.
//!
//! ## What checks it
//!
//! `tests/cross_language_agreement.rs` compiles the emitted C++, Java
//! and TypeScript against the runtimes in `crates/amqp-bridge/runtime/`,
//! runs them over one sample, and compares the octets and the JSON
//! against `crate::codegen::encode`. Where a toolchain is missing it
//! names the one it could not find and skips loudly.

use alloc::borrow::ToOwned;
use alloc::boxed::Box as AllocBox;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::{cpp, java, typescript};

use crate::codegen::encode::PlanSet;
use crate::codegen::plan::{BodyForm, Descriptor, FieldEncoding, FieldPlan, TypePlan};

/// Which column to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodegenLanguage {
    /// C++, against `runtime/cpp/zerodds_amqp_codec.hpp`.
    Cpp,
    /// Java, against `runtime/java/AmqpCodec.java`.
    Java,
    /// TypeScript, against `runtime/typescript/codec.ts`.
    TypeScript,
}

impl CodegenLanguage {
    /// Every column.
    pub const ALL: [Self; 3] = [Self::Cpp, Self::Java, Self::TypeScript];

    /// How the column is written in a diagnostic.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpp => "C++",
            Self::Java => "Java",
            Self::TypeScript => "TypeScript",
        }
    }
}

/// The emission refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitError {
    /// A nested composite whose own plan is not in the set, so the
    /// helper to call it by has no name.
    NoPlanFor(String),
}

impl core::fmt::Display for EmitError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoPlanFor(name) => write!(
                f,
                "the nested composite `{name}` has no plan in the set, so no helper names it"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EmitError {}

/// The runtime a column's emitted helpers call.
///
/// Shipped source and not generated code: each runtime implements OASIS
/// AMQP 1.0 Part 1 §1.6 from the clause, which is what makes a column a
/// second reading of the specification rather than a rendering of the
/// Rust encoder's answer.
#[must_use]
pub const fn runtime_source(language: CodegenLanguage) -> &'static str {
    match language {
        CodegenLanguage::Cpp => include_str!("../../runtime/cpp/zerodds_amqp_codec.hpp"),
        CodegenLanguage::Java => include_str!("../../runtime/java/AmqpCodec.java"),
        CodegenLanguage::TypeScript => include_str!("../../runtime/typescript/codec.ts"),
    }
}

/// The `Time_t` carrier the runtime ships, for the columns that need a
/// declaration of it beside the codec.
#[must_use]
pub const fn time_carrier_source() -> &'static str {
    include_str!("../../runtime/java/TimeT.java")
}

/// The helpers for every plan in the set, in the set's order.
///
/// # Errors
/// [`EmitError::NoPlanFor`] where a field's nested composite is not in
/// the set.
pub fn helpers_for(plans: &PlanSet, language: CodegenLanguage) -> Result<String, EmitError> {
    let mut out = String::new();
    for plan in plans.plans() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&helper_for(plans, plan, language)?);
    }
    Ok(out)
}

/// The two helpers for one plan.
///
/// `plans` is still the whole set: a nested composite's helper is named
/// after **its** plan, which this one does not carry. Separate from
/// [`helpers_for`] because Java allows one public type per compilation
/// unit, so a caller writing files needs the declarations one at a time.
///
/// # Errors
/// [`EmitError::NoPlanFor`] where a field's nested composite is not in
/// the set.
pub fn helper_for(
    plans: &PlanSet,
    plan: &TypePlan,
    language: CodegenLanguage,
) -> Result<String, EmitError> {
    let amqp = walk_amqp(plans, plan)?;
    let json = walk_json(plans, plan)?;
    Ok(render_helpers(language, plan, &amqp, &json))
}

/// The carrier declarations for every plan in the set.
///
/// The type the helpers take, in the public-member shape the module
/// documentation names. A binding that declares its own types does not
/// need these; the cross-language test does.
#[must_use]
pub fn carriers_for(plans: &PlanSet, language: CodegenLanguage) -> String {
    let mut out = String::new();
    for plan in plans.plans() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&carrier_for(plan, language));
    }
    out
}

/// The carrier declaration for one plan.
#[must_use]
pub fn carrier_for(plan: &TypePlan, language: CodegenLanguage) -> String {
    render_carrier(language, plan)
}

// ---------------------------------------------------------------------
// The shared walk.
// ---------------------------------------------------------------------

/// An expression in the emitted program.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    /// A local, by name.
    Local(String),
    /// A member of another value: `v_`, `x` is `v_.x`.
    Member(AllocBox<Value>, String),
    /// A call into the runtime, by the name every column spells the same.
    Runtime(&'static str, Vec<Value>),
    /// A call to a type's generated helper.
    Helper {
        /// The plan's `helper_suffix`.
        suffix: String,
        /// The simple name, which the Java column needs for the class.
        simple: String,
        /// `true` for `toAmqpValue`, `false` for `toJsonString`.
        amqp: bool,
        /// The argument.
        argument: AllocBox<Value>,
    },
    /// A string literal.
    Str(String),
    /// An integer literal, in decimal.
    Int(u32),
    /// The descriptor of §7.2.1, in the column's own literal form.
    DescriptorLiteral(u64),
    /// An array of string literals, for an enumeration's names.
    StringArray(Vec<String>),
    /// The value inside the language's optional form.
    Unwrap(AllocBox<Value>),
    /// A map iteration binding's key half.
    EntryKey(String),
    /// A map iteration binding's value half.
    EntryPayload(String),
}

impl Value {
    fn local(name: &str) -> Self {
        Self::Local(name.to_owned())
    }

    fn member(self, name: &str) -> Self {
        Self::Member(AllocBox::new(self), name.to_owned())
    }
}

/// A statement in the emitted program.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Op {
    /// A list of encoded field octets.
    DeclareBytesList(String),
    /// Append one encoded value to such a list.
    PushBytes(String, Value),
    /// A string accumulator.
    DeclareString(String, Value),
    /// Append to a string accumulator.
    AppendString(String, Value),
    /// A boolean flag.
    DeclareBool(String, bool),
    /// Set a boolean flag.
    SetBool(String, bool),
    /// The language's presence test on an optional member.
    IfPresent {
        subject: Value,
        then: Vec<Op>,
        otherwise: Vec<Op>,
    },
    /// `if (!flag)`, which is how the JSON walk writes its separators.
    IfNotFlag { flag: String, then: Vec<Op> },
    /// Iterate a sequence or an array.
    ForEach {
        binding: String,
        element: FieldEncoding,
        range: Value,
        body: Vec<Op>,
    },
    /// Iterate a map's entries.
    ForEachEntry {
        binding: String,
        key: FieldEncoding,
        value: FieldEncoding,
        range: Value,
        body: Vec<Op>,
    },
    /// `return <octets>;`
    ReturnBytes(Value),
    /// `return <string>;`
    ReturnString(String),
}

/// A counter for the locals a nested walk needs, so two sequences in one
/// helper do not both declare `seq0_`.
#[derive(Debug, Default)]
struct Locals {
    next: u32,
}

impl Locals {
    fn take(&mut self, prefix: &str) -> String {
        let name = format!("{prefix}{}_", self.next);
        self.next += 1;
        name
    }
}

/// §8.1.3 — the walk that writes the described composite.
fn walk_amqp(plans: &PlanSet, plan: &TypePlan) -> Result<Vec<Op>, EmitError> {
    let mut locals = Locals::default();
    let mut ops = alloc::vec![Op::DeclareBytesList("f_".to_owned())];
    let subject = Value::local("v_");
    match plan.body {
        BodyForm::List => {
            for field in &plan.fields {
                ops.extend(push_field(plans, plan, field, &subject, "f_", &mut locals)?);
            }
            ops.push(Op::ReturnBytes(described(
                plan,
                Value::Runtime("encodeList", alloc::vec![Value::local("f_")]),
            )));
        }
        BodyForm::Map => {
            for field in &plan.fields {
                // §7.2.2.5 — the key is the member ID, and an absent
                // optional has no entry at all: a map has no positional
                // slot to leave a `null` in.
                let key = Op::PushBytes(
                    "f_".to_owned(),
                    Value::Runtime("encodeUint", alloc::vec![Value::Int(field.id)]),
                );
                let value = encode_value(
                    plans,
                    &field.encoding,
                    &subject.clone().member(&field.name),
                    "f_",
                    &mut locals,
                    field.optional,
                )?;
                if field.optional {
                    let mut then = alloc::vec![key];
                    then.extend(value);
                    ops.push(Op::IfPresent {
                        subject: subject.clone().member(&field.name),
                        then,
                        otherwise: Vec::new(),
                    });
                } else {
                    ops.push(key);
                    ops.extend(value);
                }
            }
            ops.push(Op::ReturnBytes(described(
                plan,
                Value::Runtime("encodeMap", alloc::vec![Value::local("f_")]),
            )));
        }
        BodyForm::Union => {
            // §7.2.3 — the discriminator, then the active branch or a
            // `null`. The branches are written as a chain so exactly one
            // of them reaches the list.
            if let Some(discriminator) = plan.fields.first() {
                ops.extend(encode_value(
                    plans,
                    &discriminator.encoding,
                    &subject.clone().member(&discriminator.name),
                    "f_",
                    &mut locals,
                    false,
                )?);
                let mut chain: Vec<Op> = alloc::vec![Op::PushBytes(
                    "f_".to_owned(),
                    Value::Runtime("encodeNull", Vec::new()),
                )];
                for field in plan.fields.iter().skip(1).rev() {
                    chain = alloc::vec![Op::IfPresent {
                        subject: subject.clone().member(&field.name),
                        then: encode_value(
                            plans,
                            &field.encoding,
                            &subject.clone().member(&field.name),
                            "f_",
                            &mut locals,
                            true,
                        )?,
                        otherwise: chain,
                    }];
                }
                ops.extend(chain);
            }
            ops.push(Op::ReturnBytes(described(
                plan,
                Value::Runtime("encodeList", alloc::vec![Value::local("f_")]),
            )));
        }
    }
    Ok(ops)
}

/// §7.2.1 — the descriptor around a body.
fn described(plan: &TypePlan, body: Value) -> Value {
    match &plan.descriptor {
        Descriptor::Ulong(code) => Value::Runtime(
            "describedUlong",
            alloc::vec![Value::DescriptorLiteral(*code), body],
        ),
        Descriptor::Symbol(symbol) => Value::Runtime(
            "describedSymbol",
            alloc::vec![Value::Str(symbol.clone()), body],
        ),
    }
}

/// One field's contribution to a positional body list.
fn push_field(
    plans: &PlanSet,
    plan: &TypePlan,
    field: &FieldPlan,
    subject: &Value,
    list: &str,
    locals: &mut Locals,
) -> Result<Vec<Op>, EmitError> {
    let access = subject.clone().member(&field.name);
    let inner = encode_value(
        plans,
        &field.encoding,
        &access,
        list,
        locals,
        field.optional,
    )?;
    if field.optional && plan.body == BodyForm::List {
        // §7.2.2.1 — an absent optional is the AMQP `null` at its
        // positional slot and is never omitted.
        return Ok(alloc::vec![Op::IfPresent {
            subject: access,
            then: inner,
            otherwise: alloc::vec![Op::PushBytes(
                list.to_owned(),
                Value::Runtime("encodeNull", Vec::new()),
            )],
        }]);
    }
    Ok(inner)
}

/// The ops that push one value, under its encoding, onto `list`.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; the walk
/// descends a [`FieldEncoding`] tree, whose depth `plan::encoding_of`
/// caps at its `MAX_TYPE_DEPTH`). `FieldEncoding::Composite` is a leaf:
/// a nested composite becomes a call to its own generated helper, so
/// the emitter never follows the plan graph and a recursive IDL type
/// cannot cycle here.
fn encode_value(
    plans: &PlanSet,
    encoding: &FieldEncoding,
    access: &Value,
    list: &str,
    locals: &mut Locals,
    optional: bool,
) -> Result<Vec<Op>, EmitError> {
    let read = if optional {
        Value::Unwrap(AllocBox::new(access.clone()))
    } else {
        access.clone()
    };
    Ok(match encoding {
        FieldEncoding::Sequence(inner) | FieldEncoding::Array(inner) => {
            let nested = locals.take("seq");
            let binding = locals.take("e");
            let body = encode_value(
                plans,
                inner,
                &Value::Local(binding.clone()),
                &nested,
                locals,
                false,
            )?;
            alloc::vec![
                Op::DeclareBytesList(nested.clone()),
                Op::ForEach {
                    binding,
                    element: (**inner).clone(),
                    range: read,
                    body,
                },
                Op::PushBytes(
                    list.to_owned(),
                    Value::Runtime("encodeList", alloc::vec![Value::Local(nested)]),
                ),
            ]
        }
        FieldEncoding::Map(key, value) => {
            let nested = locals.take("map");
            let binding = locals.take("e");
            let mut inner_ops = encode_value(
                plans,
                key,
                &Value::EntryKey(binding.clone()),
                &nested,
                locals,
                false,
            )?;
            inner_ops.extend(encode_value(
                plans,
                value,
                &Value::EntryPayload(binding.clone()),
                &nested,
                locals,
                false,
            )?);
            alloc::vec![
                Op::DeclareBytesList(nested.clone()),
                Op::ForEachEntry {
                    binding,
                    key: (**key).clone(),
                    value: (**value).clone(),
                    range: read,
                    body: inner_ops,
                },
                Op::PushBytes(
                    list.to_owned(),
                    Value::Runtime("encodeMap", alloc::vec![Value::Local(nested)]),
                ),
            ]
        }
        FieldEncoding::Composite(name) => {
            let nested = plans
                .get(name)
                .ok_or_else(|| EmitError::NoPlanFor(name.clone()))?;
            alloc::vec![Op::PushBytes(
                list.to_owned(),
                Value::Helper {
                    suffix: nested.helper_suffix(),
                    simple: nested.simple_name().to_owned(),
                    amqp: true,
                    argument: AllocBox::new(read),
                },
            )]
        }
        FieldEncoding::Time => alloc::vec![Op::PushBytes(
            list.to_owned(),
            Value::Runtime(
                "encodeTimestamp",
                alloc::vec![Value::Runtime(
                    "timeMillis",
                    alloc::vec![read.clone().member("sec"), read.member("nanosec")],
                )],
            ),
        )],
        FieldEncoding::Enumeration { literals, .. } => alloc::vec![Op::PushBytes(
            list.to_owned(),
            Value::Runtime(
                "encodeSymbol",
                alloc::vec![Value::Runtime(
                    "enumName",
                    alloc::vec![read, Value::StringArray(literals.clone())],
                )],
            ),
        )],
        scalar => alloc::vec![Op::PushBytes(
            list.to_owned(),
            Value::Runtime(scalar_encoder(scalar), alloc::vec![read]),
        )],
    })
}

/// The runtime function for a scalar encoding.
const fn scalar_encoder(encoding: &FieldEncoding) -> &'static str {
    match encoding {
        FieldEncoding::Boolean => "encodeBoolean",
        FieldEncoding::Ubyte => "encodeUbyte",
        FieldEncoding::Byte => "encodeByte",
        FieldEncoding::Ushort => "encodeUshort",
        FieldEncoding::Short => "encodeShort",
        FieldEncoding::Uint => "encodeUint",
        FieldEncoding::Int => "encodeInt",
        FieldEncoding::Ulong => "encodeUlong",
        FieldEncoding::Long => "encodeLong",
        FieldEncoding::Float => "encodeFloat",
        // §7.1.2 — `long double` narrows to binary64.
        FieldEncoding::Double | FieldEncoding::LongDouble => "encodeDouble",
        FieldEncoding::Char | FieldEncoding::WChar => "encodeChar",
        FieldEncoding::Str | FieldEncoding::WStr => "encodeStr",
        FieldEncoding::Binary => "encodeBinary",
        // The compound arms are handled by `encode_value` before this
        // is reached; the arm is written out so a new encoding is a
        // compile error rather than a `null` on the wire.
        FieldEncoding::Sequence(_)
        | FieldEncoding::Array(_)
        | FieldEncoding::Map(_, _)
        | FieldEncoding::Composite(_)
        | FieldEncoding::Enumeration { .. }
        | FieldEncoding::Time => "encodeNull",
    }
}

/// §8.1.2 — the walk that writes the JSON document.
fn walk_json(plans: &PlanSet, plan: &TypePlan) -> Result<Vec<Op>, EmitError> {
    let mut locals = Locals::default();
    let mut ops = alloc::vec![Op::DeclareString(
        "j_".to_owned(),
        Value::Str("{".to_owned())
    )];
    let subject = Value::local("v_");
    match plan.body {
        // §8.1.2 — "union: JSON object with two keys: discriminator and
        // value. The value key SHALL be omitted for empty branches."
        BodyForm::Union => {
            if let Some(discriminator) = plan.fields.first() {
                ops.push(Op::AppendString(
                    "j_".to_owned(),
                    Value::Str("\"discriminator\":".to_owned()),
                ));
                ops.extend(json_ops(
                    plans,
                    &discriminator.encoding,
                    &subject.clone().member(&discriminator.name),
                    "j_",
                    &mut locals,
                    false,
                )?);
                let mut chain: Vec<Op> = Vec::new();
                for field in plan.fields.iter().skip(1).rev() {
                    let mut then = alloc::vec![Op::AppendString(
                        "j_".to_owned(),
                        Value::Str(",\"value\":".to_owned()),
                    )];
                    then.extend(json_ops(
                        plans,
                        &field.encoding,
                        &subject.clone().member(&field.name),
                        "j_",
                        &mut locals,
                        true,
                    )?);
                    chain = alloc::vec![Op::IfPresent {
                        subject: subject.clone().member(&field.name),
                        then,
                        otherwise: chain,
                    }];
                }
                ops.extend(chain);
            }
        }
        // §8.1.2 — "struct: JSON object whose keys are the IDL field
        // names. Optional fields with absent values SHOULD be omitted."
        // The `@mutable` map body is a §7.2 decision and does not reach
        // JSON: the object's keys are the field names in both.
        BodyForm::List | BodyForm::Map => {
            let flag = locals.take("first");
            ops.push(Op::DeclareBool(flag.clone(), true));
            for field in &plan.fields {
                let access = subject.clone().member(&field.name);
                let mut write = alloc::vec![
                    Op::IfNotFlag {
                        flag: flag.clone(),
                        then: alloc::vec![Op::AppendString(
                            "j_".to_owned(),
                            Value::Str(",".to_owned()),
                        )],
                    },
                    Op::SetBool(flag.clone(), false),
                    Op::AppendString(
                        "j_".to_owned(),
                        Value::Str(format!("{}:", json_key(&field.name))),
                    ),
                ];
                write.extend(json_ops(
                    plans,
                    &field.encoding,
                    &access,
                    "j_",
                    &mut locals,
                    field.optional,
                )?);
                if field.optional {
                    ops.push(Op::IfPresent {
                        subject: access,
                        then: write,
                        otherwise: Vec::new(),
                    });
                } else {
                    ops.extend(write);
                }
            }
        }
    }
    ops.push(Op::AppendString(
        "j_".to_owned(),
        Value::Str("}".to_owned()),
    ));
    ops.push(Op::ReturnString("j_".to_owned()));
    Ok(ops)
}

/// A JSON object key, escaped once here rather than at run time.
fn json_key(name: &str) -> String {
    let mut out = String::from("\"");
    for character in name.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// §8.1.2 — the ops that append one value's JSON to `target`.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; the walk
/// descends a [`FieldEncoding`] tree, whose depth `plan::encoding_of`
/// caps at its `MAX_TYPE_DEPTH`). As in [`encode_value`], a nested
/// composite is a leaf that becomes a call to its own helper.
fn json_ops(
    plans: &PlanSet,
    encoding: &FieldEncoding,
    access: &Value,
    target: &str,
    locals: &mut Locals,
    optional: bool,
) -> Result<Vec<Op>, EmitError> {
    let read = if optional {
        Value::Unwrap(AllocBox::new(access.clone()))
    } else {
        access.clone()
    };
    let append = |value: Value| Op::AppendString(target.to_owned(), value);
    Ok(match encoding {
        // §8.1.2 — "sequence and array: JSON array."
        FieldEncoding::Sequence(inner) | FieldEncoding::Array(inner) => {
            let flag = locals.take("first");
            let binding = locals.take("e");
            let mut body = alloc::vec![
                Op::IfNotFlag {
                    flag: flag.clone(),
                    then: alloc::vec![append(Value::Str(",".to_owned()))],
                },
                Op::SetBool(flag.clone(), false),
            ];
            body.extend(json_ops(
                plans,
                inner,
                &Value::Local(binding.clone()),
                target,
                locals,
                false,
            )?);
            alloc::vec![
                append(Value::Str("[".to_owned())),
                Op::DeclareBool(flag, true),
                Op::ForEach {
                    binding,
                    element: (**inner).clone(),
                    range: read,
                    body,
                },
                append(Value::Str("]".to_owned())),
            ]
        }
        // §8.1.2 — "map: JSON object." A JSON key is a string, so a
        // numeric key is written as its own scalar inside the quotes
        // rather than dropped.
        FieldEncoding::Map(key, value) => {
            let flag = locals.take("first");
            let binding = locals.take("e");
            let mut body = alloc::vec![
                Op::IfNotFlag {
                    flag: flag.clone(),
                    then: alloc::vec![append(Value::Str(",".to_owned()))],
                },
                Op::SetBool(flag.clone(), false),
                append(json_object_key(key, &Value::EntryKey(binding.clone()))),
                append(Value::Str(":".to_owned())),
            ];
            body.extend(json_ops(
                plans,
                value,
                &Value::EntryPayload(binding.clone()),
                target,
                locals,
                false,
            )?);
            alloc::vec![
                append(Value::Str("{".to_owned())),
                Op::DeclareBool(flag, true),
                Op::ForEachEntry {
                    binding,
                    key: (**key).clone(),
                    value: (**value).clone(),
                    range: read,
                    body,
                },
                append(Value::Str("}".to_owned())),
            ]
        }
        FieldEncoding::Composite(name) => {
            let nested = plans
                .get(name)
                .ok_or_else(|| EmitError::NoPlanFor(name.clone()))?;
            alloc::vec![append(Value::Helper {
                suffix: nested.helper_suffix(),
                simple: nested.simple_name().to_owned(),
                amqp: false,
                argument: AllocBox::new(read),
            })]
        }
        // §8.1.2 — "Time_t: JSON object {"sec": n, "nanosec": m}."
        FieldEncoding::Time => alloc::vec![append(Value::Runtime(
            "jsonTime",
            alloc::vec![read.clone().member("sec"), read.member("nanosec")],
        ))],
        // §8.1.2 — "enum: JSON string of the symbolic name."
        FieldEncoding::Enumeration { literals, .. } => alloc::vec![append(Value::Runtime(
            "jsonStr",
            alloc::vec![Value::Runtime(
                "enumName",
                alloc::vec![read, Value::StringArray(literals.clone())],
            )],
        ))],
        scalar => alloc::vec![append(Value::Runtime(
            json_writer(scalar),
            alloc::vec![read],
        ))],
    })
}

/// A map key, as the quoted JSON string an object key must be.
fn json_object_key(encoding: &FieldEncoding, access: &Value) -> Value {
    match encoding {
        FieldEncoding::Str | FieldEncoding::WStr => {
            Value::Runtime("jsonStr", alloc::vec![access.clone()])
        }
        other => Value::Runtime(
            "jsonStr",
            alloc::vec![Value::Runtime(
                json_writer(other),
                alloc::vec![access.clone()]
            )],
        ),
    }
}

/// The runtime function that writes a scalar's JSON.
const fn json_writer(encoding: &FieldEncoding) -> &'static str {
    match encoding {
        FieldEncoding::Boolean => "jsonBool",
        FieldEncoding::Ubyte
        | FieldEncoding::Ushort
        | FieldEncoding::Uint
        | FieldEncoding::Ulong => "jsonU64",
        FieldEncoding::Byte | FieldEncoding::Short | FieldEncoding::Int | FieldEncoding::Long => {
            "jsonI64"
        }
        FieldEncoding::Float | FieldEncoding::Double | FieldEncoding::LongDouble => "jsonDouble",
        FieldEncoding::Char | FieldEncoding::WChar => "jsonChar",
        FieldEncoding::Str | FieldEncoding::WStr => "jsonStr",
        FieldEncoding::Binary => "jsonBase64",
        FieldEncoding::Sequence(_)
        | FieldEncoding::Array(_)
        | FieldEncoding::Map(_, _)
        | FieldEncoding::Composite(_)
        | FieldEncoding::Enumeration { .. }
        | FieldEncoding::Time => "jsonStr",
    }
}

// ---------------------------------------------------------------------
// Rendering.
// ---------------------------------------------------------------------

fn render_helpers(language: CodegenLanguage, plan: &TypePlan, amqp: &[Op], json: &[Op]) -> String {
    match language {
        CodegenLanguage::Cpp => cpp_helpers(plan, amqp, json),
        CodegenLanguage::Java => java_helpers(plan, amqp, json),
        CodegenLanguage::TypeScript => typescript_helpers(plan, amqp, json),
    }
}

fn render_carrier(language: CodegenLanguage, plan: &TypePlan) -> String {
    match language {
        CodegenLanguage::Cpp => cpp_carrier(plan),
        CodegenLanguage::Java => java_carrier(plan),
        CodegenLanguage::TypeScript => typescript_carrier(plan),
    }
}

// -- C++ --------------------------------------------------------------

fn cpp_runtime_path(name: &str) -> Path {
    Path::new(&["zerodds", "amqp", name])
}

/// The `C++` spelling of one [`Value`].
///
/// zerodds-lint: recursion-depth 8 (a [`Value`] is a fixed shape per
/// encoding and its depth does not grow with the type's: a nested
/// collection's body starts from a fresh `Value::Local` binding rather
/// than extending the enclosing value, so the deepest tree the two
/// walks build is `Runtime(Runtime(Member(Unwrap(Member(Local)))))` —
/// the `Time` arm, six levels)
fn cpp_expr(value: &Value) -> cpp::Expr {
    match value {
        Value::Local(name) => cpp::Expr::Path(Path::single(name)),
        Value::Member(base, name) => cpp::Expr::Member {
            value: AllocBox::new(cpp_expr(base)),
            name: Ident::new(name),
        },
        Value::Runtime(name, args) => cpp::Expr::Call {
            callee: AllocBox::new(cpp::Expr::Path(cpp_runtime_path(name))),
            args: args.iter().map(cpp_expr).collect(),
        },
        Value::Helper {
            suffix,
            amqp,
            argument,
            ..
        } => cpp::Expr::Call {
            callee: AllocBox::new(cpp::Expr::Path(Path::single(&helper_name(*amqp, suffix)))),
            args: alloc::vec![cpp_expr(argument)],
        },
        Value::Str(text) => cpp::Expr::Literal(Literal::string(text)),
        Value::Int(value) => cpp::Expr::Literal(Literal::int(i128::from(*value))),
        Value::DescriptorLiteral(code) => {
            cpp::Expr::Literal(Literal::Int(format!("0x{code:016X}ULL")))
        }
        Value::StringArray(items) => cpp::Expr::Construct {
            type_: AllocBox::new(cpp::Type::template(
                Path::new(&["std", "vector"]),
                alloc::vec![cpp::Type::named(Path::new(&["std", "string"]))],
            )),
            args: items
                .iter()
                .map(|item| cpp::Expr::Literal(Literal::string(item)))
                .collect(),
            braced: true,
        },
        Value::Unwrap(inner) => cpp::Expr::Paren(AllocBox::new(cpp::Expr::Deref(AllocBox::new(
            cpp_expr(inner),
        )))),
        Value::EntryKey(binding) => cpp::Expr::Member {
            value: AllocBox::new(cpp::Expr::Path(Path::single(binding))),
            name: Ident::new("first"),
        },
        Value::EntryPayload(binding) => cpp::Expr::Member {
            value: AllocBox::new(cpp::Expr::Path(Path::single(binding))),
            name: Ident::new("second"),
        },
    }
}

/// The `C++` statements for a list of [`Op`]s.
///
/// zerodds-lint: recursion-depth 64 (the nested block of a `ForEach`,
/// a `ForEachEntry` or an `IfPresent` — [`encode_value`] and
/// [`json_ops`] nest one per collection level, so the [`Op`] tree is as
/// deep as the [`FieldEncoding`] tree `plan::encoding_of` caps at its
/// `MAX_TYPE_DEPTH`)
fn cpp_ops(ops: &[Op]) -> Vec<cpp::Statement> {
    let mut out = Vec::new();
    for op in ops {
        match op {
            Op::DeclareBytesList(name) => out.push(cpp::Statement::Declare {
                type_: cpp::Type::template(
                    Path::new(&["std", "vector"]),
                    alloc::vec![cpp::Type::named(cpp_runtime_path("Bytes"))],
                ),
                name: Ident::new(name),
                value: None,
            }),
            Op::PushBytes(list, value) => out.push(cpp::Statement::Expr(cpp::Expr::MethodCall {
                receiver: AllocBox::new(cpp::Expr::Path(Path::single(list))),
                name: Ident::new("push_back"),
                args: alloc::vec![cpp_expr(value)],
            })),
            Op::DeclareString(name, value) => out.push(cpp::Statement::Declare {
                type_: cpp::Type::named(Path::new(&["std", "string"])),
                name: Ident::new(name),
                value: Some(cpp_expr(value)),
            }),
            Op::AppendString(name, value) => out.push(cpp::Statement::Expr(cpp::Expr::Binary {
                op: "+=",
                lhs: AllocBox::new(cpp::Expr::Path(Path::single(name))),
                rhs: AllocBox::new(cpp_expr(value)),
            })),
            Op::DeclareBool(name, value) => out.push(cpp::Statement::Declare {
                type_: cpp::Type::Primitive(cpp::Primitive::Bool),
                name: Ident::new(name),
                value: Some(cpp::Expr::Literal(Literal::Bool(*value))),
            }),
            Op::SetBool(name, value) => out.push(cpp::Statement::Expr(cpp::Expr::Binary {
                op: "=",
                lhs: AllocBox::new(cpp::Expr::Path(Path::single(name))),
                rhs: AllocBox::new(cpp::Expr::Literal(Literal::Bool(*value))),
            })),
            Op::IfPresent {
                subject,
                then,
                otherwise,
            } => out.push(cpp::Statement::If {
                condition: cpp::Expr::MethodCall {
                    receiver: AllocBox::new(cpp_expr(subject)),
                    name: Ident::new("has_value"),
                    args: Vec::new(),
                },
                then: cpp::Block {
                    statements: cpp_ops(then),
                },
                otherwise: if otherwise.is_empty() {
                    None
                } else {
                    Some(cpp::Block {
                        statements: cpp_ops(otherwise),
                    })
                },
            }),
            Op::IfNotFlag { flag, then } => out.push(cpp::Statement::If {
                condition: cpp::Expr::Unary {
                    op: "!",
                    value: AllocBox::new(cpp::Expr::Path(Path::single(flag))),
                },
                then: cpp::Block {
                    statements: cpp_ops(then),
                },
                otherwise: None,
            }),
            Op::ForEach {
                binding,
                range,
                body,
                ..
            }
            | Op::ForEachEntry {
                binding,
                range,
                body,
                ..
            } => out.push(cpp::Statement::ForEach {
                binding: Ident::new(binding),
                range: cpp_expr(range),
                body: cpp::Block {
                    statements: cpp_ops(body),
                },
            }),
            Op::ReturnBytes(value) => out.push(cpp::Statement::Return(Some(cpp_expr(value)))),
            Op::ReturnString(name) => out.push(cpp::Statement::Return(Some(cpp::Expr::Path(
                Path::single(name),
            )))),
        }
    }
    out
}

fn cpp_type_of(encoding: &FieldEncoding, optional: bool) -> cpp::Type {
    let inner = match encoding {
        FieldEncoding::Boolean => cpp::Type::Primitive(cpp::Primitive::Bool),
        FieldEncoding::Ubyte => cpp::Type::Primitive(cpp::Primitive::UInt8),
        FieldEncoding::Byte => cpp::Type::Primitive(cpp::Primitive::Int8),
        FieldEncoding::Ushort => cpp::Type::Primitive(cpp::Primitive::UInt16),
        FieldEncoding::Short => cpp::Type::Primitive(cpp::Primitive::Int16),
        FieldEncoding::Uint => cpp::Type::Primitive(cpp::Primitive::UInt32),
        FieldEncoding::Int | FieldEncoding::Enumeration { .. } => {
            cpp::Type::Primitive(cpp::Primitive::Int32)
        }
        FieldEncoding::Ulong => cpp::Type::Primitive(cpp::Primitive::UInt64),
        FieldEncoding::Long => cpp::Type::Primitive(cpp::Primitive::Int64),
        FieldEncoding::Float => cpp::Type::Primitive(cpp::Primitive::Float),
        FieldEncoding::Double | FieldEncoding::LongDouble => {
            cpp::Type::Primitive(cpp::Primitive::Double)
        }
        FieldEncoding::Char | FieldEncoding::WChar => cpp::Type::Primitive(cpp::Primitive::UInt32),
        FieldEncoding::Str | FieldEncoding::WStr => cpp::Type::named(Path::new(&["std", "string"])),
        FieldEncoding::Binary => cpp::Type::named(cpp_runtime_path("Bytes")),
        FieldEncoding::Sequence(inner) | FieldEncoding::Array(inner) => cpp::Type::template(
            Path::new(&["std", "vector"]),
            alloc::vec![cpp_type_of(inner, false)],
        ),
        FieldEncoding::Map(key, value) => cpp::Type::template(
            Path::new(&["std", "map"]),
            alloc::vec![cpp_type_of(key, false), cpp_type_of(value, false)],
        ),
        FieldEncoding::Composite(name) => cpp::Type::named(Path::single(simple_of(name))),
        FieldEncoding::Time => cpp::Type::named(cpp_runtime_path("TimeT")),
    };
    if optional {
        return cpp::Type::template(Path::new(&["std", "optional"]), alloc::vec![inner]);
    }
    inner
}

fn cpp_carrier(plan: &TypePlan) -> String {
    let members = plan
        .fields
        .iter()
        .map(|field| {
            cpp::ClassMember::Field(cpp::Field {
                doc: Doc::none(),
                type_: cpp_type_of(&field.encoding, field.optional),
                name: Ident::new(&cpp::escape_keyword(&field.name)),
                dims: Vec::new(),
                default: None,
                is_static: false,
                is_constexpr: false,
                bits: None,
            })
        })
        .collect();
    zerodds_lang_render::cpp::decl(&cpp::Decl::Class(cpp::Class {
        doc: Doc::line(&format!("The carrier for `{}`.", plan.scoped_name)),
        is_struct: true,
        name: Ident::new(plan.simple_name()),
        bases: Vec::new(),
        sections: alloc::vec![cpp::Section {
            access: cpp::Access::Public,
            members,
        }],
    }))
}

fn cpp_helpers(plan: &TypePlan, amqp: &[Op], json: &[Op]) -> String {
    let subject = cpp::Param {
        type_: cpp::Type::Ref(AllocBox::new(cpp::Type::Const(AllocBox::new(
            cpp::Type::named(Path::single(plan.simple_name())),
        )))),
        name: Some(Ident::new("v_")),
        default: None,
    };
    let amqp_function = cpp::Decl::Function(cpp::Function {
        doc: Doc::line(&format!(
            "DDS-AMQP 1.0 §8.1.3 — `{}` as a described composite.",
            plan.scoped_name
        )),
        returns: cpp::Type::named(cpp_runtime_path("Bytes")),
        name: Ident::new(&helper_name(true, &plan.helper_suffix())),
        params: alloc::vec![subject.clone()],
        is_inline: true,
        is_static: false,
        is_constexpr: false,
        is_noexcept: false,
        is_maybe_unused: false,
        body: cpp::FunctionBody::Block(cpp::Block {
            statements: cpp_ops(amqp),
        }),
    });
    let json_function = cpp::Decl::Function(cpp::Function {
        doc: Doc::line(&format!(
            "DDS-AMQP 1.0 §8.1.2 — `{}` as a JSON document.",
            plan.scoped_name
        )),
        returns: cpp::Type::named(Path::new(&["std", "string"])),
        name: Ident::new(&helper_name(false, &plan.helper_suffix())),
        params: alloc::vec![subject],
        is_inline: true,
        is_static: false,
        is_constexpr: false,
        is_noexcept: false,
        is_maybe_unused: false,
        body: cpp::FunctionBody::Block(cpp::Block {
            statements: cpp_ops(json),
        }),
    });
    format!(
        "{}\n{}",
        zerodds_lang_render::cpp::decl(&amqp_function),
        zerodds_lang_render::cpp::decl(&json_function)
    )
}

// -- Java -------------------------------------------------------------

fn java_runtime() -> java::Expr {
    java::Expr::Path(Path::new(&["org", "zerodds", "amqp", "AmqpCodec"]))
}

/// The `Java` spelling of one [`Value`].
///
/// zerodds-lint: recursion-depth 8 (a [`Value`] is a fixed shape per
/// encoding and its depth does not grow with the type's: a nested
/// collection's body starts from a fresh `Value::Local` binding rather
/// than extending the enclosing value, so the deepest tree the two
/// walks build is `Runtime(Runtime(Member(Unwrap(Member(Local)))))` —
/// the `Time` arm, six levels)
fn java_expr(value: &Value) -> java::Expr {
    match value {
        Value::Local(name) => java::Expr::Path(Path::single(name)),
        Value::Member(base, name) => java::Expr::Field {
            value: AllocBox::new(java_expr(base)),
            name: Ident::new(name),
        },
        Value::Runtime(name, args) => java::Expr::MethodCall {
            receiver: Some(AllocBox::new(java_runtime())),
            name: Ident::new(name),
            args: args.iter().map(java_expr).collect(),
        },
        Value::Helper {
            simple,
            amqp,
            argument,
            ..
        } => java::Expr::MethodCall {
            receiver: Some(AllocBox::new(java::Expr::Path(Path::single(&format!(
                "{simple}AmqpCodec"
            ))))),
            name: Ident::new(if *amqp { "toAmqpValue" } else { "toJsonString" }),
            args: alloc::vec![java_expr(argument)],
        },
        Value::Str(text) => java::Expr::Literal(Literal::string(text)),
        Value::Int(value) => java::Expr::Literal(Literal::int(i128::from(*value))),
        Value::DescriptorLiteral(code) => {
            java::Expr::Literal(Literal::Int(format!("0x{code:016X}L")))
        }
        Value::StringArray(items) => java::Expr::NewArray {
            element: AllocBox::new(java::Type::named(Path::new(&["java", "lang", "String"]))),
            lengths: Vec::new(),
            elements: items
                .iter()
                .map(|item| java::Expr::Literal(Literal::string(item)))
                .collect(),
        },
        // A boxed Java reference already is the value; there is nothing
        // to unwrap and autounboxing takes it from there.
        Value::Unwrap(inner) => java_expr(inner),
        Value::EntryKey(binding) => java::Expr::MethodCall {
            receiver: Some(AllocBox::new(java::Expr::Path(Path::single(binding)))),
            name: Ident::new("getKey"),
            args: Vec::new(),
        },
        Value::EntryPayload(binding) => java::Expr::MethodCall {
            receiver: Some(AllocBox::new(java::Expr::Path(Path::single(binding)))),
            name: Ident::new("getValue"),
            args: Vec::new(),
        },
    }
}

fn java_bytes() -> java::Type {
    java::Type::Array(AllocBox::new(java::Type::Primitive(java::Primitive::Byte)))
}

/// The `Java` statements for a list of [`Op`]s.
///
/// zerodds-lint: recursion-depth 64 (the nested block of a `ForEach`,
/// a `ForEachEntry` or an `IfPresent` — [`encode_value`] and
/// [`json_ops`] nest one per collection level, so the [`Op`] tree is as
/// deep as the [`FieldEncoding`] tree `plan::encoding_of` caps at its
/// `MAX_TYPE_DEPTH`)
fn java_ops(ops: &[Op]) -> Vec<java::Statement> {
    let mut out = Vec::new();
    for op in ops {
        match op {
            Op::DeclareBytesList(name) => out.push(java::Statement::Declare {
                final_: false,
                type_: java::Type::generic(
                    Path::new(&["java", "util", "List"]),
                    alloc::vec![java_bytes()],
                ),
                name: Ident::new(name),
                value: Some(java::Expr::New {
                    type_: AllocBox::new(java::Type::generic(
                        Path::new(&["java", "util", "ArrayList"]),
                        alloc::vec![java_bytes()],
                    )),
                    args: Vec::new(),
                }),
            }),
            Op::PushBytes(list, value) => out.push(java::Statement::Expr(java::Expr::MethodCall {
                receiver: Some(AllocBox::new(java::Expr::Path(Path::single(list)))),
                name: Ident::new("add"),
                args: alloc::vec![java_expr(value)],
            })),
            Op::DeclareString(name, value) => out.push(java::Statement::Declare {
                final_: false,
                type_: java::Type::named(Path::new(&["java", "lang", "String"])),
                name: Ident::new(name),
                value: Some(java_expr(value)),
            }),
            Op::AppendString(name, value) => out.push(java::Statement::Expr(java::Expr::Binary {
                op: "+=",
                lhs: AllocBox::new(java::Expr::Path(Path::single(name))),
                rhs: AllocBox::new(java_expr(value)),
            })),
            Op::DeclareBool(name, value) => out.push(java::Statement::Declare {
                final_: false,
                type_: java::Type::Primitive(java::Primitive::Boolean),
                name: Ident::new(name),
                value: Some(java::Expr::Literal(Literal::Bool(*value))),
            }),
            Op::SetBool(name, value) => out.push(java::Statement::Expr(java::Expr::Binary {
                op: "=",
                lhs: AllocBox::new(java::Expr::Path(Path::single(name))),
                rhs: AllocBox::new(java::Expr::Literal(Literal::Bool(*value))),
            })),
            Op::IfPresent {
                subject,
                then,
                otherwise,
            } => out.push(java::Statement::If {
                condition: java::Expr::Binary {
                    op: "!=",
                    lhs: AllocBox::new(java_expr(subject)),
                    rhs: AllocBox::new(java::Expr::Literal(Literal::Null)),
                },
                then: java::Block {
                    statements: java_ops(then),
                },
                otherwise: if otherwise.is_empty() {
                    None
                } else {
                    Some(java::Block {
                        statements: java_ops(otherwise),
                    })
                },
            }),
            Op::IfNotFlag { flag, then } => out.push(java::Statement::If {
                condition: java::Expr::Binary {
                    op: "==",
                    lhs: AllocBox::new(java::Expr::Path(Path::single(flag))),
                    rhs: AllocBox::new(java::Expr::Literal(Literal::Bool(false))),
                },
                then: java::Block {
                    statements: java_ops(then),
                },
                otherwise: None,
            }),
            Op::ForEach {
                binding,
                element,
                range,
                body,
            } => out.push(java::Statement::ForEach {
                type_: java_type_of(element, true),
                binding: Ident::new(binding),
                range: java_expr(range),
                body: java::Block {
                    statements: java_ops(body),
                },
            }),
            Op::ForEachEntry {
                binding,
                key,
                value,
                range,
                body,
            } => out.push(java::Statement::ForEach {
                type_: java::Type::generic(
                    Path::new(&["java", "util", "Map", "Entry"]),
                    alloc::vec![java_type_of(key, true), java_type_of(value, true)],
                ),
                binding: Ident::new(binding),
                range: java::Expr::MethodCall {
                    receiver: Some(AllocBox::new(java_expr(range))),
                    name: Ident::new("entrySet"),
                    args: Vec::new(),
                },
                body: java::Block {
                    statements: java_ops(body),
                },
            }),
            Op::ReturnBytes(value) => out.push(java::Statement::Return(Some(java_expr(value)))),
            Op::ReturnString(name) => out.push(java::Statement::Return(Some(java::Expr::Path(
                Path::single(name),
            )))),
        }
    }
    out
}

/// The Java type of an encoding. `boxed` is what a generic argument and
/// an `@optional` member need — Java has no `List<int>`.
fn java_type_of(encoding: &FieldEncoding, boxed: bool) -> java::Type {
    let named = |segments: &[&str]| java::Type::named(Path::new(segments));
    match encoding {
        FieldEncoding::Boolean => {
            if boxed {
                named(&["java", "lang", "Boolean"])
            } else {
                java::Type::Primitive(java::Primitive::Boolean)
            }
        }
        FieldEncoding::Ubyte
        | FieldEncoding::Byte
        | FieldEncoding::Ushort
        | FieldEncoding::Short
        | FieldEncoding::Char
        | FieldEncoding::WChar
        | FieldEncoding::Int
        | FieldEncoding::Enumeration { .. } => {
            if boxed {
                named(&["java", "lang", "Integer"])
            } else {
                java::Type::Primitive(java::Primitive::Int)
            }
        }
        FieldEncoding::Uint | FieldEncoding::Ulong | FieldEncoding::Long => {
            if boxed {
                named(&["java", "lang", "Long"])
            } else {
                java::Type::Primitive(java::Primitive::Long)
            }
        }
        FieldEncoding::Float => {
            if boxed {
                named(&["java", "lang", "Float"])
            } else {
                java::Type::Primitive(java::Primitive::Float)
            }
        }
        FieldEncoding::Double | FieldEncoding::LongDouble => {
            if boxed {
                named(&["java", "lang", "Double"])
            } else {
                java::Type::Primitive(java::Primitive::Double)
            }
        }
        FieldEncoding::Str | FieldEncoding::WStr => named(&["java", "lang", "String"]),
        FieldEncoding::Binary => java_bytes(),
        FieldEncoding::Sequence(inner) | FieldEncoding::Array(inner) => java::Type::generic(
            Path::new(&["java", "util", "List"]),
            alloc::vec![java_type_of(inner, true)],
        ),
        FieldEncoding::Map(key, value) => java::Type::generic(
            Path::new(&["java", "util", "Map"]),
            alloc::vec![java_type_of(key, true), java_type_of(value, true)],
        ),
        FieldEncoding::Composite(name) => java::Type::named(Path::single(simple_of(name))),
        FieldEncoding::Time => named(&["org", "zerodds", "amqp", "TimeT"]),
    }
}

fn java_carrier(plan: &TypePlan) -> String {
    let members = plan
        .fields
        .iter()
        .map(|field| {
            java::Member::Field(java::Field {
                doc: Doc::none(),
                annotations: Vec::new(),
                modifiers: alloc::vec![java::Modifier::Public],
                type_: java_type_of(&field.encoding, field.optional),
                name: Ident::new(&java::escape_keyword(&field.name)),
                value: None,
            })
        })
        .collect();
    zerodds_lang_render::java::type_decl(&java::TypeDecl::Class(java::Class {
        doc: Doc::line(&format!("The carrier for `{}`.", plan.scoped_name)),
        annotations: Vec::new(),
        modifiers: alloc::vec![java::Modifier::Public],
        name: Ident::new(plan.simple_name()),
        generics: Vec::new(),
        extends: None,
        implements: Vec::new(),
        members,
    }))
}

fn java_helpers(plan: &TypePlan, amqp: &[Op], json: &[Op]) -> String {
    let subject = || java::Param {
        annotations: Vec::new(),
        final_: false,
        type_: java::Type::named(Path::single(plan.simple_name())),
        name: Ident::new("v_"),
        varargs: false,
    };
    let method = |name: &str, returns: java::Type, doc: &str, body: &[Op]| {
        java::Member::Method(java::Method {
            doc: Doc::line(doc),
            annotations: Vec::new(),
            modifiers: alloc::vec![java::Modifier::Public, java::Modifier::Static],
            generics: Vec::new(),
            returns,
            name: Ident::new(name),
            params: alloc::vec![subject()],
            throws: Vec::new(),
            body: Some(java::Block {
                statements: java_ops(body),
            }),
        })
    };
    zerodds_lang_render::java::type_decl(&java::TypeDecl::Class(java::Class {
        doc: Doc::line(&format!(
            "DDS-AMQP 1.0 §8.1.2 and §8.1.3 for `{}`.",
            plan.scoped_name
        )),
        annotations: Vec::new(),
        modifiers: alloc::vec![java::Modifier::Public, java::Modifier::Final],
        name: Ident::new(&format!("{}AmqpCodec", plan.simple_name())),
        generics: Vec::new(),
        extends: None,
        implements: Vec::new(),
        members: alloc::vec![
            method(
                "toAmqpValue",
                java_bytes(),
                &format!(
                    "DDS-AMQP 1.0 §8.1.3 — `{}` as a described composite.",
                    plan.scoped_name
                ),
                amqp,
            ),
            method(
                "toJsonString",
                java::Type::named(Path::new(&["java", "lang", "String"])),
                &format!(
                    "DDS-AMQP 1.0 §8.1.2 — `{}` as a JSON document.",
                    plan.scoped_name
                ),
                json,
            ),
        ],
    }))
}

// -- TypeScript -------------------------------------------------------

/// The `TypeScript` spelling of one [`Value`].
///
/// zerodds-lint: recursion-depth 8 (a [`Value`] is a fixed shape per
/// encoding and its depth does not grow with the type's: a nested
/// collection's body starts from a fresh `Value::Local` binding rather
/// than extending the enclosing value, so the deepest tree the two
/// walks build is `Runtime(Runtime(Member(Unwrap(Member(Local)))))` —
/// the `Time` arm, six levels)
fn typescript_expr(value: &Value) -> typescript::Expr {
    match value {
        Value::Local(name) => typescript::Expr::Path(Path::single(name)),
        Value::Member(base, name) => typescript::Expr::Member {
            object: AllocBox::new(typescript_expr(base)),
            name: Ident::new(name),
        },
        Value::Runtime(name, args) => typescript::Expr::Call {
            callee: AllocBox::new(typescript::Expr::Path(Path::new(&["codec", name]))),
            args: args.iter().map(typescript_expr).collect(),
        },
        Value::Helper {
            suffix,
            amqp,
            argument,
            ..
        } => typescript::Expr::Call {
            callee: AllocBox::new(typescript::Expr::Path(Path::single(&helper_name(
                *amqp, suffix,
            )))),
            args: alloc::vec![typescript_expr(argument)],
        },
        Value::Str(text) => typescript::Expr::Literal(Literal::string(text)),
        Value::Int(value) => typescript::Expr::Literal(Literal::int(i128::from(*value))),
        Value::DescriptorLiteral(code) => {
            typescript::Expr::Literal(Literal::Int(format!("0x{code:016X}n")))
        }
        Value::StringArray(items) => typescript::Expr::Array(
            items
                .iter()
                .map(|item| typescript::Expr::Literal(Literal::string(item)))
                .collect(),
        ),
        // `strict` mode narrows a `T | undefined` only inside the
        // presence test's block, and the walk emits the read inside it,
        // so the assertion is what tells the compiler that.
        Value::Unwrap(inner) => typescript::Expr::NonNull(AllocBox::new(typescript_expr(inner))),
        Value::EntryKey(binding) => typescript::Expr::Index {
            object: AllocBox::new(typescript::Expr::Path(Path::single(binding))),
            index: AllocBox::new(typescript::Expr::Literal(Literal::int(0))),
        },
        Value::EntryPayload(binding) => typescript::Expr::Index {
            object: AllocBox::new(typescript::Expr::Path(Path::single(binding))),
            index: AllocBox::new(typescript::Expr::Literal(Literal::int(1))),
        },
    }
}

/// The `TypeScript` statements for a list of [`Op`]s.
///
/// zerodds-lint: recursion-depth 64 (the nested block of a `ForEach`,
/// a `ForEachEntry` or an `IfPresent` — [`encode_value`] and
/// [`json_ops`] nest one per collection level, so the [`Op`] tree is as
/// deep as the [`FieldEncoding`] tree `plan::encoding_of` caps at its
/// `MAX_TYPE_DEPTH`)
fn typescript_ops(ops: &[Op]) -> Vec<typescript::Statement> {
    let mut out = Vec::new();
    for op in ops {
        match op {
            Op::DeclareBytesList(name) => out.push(typescript::Statement::Declare {
                kind: typescript::VarKind::Const,
                name: Ident::new(name),
                type_: Some(typescript::Type::Array(AllocBox::new(
                    typescript::Type::named(Path::single("Uint8Array")),
                ))),
                value: Some(typescript::Expr::Array(Vec::new())),
            }),
            Op::PushBytes(list, value) => {
                out.push(typescript::Statement::Expr(typescript::Expr::Call {
                    callee: AllocBox::new(typescript::Expr::Member {
                        object: AllocBox::new(typescript::Expr::Path(Path::single(list))),
                        name: Ident::new("push"),
                    }),
                    args: alloc::vec![typescript_expr(value)],
                }));
            }
            Op::DeclareString(name, value) => out.push(typescript::Statement::Declare {
                kind: typescript::VarKind::Let,
                name: Ident::new(name),
                type_: Some(typescript::Type::string()),
                value: Some(typescript_expr(value)),
            }),
            Op::AppendString(name, value) => {
                out.push(typescript::Statement::Expr(typescript::Expr::Binary {
                    op: "+=",
                    lhs: AllocBox::new(typescript::Expr::Path(Path::single(name))),
                    rhs: AllocBox::new(typescript_expr(value)),
                }));
            }
            Op::DeclareBool(name, value) => out.push(typescript::Statement::Declare {
                kind: typescript::VarKind::Let,
                name: Ident::new(name),
                type_: Some(typescript::Type::Keyword(typescript::Keyword::Boolean)),
                value: Some(typescript::Expr::Literal(Literal::Bool(*value))),
            }),
            Op::SetBool(name, value) => {
                out.push(typescript::Statement::Expr(typescript::Expr::Binary {
                    op: "=",
                    lhs: AllocBox::new(typescript::Expr::Path(Path::single(name))),
                    rhs: AllocBox::new(typescript::Expr::Literal(Literal::Bool(*value))),
                }));
            }
            Op::IfPresent {
                subject,
                then,
                otherwise,
            } => out.push(typescript::Statement::If {
                condition: typescript::Expr::Binary {
                    op: "!==",
                    lhs: AllocBox::new(typescript_expr(subject)),
                    rhs: AllocBox::new(typescript::Expr::Undefined),
                },
                then: typescript::Block {
                    statements: typescript_ops(then),
                },
                otherwise: if otherwise.is_empty() {
                    None
                } else {
                    Some(typescript::Block {
                        statements: typescript_ops(otherwise),
                    })
                },
            }),
            Op::IfNotFlag { flag, then } => out.push(typescript::Statement::If {
                condition: typescript::Expr::Unary {
                    op: "!",
                    value: AllocBox::new(typescript::Expr::Path(Path::single(flag))),
                },
                then: typescript::Block {
                    statements: typescript_ops(then),
                },
                otherwise: None,
            }),
            Op::ForEach {
                binding,
                range,
                body,
                ..
            }
            | Op::ForEachEntry {
                binding,
                range,
                body,
                ..
            } => out.push(typescript::Statement::ForOf {
                kind: typescript::VarKind::Const,
                binding: Ident::new(binding),
                range: typescript_expr(range),
                body: typescript::Block {
                    statements: typescript_ops(body),
                },
            }),
            Op::ReturnBytes(value) => {
                out.push(typescript::Statement::Return(Some(typescript_expr(value))));
            }
            Op::ReturnString(name) => out.push(typescript::Statement::Return(Some(
                typescript::Expr::Path(Path::single(name)),
            ))),
        }
    }
    out
}

/// The TypeScript spelling of one [`FieldEncoding`].
///
/// zerodds-lint: recursion-depth 64 (collection nesting; the
/// [`FieldEncoding`] tree's depth is capped by `plan::encoding_of`'s
/// `MAX_TYPE_DEPTH`). `FieldEncoding::Composite` is a leaf — it is
/// spelled as the carrier interface's own name — so a recursive IDL
/// type does not cycle here.
fn typescript_type_of(encoding: &FieldEncoding, optional: bool) -> typescript::Type {
    let inner = match encoding {
        FieldEncoding::Boolean => typescript::Type::Keyword(typescript::Keyword::Boolean),
        FieldEncoding::Ubyte
        | FieldEncoding::Byte
        | FieldEncoding::Ushort
        | FieldEncoding::Short
        | FieldEncoding::Uint
        | FieldEncoding::Int
        | FieldEncoding::Float
        | FieldEncoding::Double
        | FieldEncoding::LongDouble
        | FieldEncoding::Char
        | FieldEncoding::WChar
        | FieldEncoding::Enumeration { .. } => typescript::Type::number(),
        FieldEncoding::Ulong | FieldEncoding::Long => typescript::Type::bigint(),
        FieldEncoding::Str | FieldEncoding::WStr => typescript::Type::string(),
        FieldEncoding::Binary => typescript::Type::named(Path::single("Uint8Array")),
        FieldEncoding::Sequence(inner) | FieldEncoding::Array(inner) => {
            typescript::Type::Array(AllocBox::new(typescript_type_of(inner, false)))
        }
        FieldEncoding::Map(key, value) => typescript::Type::generic(
            Path::single("Map"),
            alloc::vec![
                typescript_type_of(key, false),
                typescript_type_of(value, false),
            ],
        ),
        FieldEncoding::Composite(name) => typescript::Type::named(Path::single(simple_of(name))),
        FieldEncoding::Time => typescript::Type::named(Path::new(&["codec", "TimeT"])),
    };
    if optional {
        return typescript::Type::Union(alloc::vec![
            inner,
            typescript::Type::Keyword(typescript::Keyword::Undefined),
        ]);
    }
    inner
}

fn typescript_carrier(plan: &TypePlan) -> String {
    let members = plan
        .fields
        .iter()
        .map(|field| {
            typescript::Signature::Property(typescript::Property {
                doc: Doc::none(),
                readonly: false,
                name: typescript::PropertyName::ident(&field.name),
                optional: field.optional,
                type_: typescript_type_of(&field.encoding, false),
            })
        })
        .collect();
    zerodds_lang_render::typescript::decl(&typescript::Decl::Interface(typescript::Interface {
        doc: Doc::line(&format!("The carrier for `{}`.", plan.scoped_name)),
        exported: true,
        name: Ident::new(plan.simple_name()),
        generics: Vec::new(),
        extends: Vec::new(),
        members,
    }))
}

fn typescript_helpers(plan: &TypePlan, amqp: &[Op], json: &[Op]) -> String {
    let subject = || typescript::Param {
        name: Ident::new("v_"),
        optional: false,
        rest: false,
        type_: Some(typescript::Type::named(Path::single(plan.simple_name()))),
        default: None,
    };
    let amqp_function = typescript::Decl::Function(typescript::Function {
        doc: Doc::line(&format!(
            "DDS-AMQP 1.0 §8.1.3 — `{}` as a described composite.",
            plan.scoped_name
        )),
        exported: true,
        name: Ident::new(&helper_name(true, &plan.helper_suffix())),
        generics: Vec::new(),
        params: alloc::vec![subject()],
        returns: Some(typescript::Type::named(Path::single("Uint8Array"))),
        body: Some(typescript::Block {
            statements: typescript_ops(amqp),
        }),
    });
    let json_function = typescript::Decl::Function(typescript::Function {
        doc: Doc::line(&format!(
            "DDS-AMQP 1.0 §8.1.2 — `{}` as a JSON document.",
            plan.scoped_name
        )),
        exported: true,
        name: Ident::new(&helper_name(false, &plan.helper_suffix())),
        generics: Vec::new(),
        params: alloc::vec![subject()],
        returns: Some(typescript::Type::string()),
        body: Some(typescript::Block {
            statements: typescript_ops(json),
        }),
    });
    format!(
        "{}\n{}",
        zerodds_lang_render::typescript::decl(&amqp_function),
        zerodds_lang_render::typescript::decl(&json_function)
    )
}

// -- names ------------------------------------------------------------

/// `toAmqpValue_<suffix>` and `toJsonString_<suffix>`, which is the pair
/// the predecessor of this module wrote and what a caller looks for.
fn helper_name(amqp: bool, suffix: &str) -> String {
    if amqp {
        format!("toAmqpValue_{suffix}")
    } else {
        format!("toJsonString_{suffix}")
    }
}

/// The last segment of a scoped name.
fn simple_of(scoped: &str) -> &str {
    scoped.rsplit("::").next().unwrap_or(scoped)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::codegen::plan::FieldPlan;

    fn plan() -> TypePlan {
        TypePlan {
            scoped_name: "probe::Point".to_string(),
            descriptor: Descriptor::Ulong(0x0102_0304_0506_0708),
            body: BodyForm::List,
            fields: alloc::vec![
                FieldPlan {
                    name: "x".to_string(),
                    id: 0,
                    optional: false,
                    encoding: FieldEncoding::Int,
                },
                FieldPlan {
                    name: "label".to_string(),
                    id: 1,
                    optional: false,
                    encoding: FieldEncoding::Str,
                },
            ],
        }
    }

    #[test]
    fn every_column_writes_both_helpers() {
        let plans = PlanSet::new(alloc::vec![plan()]);
        for language in CodegenLanguage::ALL {
            let text = helpers_for(&plans, language).expect("helpers");
            assert!(
                text.contains("toAmqpValue"),
                "{}: {text}",
                language.as_str()
            );
            assert!(
                text.contains("toJsonString"),
                "{}: {text}",
                language.as_str()
            );
        }
    }

    #[test]
    fn the_descriptor_takes_each_columns_own_literal_suffix() {
        let plans = PlanSet::new(alloc::vec![plan()]);
        let cpp = helpers_for(&plans, CodegenLanguage::Cpp).expect("cpp");
        let java = helpers_for(&plans, CodegenLanguage::Java).expect("java");
        let ts = helpers_for(&plans, CodegenLanguage::TypeScript).expect("ts");
        assert!(cpp.contains("0x0102030405060708ULL"), "{cpp}");
        assert!(java.contains("0x0102030405060708L"), "{java}");
        assert!(ts.contains("0x0102030405060708n"), "{ts}");
    }

    #[test]
    fn the_fields_reach_the_runtime_in_declaration_order() {
        let plans = PlanSet::new(alloc::vec![plan()]);
        let text = helpers_for(&plans, CodegenLanguage::Cpp).expect("cpp");
        let int_at = text.find("encodeInt").expect("encodeInt");
        let str_at = text.find("encodeStr").expect("encodeStr");
        assert!(int_at < str_at, "{text}");
    }

    #[test]
    fn a_helper_suffix_does_not_collide_across_two_scopings() {
        let a = TypePlan {
            scoped_name: "A::B::C".to_string(),
            ..plan()
        };
        let b = TypePlan {
            scoped_name: "A::B_C".to_string(),
            ..plan()
        };
        assert_ne!(a.helper_suffix(), b.helper_suffix());
    }

    #[test]
    fn an_absent_optional_writes_a_null_at_its_slot() {
        let mut subject = plan();
        subject.fields[1].optional = true;
        let plans = PlanSet::new(alloc::vec![subject]);
        let text = helpers_for(&plans, CodegenLanguage::Cpp).expect("cpp");
        assert!(text.contains("has_value()"), "{text}");
        assert!(text.contains("encodeNull()"), "{text}");
    }

    #[test]
    fn a_nested_composite_without_a_plan_is_named_rather_than_skipped() {
        let mut subject = plan();
        subject.fields[0].encoding = FieldEncoding::Composite("probe::Missing".to_string());
        let plans = PlanSet::new(alloc::vec![subject]);
        assert_eq!(
            helpers_for(&plans, CodegenLanguage::Cpp),
            Err(EmitError::NoPlanFor("probe::Missing".to_string()))
        );
    }

    #[test]
    fn the_runtimes_are_shipped_and_not_empty() {
        for language in CodegenLanguage::ALL {
            let source = runtime_source(language);
            assert!(source.len() > 1000, "{}", language.as_str());
            assert!(source.contains("encodeList"), "{}", language.as_str());
        }
    }
}

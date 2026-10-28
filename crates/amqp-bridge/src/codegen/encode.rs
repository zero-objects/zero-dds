// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-AMQP 1.0 §8.1.2 and §8.1.3 — the two body encodings, from a
//! [`TypePlan`] and a sample.
//!
//! This is the reference the three language emitters are checked
//! against. It is not a second reading of §7: the decisions about which
//! AMQP code a field takes and what body form the composite is in are
//! [`TypePlan`]'s, made once, and this walks them.
//!
//! ## Why a sample value model
//!
//! `crates/amqp-endpoint/`'s `encode_dds_to_amqp_body` takes XCDR2
//! octets and no type, which is why its JSON mode writes
//! `{"_xcdr2":"<hex>"}` and says so in a comment: §8.1.2 "requires type
//! reflection" and the function has none. [`SampleValue`] is the other
//! half — a sample as field values rather than as octets — so the
//! clause's rules can be applied and tested. What decodes XCDR2 into
//! one is a caller's business; `zerodds-types`' dynamic data is the
//! obvious source and this crate does not depend on it.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::codegen::plan::{BodyForm, Descriptor, FieldEncoding, TypePlan};
use crate::extended_types::AmqpExtValue;

/// Every plan a sample can reach, by scoped name.
///
/// §7.2.2.4: "Nested composite types (struct fields whose type is itself
/// a composite) SHALL themselves be encoded as described composites
/// recursively." A nested composite's descriptor is its **own** type's,
/// which the enclosing plan does not carry, so the encoder needs the set
/// rather than one plan. A field whose composite is not in the set is
/// [`EncodeError::NoPlanFor`], which names the type — the alternative
/// is a body list with no descriptor in front of it, which is a
/// different value that decodes without complaint.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlanSet {
    plans: Vec<TypePlan>,
}

impl PlanSet {
    /// A set over the plans, in any order.
    #[must_use]
    pub fn new(plans: Vec<TypePlan>) -> Self {
        Self { plans }
    }

    /// The plan for a scoped name.
    #[must_use]
    pub fn get(&self, scoped_name: &str) -> Option<&TypePlan> {
        self.plans
            .iter()
            .find(|plan| plan.scoped_name == scoped_name)
    }

    /// Every plan, in the order the set was built.
    #[must_use]
    pub fn plans(&self) -> &[TypePlan] {
        &self.plans
    }
}

/// A DDS sample, as field values.
#[derive(Debug, Clone, PartialEq)]
pub enum SampleValue {
    /// A `boolean`.
    Bool(bool),
    /// Any signed IDL integer, and an enumeration's ordinal.
    Int(i64),
    /// Any unsigned IDL integer.
    Uint(u64),
    /// A `float`, `double` or `long double`.
    Float(f64),
    /// A `char` or `wchar`, as a code point.
    Char(char),
    /// A `string` or `wstring`, already UTF-8.
    Str(String),
    /// An octet sequence or array.
    Bytes(Vec<u8>),
    /// A `sequence`, an array, or a union's branch list.
    Seq(Vec<SampleValue>),
    /// A `map`, in the order the sample holds it.
    Map(Vec<(SampleValue, SampleValue)>),
    /// A nested composite's fields, in the plan's order. `None` is an
    /// absent `@optional` member.
    Composite(Vec<Option<SampleValue>>),
    /// A `DDS::Time_t`.
    Time {
        /// Whole seconds.
        sec: i64,
        /// Nanoseconds within the second.
        nanosec: u32,
    },
}

/// How deep a sample walk descends before it refuses.
///
/// [`FieldEncoding::Composite`] is a leaf of one plan's type tree: the
/// walk leaves that plan and picks the nested one out of the
/// [`PlanSet`], so `MAX_TYPE_DEPTH` in [`super::plan`] — which bounds
/// the alias, sequence, array and map nesting *inside* a plan — says
/// nothing about how far the composite chain runs. A recursive IDL type
/// (`A` holding a `sequence<B>` holding a `sequence<A>`) has a plan
/// graph with a cycle in it, and the only thing that stops the walk is
/// the sample running out of nesting.
///
/// That is a termination argument and not a bound, so this is the
/// bound: a sample nested past it is refused by name through
/// [`EncodeError::TooDeep`] rather than by a stack overflow. 64 is what
/// the decoders that build such a sample already stop at —
/// `zerodds-wire-codec`'s `MAX_DEPTH` and `zerodds-types`' — so no
/// sample a decoder in this tree produces can reach it.
const MAX_SAMPLE_DEPTH: u32 = 64;

/// The encoding refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// The sample's shape does not match the plan's — a `Str` where the
    /// plan says `Boolean`, or a composite with the wrong field count.
    /// Carries the field path.
    Mismatch(String),
    /// §8.1.2 — "Floating-point NaN and infinities are not
    /// representable in JSON and SHALL cause the encoder to raise an
    /// error."
    NotRepresentableInJson(String),
    /// §7.1.4.1 — an IDL `char` outside the UTF-8 ASCII subset.
    CharOutsideAscii(u32),
    /// An enumeration ordinal with no literal at that position.
    UnknownEnumerator(i64),
    /// §7.2.2.4 — a nested composite whose own plan is not in the
    /// [`PlanSet`], so its descriptor cannot be written.
    NoPlanFor(String),
    /// The sample nests deeper than `MAX_SAMPLE_DEPTH`, which a
    /// recursive IDL type's plan graph admits and no decoder in this
    /// tree produces. Carries the field path the walk stopped at.
    TooDeep(String),
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Mismatch(path) => write!(f, "the sample does not match the plan at `{path}`"),
            Self::NotRepresentableInJson(path) => write!(
                f,
                "DDS-AMQP 1.0 §8.1.2: `{path}` is NaN or an infinity, which JSON cannot write"
            ),
            Self::CharOutsideAscii(code) => write!(
                f,
                "DDS-AMQP 1.0 §7.1.4.1: the IDL char U+{code:04X} is outside the ASCII subset"
            ),
            Self::UnknownEnumerator(value) => {
                write!(f, "no enumerator has the ordinal {value}")
            }
            Self::NoPlanFor(name) => write!(
                f,
                "DDS-AMQP 1.0 §7.2.2.4: the nested composite `{name}` has no plan in the set"
            ),
            Self::TooDeep(path) => write!(
                f,
                "the sample nests past {MAX_SAMPLE_DEPTH} composites at `{path}`"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EncodeError {}

/// What §7.2.2.2's optional trailing-null trimming does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EncodeOptions {
    /// §7.2.2.2 — "A run of trailing absent optional fields MAY be
    /// encoded by shortening the list."
    ///
    /// `false` by default: the long form is valid for every decoder and
    /// the short one only for a decoder that implements the clause's
    /// second sentence. A sender that knows its peer turns it on.
    pub trim_trailing_absent: bool,
}

/// §8.1.3 — the sample as the described composite of §7.2.
///
/// # Errors
/// [`EncodeError`] where the sample's shape does not match the plan's,
/// or where a value is outside what §7.1 admits.
pub fn described_composite(
    plans: &PlanSet,
    plan: &TypePlan,
    sample: &[Option<SampleValue>],
    options: EncodeOptions,
) -> Result<AmqpExtValue, EncodeError> {
    described_composite_at(plans, plan, sample, options, 0)
}

/// [`described_composite`], carrying how many composites the sample has
/// already nested through.
///
/// zerodds-lint: recursion-depth 64 (composite nesting; bounded by
/// `MAX_SAMPLE_DEPTH`, which returns `EncodeError::TooDeep`)
fn described_composite_at(
    plans: &PlanSet,
    plan: &TypePlan,
    sample: &[Option<SampleValue>],
    options: EncodeOptions,
    depth: u32,
) -> Result<AmqpExtValue, EncodeError> {
    if depth > MAX_SAMPLE_DEPTH {
        return Err(EncodeError::TooDeep(plan.scoped_name.clone()));
    }
    let body = body_of(plans, plan, sample, options, depth)?;
    Ok(match &plan.descriptor {
        Descriptor::Ulong(code) => AmqpExtValue::Described {
            descriptor: *code,
            value: Box::new(body),
        },
        // `AmqpExtValue::Described` carries a `ulong` descriptor, which
        // is §7.2.1.1's form. §7.2.1.2's is a `symbol`, and it is
        // written as the two-element list AMQP §1.3.4 defines a
        // described type to be: the descriptor and then the value.
        // Encoding it as a `Described` with a hashed `ulong` would be
        // the other form under this form's name.
        Descriptor::Symbol(symbol) => {
            AmqpExtValue::List(alloc::vec![AmqpExtValue::Symbol(symbol.clone()), body,])
        }
    })
}

fn body_of(
    plans: &PlanSet,
    plan: &TypePlan,
    sample: &[Option<SampleValue>],
    options: EncodeOptions,
    depth: u32,
) -> Result<AmqpExtValue, EncodeError> {
    match plan.body {
        BodyForm::List => {
            let mut items = Vec::new();
            for (field, value) in plan.fields.iter().zip(sample.iter()) {
                items.push(match value {
                    // §7.2.2.1 — an absent optional is the AMQP `null`
                    // at its positional slot and is never omitted.
                    None => AmqpExtValue::Null,
                    Some(value) => {
                        value_of(plans, &field.encoding, value, &field.name, options, depth)?
                    }
                });
            }
            if sample.len() != plan.fields.len() {
                return Err(EncodeError::Mismatch(plan.scoped_name.clone()));
            }
            if options.trim_trailing_absent {
                while matches!(items.last(), Some(AmqpExtValue::Null)) {
                    items.pop();
                }
            }
            Ok(AmqpExtValue::List(items))
        }
        BodyForm::Map => {
            if sample.len() != plan.fields.len() {
                return Err(EncodeError::Mismatch(plan.scoped_name.clone()));
            }
            let mut entries = Vec::new();
            for (field, value) in plan.fields.iter().zip(sample.iter()) {
                // §7.2.2.5's map is keyed by the declared member IDs.
                // An absent optional has no entry: a map has no
                // positional slot to leave a `null` in.
                let Some(value) = value else {
                    continue;
                };
                entries.push((
                    AmqpExtValue::Uint(field.id),
                    value_of(plans, &field.encoding, value, &field.name, options, depth)?,
                ));
            }
            Ok(AmqpExtValue::Map(entries))
        }
        BodyForm::Union => union_body(plans, plan, sample, options, depth),
    }
}

/// §7.2.3 — "a 2-element list: the discriminator value and the active
/// branch's value (or `null` if the branch is empty)".
///
/// The sample carries one entry per plan field: the discriminator, then
/// the branches, of which at most one is `Some`. That is what "the
/// active branch" is, and a sample with two live branches is a sample
/// the union cannot hold.
fn union_body(
    plans: &PlanSet,
    plan: &TypePlan,
    sample: &[Option<SampleValue>],
    options: EncodeOptions,
    depth: u32,
) -> Result<AmqpExtValue, EncodeError> {
    if sample.len() != plan.fields.len() || plan.fields.is_empty() {
        return Err(EncodeError::Mismatch(plan.scoped_name.clone()));
    }
    let discriminator = sample
        .first()
        .and_then(Option::as_ref)
        .ok_or_else(|| EncodeError::Mismatch(format!("{}::discriminator", plan.scoped_name)))?;
    let head = value_of(
        plans,
        &plan.fields[0].encoding,
        discriminator,
        "discriminator",
        options,
        depth,
    )?;
    let mut branch = AmqpExtValue::Null;
    let mut live = 0_usize;
    for (field, value) in plan.fields.iter().zip(sample.iter()).skip(1) {
        if let Some(value) = value {
            branch = value_of(plans, &field.encoding, value, &field.name, options, depth)?;
            live += 1;
        }
    }
    if live > 1 {
        return Err(EncodeError::Mismatch(format!(
            "{}: {live} branches are live at once",
            plan.scoped_name
        )));
    }
    Ok(AmqpExtValue::List(alloc::vec![head, branch]))
}

/// §7.1 — one field's value under its encoding.
///
/// zerodds-lint: recursion-depth 64 (collection and composite nesting;
/// bounded by `MAX_SAMPLE_DEPTH`, which returns `EncodeError::TooDeep`)
fn value_of(
    plans: &PlanSet,
    encoding: &FieldEncoding,
    value: &SampleValue,
    path: &str,
    options: EncodeOptions,
    depth: u32,
) -> Result<AmqpExtValue, EncodeError> {
    if depth > MAX_SAMPLE_DEPTH {
        return Err(EncodeError::TooDeep(path.to_string()));
    }
    let mismatch = || EncodeError::Mismatch(path.to_string());
    Ok(match (encoding, value) {
        (FieldEncoding::Boolean, SampleValue::Bool(v)) => AmqpExtValue::Boolean(*v),
        (FieldEncoding::Ubyte, SampleValue::Uint(v)) => {
            AmqpExtValue::Ubyte(u8::try_from(*v).map_err(|_| mismatch())?)
        }
        (FieldEncoding::Byte, SampleValue::Int(v)) => {
            AmqpExtValue::Byte(i8::try_from(*v).map_err(|_| mismatch())?)
        }
        (FieldEncoding::Ushort, SampleValue::Uint(v)) => {
            AmqpExtValue::Ushort(u16::try_from(*v).map_err(|_| mismatch())?)
        }
        (FieldEncoding::Short, SampleValue::Int(v)) => {
            AmqpExtValue::Short(i16::try_from(*v).map_err(|_| mismatch())?)
        }
        (FieldEncoding::Uint, SampleValue::Uint(v)) => {
            AmqpExtValue::Uint(u32::try_from(*v).map_err(|_| mismatch())?)
        }
        (FieldEncoding::Int, SampleValue::Int(v)) => {
            AmqpExtValue::Int(i32::try_from(*v).map_err(|_| mismatch())?)
        }
        (FieldEncoding::Ulong, SampleValue::Uint(v)) => AmqpExtValue::Ulong(*v),
        (FieldEncoding::Long, SampleValue::Int(v)) => AmqpExtValue::Long(*v),
        // §7.1's table gives `float` the 4-octet code, so the value is
        // narrowed here and not carried as a `double` with the right
        // code in front of it.
        #[allow(clippy::cast_possible_truncation)]
        (FieldEncoding::Float, SampleValue::Float(v)) => AmqpExtValue::Float(*v as f32),
        // §7.1.2 — `long double` narrows to binary64, which is the same
        // arm as `double` because [`SampleValue::Float`] is already an
        // `f64`. The narrowing happened where the sample was built.
        (FieldEncoding::Double | FieldEncoding::LongDouble, SampleValue::Float(v)) => {
            AmqpExtValue::Double(*v)
        }
        (FieldEncoding::Char, SampleValue::Char(c)) => {
            // §7.1.4.1 — the IDL `char` is the ASCII subset.
            if !c.is_ascii() {
                return Err(EncodeError::CharOutsideAscii(u32::from(*c)));
            }
            AmqpExtValue::Char(*c)
        }
        (FieldEncoding::WChar, SampleValue::Char(c)) => AmqpExtValue::Char(*c),
        (FieldEncoding::Str | FieldEncoding::WStr, SampleValue::Str(s)) => {
            AmqpExtValue::Str(s.clone())
        }
        (FieldEncoding::Binary, SampleValue::Bytes(bytes)) => AmqpExtValue::Binary(bytes.clone()),
        (FieldEncoding::Enumeration { literals, .. }, SampleValue::Int(ordinal)) => {
            AmqpExtValue::Symbol(enumerator_name(literals, *ordinal)?)
        }
        (FieldEncoding::Sequence(inner) | FieldEncoding::Array(inner), SampleValue::Seq(items)) => {
            let mut out = Vec::new();
            for (index, item) in items.iter().enumerate() {
                out.push(value_of(
                    plans,
                    inner,
                    item,
                    &format!("{path}[{index}]"),
                    options,
                    depth + 1,
                )?);
            }
            AmqpExtValue::List(out)
        }
        (FieldEncoding::Map(key, value), SampleValue::Map(entries)) => {
            let mut out = Vec::new();
            for (index, (k, v)) in entries.iter().enumerate() {
                out.push((
                    value_of(
                        plans,
                        key,
                        k,
                        &format!("{path}.key[{index}]"),
                        options,
                        depth + 1,
                    )?,
                    value_of(
                        plans,
                        value,
                        v,
                        &format!("{path}[{index}]"),
                        options,
                        depth + 1,
                    )?,
                ));
            }
            AmqpExtValue::Map(out)
        }
        // §7.1.3 — the standard mapping: milliseconds since the Unix
        // epoch. The sub-millisecond remainder is the endpoint's
        // `dds:nsec` application property and is not in the body.
        (FieldEncoding::Time, SampleValue::Time { sec, nanosec }) => AmqpExtValue::Timestamp(
            sec.saturating_mul(1000)
                .saturating_add(i64::from(*nanosec / 1_000_000)),
        ),
        // §7.2.2.4 — a nested composite is a described composite of its
        // own, descriptor included, which is why the encoder needs the
        // whole [`PlanSet`] and not one plan.
        (FieldEncoding::Composite(name), SampleValue::Composite(fields)) => {
            let nested = plans
                .get(name)
                .ok_or_else(|| EncodeError::NoPlanFor(name.clone()))?;
            described_composite_at(plans, nested, fields, options, depth + 1)?
        }
        _ => return Err(mismatch()),
    })
}

/// The enumerator at an ordinal position.
fn enumerator_name(literals: &[String], ordinal: i64) -> Result<String, EncodeError> {
    usize::try_from(ordinal)
        .ok()
        .and_then(|index| literals.get(index))
        .cloned()
        .ok_or(EncodeError::UnknownEnumerator(ordinal))
}

// ---------------------------------------------------------------------
// §8.1.2 — JSON.
// ---------------------------------------------------------------------

/// §8.1.2 — the sample as a UTF-8 JSON document.
///
/// # Errors
/// [`EncodeError`], including
/// [`EncodeError::NotRepresentableInJson`] for a NaN or an infinity,
/// which the clause requires the encoder to refuse rather than write.
pub fn json_document(
    plans: &PlanSet,
    plan: &TypePlan,
    sample: &[Option<SampleValue>],
) -> Result<String, EncodeError> {
    let mut out = String::new();
    json_body(plans, plan, sample, &mut out, 0)?;
    Ok(out)
}

/// zerodds-lint: recursion-depth 64 (composite nesting, via
/// `json_value`; bounded by `MAX_SAMPLE_DEPTH`, which returns
/// `EncodeError::TooDeep`)
fn json_body(
    plans: &PlanSet,
    plan: &TypePlan,
    sample: &[Option<SampleValue>],
    out: &mut String,
    depth: u32,
) -> Result<(), EncodeError> {
    if depth > MAX_SAMPLE_DEPTH {
        return Err(EncodeError::TooDeep(plan.scoped_name.clone()));
    }
    if sample.len() != plan.fields.len() {
        return Err(EncodeError::Mismatch(plan.scoped_name.clone()));
    }
    match plan.body {
        // §8.1.2 — "union: JSON object with two keys: discriminator
        // and value. The value key SHALL be omitted for empty
        // branches."
        BodyForm::Union => {
            out.push_str("{\"discriminator\":");
            let discriminator = sample
                .first()
                .and_then(Option::as_ref)
                .ok_or_else(|| EncodeError::Mismatch(plan.scoped_name.clone()))?;
            json_value(
                plans,
                &plan.fields[0].encoding,
                discriminator,
                "discriminator",
                out,
                depth,
            )?;
            for (field, value) in plan.fields.iter().zip(sample.iter()).skip(1) {
                if let Some(value) = value {
                    out.push_str(",\"value\":");
                    json_value(plans, &field.encoding, value, &field.name, out, depth)?;
                    break;
                }
            }
            out.push('}');
        }
        // §8.1.2 — "struct: JSON object whose keys are the IDL field
        // names. Optional fields with absent values SHOULD be omitted
        // (rather than encoded as null)." The `@mutable` map form is a
        // §7.2 body decision and does not reach JSON: the object's keys
        // are the field names in both.
        BodyForm::List | BodyForm::Map => {
            out.push('{');
            let mut first = true;
            for (field, value) in plan.fields.iter().zip(sample.iter()) {
                let Some(value) = value else {
                    continue;
                };
                if !first {
                    out.push(',');
                }
                first = false;
                json_string(&field.name, out);
                out.push(':');
                json_value(plans, &field.encoding, value, &field.name, out, depth)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// zerodds-lint: recursion-depth 64 (collection and composite nesting;
/// bounded by `MAX_SAMPLE_DEPTH`, which returns `EncodeError::TooDeep`)
fn json_value(
    plans: &PlanSet,
    encoding: &FieldEncoding,
    value: &SampleValue,
    path: &str,
    out: &mut String,
    depth: u32,
) -> Result<(), EncodeError> {
    if depth > MAX_SAMPLE_DEPTH {
        return Err(EncodeError::TooDeep(path.to_string()));
    }
    let mismatch = || EncodeError::Mismatch(path.to_string());
    match (encoding, value) {
        (FieldEncoding::Boolean, SampleValue::Bool(v)) => {
            out.push_str(if *v { "true" } else { "false" });
        }
        (
            FieldEncoding::Byte | FieldEncoding::Short | FieldEncoding::Int | FieldEncoding::Long,
            SampleValue::Int(v),
        ) => out.push_str(&v.to_string()),
        (
            FieldEncoding::Ubyte
            | FieldEncoding::Ushort
            | FieldEncoding::Uint
            | FieldEncoding::Ulong,
            SampleValue::Uint(v),
        ) => out.push_str(&v.to_string()),
        (
            FieldEncoding::Float | FieldEncoding::Double | FieldEncoding::LongDouble,
            SampleValue::Float(v),
        ) => {
            if !v.is_finite() {
                return Err(EncodeError::NotRepresentableInJson(path.to_string()));
            }
            out.push_str(&json_number(*v));
        }
        (FieldEncoding::Char | FieldEncoding::WChar, SampleValue::Char(c)) => {
            if matches!(encoding, FieldEncoding::Char) && !c.is_ascii() {
                return Err(EncodeError::CharOutsideAscii(u32::from(*c)));
            }
            let mut buffer = [0_u8; 4];
            json_string(c.encode_utf8(&mut buffer), out);
        }
        (FieldEncoding::Str | FieldEncoding::WStr, SampleValue::Str(s)) => json_string(s, out),
        // §8.1.2 — "octet sequences: Base64 (RFC 4648) string."
        (FieldEncoding::Binary, SampleValue::Bytes(bytes)) => {
            json_string(&base64(bytes), out);
        }
        // §8.1.2 — "enum: JSON string of the symbolic name."
        (FieldEncoding::Enumeration { literals, .. }, SampleValue::Int(ordinal)) => {
            json_string(&enumerator_name(literals, *ordinal)?, out);
        }
        (FieldEncoding::Sequence(inner) | FieldEncoding::Array(inner), SampleValue::Seq(items)) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                json_value(
                    plans,
                    inner,
                    item,
                    &format!("{path}[{index}]"),
                    out,
                    depth + 1,
                )?;
            }
            out.push(']');
        }
        // §8.1.2 — "map: JSON object." A JSON key is a string, so a
        // non-string key is written as its own JSON scalar inside the
        // quotes rather than dropped.
        (FieldEncoding::Map(key, value), SampleValue::Map(entries)) => {
            out.push('{');
            for (index, (k, v)) in entries.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                let mut rendered = String::new();
                json_value(
                    plans,
                    key,
                    k,
                    &format!("{path}.key[{index}]"),
                    &mut rendered,
                    depth + 1,
                )?;
                json_string(rendered.trim_matches('"'), out);
                out.push(':');
                json_value(plans, value, v, &format!("{path}[{index}]"), out, depth + 1)?;
            }
            out.push('}');
        }
        // §7.2.2.4 through §8.1.2 — a nested composite is its own JSON
        // object, written by its own plan.
        (FieldEncoding::Composite(name), SampleValue::Composite(fields)) => {
            let nested = plans
                .get(name)
                .ok_or_else(|| EncodeError::NoPlanFor(name.clone()))?;
            json_body(plans, nested, fields, out, depth + 1)?;
        }
        // §8.1.2 — "Time_t: JSON object {\"sec\": n, \"nanosec\": m}."
        (FieldEncoding::Time, SampleValue::Time { sec, nanosec }) => {
            out.push_str("{\"sec\":");
            out.push_str(&sec.to_string());
            out.push_str(",\"nanosec\":");
            out.push_str(&nanosec.to_string());
            out.push('}');
        }
        _ => return Err(mismatch()),
    }
    Ok(())
}

/// A JSON number that round-trips: RFC 8259 has no `NaN`, no `Infinity`
/// and no trailing `.` — and Rust's `{}` writes `1` for `1.0_f64`,
/// which is a valid JSON number and a different JSON type to a reader
/// that distinguishes them.
fn json_number(value: f64) -> String {
    let written = format!("{value}");
    if written.contains(['.', 'e', 'E']) {
        return written;
    }
    format!("{written}.0")
}

/// RFC 8259 §7 — the escapes a JSON string must carry.
fn json_string(value: &str, out: &mut String) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            // "All Unicode characters may be placed within the quotation
            // marks, except for the characters that MUST be escaped:
            // quotation mark, reverse solidus, and the control
            // characters (U+0000 through U+001F)."
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// RFC 4648 §4 — base64 with padding.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map_or(0, u32::from);
        let b2 = chunk.get(2).copied().map_or(0, u32::from);
        let triple = (b0 << 16) | (b1 << 8) | b2;
        let indices = [
            (triple >> 18) & 0x3F,
            (triple >> 12) & 0x3F,
            (triple >> 6) & 0x3F,
            triple & 0x3F,
        ];
        for (position, index) in indices.iter().enumerate() {
            if position > chunk.len() {
                out.push('=');
            } else {
                out.push(char::from(ALPHABET[*index as usize]));
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::codegen::plan::FieldPlan;

    fn field(name: &str, encoding: FieldEncoding) -> FieldPlan {
        FieldPlan {
            name: name.to_string(),
            id: 0,
            optional: false,
            encoding,
        }
    }

    fn make_plan(body: BodyForm, fields: Vec<FieldPlan>) -> TypePlan {
        TypePlan {
            scoped_name: "probe::T".to_string(),
            descriptor: Descriptor::Ulong(0x0102_0304_0506_0708),
            body,
            fields,
        }
    }

    #[test]
    fn base64_matches_rfc_4648_test_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn an_octet_sequence_is_base64_in_json_and_binary_on_the_wire() {
        let plan = make_plan(
            BodyForm::List,
            alloc::vec![field("blob", FieldEncoding::Binary)],
        );
        let sample = alloc::vec![Some(SampleValue::Bytes(alloc::vec![
            0xDE, 0xAD, 0xBE, 0xEF
        ]))];
        assert_eq!(
            json_document(&PlanSet::default(), &plan, &sample).expect("json"),
            "{\"blob\":\"3q2+7w==\"}"
        );
        let value = described_composite(
            &PlanSet::default(),
            &plan,
            &sample,
            EncodeOptions::default(),
        )
        .expect("amqp");
        let AmqpExtValue::Described { value, .. } = value else {
            panic!("a described composite");
        };
        assert_eq!(
            *value,
            AmqpExtValue::List(alloc::vec![AmqpExtValue::Binary(alloc::vec![
                0xDE, 0xAD, 0xBE, 0xEF
            ])])
        );
    }

    #[test]
    fn an_enumeration_is_its_symbolic_name_in_both_modes() {
        let encoding = FieldEncoding::Enumeration {
            name: "probe::Colour".to_string(),
            literals: alloc::vec!["RED".to_string(), "GREEN".to_string()],
        };
        let plan = make_plan(BodyForm::List, alloc::vec![field("c", encoding)]);
        let sample = alloc::vec![Some(SampleValue::Int(1))];
        assert_eq!(
            json_document(&PlanSet::default(), &plan, &sample).expect("json"),
            "{\"c\":\"GREEN\"}"
        );
        let value = described_composite(
            &PlanSet::default(),
            &plan,
            &sample,
            EncodeOptions::default(),
        )
        .expect("amqp");
        let AmqpExtValue::Described { value, .. } = value else {
            panic!("a described composite");
        };
        assert_eq!(
            *value,
            AmqpExtValue::List(alloc::vec![AmqpExtValue::Symbol("GREEN".to_string())])
        );
    }

    #[test]
    fn an_ordinal_with_no_literal_is_refused() {
        let encoding = FieldEncoding::Enumeration {
            name: "probe::Colour".to_string(),
            literals: alloc::vec!["RED".to_string()],
        };
        let plan = make_plan(BodyForm::List, alloc::vec![field("c", encoding)]);
        let sample = alloc::vec![Some(SampleValue::Int(7))];
        assert_eq!(
            json_document(&PlanSet::default(), &plan, &sample),
            Err(EncodeError::UnknownEnumerator(7))
        );
    }

    #[test]
    fn an_absent_optional_is_null_in_the_list_and_absent_from_the_object() {
        let mut plan = make_plan(
            BodyForm::List,
            alloc::vec![
                field("a", FieldEncoding::Int),
                field("b", FieldEncoding::Int),
                field("c", FieldEncoding::Int),
            ],
        );
        for f in &mut plan.fields {
            f.optional = true;
        }
        let sample = alloc::vec![Some(SampleValue::Int(1)), None, Some(SampleValue::Int(3)),];
        // §8.1.2: absent optionals are omitted from the object.
        assert_eq!(
            json_document(&PlanSet::default(), &plan, &sample).expect("json"),
            "{\"a\":1,\"c\":3}"
        );
        // §7.2.2.1: and are `null` at their slot in the list.
        let value = described_composite(
            &PlanSet::default(),
            &plan,
            &sample,
            EncodeOptions::default(),
        )
        .expect("amqp");
        let AmqpExtValue::Described { value, .. } = value else {
            panic!("a described composite");
        };
        assert_eq!(
            *value,
            AmqpExtValue::List(alloc::vec![
                AmqpExtValue::Int(1),
                AmqpExtValue::Null,
                AmqpExtValue::Int(3),
            ])
        );
    }

    #[test]
    fn trailing_absent_fields_are_kept_by_default_and_trimmed_on_request() {
        let plan = make_plan(
            BodyForm::List,
            alloc::vec![
                field("a", FieldEncoding::Int),
                field("b", FieldEncoding::Int),
                field("c", FieldEncoding::Int),
            ],
        );
        let sample = alloc::vec![Some(SampleValue::Int(1)), None, None];
        let long = described_composite(
            &PlanSet::default(),
            &plan,
            &sample,
            EncodeOptions::default(),
        )
        .expect("long");
        let short = described_composite(
            &PlanSet::default(),
            &plan,
            &sample,
            EncodeOptions {
                trim_trailing_absent: true,
            },
        )
        .expect("short");
        let AmqpExtValue::Described { value: long, .. } = long else {
            panic!("a described composite");
        };
        let AmqpExtValue::Described { value: short, .. } = short else {
            panic!("a described composite");
        };
        assert_eq!(
            *long,
            AmqpExtValue::List(alloc::vec![
                AmqpExtValue::Int(1),
                AmqpExtValue::Null,
                AmqpExtValue::Null,
            ])
        );
        assert_eq!(
            *short,
            AmqpExtValue::List(alloc::vec![AmqpExtValue::Int(1)])
        );
    }

    #[test]
    fn a_mutable_body_is_a_map_keyed_by_member_id() {
        let mut plan = make_plan(
            BodyForm::Map,
            alloc::vec![
                field("a", FieldEncoding::Int),
                field("b", FieldEncoding::Int),
            ],
        );
        plan.fields[0].id = 10;
        plan.fields[1].id = 20;
        let sample = alloc::vec![Some(SampleValue::Int(1)), Some(SampleValue::Int(2))];
        let value = described_composite(
            &PlanSet::default(),
            &plan,
            &sample,
            EncodeOptions::default(),
        )
        .expect("amqp");
        let AmqpExtValue::Described { value, .. } = value else {
            panic!("a described composite");
        };
        assert_eq!(
            *value,
            AmqpExtValue::Map(alloc::vec![
                (AmqpExtValue::Uint(10), AmqpExtValue::Int(1)),
                (AmqpExtValue::Uint(20), AmqpExtValue::Int(2)),
            ])
        );
        // The JSON object's keys are the field names in both body forms.
        assert_eq!(
            json_document(&PlanSet::default(), &plan, &sample).expect("json"),
            "{\"a\":1,\"b\":2}"
        );
    }

    #[test]
    fn a_union_is_a_two_element_list_and_a_two_key_object() {
        let plan = make_plan(
            BodyForm::Union,
            alloc::vec![
                field("discriminator", FieldEncoding::Int),
                field("n", FieldEncoding::Int),
                field("s", FieldEncoding::Str),
            ],
        );
        let sample = alloc::vec![
            Some(SampleValue::Int(2)),
            None,
            Some(SampleValue::Str("hello".to_string())),
        ];
        let value = described_composite(
            &PlanSet::default(),
            &plan,
            &sample,
            EncodeOptions::default(),
        )
        .expect("amqp");
        let AmqpExtValue::Described { value, .. } = value else {
            panic!("a described composite");
        };
        assert_eq!(
            *value,
            AmqpExtValue::List(alloc::vec![
                AmqpExtValue::Int(2),
                AmqpExtValue::Str("hello".to_string()),
            ])
        );
        assert_eq!(
            json_document(&PlanSet::default(), &plan, &sample).expect("json"),
            "{\"discriminator\":2,\"value\":\"hello\"}"
        );
    }

    #[test]
    fn an_empty_union_branch_is_null_and_the_json_value_key_is_absent() {
        let plan = make_plan(
            BodyForm::Union,
            alloc::vec![
                field("discriminator", FieldEncoding::Int),
                field("n", FieldEncoding::Int),
            ],
        );
        let sample = alloc::vec![Some(SampleValue::Int(9)), None];
        let value = described_composite(
            &PlanSet::default(),
            &plan,
            &sample,
            EncodeOptions::default(),
        )
        .expect("amqp");
        let AmqpExtValue::Described { value, .. } = value else {
            panic!("a described composite");
        };
        assert_eq!(
            *value,
            AmqpExtValue::List(alloc::vec![AmqpExtValue::Int(9), AmqpExtValue::Null])
        );
        assert_eq!(
            json_document(&PlanSet::default(), &plan, &sample).expect("json"),
            "{\"discriminator\":9}"
        );
    }

    #[test]
    fn a_nan_and_an_infinity_are_refused_rather_than_written() {
        let plan = make_plan(
            BodyForm::List,
            alloc::vec![field("x", FieldEncoding::Double)],
        );
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                json_document(
                    &PlanSet::default(),
                    &plan,
                    &alloc::vec![Some(SampleValue::Float(value))]
                ),
                Err(EncodeError::NotRepresentableInJson("x".to_string()))
            );
        }
        // §8.1.3 has no such restriction: AMQP `double` carries them.
        assert!(
            described_composite(
                &PlanSet::default(),
                &plan,
                &alloc::vec![Some(SampleValue::Float(f64::NAN))],
                EncodeOptions::default()
            )
            .is_ok()
        );
    }

    #[test]
    fn an_idl_char_outside_ascii_is_refused() {
        let plan = make_plan(BodyForm::List, alloc::vec![field("c", FieldEncoding::Char)]);
        let sample = alloc::vec![Some(SampleValue::Char('ä'))];
        assert_eq!(
            described_composite(
                &PlanSet::default(),
                &plan,
                &sample,
                EncodeOptions::default()
            ),
            Err(EncodeError::CharOutsideAscii(0xE4))
        );
        // A `wchar` takes the same code point without complaint.
        let wide = make_plan(
            BodyForm::List,
            alloc::vec![field("c", FieldEncoding::WChar)],
        );
        assert!(
            described_composite(
                &PlanSet::default(),
                &wide,
                &sample,
                EncodeOptions::default()
            )
            .is_ok()
        );
    }

    #[test]
    fn a_time_is_milliseconds_on_the_wire_and_two_keys_in_json() {
        let plan = make_plan(BodyForm::List, alloc::vec![field("t", FieldEncoding::Time)]);
        let sample = alloc::vec![Some(SampleValue::Time {
            sec: 3,
            nanosec: 500_000_000,
        })];
        let value = described_composite(
            &PlanSet::default(),
            &plan,
            &sample,
            EncodeOptions::default(),
        )
        .expect("amqp");
        let AmqpExtValue::Described { value, .. } = value else {
            panic!("a described composite");
        };
        assert_eq!(
            *value,
            AmqpExtValue::List(alloc::vec![AmqpExtValue::Timestamp(3_500)])
        );
        assert_eq!(
            json_document(&PlanSet::default(), &plan, &sample).expect("json"),
            "{\"t\":{\"sec\":3,\"nanosec\":500000000}}"
        );
    }

    #[test]
    fn the_json_escapes_are_rfc_8259s() {
        let mut out = String::new();
        json_string("a\"b\\c\nd\te\u{1}f", &mut out);
        assert_eq!(out, "\"a\\\"b\\\\c\\nd\\te\\u0001f\"");
    }

    #[test]
    fn the_full_descriptor_form_is_a_symbol_and_not_a_hashed_ulong() {
        let mut plan = make_plan(BodyForm::List, alloc::vec![field("a", FieldEncoding::Int)]);
        plan.descriptor = Descriptor::full(&[0xDE, 0xAD, 0xBE, 0xEF]);
        let sample = alloc::vec![Some(SampleValue::Int(1))];
        let value = described_composite(
            &PlanSet::default(),
            &plan,
            &sample,
            EncodeOptions::default(),
        )
        .expect("amqp");
        assert_eq!(
            value,
            AmqpExtValue::List(alloc::vec![
                AmqpExtValue::Symbol("dds:type:deadbeef".to_string()),
                AmqpExtValue::List(alloc::vec![AmqpExtValue::Int(1)]),
            ])
        );
    }

    #[test]
    fn the_described_composite_encodes_to_octets_the_codec_reads_back() {
        let plan = make_plan(
            BodyForm::List,
            alloc::vec![
                field("n", FieldEncoding::Int),
                field("s", FieldEncoding::Str),
            ],
        );
        let sample = alloc::vec![
            Some(SampleValue::Int(-7)),
            Some(SampleValue::Str("hello".to_string())),
        ];
        let value = described_composite(
            &PlanSet::default(),
            &plan,
            &sample,
            EncodeOptions::default(),
        )
        .expect("amqp");
        let octets = value.encode().expect("encode");
        let (back, consumed) = AmqpExtValue::decode(&octets).expect("decode");
        assert_eq!(consumed, octets.len());
        assert_eq!(back, value);
    }
}

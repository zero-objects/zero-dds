// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The key holder of a sample read from its body (XTypes 1.3 §7.6.8): the
//! key members are read and written into the holder, every other member
//! is stepped over by its plan, and no value of the type is built. What a
//! DataReader admits a sample by when nothing asks for the value.

use alloc::vec::Vec;

use crate::ParameterRole;
use crate::error::WireError;
use crate::reader::Reader;
use crate::writer::Writer;

use super::desc::Primitive;
use super::plan::{Header, PlanSet, ValuePlan};

/// How deep the read goes into a value: structs, sequences and arrays
/// inside each other, as `crate::MAX_VALUE_DEPTH` bounds the decoders.
const MAX_DEPTH: usize = 64;

/// The plans a read walks: the data plans of the body's encapsulation and
/// the key-holder plans (those of the XCDR2 set).
#[derive(Clone, Copy)]
struct Plans<'p> {
    data: &'p PlanSet,
    holders: &'p PlanSet,
}

/// The key holder of the sample whose body `reader` stands at (XTypes 1.3
/// §7.6.8), PLAIN_CDR2 into `holder`: of the struct `data_root` of the
/// data plans, the members the holder plan `holder_root` of `holders`
/// names are read and written in the holder's order; every other member
/// is stepped over. `Ok(false)` where a plan has a shape this read does
/// not take — a final union on the way, an `@optional` key member, a key
/// member the writer left out, a PL_CDR1 parameter list, a sequence count
/// the rest of the stream cannot hold — and the caller reads the whole
/// sample instead; `holder` then holds a part of a holder.
///
/// What is stepped over is not checked beyond its framing; a full decode
/// of the sample still refuses what it refuses.
///
/// # Errors
/// What the stream refuses on the way: a truncated value, a sequence over
/// its bound, a must-understand member the type does not declare, a value
/// nested deeper than 64.
pub fn key_holder_from_body(
    reader: &mut Reader<'_>,
    data: &PlanSet,
    data_root: usize,
    holders: &PlanSet,
    holder_root: usize,
    holder: &mut Writer,
) -> Result<bool, WireError> {
    struct_key(
        reader,
        holder,
        Plans { data, holders },
        data_root,
        holder_root,
        0,
    )
}

fn no_plan() -> WireError {
    WireError::refused(0, "a layout plan for every member the type declares")
}

fn too_deep(reader: &Reader<'_>) -> WireError {
    WireError::refused(reader.position(), "a value nested at most 64 deep")
}

/// The key members of one struct, into the holder in the holder's order:
/// the struct's members are walked once in the stream's order, the reader
/// as it stands at each one's value is kept — a PL_CDR1 parameter sets its
/// own alignment origin — and each key member is then read from there.
///
/// zerodds-lint: recursion-depth 64 — the `depth` argument, through
/// `value_key`; past it the read refuses.
fn struct_key<'a>(
    reader: &mut Reader<'a>,
    holder: &mut Writer,
    plans: Plans<'_>,
    data_index: usize,
    holder_index: usize,
    depth: usize,
) -> Result<bool, WireError> {
    if depth > MAX_DEPTH {
        return Err(too_deep(reader));
    }
    let plan = plans.data.structs.get(data_index).ok_or_else(no_plan)?;
    let holder_plan = plans
        .holders
        .holders
        .get(holder_index)
        .ok_or_else(no_plan)?;
    if plan.needs_sentinel {
        return Ok(false);
    }
    let mut starts: Vec<Option<Reader<'a>>> = alloc::vec![None; plan.members.len()];
    let end = if plan.needs_dheader {
        let length = reader.read_dheader()?;
        Some(reader.position() + length)
    } else {
        None
    };
    if plan.by_member_id {
        // Rule (21): the members of the DHEADER's extent, each behind its
        // EMHEADER, in the writer's order.
        let Some(end) = end else {
            return Ok(false);
        };
        while reader.position() < end {
            let header = reader.read_emheader()?;
            let ends = reader.position() + header.extent;
            let index = plan
                .members
                .iter()
                .position(|frame| frame.member_id == header.member_id);
            if let Some(slot) = index.and_then(|index| starts.get_mut(index)) {
                *slot = Some(reader.clone());
            } else if header.must_understand {
                return Err(WireError::refused(
                    reader.position(),
                    "a must-understand member this type does not declare, per §7.2.2.4.4.4.6",
                ));
            }
            reader.skip_to(ends)?;
        }
    } else {
        for (slot, frame) in starts.iter_mut().zip(&plan.members) {
            // An `@appendable` writer may stop before the last member.
            if end.is_some_and(|end| reader.position() >= end) {
                break;
            }
            match frame.header {
                Header::None => {}
                Header::OptionalFlag => {
                    if !reader.read_bool()? {
                        continue;
                    }
                }
                // Rule (19): an `@optional` member under PL_CDR1, absent
                // where its parameter is empty.
                Header::Pid {
                    role: ParameterRole::Optional,
                    ..
                } => {
                    let pid = reader.read_pid_as(ParameterRole::Optional)?;
                    if pid.length != 0 {
                        *slot = Some(reader.clone());
                    }
                    reader.skip_parameter(pid)?;
                    continue;
                }
                Header::Pid { .. } | Header::EmHeader { .. } => return Ok(false),
            }
            *slot = Some(reader.clone());
            if !skip_value(reader, plans.data, &frame.value, depth + 1)? {
                return Ok(false);
            }
        }
    }
    if let Some(end) = end {
        reader.skip_to(end)?;
    }
    for (declared, frame) in &holder_plan.members {
        if frame.header != Header::None {
            return Ok(false);
        }
        // A key member the writer left out has no reader kept: the caller
        // reads the whole sample.
        let member = starts.get_mut(*declared).and_then(Option::take);
        let data_frame = plan.members.get(*declared);
        let (Some(mut member), Some(data_frame)) = (member, data_frame) else {
            return Ok(false);
        };
        if !value_key(
            &mut member,
            holder,
            plans,
            &data_frame.value,
            &frame.value,
            depth + 1,
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// One key member's value, read by its data plan and written by its
/// holder plan.
///
/// zerodds-lint: recursion-depth 64 — the `depth` argument, through
/// `struct_key`; past it the read refuses.
fn value_key(
    reader: &mut Reader<'_>,
    holder: &mut Writer,
    plans: Plans<'_>,
    data: &ValuePlan,
    held: &ValuePlan,
    depth: usize,
) -> Result<bool, WireError> {
    if depth > MAX_DEPTH {
        return Err(too_deep(reader));
    }
    match (data, held) {
        (ValuePlan::Primitive(kind), ValuePlan::Primitive(held_kind)) if kind == held_kind => {
            copy_primitive(reader, holder, *kind)?;
            Ok(true)
        }
        (
            ValuePlan::Enum {
                holder_bits: bits, ..
            },
            ValuePlan::Enum {
                holder_bits: held_bits,
                ..
            },
        ) if bits == held_bits => copy_bits(reader, holder, *bits),
        (ValuePlan::String { bound }, ValuePlan::String { .. }) => {
            let text = reader.read_str()?;
            match bound {
                Some(bound) => holder.write_string_bounded(text, *bound)?,
                None => holder.write_string(text),
            }
            Ok(true)
        }
        (
            ValuePlan::Sequence {
                element,
                bound,
                delimited,
            },
            ValuePlan::Sequence {
                element: held_element,
                delimited: held_delimited,
                ..
            },
        ) => {
            let end = delimited_end(reader, *delimited)?;
            let at = reader.position();
            let count = reader.read_u32()? as usize;
            if bound.is_some_and(|bound| count > bound) {
                return Err(WireError::refused(
                    at,
                    "a sequence no longer than its bound",
                ));
            }
            if count > reader.remaining() {
                return Ok(false);
            }
            let header = held_delimited.then(|| holder.write_dheader());
            holder.write_u32(u32::try_from(count).map_err(|_| no_plan())?);
            for _ in 0..count {
                if !value_key(reader, holder, plans, element, held_element, depth + 1)? {
                    return Ok(false);
                }
            }
            finish(reader, holder, end, header)
        }
        (
            ValuePlan::Array {
                element,
                dims,
                delimited,
            },
            ValuePlan::Array {
                element: held_element,
                delimited: held_delimited,
                ..
            },
        ) => {
            let Some(count) = dims.iter().try_fold(1usize, |n, d| n.checked_mul(*d)) else {
                return Ok(false);
            };
            let end = delimited_end(reader, *delimited)?;
            let header = held_delimited.then(|| holder.write_dheader());
            for _ in 0..count {
                if !value_key(reader, holder, plans, element, held_element, depth + 1)? {
                    return Ok(false);
                }
            }
            finish(reader, holder, end, header)
        }
        (ValuePlan::Struct(data_index), ValuePlan::Holder(holder_index)) => {
            struct_key(reader, holder, plans, *data_index, *holder_index, depth + 1)
        }
        _ => Ok(false),
    }
}

/// Where a DHEADER in front of a value says it ends.
fn delimited_end(reader: &mut Reader<'_>, delimited: bool) -> Result<Option<usize>, WireError> {
    if !delimited {
        return Ok(None);
    }
    let length = reader.read_dheader()?;
    Ok(Some(reader.position() + length))
}

/// Closes the holder's DHEADER and steps to the end of the stream's.
fn finish(
    reader: &mut Reader<'_>,
    holder: &mut Writer,
    end: Option<usize>,
    header: Option<crate::writer::DHeader>,
) -> Result<bool, WireError> {
    if let Some(header) = header {
        holder.try_finish_dheader(header)?;
    }
    if let Some(end) = end {
        reader.skip_to(end)?;
    }
    Ok(true)
}

/// A primitive from the stream into the holder; a float goes as its bits.
fn copy_primitive(
    reader: &mut Reader<'_>,
    holder: &mut Writer,
    kind: Primitive,
) -> Result<(), WireError> {
    match kind {
        Primitive::Boolean => holder.write_bool(reader.read_bool()?),
        Primitive::Octet | Primitive::Char | Primitive::Int8 | Primitive::UInt8 => {
            holder.write_u8(reader.read_u8()?);
        }
        Primitive::Int16 | Primitive::UInt16 => holder.write_u16(reader.read_u16()?),
        Primitive::Int32 | Primitive::UInt32 | Primitive::Float32 => {
            holder.write_u32(reader.read_u32()?);
        }
        Primitive::Int64 | Primitive::UInt64 | Primitive::Float64 => {
            holder.write_u64(reader.read_u64()?);
        }
    }
    Ok(())
}

/// An enumerator from the stream into the holder, as its holder's bits.
fn copy_bits(reader: &mut Reader<'_>, holder: &mut Writer, bits: u16) -> Result<bool, WireError> {
    match bits {
        8 => holder.write_u8(reader.read_u8()?),
        16 => holder.write_u16(reader.read_u16()?),
        32 => holder.write_u32(reader.read_u32()?),
        _ => return Ok(false),
    }
    Ok(true)
}

/// The width of a value every one of which takes the same octets.
fn fixed_width(plan: &ValuePlan) -> Option<usize> {
    match plan {
        ValuePlan::Primitive(kind) => Some(match kind {
            Primitive::Boolean
            | Primitive::Octet
            | Primitive::Char
            | Primitive::Int8
            | Primitive::UInt8 => 1,
            Primitive::Int16 | Primitive::UInt16 => 2,
            Primitive::Int32 | Primitive::UInt32 | Primitive::Float32 => 4,
            Primitive::Int64 | Primitive::UInt64 | Primitive::Float64 => 8,
        }),
        ValuePlan::Enum { holder_bits, .. } => Some(usize::from(*holder_bits / 8)),
        _ => None,
    }
}

/// Steps over a value by its plan. `Ok(false)` for a shape it does not
/// take: a final union, a PL_CDR1 parameter list.
///
/// zerodds-lint: recursion-depth 64 — the `depth` argument; past it the
/// read refuses.
fn skip_value(
    reader: &mut Reader<'_>,
    data: &PlanSet,
    plan: &ValuePlan,
    depth: usize,
) -> Result<bool, WireError> {
    if depth > MAX_DEPTH {
        return Err(too_deep(reader));
    }
    if let Some(width) = fixed_width(plan) {
        reader.skip_primitive(width)?;
        return Ok(true);
    }
    match plan {
        ValuePlan::String { .. } => reader.skip_string()?,
        ValuePlan::Sequence {
            element, delimited, ..
        } => {
            if *delimited {
                reader.skip_delimited()?;
            } else if let Some(width) = fixed_width(element) {
                reader.skip_sequence(width, false)?;
            } else {
                let count = reader.read_u32()? as usize;
                if count > reader.remaining() {
                    return Ok(false);
                }
                for _ in 0..count {
                    if !skip_value(reader, data, element, depth + 1)? {
                        return Ok(false);
                    }
                }
            }
        }
        ValuePlan::Array {
            element,
            dims,
            delimited,
        } => {
            let Some(count) = dims.iter().try_fold(1usize, |n, d| n.checked_mul(*d)) else {
                return Ok(false);
            };
            if *delimited {
                reader.skip_delimited()?;
            } else if let Some(width) = fixed_width(element) {
                reader.skip_array(count, width, false)?;
            } else {
                for _ in 0..count {
                    if !skip_value(reader, data, element, depth + 1)? {
                        return Ok(false);
                    }
                }
            }
        }
        ValuePlan::Struct(index) => {
            let plan = data.structs.get(*index).ok_or_else(no_plan)?;
            if plan.needs_dheader {
                reader.skip_delimited()?;
                return Ok(true);
            }
            if plan.needs_sentinel {
                return Ok(false);
            }
            for frame in &plan.members {
                match frame.header {
                    Header::None => {}
                    Header::OptionalFlag => {
                        if !reader.read_bool()? {
                            continue;
                        }
                    }
                    Header::Pid {
                        role: ParameterRole::Optional,
                        ..
                    } => {
                        let pid = reader.read_pid_as(ParameterRole::Optional)?;
                        reader.skip_parameter(pid)?;
                        continue;
                    }
                    Header::Pid { .. } | Header::EmHeader { .. } => return Ok(false),
                }
                if !skip_value(reader, data, &frame.value, depth + 1)? {
                    return Ok(false);
                }
            }
        }
        ValuePlan::Union(index) => {
            let plan = data.unions.get(*index).ok_or_else(no_plan)?;
            if !plan.needs_dheader {
                return Ok(false);
            }
            reader.skip_delimited()?;
        }
        // Stepped over above, by their width.
        ValuePlan::Primitive(_) | ValuePlan::Enum { .. } => {}
        // Only a key holder's plans name one.
        ValuePlan::Holder(_) => return Ok(false),
    }
    Ok(true)
}

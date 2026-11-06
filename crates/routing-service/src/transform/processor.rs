// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The dynamic sample processor: decode → filter → transform → encode.

use super::codec::{self, DynValue};
use super::shape::{Member, ScalarKind, TypeRegistry, TypeShape};
use super::sqlbridge::{CompiledFilter, DecodedRow};
use crate::config::Route;
use crate::error::RoutingError;
use crate::forwarding::{Processed, SampleProcessor};
use zerodds_wire::Endianness;

/// Where an output member's value comes from.
enum OutSlot {
    /// Copy input member at this index (kinds match).
    Copy(usize),
    /// A constant value.
    Const(DynValue),
}

/// Decodes each input sample, optionally drops it via a SQL filter, optionally
/// remaps it to an output shape, and re-encodes.
pub struct DynamicProcessor {
    in_shape: TypeShape,
    out_shape: TypeShape,
    filter: Option<CompiledFilter>,
    /// `None` = no transform (output == input shape); filter-only forwards the
    /// original bytes when it passes.
    plan: Option<Vec<OutSlot>>,
}

impl DynamicProcessor {
    /// Builds the processor for `route`, resolving the input and output shapes
    /// from `shapes`.
    ///
    /// # Errors
    /// [`RoutingError`] when a shape is missing, the filter fails to parse, or
    /// a transform rule references an unknown/incompatible member.
    pub fn build(
        route: &Route,
        in_type: &str,
        out_type: &str,
        shapes: &TypeRegistry,
    ) -> crate::error::Result<Self> {
        let in_shape = shapes
            .get(in_type)
            .ok_or_else(|| RoutingError::TypeShape {
                route: route.name.clone(),
                reason: format!("input type '{in_type}' not registered"),
            })?
            .clone();
        let out_shape = shapes
            .get(out_type)
            .ok_or_else(|| RoutingError::TypeShape {
                route: route.name.clone(),
                reason: format!("output type '{out_type}' not registered"),
            })?
            .clone();

        let filter = match &route.filter {
            Some(f) => Some(CompiledFilter::build(
                &route.name,
                &f.expression,
                &f.parameters,
            )?),
            None => None,
        };

        let plan = match &route.transform {
            None if in_shape == out_shape => None,
            // Different shapes but no explicit transform: an implicit by-name
            // copy still needs a plan (e.g. output reorders/subsets members).
            None => Some(build_plan(
                route,
                &in_shape,
                &out_shape,
                &Default::default(),
            )?),
            Some(t) => Some(build_plan(route, &in_shape, &out_shape, t)?),
        };

        Ok(Self {
            in_shape,
            out_shape,
            filter,
            plan,
        })
    }
}

/// How a body under `identifier` is laid out (DDSI-RTPS 2.5 §10.5,
/// DDS-XTypes 1.3 Table 60 and Table 39): XCDR2 or XCDR1, a DHEADER in
/// front of the members or not, the byte order. `None` for a parameter
/// list (`PL_CDR`, `PL_CDR2`: a `@mutable` type, whose members a flat shape
/// does not describe) and for an identifier that is no CDR format.
fn body_form(identifier: [u8; 2]) -> Option<(bool, bool, Endianness)> {
    if identifier[0] != 0x00 {
        return None;
    }
    let endianness = if identifier[1] & 0x01 == 0 {
        Endianness::Big
    } else {
        Endianness::Little
    };
    let (xcdr2, delimited) = match identifier[1] & !0x01 {
        0x00 => (false, false),
        0x06 | 0x10 => (true, false),
        0x08 | 0x14 => (true, true),
        _ => return None,
    };
    Some((xcdr2, delimited, endianness))
}

/// The identifier of an XCDR2 (else XCDR1) body of a shape that is
/// `appendable` or not, in `endianness` (DDS-XTypes 1.3 Table 60).
fn identifier_of(xcdr2: bool, appendable: bool, endianness: Endianness) -> [u8; 2] {
    let id = match (xcdr2, appendable) {
        (false, _) => 0x00,
        (true, false) => 0x06,
        (true, true) => 0x08,
    };
    [0x00, id | u8::from(endianness == Endianness::Little)]
}

impl SampleProcessor for DynamicProcessor {
    fn process(&mut self, payload: &[u8], representation_identifier: [u8; 2]) -> Processed {
        // Decode the input only when needed (filter or transform present).
        let need_decode = self.filter.is_some() || self.plan.is_some();
        if !need_decode {
            return Processed::Forward(payload.to_vec(), representation_identifier);
        }
        let Some((xcdr2, delimited, endianness)) = body_form(representation_identifier) else {
            return Processed::Refused(format!(
                "representation identifier {representation_identifier:02x?} is no plain or \
                 delimited CDR body"
            ));
        };
        // XCDR2 frames an @appendable body with a DHEADER and a @final one
        // without (DDS-XTypes 1.3 §7.4.3.4.1, §7.4.3.4.2): the shape has to
        // say the same as the header, or every member is read at the wrong
        // offset.
        if xcdr2 && delimited != self.in_shape.appendable {
            return Processed::Refused(format!(
                "representation identifier {representation_identifier:02x?} does not frame \
                 the body the input shape `{}` describes",
                self.in_shape.name
            ));
        }
        let in_values = match codec::decode(&self.in_shape, payload, xcdr2, endianness) {
            Ok(values) => values,
            Err(error) => return Processed::Refused(error.to_string()),
        };

        if let Some(f) = &self.filter {
            let row = DecodedRow::new(&self.in_shape, &in_values);
            if !f.matches(&row) {
                return Processed::Filtered;
            }
        }

        match &self.plan {
            None => {
                // Filter passed, no transform → forward original bytes verbatim.
                Processed::Forward(payload.to_vec(), representation_identifier)
            }
            Some(plan) => {
                let out_values: Vec<DynValue> = plan
                    .iter()
                    .map(|slot| match slot {
                        OutSlot::Copy(i) => in_values[*i].clone(),
                        OutSlot::Const(v) => v.clone(),
                    })
                    .collect();
                // The output is written in the version and the byte order
                // of the input, framed as the output shape is.
                match codec::encode(&self.out_shape, &out_values, xcdr2, endianness) {
                    Ok(bytes) => Processed::Forward(
                        bytes,
                        identifier_of(xcdr2, self.out_shape.appendable, endianness),
                    ),
                    Err(error) => Processed::Refused(error.to_string()),
                }
            }
        }
    }
}

fn build_plan(
    route: &Route,
    in_shape: &TypeShape,
    out_shape: &TypeShape,
    transform: &crate::config::Transform,
) -> crate::error::Result<Vec<OutSlot>> {
    let err = |reason: String| RoutingError::Transform {
        route: route.name.clone(),
        reason,
    };

    // drop: each dropped member must exist in the input and be absent from the
    // output (the output shape simply omits it).
    for d in &transform.drop {
        if !in_shape.has(d) {
            return Err(err(format!("drop: input has no member '{d}'")));
        }
        if out_shape.has(d) {
            return Err(err(format!(
                "drop: member '{d}' is still present in the output shape"
            )));
        }
    }

    let mut plan = Vec::with_capacity(out_shape.members.len());
    for out_m in &out_shape.members {
        // 1. constant assignment wins.
        if let Some(rule) = transform.set_const.iter().find(|r| r.field == out_m.name) {
            plan.push(OutSlot::Const(
                parse_const(out_m, &rule.value).map_err(err)?,
            ));
            continue;
        }
        // 2. explicit rename.
        if let Some(rule) = transform.rename.iter().find(|r| r.to == out_m.name) {
            let i = in_shape
                .index_of(&rule.from)
                .ok_or_else(|| err(format!("rename: input has no member '{}'", rule.from)))?;
            check_kind(out_m, &in_shape.members[i]).map_err(err)?;
            plan.push(OutSlot::Copy(i));
            continue;
        }
        // 3. implicit by-name copy.
        let i = in_shape.index_of(&out_m.name).ok_or_else(|| {
            err(format!(
                "output member '{}' has no source (no const, rename, or matching input member)",
                out_m.name
            ))
        })?;
        check_kind(out_m, &in_shape.members[i]).map_err(err)?;
        plan.push(OutSlot::Copy(i));
    }
    Ok(plan)
}

fn check_kind(out_m: &Member, in_m: &Member) -> core::result::Result<(), String> {
    if out_m.kind == in_m.kind {
        Ok(())
    } else {
        Err(format!(
            "member '{}': input kind {:?} != output kind {:?} (no implicit coercion)",
            out_m.name, in_m.kind, out_m.kind
        ))
    }
}

fn parse_const(m: &Member, lit: &str) -> core::result::Result<DynValue, String> {
    let bad = || {
        format!(
            "constant '{lit}' invalid for member '{}' ({:?})",
            m.name, m.kind
        )
    };
    let v = match m.kind {
        ScalarKind::Bool => DynValue::Bool(match lit {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => return Err(bad()),
        }),
        ScalarKind::U8 => DynValue::U8(lit.parse().map_err(|_| bad())?),
        ScalarKind::I8 => DynValue::I8(lit.parse().map_err(|_| bad())?),
        ScalarKind::U16 => DynValue::U16(lit.parse().map_err(|_| bad())?),
        ScalarKind::I16 => DynValue::I16(lit.parse().map_err(|_| bad())?),
        ScalarKind::U32 => DynValue::U32(lit.parse().map_err(|_| bad())?),
        ScalarKind::I32 => DynValue::I32(lit.parse().map_err(|_| bad())?),
        ScalarKind::U64 => DynValue::U64(lit.parse().map_err(|_| bad())?),
        ScalarKind::I64 => DynValue::I64(lit.parse().map_err(|_| bad())?),
        ScalarKind::F32 => DynValue::F32(lit.parse().map_err(|_| bad())?),
        ScalarKind::F64 => DynValue::F64(lit.parse().map_err(|_| bad())?),
        ScalarKind::String => {
            let s = lit
                .strip_prefix('\'')
                .and_then(|t| t.strip_suffix('\''))
                .unwrap_or(lit);
            DynValue::Str(s.to_string())
        }
    };
    Ok(v)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::{Endpoint, QosSpec, SetConstRule, Transform};
    use crate::transform::shape::Member;

    fn reg() -> TypeRegistry {
        let mut r = TypeRegistry::new();
        r.insert(TypeShape {
            name: "T".into(),
            members: vec![
                Member {
                    name: "id".into(),
                    kind: ScalarKind::U32,
                },
                Member {
                    name: "zone".into(),
                    kind: ScalarKind::String,
                },
            ],
            appendable: false,
        });
        r
    }

    fn route_with(filter: Option<&str>, transform: Option<Transform>) -> Route {
        let ep = |topic: &str| Endpoint {
            domain: 0,
            topic: topic.into(),
            type_name: "T".into(),
            keyed: false,
            partition: vec![],
            qos: QosSpec::default(),
            type_file: String::new(),
        };
        Route {
            name: "r".into(),
            input: ep("A"),
            output: ep("B"),
            filter: filter.map(|e| crate::config::ContentFilter {
                expression: e.into(),
                parameters: vec![],
            }),
            transform,
            loop_guard: true,
            preserve_source_timestamp: false,
            max_samples_per_second: None,
            tenant: None,
            relay: false,
        }
    }

    const CDR2_LE: [u8; 2] = [0x00, 0x07];

    /// A constant set by the transform replaces the member in the body the
    /// route writes. The oracle is the XCDR2 little-endian octets of
    /// `@final struct T { uint32 id; string zone; }` (DDS-XTypes 1.3
    /// §7.4.1.1.2: a string is its length with the terminating NUL, then the
    /// characters and the NUL; §7.4.2: no DHEADER for `@final`).
    #[test]
    fn xtypes_7_4_1_1_2_a_constant_set_by_the_transform_is_written_in_the_body() {
        let t = Transform {
            rename: vec![],
            set_const: vec![SetConstRule {
                field: "zone".into(),
                value: "'FIXED'".into(),
            }],
            drop: vec![],
        };
        let mut p = DynamicProcessor::build(&route_with(None, Some(t)), "T", "T", &reg()).unwrap();
        // id = 9, zone = "orig"
        let input = [0x09, 0, 0, 0, 0x05, 0, 0, 0, b'o', b'r', b'i', b'g', 0];
        // id = 9, zone = "FIXED"
        let output = [
            0x09, 0, 0, 0, 0x06, 0, 0, 0, b'F', b'I', b'X', b'E', b'D', 0,
        ];
        assert_eq!(
            p.process(&input, CDR2_LE),
            Processed::Forward(output.to_vec(), CDR2_LE)
        );
    }
}

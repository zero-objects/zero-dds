// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Recursive Type-Walker — Spec §9.2.4-§9.2.8.
//!
//! Consumes a [`DdsType`] (lightweight DDS-XTypes tree) and
//! emits OPC-UA node specs. Complete recursion: nested
//! structures, sequence-of-struct, Map<String, Sequence<Struct>>,
//! alias resolution etc. — the walker applies the mapping rules from
//! §9.2 to each type independently and recurses into the
//! members.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::naming::{data_type_name, variable_type_name};
use super::node_spec::{NodeSpec, ReferenceKind, TypeRef, ValueRank, VariableSpec};

/// DDS-XTypes type tree — lightweight model that the mapping walker
/// processes. Callers convert their own TypeObject representations
/// into this model (e.g. from `crates/types/`).
#[derive(Debug, Clone, PartialEq)]
pub enum DdsType {
    /// `boolean`. Spec Tab 9.2.
    Boolean,
    /// `int8`/`SByte`.
    Int8,
    /// `uint8`/`Byte`.
    UInt8,
    /// `int16`.
    Int16,
    /// `uint16`.
    UInt16,
    /// `int32`.
    Int32,
    /// `uint32`.
    UInt32,
    /// `int64`.
    Int64,
    /// `uint64`.
    UInt64,
    /// `float32`.
    Float32,
    /// `float64`.
    Float64,
    /// `char`.
    Char8,
    /// `wchar`.
    Char16,
    /// `string`.
    String8,
    /// `wstring`.
    String16,
    /// Enumeration with name + values.
    Enum(EnumDef),
    /// Bitmask with name + bitflags + bound.
    Bitmask(BitmaskDef),
    /// `typedef` alias (transparent — the walker resolves it).
    Alias {
        /// Alias name.
        name: String,
        /// Resolved element type.
        target: alloc::boxed::Box<DdsType>,
    },
    /// Struct (aggregated).
    Struct(StructDef),
    /// Union (aggregated).
    Union(UnionDef),
    /// Array — fixed-size N-dim.
    Array {
        /// Element type.
        element: alloc::boxed::Box<DdsType>,
        /// Per-dimension bound (outer-first).
        dimensions: Vec<u32>,
    },
    /// Sequence — bounded or unbounded.
    Sequence {
        /// Element type.
        element: alloc::boxed::Box<DdsType>,
        /// Max bound (`None` = unbounded).
        bound: Option<u32>,
    },
    /// Map<K, V>.
    Map {
        /// Key type.
        key: alloc::boxed::Box<DdsType>,
        /// Value type.
        value: alloc::boxed::Box<DdsType>,
        /// Max bound (`None` = unbounded).
        bound: Option<u32>,
    },
}

/// Enumeration description — Spec §9.2.3.1.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    /// IDL name.
    pub name: String,
    /// List `(name, value)` of the enumerators.
    pub literals: Vec<(String, i64)>,
}

/// Bitmask description — Spec §9.2.3.2.
#[derive(Debug, Clone, PartialEq)]
pub struct BitmaskDef {
    /// IDL name.
    pub name: String,
    /// Bound (bit count, max).
    pub bound: u32,
    /// List `(name, position)` of the bitflags. Positions without an entry
    /// are filled in the OPC-UA OptionSetValues with `UndefinedPosition_<N>`
    /// (Spec Tab 9.11).
    pub bits: Vec<(String, u32)>,
}

/// Struct description — Spec §9.2.4.1.
#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    /// IDL name.
    pub name: String,
    /// Members in source order.
    pub members: Vec<MemberDef>,
}

/// Union description — Spec §9.2.4.2.
#[derive(Debug, Clone, PartialEq)]
pub struct UnionDef {
    /// IDL name.
    pub name: String,
    /// Discriminator type — must be in
    /// `{Boolean, Byte, Char8, Char16, Int16, UInt16, Int32, UInt32,
    /// Int64, UInt64, Enum, Bitmask}` (Spec §9.2.4.2.1).
    pub discriminator: alloc::boxed::Box<DdsType>,
    /// Cases in source order. Spec §9.2.4.2.2: switch values
    /// are assigned consecutively 1..N.
    pub cases: Vec<UnionCase>,
}

/// Union case — Spec §9.2.4.2.2.
#[derive(Debug, Clone, PartialEq)]
pub struct UnionCase {
    /// Member name.
    pub name: String,
    /// Member type.
    pub member_type: DdsType,
    /// `true` if this is the `default` case.
    pub is_default: bool,
}

/// Member of a struct/union — Spec §9.2.4.1/§9.2.6.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberDef {
    /// Field name (= BrowseName, Spec §9.2.4.1).
    pub name: String,
    /// Element type (can itself be struct/sequence/...).
    pub member_type: DdsType,
    /// `@optional` — Spec Tab 9.16 ModelingRule "Optional" vs
    /// "Mandatory".
    pub is_optional: bool,
    /// Spec §9.2.8 — `@key` propagates to the variable as an IsKey property.
    pub is_key: bool,
}

impl MemberDef {
    /// Spec Tab 9.16 ModelingRule.
    #[must_use]
    pub fn modeling_rule(&self) -> &'static str {
        if self.is_optional {
            "Optional"
        } else {
            "Mandatory"
        }
    }
}

/// Member category — diagnostic aid for the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    /// Primitive/string/enum/bitmask (atomic).
    Atomic,
    /// Aggregated (Struct/Union).
    Aggregated,
    /// Collection (Array/Sequence/Map).
    Collection,
    /// Alias (transparent).
    Alias,
}

impl DdsType {
    /// Returns the member category of the type.
    #[must_use]
    pub fn member_kind(&self) -> MemberKind {
        match self {
            Self::Boolean
            | Self::Int8
            | Self::UInt8
            | Self::Int16
            | Self::UInt16
            | Self::Int32
            | Self::UInt32
            | Self::Int64
            | Self::UInt64
            | Self::Float32
            | Self::Float64
            | Self::Char8
            | Self::Char16
            | Self::String8
            | Self::String16
            | Self::Enum(_)
            | Self::Bitmask(_) => MemberKind::Atomic,
            Self::Struct(_) | Self::Union(_) => MemberKind::Aggregated,
            Self::Array { .. } | Self::Sequence { .. } | Self::Map { .. } => MemberKind::Collection,
            Self::Alias { .. } => MemberKind::Alias,
        }
    }

    /// Resolves the alias layer transparently — returns the underlying type.
    #[must_use]
    pub fn resolve_alias(&self) -> &Self {
        match self {
            Self::Alias { target, .. } => target.resolve_alias(),
            other => other,
        }
    }

    /// IDL type spec as a string for the DataType pointer (Spec Tab
    /// 9.1/9.2 — primitive mapping name).
    #[must_use]
    pub fn opc_ua_data_type(&self) -> String {
        match self.resolve_alias() {
            Self::Boolean => "Boolean".to_string(),
            Self::Int8 => "SByte".to_string(),
            Self::UInt8 => "Byte".to_string(),
            Self::Int16 => "Int16".to_string(),
            Self::UInt16 => "UInt16".to_string(),
            Self::Int32 => "Int32".to_string(),
            Self::UInt32 => "UInt32".to_string(),
            Self::Int64 => "Int64".to_string(),
            Self::UInt64 => "UInt64".to_string(),
            Self::Float32 => "Float".to_string(),
            Self::Float64 => "Double".to_string(),
            Self::Char8 => "Byte".to_string(),
            Self::Char16 => "UInt16".to_string(),
            Self::String8 => "String".to_string(),
            Self::String16 => "String".to_string(),
            Self::Enum(e) => data_type_name(&e.name),
            Self::Bitmask(b) => data_type_name(&b.name),
            Self::Struct(s) => data_type_name(&s.name),
            Self::Union(u) => data_type_name(&u.name),
            Self::Array { element, .. } | Self::Sequence { element, .. } => {
                element.opc_ua_data_type()
            }
            Self::Map { .. } => "MapEntry".to_string(),
            // resolve_alias removed the alias above — the fallback returns
            // a safe default instead of a clippy-disallowed unreachable!
            Self::Alias { .. } => "BaseDataType".to_string(),
        }
    }
}

/// Walker output — the list of emitted node specs + non-fatal
/// diagnostic notes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WalkOutput {
    /// Node specs in generation order.
    pub nodes: Vec<NodeSpec>,
    /// Diagnostic: list of the top-level type names that were generated
    /// via the walker (for caller-side validation).
    pub generated_types: Vec<String>,
}

/// Walker error — spec conformance cannot be unambiguously established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalkError {
    /// Spec §9.2.4.2.1 normative: the discriminator must be in the spec list
    /// (Boolean/Byte/Char8/Char16/Int16/UInt16/Int32/UInt32/
    /// Int64/UInt64/Enum/Bitmask). Other types are unsupported.
    InvalidUnionDiscriminator(String),
    /// Spec §9.2.4.2.2 normative: unions with > 2^32-1 cases are
    /// "unsupported by this specification".
    UnionTooManyCases(usize),
    /// Spec §9.2.5: a map bound > 2^32-1 is not representable.
    MapBoundOverflow,
}

/// Main function — emits the OPC-UA node specs for the given
/// DDS type. Recursive for nested types.
///
/// # Errors
/// See [`WalkError`].
pub fn walk_dds_type(ty: &DdsType) -> Result<WalkOutput, WalkError> {
    let mut out = WalkOutput::default();
    walk(ty, &mut out)?;
    Ok(out)
}

/// zerodds-lint: recursion-depth 64
///
/// Bound: DDS-XTypes type trees are very flat in practice
/// (typically <10 levels). 64 covers even pathological config files
/// with nested maps/sequences/structs; deeper is a
/// caller bug, not a valid spec type.
fn walk(ty: &DdsType, out: &mut WalkOutput) -> Result<(), WalkError> {
    match ty {
        DdsType::Alias { target, .. } => walk(target, out),
        DdsType::Enum(e) => emit_enum(e, out),
        DdsType::Bitmask(b) => emit_bitmask(b, out),
        DdsType::Struct(s) => emit_struct(s, out),
        DdsType::Union(u) => emit_union(u, out),
        DdsType::Array { element, .. } | DdsType::Sequence { element, .. } => walk(element, out),
        DdsType::Map { key, value, bound } => {
            if let Some(b) = bound {
                // u32 bound is always in u32 range — overflow-free.
                let _ = *b;
            }
            walk(key, out)?;
            walk(value, out)?;
            Ok(())
        }
        // Primitive / strings — no own DataTypes (they use
        // OPC-UA builtin types directly, Spec Tab 9.2).
        DdsType::Boolean
        | DdsType::Int8
        | DdsType::UInt8
        | DdsType::Int16
        | DdsType::UInt16
        | DdsType::Int32
        | DdsType::UInt32
        | DdsType::Int64
        | DdsType::UInt64
        | DdsType::Float32
        | DdsType::Float64
        | DdsType::Char8
        | DdsType::Char16
        | DdsType::String8
        | DdsType::String16 => Ok(()),
    }
}

fn emit_enum(e: &EnumDef, out: &mut WalkOutput) -> Result<(), WalkError> {
    let dt_name = data_type_name(&e.name);
    let mut node = NodeSpec::data_type(dt_name.clone(), TypeRef::new("Enumeration"));
    node = node.with_ref(ReferenceKind::HasProperty, TypeRef::new("EnumValues"));
    out.nodes.push(node);
    out.generated_types.push(dt_name);
    Ok(())
}

fn emit_bitmask(b: &BitmaskDef, out: &mut WalkOutput) -> Result<(), WalkError> {
    let dt_name = data_type_name(&b.name);
    let mut node = NodeSpec::data_type(dt_name.clone(), TypeRef::new("OptionSet"));
    node = node.with_ref(ReferenceKind::HasProperty, TypeRef::new("OptionSetValues"));
    out.nodes.push(node);
    out.generated_types.push(dt_name);
    Ok(())
}

/// zerodds-lint: recursion-depth 64
///
/// Indirectly recursive via `walk` (a member type can be struct/sequence/map).
/// Same bound as `walk` (see there).
fn emit_struct(s: &StructDef, out: &mut WalkOutput) -> Result<(), WalkError> {
    // Spec §9.2.4.1.2 — DataType (subtype of Structure).
    let dt_name = data_type_name(&s.name);
    let vt_name = variable_type_name(&s.name);

    let mut dt_node = NodeSpec::data_type(dt_name.clone(), TypeRef::new("Structure"));

    // VariableType with HasComponent references per member (Tab 9.16).
    let mut vt_node = NodeSpec::variable_type(vt_name.clone(), TypeRef::new(dt_name.clone()));
    for m in &s.members {
        // Member variable (in the VariableType) — BrowseName = member name.
        let var_spec = build_member_variable_spec(&m.member_type);
        let _member_var = NodeSpec::variable(m.name.clone(), var_spec.clone());
        // The VariableType links via HasComponent to the member variable.
        vt_node = vt_node.with_ref(ReferenceKind::HasComponent, TypeRef::new(m.name.clone()));

        // The DataType itself collects the members as fields via Tab 9.15
        // also via HasComponent (to BaseDataVariableType).
        dt_node = dt_node.with_ref(ReferenceKind::HasComponent, TypeRef::new(m.name.clone()));

        // Recursion: the member type can itself be a struct/sequence/...
        walk(&m.member_type, out)?;
    }

    out.nodes.push(dt_node);
    out.nodes.push(vt_node);
    out.generated_types.push(dt_name);
    out.generated_types.push(vt_name);
    Ok(())
}

/// zerodds-lint: recursion-depth 64
///
/// Indirectly recursive via `walk` (a case member type can be struct/sequence/map).
/// Same bound as `walk` (see there).
fn emit_union(u: &UnionDef, out: &mut WalkOutput) -> Result<(), WalkError> {
    // Spec §9.2.4.2.1: the discriminator must be allowed.
    if !is_valid_union_discriminator(&u.discriminator) {
        return Err(WalkError::InvalidUnionDiscriminator(
            u.discriminator.opc_ua_data_type(),
        ));
    }
    // Spec §9.2.4.2.2: maximal 2^32-1 Cases.
    if u.cases.len() >= u32::MAX as usize {
        return Err(WalkError::UnionTooManyCases(u.cases.len()));
    }

    let dt_name = data_type_name(&u.name);
    let mut node = NodeSpec::data_type(dt_name.clone(), TypeRef::new("Union"));
    // SwitchField as the first member with consecutive 1..N switch values.
    node = node.with_ref(ReferenceKind::HasComponent, TypeRef::new("SwitchField"));

    for c in &u.cases {
        node = node.with_ref(ReferenceKind::HasComponent, TypeRef::new(c.name.clone()));
        // Recurse in Member-Type.
        walk(&c.member_type, out)?;
    }
    out.nodes.push(node);
    out.generated_types.push(dt_name);
    Ok(())
}

fn is_valid_union_discriminator(t: &DdsType) -> bool {
    matches!(
        t.resolve_alias(),
        DdsType::Boolean
            | DdsType::UInt8
            | DdsType::Char8
            | DdsType::Char16
            | DdsType::Int16
            | DdsType::UInt16
            | DdsType::Int32
            | DdsType::UInt32
            | DdsType::Int64
            | DdsType::UInt64
            | DdsType::Enum(_)
            | DdsType::Bitmask(_)
    )
}

fn build_member_variable_spec(t: &DdsType) -> VariableSpec {
    match t.resolve_alias() {
        DdsType::Array {
            element,
            dimensions,
        } => VariableSpec {
            data_type: TypeRef::new(element.opc_ua_data_type()),
            value_rank: ValueRank(i32::try_from(dimensions.len()).unwrap_or(1).max(1)),
            array_dimensions: dimensions.clone(),
            type_definition: TypeRef::new("BaseDataVariableType"),
        },
        DdsType::Sequence { element, bound } => VariableSpec {
            data_type: TypeRef::new(element.opc_ua_data_type()),
            value_rank: ValueRank(1),
            // Spec §9.2.5.2.x: a sequence without an explicit bound gets
            // ArrayDimensions = [0] (= "unbounded"); with a bound = [N].
            array_dimensions: alloc::vec![bound.unwrap_or(0)],
            type_definition: TypeRef::new("BaseDataVariableType"),
        },
        scalar => VariableSpec {
            data_type: TypeRef::new(scalar.opc_ua_data_type()),
            value_rank: ValueRank::SCALAR,
            array_dimensions: Vec::new(),
            type_definition: TypeRef::new("BaseDataVariableType"),
        },
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn shape_struct() -> StructDef {
        StructDef {
            name: "ShapeType".into(),
            members: alloc::vec![
                MemberDef {
                    name: "color".into(),
                    member_type: DdsType::String8,
                    is_optional: false,
                    is_key: false,
                },
                MemberDef {
                    name: "x".into(),
                    member_type: DdsType::Int32,
                    is_optional: false,
                    is_key: false,
                },
                MemberDef {
                    name: "y".into(),
                    member_type: DdsType::Int32,
                    is_optional: false,
                    is_key: false,
                },
                MemberDef {
                    name: "shapesize".into(),
                    member_type: DdsType::Int32,
                    is_optional: false,
                    is_key: false,
                },
            ],
        }
    }

    #[test]
    fn struct_emits_data_type_and_variable_type() {
        let out = walk_dds_type(&DdsType::Struct(shape_struct())).unwrap();
        assert!(out.generated_types.iter().any(|s| s == "ShapeTypeDataType"));
        assert!(
            out.generated_types
                .iter()
                .any(|s| s == "ShapeTypeVariableType")
        );
    }

    #[test]
    fn struct_dt_subtypes_structure() {
        let out = walk_dds_type(&DdsType::Struct(shape_struct())).unwrap();
        let dt = out
            .nodes
            .iter()
            .find(|n| n.browse_name == "ShapeTypeDataType")
            .unwrap();
        assert_eq!(dt.subtype_of, Some(TypeRef::new("Structure")));
    }

    #[test]
    fn nested_struct_recurses() {
        // outer struct with one inner-struct member.
        let inner = StructDef {
            name: "Inner".into(),
            members: alloc::vec![MemberDef {
                name: "v".into(),
                member_type: DdsType::Int32,
                is_optional: false,
                is_key: false,
            }],
        };
        let outer = StructDef {
            name: "Outer".into(),
            members: alloc::vec![MemberDef {
                name: "inner".into(),
                member_type: DdsType::Struct(inner),
                is_optional: false,
                is_key: false,
            }],
        };
        let out = walk_dds_type(&DdsType::Struct(outer)).unwrap();
        assert!(out.generated_types.iter().any(|s| s == "OuterDataType"));
        assert!(out.generated_types.iter().any(|s| s == "InnerDataType"));
    }

    #[test]
    fn enum_emits_enumeration_subtype() {
        let e = EnumDef {
            name: "Color".into(),
            literals: alloc::vec![("RED".into(), 0), ("GREEN".into(), 1)],
        };
        let out = walk_dds_type(&DdsType::Enum(e)).unwrap();
        let dt = out
            .nodes
            .iter()
            .find(|n| n.browse_name == "ColorDataType")
            .unwrap();
        assert_eq!(dt.subtype_of, Some(TypeRef::new("Enumeration")));
        assert!(
            dt.references
                .iter()
                .any(|r| r.kind == ReferenceKind::HasProperty && r.target.0 == "EnumValues")
        );
    }

    #[test]
    fn bitmask_emits_optionset_subtype() {
        let b = BitmaskDef {
            name: "Permissions".into(),
            bound: 3,
            bits: alloc::vec![("READ".into(), 0), ("WRITE".into(), 1)],
        };
        let out = walk_dds_type(&DdsType::Bitmask(b)).unwrap();
        let dt = out
            .nodes
            .iter()
            .find(|n| n.browse_name == "PermissionsDataType")
            .unwrap();
        assert_eq!(dt.subtype_of, Some(TypeRef::new("OptionSet")));
    }

    #[test]
    fn union_emits_union_subtype_with_switchfield() {
        let u = UnionDef {
            name: "ElementValue".into(),
            discriminator: alloc::boxed::Box::new(DdsType::Int32),
            cases: alloc::vec![
                UnionCase {
                    name: "int16_value".into(),
                    member_type: DdsType::Int16,
                    is_default: false,
                },
                UnionCase {
                    name: "int64_value".into(),
                    member_type: DdsType::Int64,
                    is_default: true,
                },
            ],
        };
        let out = walk_dds_type(&DdsType::Union(u)).unwrap();
        let dt = out
            .nodes
            .iter()
            .find(|n| n.browse_name == "ElementValueDataType")
            .unwrap();
        assert_eq!(dt.subtype_of, Some(TypeRef::new("Union")));
        assert!(dt.references.iter().any(|r| r.target.0 == "SwitchField"));
    }

    #[test]
    fn union_with_invalid_discriminator_is_rejected() {
        let u = UnionDef {
            name: "Bad".into(),
            // Float is not an allowed discriminator (Spec §9.2.4.2.1).
            discriminator: alloc::boxed::Box::new(DdsType::Float32),
            cases: alloc::vec![],
        };
        let err = walk_dds_type(&DdsType::Union(u)).unwrap_err();
        assert!(matches!(err, WalkError::InvalidUnionDiscriminator(_)));
    }

    #[test]
    fn array_of_struct_recurses_into_struct() {
        // Spec §9.2.5.1.2.3 — Array of Structure: emittiert
        // <StructName>DataType + <StructName>VariableType.
        let array = DdsType::Array {
            element: alloc::boxed::Box::new(DdsType::Struct(shape_struct())),
            dimensions: alloc::vec![3],
        };
        let out = walk_dds_type(&array).unwrap();
        assert!(out.generated_types.iter().any(|s| s == "ShapeTypeDataType"));
    }

    #[test]
    fn sequence_of_struct_recurses() {
        let seq = DdsType::Sequence {
            element: alloc::boxed::Box::new(DdsType::Struct(shape_struct())),
            bound: Some(10),
        };
        let out = walk_dds_type(&seq).unwrap();
        assert!(out.generated_types.iter().any(|s| s == "ShapeTypeDataType"));
    }

    #[test]
    fn map_recurses_into_key_and_value() {
        let map = DdsType::Map {
            key: alloc::boxed::Box::new(DdsType::String8),
            value: alloc::boxed::Box::new(DdsType::Struct(shape_struct())),
            bound: None,
        };
        let out = walk_dds_type(&map).unwrap();
        assert!(out.generated_types.iter().any(|s| s == "ShapeTypeDataType"));
    }

    #[test]
    fn alias_is_transparent() {
        let aliased = DdsType::Alias {
            name: "MyShape".into(),
            target: alloc::boxed::Box::new(DdsType::Struct(shape_struct())),
        };
        let out = walk_dds_type(&aliased).unwrap();
        // The alias itself emits no node, but the target struct does.
        assert!(out.generated_types.iter().any(|s| s == "ShapeTypeDataType"));
    }

    #[test]
    fn member_modeling_rule_reflects_optional() {
        let m = MemberDef {
            name: "x".into(),
            member_type: DdsType::Int32,
            is_optional: true,
            is_key: false,
        };
        assert_eq!(m.modeling_rule(), "Optional");
        let m2 = MemberDef {
            is_optional: false,
            ..m
        };
        assert_eq!(m2.modeling_rule(), "Mandatory");
    }

    #[test]
    fn primitive_dds_to_opc_ua_names_match_spec_tab_92() {
        // Spec Tab 9.2: SByte/Byte/Int16/etc.
        assert_eq!(DdsType::Boolean.opc_ua_data_type(), "Boolean");
        assert_eq!(DdsType::Int8.opc_ua_data_type(), "SByte");
        assert_eq!(DdsType::UInt8.opc_ua_data_type(), "Byte");
        assert_eq!(DdsType::Int32.opc_ua_data_type(), "Int32");
        assert_eq!(DdsType::Float64.opc_ua_data_type(), "Double");
        assert_eq!(DdsType::String8.opc_ua_data_type(), "String");
    }

    #[test]
    fn alias_resolve_unwraps_recursively() {
        let nested = DdsType::Alias {
            name: "Outer".into(),
            target: alloc::boxed::Box::new(DdsType::Alias {
                name: "Inner".into(),
                target: alloc::boxed::Box::new(DdsType::Int32),
            }),
        };
        assert_eq!(nested.resolve_alias(), &DdsType::Int32);
    }
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Recursive Type-Walker — Spec §9.2.4-§9.2.8.
//!
//! Konsumiert eine [`DdsType`] (lightweight DDS-XTypes-Tree) und
//! emittiert OPC-UA-Node-Specs. Vollstaendige Rekursion: nested
//! Structures, Sequence-of-Struct, Map<String, Sequence<Struct>>,
//! Alias-Aufloesung etc. — der Walker wendet die Mapping-Regeln aus
//! §9.2 fuer jeden Typ unabhaengig an und springt rekursiv in die
//! Member.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::naming::{data_type_name, variable_type_name};
use super::node_spec::{NodeSpec, ReferenceKind, TypeRef, ValueRank, VariableSpec};

/// DDS-XTypes-Type-Tree — Lightweight-Modell, das das Mapping-Walker
/// verarbeitet. Caller konvertieren ihre eigenen TypeObject-Repraesen-
/// tationen in dieses Modell (z.B. aus `crates/types/`).
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
    /// Enumeration mit Name + Werten.
    Enum(EnumDef),
    /// Bitmask mit Name + Bitflags + bound.
    Bitmask(BitmaskDef),
    /// `typedef`-Alias (transparent — Walker resolved den).
    Alias {
        /// Alias-Name.
        name: String,
        /// Aufgeloester Element-Type.
        target: alloc::boxed::Box<DdsType>,
    },
    /// Struct (Aggregated).
    Struct(StructDef),
    /// Union (Aggregated).
    Union(UnionDef),
    /// Array — fixed-size N-dim.
    Array {
        /// Element-Type.
        element: alloc::boxed::Box<DdsType>,
        /// Pro-Dim-Bound (Outer-First).
        dimensions: Vec<u32>,
    },
    /// Sequence — bounded oder unbounded.
    Sequence {
        /// Element-Type.
        element: alloc::boxed::Box<DdsType>,
        /// Max-Bound (`None` = unbounded).
        bound: Option<u32>,
    },
    /// Map<K, V>.
    Map {
        /// Key-Type.
        key: alloc::boxed::Box<DdsType>,
        /// Value-Type.
        value: alloc::boxed::Box<DdsType>,
        /// Max-Bound (`None` = unbounded).
        bound: Option<u32>,
    },
}

/// Enumeration-Beschreibung — Spec §9.2.3.1.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    /// IDL-Name.
    pub name: String,
    /// Liste `(Name, Value)` der Enumeratoren.
    pub literals: Vec<(String, i64)>,
}

/// Bitmask-Beschreibung — Spec §9.2.3.2.
#[derive(Debug, Clone, PartialEq)]
pub struct BitmaskDef {
    /// IDL-Name.
    pub name: String,
    /// Bound (Bit-Anzahl, max).
    pub bound: u32,
    /// Liste `(Name, Position)` der Bitflags. Positionen ohne Eintrag
    /// werden im OPC-UA-OptionSetValues mit `UndefinedPosition_<N>`
    /// aufgefuellt (Spec Tab 9.11).
    pub bits: Vec<(String, u32)>,
}

/// Struct-Beschreibung — Spec §9.2.4.1.
#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    /// IDL-Name.
    pub name: String,
    /// Members in Source-Reihenfolge.
    pub members: Vec<MemberDef>,
}

/// Union-Beschreibung — Spec §9.2.4.2.
#[derive(Debug, Clone, PartialEq)]
pub struct UnionDef {
    /// IDL-Name.
    pub name: String,
    /// Discriminator-Type — muss in
    /// `{Boolean, Byte, Char8, Char16, Int16, UInt16, Int32, UInt32,
    /// Int64, UInt64, Enum, Bitmask}` sein (Spec §9.2.4.2.1).
    pub discriminator: alloc::boxed::Box<DdsType>,
    /// Cases in Source-Reihenfolge. Spec §9.2.4.2.2: Switch-Values
    /// werden 1..N konsekutiv vergeben.
    pub cases: Vec<UnionCase>,
}

/// Union-Case — Spec §9.2.4.2.2.
#[derive(Debug, Clone, PartialEq)]
pub struct UnionCase {
    /// Member-Name.
    pub name: String,
    /// Member-Type.
    pub member_type: DdsType,
    /// `true` wenn das der `default`-Case ist.
    pub is_default: bool,
}

/// Member eines Struct/Union — Spec §9.2.4.1/§9.2.6.
#[derive(Debug, Clone, PartialEq)]
pub struct MemberDef {
    /// Field-Name (= BrowseName, Spec §9.2.4.1).
    pub name: String,
    /// Element-Type (kann selbst Struct/Sequence/...).
    pub member_type: DdsType,
    /// `@optional` — Spec Tab 9.16 ModelingRule "Optional" vs
    /// "Mandatory".
    pub is_optional: bool,
    /// Spec §9.2.8 — `@key` propagiert auf Variable als IsKey-Property.
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

/// Member-Kategorie — Diagnose-Hilfe fuer Caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    /// Primitiv/String/Enum/Bitmask (atomar).
    Atomic,
    /// Aggregated (Struct/Union).
    Aggregated,
    /// Collection (Array/Sequence/Map).
    Collection,
    /// Alias (transparent).
    Alias,
}

impl DdsType {
    /// Liefert die Member-Kategorie des Typs.
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

    /// Resolved Alias-Layer transparent — gibt den Underlying-Type zurueck.
    #[must_use]
    pub fn resolve_alias(&self) -> &Self {
        match self {
            Self::Alias { target, .. } => target.resolve_alias(),
            other => other,
        }
    }

    /// IDL-Type-Spec als String fuer den DataType-Pointer (Spec Tab
    /// 9.1/9.2 — primitiver Mapping-Name).
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
            // resolve_alias removed Alias above — fallback liefert
            // einen sicheren Default statt clippy-disallowed unreachable!
            Self::Alias { .. } => "BaseDataType".to_string(),
        }
    }
}

/// Walker-Output — die Liste emittierter Node-Specs + nicht-fatale
/// Diagnose-Notizen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WalkOutput {
    /// Node-Specs in Generierungs-Reihenfolge.
    pub nodes: Vec<NodeSpec>,
    /// Diagnose: Liste der Top-Level-Type-Namen, die ueber den Walker
    /// generiert wurden (fuer Caller-Side-Validierung).
    pub generated_types: Vec<String>,
}

/// Walker-Fehler — Spec-Konformitaet ist nicht eindeutig herstellbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalkError {
    /// Spec §9.2.4.2.1 normativ: Discriminator muss in der Spec-Liste
    /// sein (Boolean/Byte/Char8/Char16/Int16/UInt16/Int32/UInt32/
    /// Int64/UInt64/Enum/Bitmask). Andere Types sind unsupported.
    InvalidUnionDiscriminator(String),
    /// Spec §9.2.4.2.2 normativ: Unions mit > 2^32-1 Cases sind
    /// "unsupported by this specification".
    UnionTooManyCases(usize),
    /// Spec §9.2.5: Map-Bound > 2^32-1 ist nicht darstellbar.
    MapBoundOverflow,
}

/// Hauptfunktion — emittiert die OPC-UA-Node-Specs fuer den uebergebenen
/// DDS-Type. Rekursiv fuer Nested-Types.
///
/// # Errors
/// Siehe [`WalkError`].
pub fn walk_dds_type(ty: &DdsType) -> Result<WalkOutput, WalkError> {
    let mut out = WalkOutput::default();
    walk(ty, &mut out)?;
    Ok(out)
}

/// zerodds-lint: recursion-depth 64
///
/// Begrenzung: DDS-XTypes-Type-Trees sind in der Praxis sehr flach
/// (typisch <10 Ebenen). 64 deckt selbst pathologische Konfig-Files
/// mit verschachtelten Maps/Sequences/Structs ab; tiefer ist ein
/// Caller-Bug, kein gueltiger Spec-Type.
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
                // u32-Bound ist immer in u32-Range — ueberlauf-frei.
                let _ = *b;
            }
            walk(key, out)?;
            walk(value, out)?;
            Ok(())
        }
        // Primitive / Strings — keine eigenen DataTypes (sie nutzen
        // OPC-UA-Builtin-Types direkt, Spec Tab 9.2).
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
/// Indirekt rekursiv via `walk` (Member-Type kann Struct/Sequence/Map).
/// Selbe Bound wie `walk` (siehe dort).
fn emit_struct(s: &StructDef, out: &mut WalkOutput) -> Result<(), WalkError> {
    // Spec §9.2.4.1.2 — DataType (Subtype von Structure).
    let dt_name = data_type_name(&s.name);
    let vt_name = variable_type_name(&s.name);

    let mut dt_node = NodeSpec::data_type(dt_name.clone(), TypeRef::new("Structure"));

    // VariableType mit HasComponent References pro Member (Tab 9.16).
    let mut vt_node = NodeSpec::variable_type(vt_name.clone(), TypeRef::new(dt_name.clone()));
    for m in &s.members {
        // Member-Variable (im VariableType) — BrowseName = Member-Name.
        let var_spec = build_member_variable_spec(&m.member_type);
        let _member_var = NodeSpec::variable(m.name.clone(), var_spec.clone());
        // VariableType verlinkt per HasComponent zur Member-Variable.
        vt_node = vt_node.with_ref(ReferenceKind::HasComponent, TypeRef::new(m.name.clone()));

        // DataType selbst sammelt die Member als Felder ueber Tab 9.15
        // ebenfalls per HasComponent (zu BaseDataVariableType).
        dt_node = dt_node.with_ref(ReferenceKind::HasComponent, TypeRef::new(m.name.clone()));

        // Rekursion: Member-Type kann selbst Struct/Sequence/...
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
/// Indirekt rekursiv via `walk` (Case-Member-Type kann Struct/Sequence/Map).
/// Selbe Bound wie `walk` (siehe dort).
fn emit_union(u: &UnionDef, out: &mut WalkOutput) -> Result<(), WalkError> {
    // Spec §9.2.4.2.1: Discriminator muss erlaubt sein.
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
    // SwitchField als erster Member mit consecutive 1..N Switch-Values.
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
            // Spec §9.2.5.2.x: Sequence ohne explizite Bound bekommt
            // ArrayDimensions = [0] (= "ungebunden"); mit Bound = [N].
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
        // outer struct mit einem inner-Struct-Member.
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
            // Float ist kein erlaubter Discriminator (Spec §9.2.4.2.1).
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
        // Alias selbst emittiert keine Node, aber das Target-Struct schon.
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

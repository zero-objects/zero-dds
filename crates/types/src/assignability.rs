// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Type-Assignability (XTypes 1.3 §7.2.4.1).
//!
//! Bestimmt ob ein Typ `T1` einem Typ `T2` zugewiesen werden kann —
//! entspricht "kompatibel fuer Publication/Subscription-Match". Die
//! Regeln haengen von Extensibility (Final/Appendable/Mutable) +
//! TypeConsistencyEnforcement ab.
//!
//! Core-Regeln fuer Primitives, Strings,
//! Collections, Aliases (via Resolver), Enums + Structs mit
//! Final/Appendable/Mutable-Semantik. Strict-vs-lax-Variante ueber
//! `AssignabilityConfig`.

use crate::resolve::{TypeRegistry, resolve_alias_chain};
use crate::type_identifier::{PrimitiveKind, TypeIdentifier};
use crate::type_object::flags::StructTypeFlag;
use crate::type_object::minimal::{MinimalStructType, MinimalTypeObject};

/// Fehler beim Flatten einer Inheritance-Kette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InheritanceError {
    /// `base_type` zeigt auf eine `EquivalenceHash`, die nicht in der
    /// Registry liegt.
    UnknownBase {
        /// Der ungeloeste Hash.
        hash: crate::type_identifier::EquivalenceHash,
    },
    /// `base_type` zeigt auf einen TypeObject, der kein Struct ist
    /// (z.B. Enum oder Alias auf Enum).
    BaseNotAStruct,
    /// Inheritance-Cycle erkannt.
    Cycle,
    /// Maximum-Depth ueberschritten.
    DepthExceeded {
        /// Limit.
        limit: usize,
    },
    /// Member-Name oder Member-ID kollidiert zwischen Base und Derived
    /// (XTypes 1.3 §7.2.2.4.5).
    InheritanceConflict {
        /// Bezugs-ID oder -Name.
        member_id: u32,
        /// Beschreibung.
        reason: &'static str,
    },
}

/// Baut die "hypothetical flat type"-Struktur aus einer Single-Inheritance-
/// Kette (XTypes 1.3 §7.2.2.4.5). Resolved `base_type` rekursiv und
/// concatenated Base- gefolgt von Derived-Membern. Bei Member-Name- oder
/// Member-ID-Kollisionen wird `InheritanceConflict` zurueckgegeben.
///
/// `max_depth` limitiert die Tiefe der Inheritance-Kette (Cycle-Schutz).
///
/// # Errors
/// Siehe [`InheritanceError`].
pub fn flatten_inheritance(
    s: &MinimalStructType,
    registry: &TypeRegistry,
    max_depth: usize,
) -> Result<MinimalStructType, InheritanceError> {
    use alloc::collections::BTreeSet;

    let mut visited: BTreeSet<crate::type_identifier::EquivalenceHash> = BTreeSet::new();
    let mut chain: alloc::vec::Vec<MinimalStructType> = alloc::vec::Vec::new();
    let mut current = s.clone();
    for _ in 0..max_depth {
        let base_ti = current.header.base_type.clone();
        chain.push(current.clone());
        match base_ti {
            TypeIdentifier::None => break,
            TypeIdentifier::EquivalenceHashMinimal(h)
            | TypeIdentifier::EquivalenceHashComplete(h) => {
                if !visited.insert(h) {
                    return Err(InheritanceError::Cycle);
                }
                let to = match registry.get_minimal(&h) {
                    Some(MinimalTypeObject::Struct(b)) => b.clone(),
                    Some(_) => return Err(InheritanceError::BaseNotAStruct),
                    None => return Err(InheritanceError::UnknownBase { hash: h }),
                };
                current = to;
            }
            _ => return Err(InheritanceError::BaseNotAStruct),
        }
    }
    if chain
        .last()
        .is_none_or(|c| c.header.base_type != TypeIdentifier::None)
    {
        return Err(InheritanceError::DepthExceeded { limit: max_depth });
    }

    // chain ist [Derived, Mid1, Mid2, ..., Root].
    // Spec §7.2.2.4.5: Konkateniere von Root zu Derived (Base-Member zuerst).
    let mut flat_members: alloc::vec::Vec<crate::type_object::minimal::MinimalStructMember> =
        alloc::vec::Vec::new();
    let mut seen_ids: BTreeSet<u32> = BTreeSet::new();
    let mut seen_names: BTreeSet<crate::type_object::common::NameHash> = BTreeSet::new();
    for st in chain.iter().rev() {
        for m in &st.member_seq {
            if !seen_ids.insert(m.common.member_id) {
                return Err(InheritanceError::InheritanceConflict {
                    member_id: m.common.member_id,
                    reason: "member_id collides between base and derived",
                });
            }
            if !seen_names.insert(m.detail) {
                return Err(InheritanceError::InheritanceConflict {
                    member_id: m.common.member_id,
                    reason: "member name_hash collides between base and derived",
                });
            }
            flat_members.push(m.clone());
        }
    }

    // Resultat: Derived's Header (incl. base_type=None nach Flatten) +
    // konkat. Member.
    let Some(derived) = chain.first().cloned() else {
        return Err(InheritanceError::DepthExceeded { limit: max_depth });
    };
    let mut flat = derived;
    flat.header.base_type = TypeIdentifier::None;
    flat.member_seq = flat_members;
    Ok(flat)
}

/// Konfiguration fuer Assignability-Checks.
///
/// Die Felder ausser `max_depth` entsprechen 1:1 den Flags der DDS-QoS
/// `TypeConsistencyEnforcement` (XTypes §7.6.3.7). `TypeMatcher`
/// uebersetzt eine konkrete TCE-Policy in dieses Struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssignabilityConfig {
    /// Erlaubt Type-Coercion (int32 ↔ int64 etc.)?
    pub allow_type_coercion: bool,
    /// Ignoriert Sequence-Bounds (Writer darf groesser als Reader sein).
    pub ignore_sequence_bounds: bool,
    /// Ignoriert String-Bounds.
    pub ignore_string_bounds: bool,
    /// Ignoriert Member-Namen — Mutable-Structs matchen dann
    /// nur via `@id`-Member-ID, nicht ueber NameHash.
    pub ignore_member_names: bool,
    /// `@ignore_literal_names` global (XTypes §7.2.4.4.7) — Enum-Compat
    /// vergleicht nur Ordinalwerte, nicht Literal-Namen. Zusaetzlich
    /// kann das per `EnumTypeFlag::IGNORE_LITERAL_NAMES` auf einer
    /// einzelnen Seite gesetzt werden; die Disjunktion gewinnt.
    pub ignore_literal_names: bool,
    /// Maximum-Depth fuer rekursives Aufloesen.
    pub max_depth: usize,
}

impl Default for AssignabilityConfig {
    fn default() -> Self {
        Self {
            allow_type_coercion: false,
            ignore_sequence_bounds: true,
            ignore_string_bounds: true,
            ignore_member_names: false,
            ignore_literal_names: false,
            max_depth: crate::resolve::DEFAULT_MAX_RESOLVE_DEPTH,
        }
    }
}

/// Ergebnis des Assignability-Checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Assignable {
    /// Kompatibel.
    Yes,
    /// Nicht kompatibel mit Begruendung.
    No(&'static str),
}

impl Assignable {
    /// `true` wenn kompatibel.
    #[must_use]
    pub const fn is_yes(&self) -> bool {
        matches!(self, Self::Yes)
    }
}

/// Prueft ob der Writer-Typ `w` fuer den Reader-Typ `r` kompatibel ist.
///
/// zerodds-lint: recursion-depth 64
///
/// **Wichtig zur Tiefen-Zahl**: die `64` ist der *Default*
/// ([`DEFAULT_MAX_RESOLVE_DEPTH`]) und **nicht** enforced durch Code —
/// `cfg.max_depth` ist runtime-konfigurierbar. Tests fahren Werte bis
/// 512 (`resolve_depth_exceeded`-Tests), Produktions-Aufrufer sollten
/// die Default-`AssignabilityConfig` nutzen, sonst den Wert explizit
/// mit der Risikobewertung abstimmen.
///
/// Rekursion via `check_direct` → `is_assignable` (nested Sequences,
/// Struct-Member).
pub fn is_assignable(
    w: &TypeIdentifier,
    r: &TypeIdentifier,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    // Alias-Resolution auf beiden Seiten.
    let Ok(w) = resolve_alias_chain(w, registry, cfg.max_depth) else {
        return Assignable::No("writer alias resolution failed");
    };
    let Ok(r) = resolve_alias_chain(r, registry, cfg.max_depth) else {
        return Assignable::No("reader alias resolution failed");
    };

    check_direct(&w, &r, registry, cfg)
}

/// zerodds-lint: recursion-depth 64
///
/// Dispatcht die Assignability-Checks pro TypeIdentifier-Variante.
/// Nested-Types (Sequence, Struct-Member) rufen `is_assignable` rekursiv
/// — Depth-Cap via `cfg.max_depth`.
fn check_direct(
    w: &TypeIdentifier,
    r: &TypeIdentifier,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    // Exakte Identitaet → immer Yes.
    if w == r {
        return Assignable::Yes;
    }

    match (w, r) {
        // Primitive → Primitive
        (TypeIdentifier::Primitive(wp), TypeIdentifier::Primitive(rp)) => {
            primitive_compatible(*wp, *rp, cfg)
        }
        // String-Kompatibilitaet. Small/Large ist nur Encoding-Detail;
        // Bounds werden per `ignore_string_bounds` gefiltert.
        (
            TypeIdentifier::String8Small { .. } | TypeIdentifier::String8Large { .. },
            TypeIdentifier::String8Small { .. } | TypeIdentifier::String8Large { .. },
        ) => {
            let (wb, rb) = (string_bound_u32_s8(w), string_bound_u32_s8(r));
            if !cfg.ignore_string_bounds && rb != 0 && wb > rb {
                Assignable::No("writer string8 bound exceeds reader bound")
            } else {
                Assignable::Yes
            }
        }
        (
            TypeIdentifier::String16Small { .. } | TypeIdentifier::String16Large { .. },
            TypeIdentifier::String16Small { .. } | TypeIdentifier::String16Large { .. },
        ) => {
            let (wb, rb) = (string_bound_u32_s16(w), string_bound_u32_s16(r));
            if !cfg.ignore_string_bounds && rb != 0 && wb > rb {
                Assignable::No("writer string16 bound exceeds reader bound")
            } else {
                Assignable::Yes
            }
        }

        // Sequence-Kompatibilitaet: Small ↔ Small/Large, Large ↔ Large/Small.
        // Small vs Large ist nur Wire-Encoding; Bounds werden auf u32
        // normalisiert und per Policy gefiltert.
        (
            TypeIdentifier::PlainSequenceSmall { .. } | TypeIdentifier::PlainSequenceLarge { .. },
            TypeIdentifier::PlainSequenceSmall { .. } | TypeIdentifier::PlainSequenceLarge { .. },
        ) => {
            let (we, wb) = sequence_parts(w);
            let (re, rb) = sequence_parts(r);
            if !cfg.ignore_sequence_bounds && rb != 0 && wb > rb {
                return Assignable::No("writer sequence bound exceeds reader bound");
            }
            is_assignable(we, re, registry, cfg)
        }

        // Array: feste Dimensionen. `bound_seq`-Vergleich ist strukturell;
        // Ignore-Bounds gilt hier nicht (Array-Bounds sind keine Policy-Sache).
        // Array: Small + Large untereinander; Bounds auf u32-Vec normalisieren
        // und strukturell vergleichen.
        (
            TypeIdentifier::PlainArraySmall { .. } | TypeIdentifier::PlainArrayLarge { .. },
            TypeIdentifier::PlainArraySmall { .. } | TypeIdentifier::PlainArrayLarge { .. },
        ) => {
            let (we, wb) = array_parts(w);
            let (re, rb) = array_parts(r);
            if wb != rb {
                return Assignable::No("array bounds differ");
            }
            is_assignable(we, re, registry, cfg)
        }

        // Map: Small ↔ Small/Large, Large ↔ Large/Small.
        (
            TypeIdentifier::PlainMapSmall { .. } | TypeIdentifier::PlainMapLarge { .. },
            TypeIdentifier::PlainMapSmall { .. } | TypeIdentifier::PlainMapLarge { .. },
        ) => {
            let (we, wk, wb) = map_parts(w);
            let (re, rk, rb) = map_parts(r);
            if !cfg.ignore_sequence_bounds && rb != 0 && wb > rb {
                return Assignable::No("writer map bound exceeds reader bound");
            }
            match is_assignable(wk, rk, registry, cfg) {
                Assignable::Yes => is_assignable(we, re, registry, cfg),
                e => e,
            }
        }

        // Hash-Refs: beide auf Minimal → strukturelle Gleichheit.
        (
            TypeIdentifier::EquivalenceHashMinimal(wh),
            TypeIdentifier::EquivalenceHashMinimal(rh),
        ) => {
            if wh == rh {
                return Assignable::Yes;
            }
            // TypeObjects aus Registry vergleichen.
            match (registry.get_minimal(wh), registry.get_minimal(rh)) {
                (Some(wobj), Some(robj)) => check_minimal_types(wobj, robj, registry, cfg),
                _ => Assignable::No("unknown type objects for hash comparison"),
            }
        }

        _ => Assignable::No("kinds do not match"),
    }
}

fn sequence_parts(ti: &TypeIdentifier) -> (&TypeIdentifier, u32) {
    match ti {
        TypeIdentifier::PlainSequenceSmall { element, bound, .. } => (element, u32::from(*bound)),
        TypeIdentifier::PlainSequenceLarge { element, bound, .. } => (element, *bound),
        _ => (ti, 0),
    }
}

fn array_parts(ti: &TypeIdentifier) -> (&TypeIdentifier, alloc::vec::Vec<u32>) {
    match ti {
        TypeIdentifier::PlainArraySmall {
            element,
            array_bounds,
            ..
        } => (
            element,
            array_bounds.iter().map(|b| u32::from(*b)).collect(),
        ),
        TypeIdentifier::PlainArrayLarge {
            element,
            array_bounds,
            ..
        } => (element, array_bounds.clone()),
        _ => (ti, alloc::vec::Vec::new()),
    }
}

fn map_parts(ti: &TypeIdentifier) -> (&TypeIdentifier, &TypeIdentifier, u32) {
    match ti {
        TypeIdentifier::PlainMapSmall {
            element,
            key,
            bound,
            ..
        } => (element, key, u32::from(*bound)),
        TypeIdentifier::PlainMapLarge {
            element,
            key,
            bound,
            ..
        } => (element, key, *bound),
        _ => (ti, ti, 0),
    }
}

fn string_bound_u32_s8(ti: &TypeIdentifier) -> u32 {
    match ti {
        TypeIdentifier::String8Small { bound } => u32::from(*bound),
        TypeIdentifier::String8Large { bound } => *bound,
        _ => 0,
    }
}

fn string_bound_u32_s16(ti: &TypeIdentifier) -> u32 {
    match ti {
        TypeIdentifier::String16Small { bound } => u32::from(*bound),
        TypeIdentifier::String16Large { bound } => *bound,
        _ => 0,
    }
}

fn primitive_compatible(
    w: PrimitiveKind,
    r: PrimitiveKind,
    cfg: &AssignabilityConfig,
) -> Assignable {
    if w == r {
        return Assignable::Yes;
    }
    if !cfg.allow_type_coercion {
        return Assignable::No("primitive kinds differ (no coercion allowed)");
    }
    // Koerzions-Matrix fuer Numerics — widening OK, narrowing nein.
    // Signed-Widening (Int8/16/32 → Int64), Unsigned-Widening (UInt8/16/32
    // → UInt64 + auch in signed-wider-Typen, da alle Werte verlustfrei
    // passen), Float-Widening (Float32 → Float64).
    use PrimitiveKind::*;
    let ok = matches!(
        (w, r),
        (Int8 | UInt8 | Byte, Int16 | Int32 | Int64)
            | (Int16 | UInt16, Int32 | Int64)
            | (Int32 | UInt32, Int64)
            | (UInt8 | Byte, UInt16 | UInt32 | UInt64)
            | (UInt16, UInt32 | UInt64)
            | (UInt32, UInt64)
            | (Float32, Float64)
    );
    if ok {
        Assignable::Yes
    } else {
        Assignable::No("primitive coercion not widening-safe")
    }
}

/// Drei-Werte-Extensibility aus den Flag-Bits. Default (keine Flags) =
/// Appendable, entsprechend XTypes §7.2.2.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StructExt {
    Final,
    Appendable,
    Mutable,
}

fn struct_extensibility(flags: StructTypeFlag) -> StructExt {
    if flags.has(StructTypeFlag::IS_FINAL) {
        StructExt::Final
    } else if flags.has(StructTypeFlag::IS_MUTABLE) {
        StructExt::Mutable
    } else {
        StructExt::Appendable
    }
}

fn check_minimal_types(
    w: &MinimalTypeObject,
    r: &MinimalTypeObject,
    registry: &TypeRegistry,
    cfg: &AssignabilityConfig,
) -> Assignable {
    match (w, r) {
        (MinimalTypeObject::Struct(ws), MinimalTypeObject::Struct(rs)) => {
            // Extensibility-Check: writer + reader muessen
            // dieselbe Extensibility-Kategorie haben (§7.2.4.4).
            // Bugfix (#10): Konsolidiere Flag-Bits auf einen drei-Werte-
            // Enum-Vergleich, um Corner-Case "beide haben keine Flag-Bits
            // gesetzt" = Appendable == Appendable korrekt zu erfassen.
            let w_ext = struct_extensibility(ws.struct_flags);
            let r_ext = struct_extensibility(rs.struct_flags);
            if w_ext != r_ext {
                return Assignable::No("extensibility mismatch");
            }
            let w_final = matches!(w_ext, StructExt::Final);
            let w_mut = matches!(w_ext, StructExt::Mutable);

            if w_final {
                // Strict gleich (selbe Anzahl + gleiche Types in Reihenfolge).
                if ws.member_seq.len() != rs.member_seq.len() {
                    return Assignable::No("final struct member count mismatch");
                }
                for (wm, rm) in ws.member_seq.iter().zip(rs.member_seq.iter()) {
                    if !cfg.ignore_member_names && wm.detail != rm.detail {
                        return Assignable::No("final struct member name-hash differs");
                    }
                    match is_assignable(
                        &wm.common.member_type_id,
                        &rm.common.member_type_id,
                        registry,
                        cfg,
                    ) {
                        Assignable::Yes => {}
                        e => return e,
                    }
                }
                Assignable::Yes
            } else if w_mut {
                // Mutable: match per @id (member_id). Reader-Member mit
                // @id=X muss im Writer existieren und kompatibel sein,
                // wenn nicht optional. Mit `ignore_member_names=false`
                // muss auch der NameHash matchen (§7.6.3.7.2.2).
                for rm in &rs.member_seq {
                    let rm_optional = rm
                        .common
                        .member_flags
                        .has(crate::type_object::flags::StructMemberFlag::IS_OPTIONAL);
                    match ws
                        .member_seq
                        .iter()
                        .find(|wm| wm.common.member_id == rm.common.member_id)
                    {
                        Some(wm) => {
                            if !cfg.ignore_member_names && wm.detail != rm.detail {
                                return Assignable::No(
                                    "mutable: member name-hash differs despite id match",
                                );
                            }
                            match is_assignable(
                                &wm.common.member_type_id,
                                &rm.common.member_type_id,
                                registry,
                                cfg,
                            ) {
                                Assignable::Yes => {}
                                e => return e,
                            }
                        }
                        None if rm_optional => {}
                        None => return Assignable::No("mutable: reader member missing in writer"),
                    }
                }
                Assignable::Yes
            } else {
                // Appendable (default): writer muss mindestens alle
                // reader-Felder als Prefix haben; extra writer-Felder OK.
                if ws.member_seq.len() < rs.member_seq.len() {
                    return Assignable::No("appendable: writer has fewer members than reader");
                }
                for (wm, rm) in ws.member_seq.iter().zip(rs.member_seq.iter()) {
                    if !cfg.ignore_member_names && wm.detail != rm.detail {
                        return Assignable::No("appendable: member name-hash differs");
                    }
                    match is_assignable(
                        &wm.common.member_type_id,
                        &rm.common.member_type_id,
                        registry,
                        cfg,
                    ) {
                        Assignable::Yes => {}
                        e => return e,
                    }
                }
                Assignable::Yes
            }
        }
        (MinimalTypeObject::Enumerated(we), MinimalTypeObject::Enumerated(re)) => {
            // §7.2.4.4.4.3: writer-Values muessen alle im reader-Set
            // enthalten sein (writer ⊆ reader). Reader darf zusaetzliche
            // Literale haben — er konsumiert weniger als er erkennt.
            // Bit-Bound muss identisch sein (Wire-Breite).
            if we.header.common.bit_bound != re.header.common.bit_bound {
                return Assignable::No("enum bit_bound mismatch");
            }
            // §7.2.4.4.7 — Default vergleicht (value, name_hash); mit
            // `@ignore_literal_names` auf einer Seite oder via Config nur (value).
            let ignore_names = cfg.ignore_literal_names
                || we
                    .enum_flags
                    .has(crate::type_object::flags::EnumTypeFlag::IGNORE_LITERAL_NAMES)
                || re
                    .enum_flags
                    .has(crate::type_object::flags::EnumTypeFlag::IGNORE_LITERAL_NAMES);
            for wl in &we.literal_seq {
                let found = re.literal_seq.iter().any(|rl| {
                    rl.common.value == wl.common.value && (ignore_names || rl.detail == wl.detail)
                });
                if !found {
                    return Assignable::No("enum writer literal unknown in reader");
                }
            }
            Assignable::Yes
        }
        _ => Assignable::No("type kinds do not match"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::builder::{Extensibility, TypeObjectBuilder};
    use crate::hash::compute_minimal_hash;
    use crate::type_object::TypeObject;

    #[test]
    fn primitive_same_kind_is_assignable() {
        let reg = TypeRegistry::new();
        let a = is_assignable(
            &TypeIdentifier::Primitive(PrimitiveKind::Int32),
            &TypeIdentifier::Primitive(PrimitiveKind::Int32),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes());
    }

    #[test]
    fn primitive_different_kind_is_not_assignable_by_default() {
        let reg = TypeRegistry::new();
        let a = is_assignable(
            &TypeIdentifier::Primitive(PrimitiveKind::Int32),
            &TypeIdentifier::Primitive(PrimitiveKind::Int64),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
    }

    #[test]
    fn primitive_widening_with_coercion_is_assignable() {
        let reg = TypeRegistry::new();
        let cfg = AssignabilityConfig {
            allow_type_coercion: true,
            ..Default::default()
        };
        assert!(
            is_assignable(
                &TypeIdentifier::Primitive(PrimitiveKind::Int32),
                &TypeIdentifier::Primitive(PrimitiveKind::Int64),
                &reg,
                &cfg,
            )
            .is_yes()
        );
        // Narrowing bleibt verboten
        assert!(
            !is_assignable(
                &TypeIdentifier::Primitive(PrimitiveKind::Int64),
                &TypeIdentifier::Primitive(PrimitiveKind::Int32),
                &reg,
                &cfg,
            )
            .is_yes()
        );
    }

    #[test]
    fn appendable_struct_with_extra_writer_field_is_assignable() {
        let mut reg = TypeRegistry::new();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let wh = compute_minimal_hash(&writer).unwrap();
        let rh = compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer.clone());
        reg.insert_minimal(rh, reader);

        assert!(
            is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    #[test]
    fn final_struct_with_extra_writer_field_is_not_assignable() {
        let mut reg = TypeRegistry::new();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Final)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Final)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let wh = compute_minimal_hash(&writer).unwrap();
        let rh = compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);

        assert!(
            !is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    #[test]
    fn mutable_struct_member_id_matching() {
        let mut reg = TypeRegistry::new();
        // Reader hat @id(1) und @id(2) — writer hat @id(2) und @id(3).
        // Reader @id(1) fehlt im writer, aber markieren wir als optional.
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                    m.id(2)
                })
                .member("c", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                    m.id(3)
                })
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                    m.id(1).optional()
                })
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                    m.id(2)
                })
                .build_minimal(),
        );
        let wh = compute_minimal_hash(&writer).unwrap();
        let rh = compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);

        assert!(
            is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    #[test]
    fn extensibility_mismatch_fails() {
        let mut reg = TypeRegistry::new();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Final)
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .build_minimal(),
        );
        let wh = compute_minimal_hash(&writer).unwrap();
        let rh = compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
    }

    #[test]
    fn string_small_and_large_interchangeable() {
        let reg = TypeRegistry::new();
        assert!(
            is_assignable(
                &TypeIdentifier::String8Small { bound: 64 },
                &TypeIdentifier::String8Large { bound: 100_000 },
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    // Silence unused import when only some Variants are matched.
    #[allow(dead_code)]
    fn _unused() -> TypeObject {
        TypeObject::Minimal(MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::dummy").build_minimal(),
        ))
    }

    // ---- Additional coverage for check_direct arms -----------------------

    use crate::type_identifier::PlainCollectionHeader;
    use alloc::boxed::Box;

    fn reg() -> TypeRegistry {
        TypeRegistry::new()
    }

    #[test]
    fn string8_vs_string16_not_assignable() {
        let a = is_assignable(
            &TypeIdentifier::String8Small { bound: 16 },
            &TypeIdentifier::String16Small { bound: 16 },
            &reg(),
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
        assert!(matches!(a, Assignable::No(msg) if msg.contains("kinds")));
    }

    #[test]
    fn string16_small_and_large_interchangeable() {
        let a = is_assignable(
            &TypeIdentifier::String16Small { bound: 32 },
            &TypeIdentifier::String16Large { bound: 10_000 },
            &reg(),
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes());
    }

    #[test]
    fn identical_type_identifiers_short_circuit_yes() {
        // Exact equality path (`w == r`) in check_direct.
        let ti = TypeIdentifier::Primitive(PrimitiveKind::UInt32);
        let a = is_assignable(&ti, &ti, &reg(), &AssignabilityConfig::default());
        assert!(a.is_yes());
    }

    #[test]
    fn sequence_writer_bound_exceeds_reader_bound_is_no() {
        let w = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 20,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        let r = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 10,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        // `ignore_sequence_bounds=false` erzwingt den Bound-Check.
        let cfg = AssignabilityConfig {
            ignore_sequence_bounds: false,
            ..Default::default()
        };
        let a = is_assignable(&w, &r, &reg(), &cfg);
        assert!(!a.is_yes());
        assert!(matches!(a, Assignable::No(msg) if msg.contains("bound")));
    }

    /// Mit `ignore_sequence_bounds=true` (TCE-Default) akzeptieren wir
    /// die Sequence, obwohl Writer-Bound > Reader-Bound.
    #[test]
    fn sequence_bounds_ignored_when_policy_allows() {
        let w = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 20,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        let r = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 10,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        assert!(is_assignable(&w, &r, &reg(), &AssignabilityConfig::default()).is_yes());
    }

    #[test]
    fn sequence_reader_unbounded_accepts_any_writer_bound() {
        // reader bound=0 (unbounded) → writer bound check skipped.
        let w = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 200,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int16)),
        };
        let r = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 0,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int16)),
        };
        assert!(is_assignable(&w, &r, &reg(), &AssignabilityConfig::default()).is_yes());
    }

    #[test]
    fn sequence_elements_must_be_assignable() {
        // Same bound, but different incompatible element kinds.
        let w = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 5,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        let r = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 5,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Float64)),
        };
        assert!(!is_assignable(&w, &r, &reg(), &AssignabilityConfig::default()).is_yes());
    }

    #[test]
    fn nested_sequence_of_sequence_assignable_when_elements_match() {
        let inner = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 5,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Byte)),
        };
        let outer = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 3,
            element: Box::new(inner.clone()),
        };
        // outer vs outer with inner bound differing: inner writer=5, reader=10 → OK
        let inner_wider = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 10,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Byte)),
        };
        let outer2 = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 3,
            element: Box::new(inner_wider),
        };
        assert!(is_assignable(&outer, &outer2, &reg(), &AssignabilityConfig::default()).is_yes());
    }

    #[test]
    fn plain_array_identical_assigns_yes() {
        let a = is_assignable(
            &TypeIdentifier::PlainArraySmall {
                header: PlainCollectionHeader::default(),
                array_bounds: alloc::vec![3, 4],
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            },
            &TypeIdentifier::PlainArraySmall {
                header: PlainCollectionHeader::default(),
                array_bounds: alloc::vec![3, 4],
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            },
            &reg(),
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes());
    }

    #[test]
    fn plain_array_diff_bounds_is_no() {
        let b = is_assignable(
            &TypeIdentifier::PlainArraySmall {
                header: PlainCollectionHeader::default(),
                array_bounds: alloc::vec![3, 4],
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            },
            &TypeIdentifier::PlainArraySmall {
                header: PlainCollectionHeader::default(),
                array_bounds: alloc::vec![3, 5],
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            },
            &reg(),
            &AssignabilityConfig::default(),
        );
        assert!(!b.is_yes());
        assert!(matches!(b, Assignable::No(msg) if msg.contains("array bounds")));
    }

    #[test]
    fn equivalence_hash_identical_short_circuits_to_yes() {
        // Bei identischen Hashes nimmt `check_direct` den Early-Exit
        // `wh == rh → Yes` (siehe match-Arm). Voraussetzung ist nur,
        // dass `resolve_alias_chain` erfolgreich ist — dafuer muss
        // der Hash in der Registry auf einen Nicht-Alias verweisen.
        let mut reg = reg();
        let to = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::T")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let h = compute_minimal_hash(&to).unwrap();
        reg.insert_minimal(h, to);
        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(h),
            &TypeIdentifier::EquivalenceHashMinimal(h),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes());
    }

    #[test]
    fn equivalence_hash_unresolved_writer_is_no() {
        // Alias-Resolution schlaegt fehl, weil der Hash nicht in der
        // Registry liegt → Frueh-Exit mit `No("writer alias resolution failed")`.
        let reg = reg();
        let wh = crate::type_identifier::EquivalenceHash([0x01; 14]);
        let rh = crate::type_identifier::EquivalenceHash([0x02; 14]);
        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
        assert!(matches!(a, Assignable::No(msg) if msg.contains("alias resolution")));
    }

    #[test]
    fn equivalence_hash_unresolved_reader_is_no() {
        // Writer ist registriert, reader nicht → Fehler beim Reader-Resolve.
        let mut reg = reg();
        let to = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::T")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let wh = compute_minimal_hash(&to).unwrap();
        reg.insert_minimal(wh, to);
        let rh = crate::type_identifier::EquivalenceHash([0x02; 14]);
        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
        assert!(matches!(a, Assignable::No(msg) if msg.contains("reader")));
    }

    #[test]
    fn mixed_kinds_report_kinds_do_not_match() {
        // Primitive vs String → kein Arm matched → `No("kinds do not match")`.
        let a = is_assignable(
            &TypeIdentifier::Primitive(PrimitiveKind::Int32),
            &TypeIdentifier::String8Small { bound: 10 },
            &reg(),
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
    }

    #[test]
    fn enum_mismatch_writer_literal_unknown_in_reader_is_no() {
        let mut reg = reg();
        let w = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("A", 1)
                .literal("B", 2)
                .build_minimal(),
        );
        let r = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("A", 1)
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
    }

    #[test]
    fn enum_bit_bound_mismatch_is_no() {
        let mut reg = reg();
        let w = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("A", 1)
                .build_minimal(),
        );
        let r = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(16)
                .literal("A", 1)
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
    }

    #[test]
    fn enum_identical_labels_is_yes() {
        let mut reg = reg();
        let w = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("A", 1)
                .literal("B", 2)
                .build_minimal(),
        );
        let r = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("A", 1)
                .literal("B", 2)
                .literal("C", 3) // reader kennt extra label, writer-labels sind subset
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes());
    }

    // ---- §7.2.2.4.5 Inheritance flatten + edge-cases ----

    #[test]
    fn flatten_inheritance_no_base_returns_struct_unchanged() {
        let s = TypeObjectBuilder::struct_type("::S")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_minimal();
        let reg = reg();
        let flat = flatten_inheritance(&s, &reg, 8).unwrap();
        assert_eq!(flat.member_seq.len(), 1);
    }

    #[test]
    fn flatten_inheritance_two_levels_concatenates_base_first() {
        // Root → Mid → Derived. Result Member-Reihenfolge: [Root, Mid, Derived].
        let mut reg = reg();
        let root = TypeObjectBuilder::struct_type("::Root")
            .member("r", TypeIdentifier::Primitive(PrimitiveKind::Int8), |m| {
                m.id(101)
            })
            .build_minimal();
        let root_h = compute_minimal_hash(&MinimalTypeObject::Struct(root.clone())).unwrap();
        reg.insert_minimal(root_h, MinimalTypeObject::Struct(root));

        let mid = TypeObjectBuilder::struct_type("::Mid")
            .base(TypeIdentifier::EquivalenceHashMinimal(root_h))
            .member("m", TypeIdentifier::Primitive(PrimitiveKind::Int16), |m| {
                m.id(202)
            })
            .build_minimal();
        let mid_h = compute_minimal_hash(&MinimalTypeObject::Struct(mid.clone())).unwrap();
        reg.insert_minimal(mid_h, MinimalTypeObject::Struct(mid));

        let derived = TypeObjectBuilder::struct_type("::Derived")
            .base(TypeIdentifier::EquivalenceHashMinimal(mid_h))
            .member("d", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(303)
            })
            .build_minimal();

        let flat = flatten_inheritance(&derived, &reg, 8).unwrap();
        // 3 Member, base_type weg.
        assert_eq!(flat.header.base_type, TypeIdentifier::None);
        assert_eq!(flat.member_seq.len(), 3);
        // Erster Member ist 'r' (root); letzter 'd' (derived).
        let first_id = flat.member_seq[0].common.member_id;
        let last_id = flat.member_seq[2].common.member_id;
        assert_ne!(first_id, last_id);
    }

    #[test]
    fn inheritance_conflict_same_id() {
        let mut reg = reg();
        let base = TypeObjectBuilder::struct_type("::B")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(7)
            })
            .build_minimal();
        let bh = compute_minimal_hash(&MinimalTypeObject::Struct(base.clone())).unwrap();
        reg.insert_minimal(bh, MinimalTypeObject::Struct(base));

        let derived = TypeObjectBuilder::struct_type("::D")
            .base(TypeIdentifier::EquivalenceHashMinimal(bh))
            .member("c", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(7) // selbe ID wie Base-Member 'a' → Konflikt
            })
            .build_minimal();
        let err = flatten_inheritance(&derived, &reg, 8).unwrap_err();
        assert!(matches!(
            err,
            InheritanceError::InheritanceConflict { reason, .. }
                if reason.contains("member_id")
        ));
    }

    #[test]
    fn inheritance_conflict_same_name() {
        let mut reg = reg();
        let base = TypeObjectBuilder::struct_type("::B")
            .member(
                "dup",
                TypeIdentifier::Primitive(PrimitiveKind::Int32),
                |m| m.id(1),
            )
            .build_minimal();
        let bh = compute_minimal_hash(&MinimalTypeObject::Struct(base.clone())).unwrap();
        reg.insert_minimal(bh, MinimalTypeObject::Struct(base));

        let derived = TypeObjectBuilder::struct_type("::D")
            .base(TypeIdentifier::EquivalenceHashMinimal(bh))
            .member(
                "dup",
                TypeIdentifier::Primitive(PrimitiveKind::Int32),
                |m| {
                    m.id(2) // andere ID, aber gleicher Name → Konflikt
                },
            )
            .build_minimal();
        let err = flatten_inheritance(&derived, &reg, 8).unwrap_err();
        assert!(matches!(
            err,
            InheritanceError::InheritanceConflict { reason, .. }
                if reason.contains("name_hash")
        ));
    }

    #[test]
    fn flat_type_construction_two_levels() {
        // §7.2.2.4.5 — die hypothetische flache Repraesentation eines
        // 2-Stufen-Inheritance-Konstrukts hat Members in Base-Derived-
        // Reihenfolge.
        let mut reg = reg();
        let base = TypeObjectBuilder::struct_type("::Base")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(1)
            })
            .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(2)
            })
            .build_minimal();
        let bh = compute_minimal_hash(&MinimalTypeObject::Struct(base.clone())).unwrap();
        reg.insert_minimal(bh, MinimalTypeObject::Struct(base));

        let derived = TypeObjectBuilder::struct_type("::Derived")
            .base(TypeIdentifier::EquivalenceHashMinimal(bh))
            .member("c", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(3)
            })
            .build_minimal();

        let flat = flatten_inheritance(&derived, &reg, 8).unwrap();
        assert_eq!(flat.member_seq.len(), 3);
        assert_eq!(flat.member_seq[0].common.member_id, 1);
        assert_eq!(flat.member_seq[1].common.member_id, 2);
        assert_eq!(flat.member_seq[2].common.member_id, 3);
    }

    #[test]
    fn two_level_inheritance_assignability_chain() {
        // Writer und Reader haben jeweils 2-Stufen-Inheritance, beide
        // produzieren dieselbe flache Member-Sequenz → assignable.
        let mut reg = reg();

        // ---- Writer-Seite ----
        let w_root = TypeObjectBuilder::struct_type("::WRoot")
            .member("r", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(1)
            })
            .build_minimal();
        let wr_h = compute_minimal_hash(&MinimalTypeObject::Struct(w_root.clone())).unwrap();
        reg.insert_minimal(wr_h, MinimalTypeObject::Struct(w_root));

        let w_mid = TypeObjectBuilder::struct_type("::WMid")
            .base(TypeIdentifier::EquivalenceHashMinimal(wr_h))
            .member("m", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(2)
            })
            .build_minimal();

        let w_flat = flatten_inheritance(&w_mid, &reg, 8).unwrap();
        assert_eq!(w_flat.member_seq.len(), 2);

        // ---- Reader-Seite (gleiche Struktur, andere Type-Namen) ----
        let r_root = TypeObjectBuilder::struct_type("::RRoot")
            .member("r", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(1)
            })
            .build_minimal();
        let rr_h = compute_minimal_hash(&MinimalTypeObject::Struct(r_root.clone())).unwrap();
        reg.insert_minimal(rr_h, MinimalTypeObject::Struct(r_root));

        let r_mid = TypeObjectBuilder::struct_type("::RMid")
            .base(TypeIdentifier::EquivalenceHashMinimal(rr_h))
            .member("m", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                m.id(2)
            })
            .build_minimal();

        let r_flat = flatten_inheritance(&r_mid, &reg, 8).unwrap();
        assert_eq!(r_flat.member_seq.len(), 2);

        // Direkter Member-by-Member-Compare via Assignability.
        let w_to = MinimalTypeObject::Struct(w_flat.clone());
        let r_to = MinimalTypeObject::Struct(r_flat.clone());
        let wh = compute_minimal_hash(&w_to).unwrap();
        let rh = compute_minimal_hash(&r_to).unwrap();
        reg.insert_minimal(wh, w_to);
        reg.insert_minimal(rh, r_to);
        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes(), "got {a:?}");
    }

    #[test]
    fn enum_not_assignable_strict_default() {
        // Selber Wert, aber unterschiedliche Namen → strict-Default
        // (keine `@ignore_literal_names`-Annotation, kein Config-Flag)
        // muss assignable=No liefern.
        let mut reg = reg();
        let w = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("RED", 1)
                .build_minimal(),
        );
        let r = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("ROUGE", 1) // selber Wert, anderer Name
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
    }

    #[test]
    fn enum_assignable_with_ignore_literal_names() {
        // Selber Wert, andere Namen — aber Config oder Flag setzt
        // ignore_literal_names; Result muss Yes sein.
        let mut reg = reg();
        let w = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("RED", 1)
                .literal("GREEN", 2)
                .build_minimal(),
        );
        let r = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("ROUGE", 1)
                .literal("VERT", 2)
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let cfg = AssignabilityConfig {
            ignore_literal_names: true,
            ..AssignabilityConfig::default()
        };
        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &cfg,
        );
        assert!(a.is_yes());
    }

    #[test]
    fn enum_assignable_with_ignore_literal_names_via_writer_flag() {
        // Wenn der Writer `EnumTypeFlag::IGNORE_LITERAL_NAMES` setzt,
        // wirkt das fuer den Vergleich genauso wie der Config-Flag.
        let mut reg = reg();
        let mut w_e = TypeObjectBuilder::enum_type("::E")
            .bit_bound(32)
            .literal("RED", 1)
            .build_minimal();
        w_e.enum_flags = crate::type_object::flags::EnumTypeFlag(
            crate::type_object::flags::EnumTypeFlag::IGNORE_LITERAL_NAMES,
        );
        let w = MinimalTypeObject::Enumerated(w_e);
        let r = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("ROUGE", 1)
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes());
    }

    #[test]
    fn enum_assignable_with_ignore_literal_names_via_reader_flag() {
        let mut reg = reg();
        let w = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::E")
                .bit_bound(32)
                .literal("RED", 1)
                .build_minimal(),
        );
        let mut r_e = TypeObjectBuilder::enum_type("::E")
            .bit_bound(32)
            .literal("ROUGE", 1)
            .build_minimal();
        r_e.enum_flags = crate::type_object::flags::EnumTypeFlag(
            crate::type_object::flags::EnumTypeFlag::IGNORE_LITERAL_NAMES,
        );
        let r = MinimalTypeObject::Enumerated(r_e);
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(a.is_yes());
    }

    #[test]
    fn struct_vs_enum_type_object_kinds_dont_match() {
        let mut reg = reg();
        let w = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let r = MinimalTypeObject::Enumerated(
            TypeObjectBuilder::enum_type("::X")
                .bit_bound(32)
                .literal("A", 1)
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&w).unwrap();
        let rh = crate::hash::compute_minimal_hash(&r).unwrap();
        reg.insert_minimal(wh, w);
        reg.insert_minimal(rh, r);

        let a = is_assignable(
            &TypeIdentifier::EquivalenceHashMinimal(wh),
            &TypeIdentifier::EquivalenceHashMinimal(rh),
            &reg,
            &AssignabilityConfig::default(),
        );
        assert!(!a.is_yes());
    }

    #[test]
    fn appendable_struct_writer_smaller_than_reader_is_no() {
        let mut reg = reg();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Appendable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&writer).unwrap();
        let rh = crate::hash::compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);

        assert!(
            !is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    #[test]
    fn mutable_reader_member_missing_in_writer_non_optional_is_no() {
        let mut reg = reg();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(2)
                })
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Mutable)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(1) // NOT optional
                })
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
                    m.id(2)
                })
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&writer).unwrap();
        let rh = crate::hash::compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);

        assert!(
            !is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    #[test]
    fn final_struct_member_count_mismatch_is_no() {
        let mut reg = reg();
        let writer = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Final)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let reader = MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::X")
                .extensibility(Extensibility::Final)
                .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
                .build_minimal(),
        );
        let wh = crate::hash::compute_minimal_hash(&writer).unwrap();
        let rh = crate::hash::compute_minimal_hash(&reader).unwrap();
        reg.insert_minimal(wh, writer);
        reg.insert_minimal(rh, reader);

        assert!(
            !is_assignable(
                &TypeIdentifier::EquivalenceHashMinimal(wh),
                &TypeIdentifier::EquivalenceHashMinimal(rh),
                &reg,
                &AssignabilityConfig::default(),
            )
            .is_yes()
        );
    }

    #[test]
    fn primitive_widening_int16_to_int64_is_assignable_with_coercion() {
        let cfg = AssignabilityConfig {
            allow_type_coercion: true,
            ..Default::default()
        };
        assert!(primitive_compatible(PrimitiveKind::Int16, PrimitiveKind::Int64, &cfg).is_yes());
        assert!(primitive_compatible(PrimitiveKind::Byte, PrimitiveKind::Int32, &cfg).is_yes());
        assert!(
            primitive_compatible(PrimitiveKind::Float32, PrimitiveKind::Float64, &cfg).is_yes()
        );
    }

    #[test]
    fn primitive_unwidening_is_rejected_even_with_coercion() {
        let cfg = AssignabilityConfig {
            allow_type_coercion: true,
            ..Default::default()
        };
        assert!(!primitive_compatible(PrimitiveKind::Float64, PrimitiveKind::Int32, &cfg).is_yes());
    }

    #[test]
    fn assignable_is_yes_matches_expectation() {
        assert!(Assignable::Yes.is_yes());
        assert!(!Assignable::No("reason").is_yes());
    }
}

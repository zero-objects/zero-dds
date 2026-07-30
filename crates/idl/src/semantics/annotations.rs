// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Typed annotation model (XTypes-relevant builtin annotations).
//!
//! Converts generic `Annotation { name, params }` from the AST into a
//! typed `BuiltinAnnotation` enum. Unknown / vendor-
//! specific annotations stay as unrecognized in the `custom` vec.
//!
//! Source of truth: XTypes §7.3.1.2 (standard annotations) +
//! IDL 4.2 §8.3.

use crate::ast::{Annotation, AnnotationParams, ConstExpr, LiteralKind};

/// Extensibility kind from `@extensibility(FINAL|APPENDABLE|MUTABLE)`
/// or the aliases `@final`, `@appendable`, `@mutable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensibilityKind {
    /// `FINAL` — strict equality.
    Final,
    /// `APPENDABLE` — prefix match.
    Appendable,
    /// `MUTABLE` — ID-based match.
    Mutable,
}

/// Kind from `@autoid(SEQUENTIAL|HASH)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoidKind {
    /// Sequential IDs starting at 0.
    Sequential,
    /// MD5-based hash of the member name.
    Hash,
}

/// Kind from `@try_construct(DISCARD|USE_DEFAULT|TRIM)`
/// (XTypes 1.3 §7.2.4.2 / §7.3.1.2.1.1 `TRY_CONSTRUCT1`/`TRY_CONSTRUCT2`).
/// Controls how a reader treats a member it cannot construct correctly
/// (a string over its bound, a sequence over its max, an enum value outside
/// `@bit_bound`). The frontend lowers this here; the bits are materialized at
/// the TypeObject layer (`to_typeobject.rs` → `StructMemberBuilder::try_construct`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryConstructKind {
    /// `DISCARD` — drop the sample (default, §7.2.2.4.4.4.4).
    Discard,
    /// `USE_DEFAULT` — substitute the member default, keep the sample.
    UseDefault,
    /// `TRIM` — truncate strings/sequences to the bound, keep the sample.
    Trim,
}

/// Typed representation of the standard builtin annotations.
#[derive(Debug, Clone, PartialEq)]
pub enum BuiltinAnnotation {
    /// `@key` (valid on a member).
    Key,
    /// `@id(n)` (valid on a member).
    Id(u32),
    /// `@optional`.
    Optional,
    /// `@shared` (Plain-Language-Binding §8.1.5; XTypes 1.3 §7.2.5.x).
    /// Marks a member mapped as a pointer/shared reference instead of
    /// embedded-by-value. In the C++ PSM -> `std::shared_ptr<T>`,
    /// in C# -> a wrapper class, in Java -> a reference type (default pointer).
    Shared,
    /// `@must_understand`.
    MustUnderstand,
    /// `@external`.
    External,
    /// `@non_serialized` (XTypes 1.3 §7.2.4.4.2). The member is program-
    /// internal storage and MUST be omitted from every wire form;
    /// assignability comparisons MUST skip the member.
    NonSerialized,
    /// `@ignore_literal_names` (XTypes 1.3 §7.2.4.4.7). On an enum
    /// type: in compat comparisons ignore literal names, compare only
    /// ordinal values.
    IgnoreLiteralNames,
    /// `@default(value)` — value as a string (the caller converts to the type).
    Default(String),
    /// `@extensibility(FINAL|APPENDABLE|MUTABLE)`.
    Extensibility(ExtensibilityKind),
    /// `@final` (shorthand).
    Final,
    /// `@appendable` (shorthand).
    Appendable,
    /// `@mutable` (shorthand).
    Mutable,
    /// `@autoid(SEQUENTIAL|HASH)`.
    Autoid(AutoidKind),
    /// `@try_construct(DISCARD|USE_DEFAULT|TRIM)` (XTypes 1.3 §7.2.4.2). The
    /// bits are consumed at the TypeObject layer (see [`TryConstructKind`]).
    TryConstruct(TryConstructKind),
    /// `@topic` (marker).
    Topic,
    /// `@nested`.
    Nested,
    /// `@unit("...")`.
    Unit(String),
    /// `@hashid("hint")`.
    HashId(Option<String>),
    /// `@range(min, max)` — strings for simple serialization.
    Range {
        /// Min literal.
        min: Option<String>,
        /// Max literal.
        max: Option<String>,
    },
    /// `@min(value)`.
    Min(String),
    /// `@max(value)`.
    Max(String),
    /// `@value(v)` — enum literal value.
    Value(String),
    /// `@position(n)` — Bitmask/Bitfield.
    Position(u32),
    /// `@bit_bound(n)`.
    BitBound(u16),
    /// `@default_literal`.
    DefaultLiteral,
    /// `@verbatim(language, placement, text)` (§8.3.5.1).
    /// Spec-compliant lowering with separate fields and a
    /// `PlacementKind` enum instead of a string placeholder.
    Verbatim(VerbatimSpec),
    /// `@ami(boolean default TRUE)` (§8.3.6.3) — asynchronous-method-
    /// invocation marker. The bool value (default `true`) signals
    /// whether the annotated op is AMI-capable.
    Ami(bool),
    /// `@service(string platform default "*")` (§8.3.6.1).
    /// Marker for an interface to be treated as a service.
    /// Platform values: `"CORBA"`, `"DDS"`, `"*"` (default).
    Service(String),
    /// `@oneway(boolean value default TRUE)` (§8.3.6.2). Marks an
    /// operation as one-way (no return value, no out/inout
    /// params). Disambiguated from the recognizer-side `oneway` keyword
    /// (Rule 120).
    OnewayAnno(bool),
}

/// `@verbatim` lowering struct (§8.3.5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbatimSpec {
    /// Codegen language, default `"*"` (all languages).
    pub language: String,
    /// Where in the generated output the text should be placed.
    pub placement: PlacementKind,
    /// Raw verbatim text.
    pub text: String,
}

/// Where `@verbatim` text should be placed in the generated output
/// (§8.3.5.1 — `PlacementKind` enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementKind {
    /// Before any other output in the file.
    BeginFile,
    /// Directly before the annotated declaration.
    BeforeDeclaration,
    /// First element inside the annotated declaration.
    BeginDeclaration,
    /// Last element inside the annotated declaration.
    EndDeclaration,
    /// Directly after the annotated declaration (spec default).
    AfterDeclaration,
    /// Last element in the file.
    EndFile,
}

impl PlacementKind {
    /// Maps a spec identifier (`BEGIN_FILE`, `AFTER_DECLARATION`, ...)
    /// to the corresponding enum value.
    #[must_use]
    pub fn from_ident(s: &str) -> Option<Self> {
        Some(match s {
            "BEGIN_FILE" => Self::BeginFile,
            "BEFORE_DECLARATION" => Self::BeforeDeclaration,
            "BEGIN_DECLARATION" => Self::BeginDeclaration,
            "END_DECLARATION" => Self::EndDeclaration,
            "AFTER_DECLARATION" => Self::AfterDeclaration,
            "END_FILE" => Self::EndFile,
            _ => return None,
        })
    }
}

/// Error during lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    /// `@id(x)` with a non-integer argument.
    InvalidIdArgument,
    /// `@extensibility(UNKNOWN)`.
    UnknownExtensibilityKind(String),
    /// `@autoid(UNKNOWN)`.
    UnknownAutoidKind(String),
    /// `@try_construct(UNKNOWN)`.
    UnknownTryConstructKind(String),
    /// Wrong argument count.
    WrongArgumentCount {
        /// Annotation name.
        annotation: String,
        /// Expected arg count.
        expected: usize,
        /// Actual.
        got: usize,
    },
    /// `@position(n)` with value > 65535 (spec §8.3.1.4: `unsigned short
    /// value` implies the range 0..=65535).
    PositionOutOfShortRange {
        /// Actual value.
        value: u32,
    },
    /// A recognized builtin annotation was given an argument of the wrong
    /// type or shape (broad-audit P1), e.g. `@autoid(1)` (integer where an
    /// enum kind is required), `@extensibility(1)` (integer where
    /// `FINAL|APPENDABLE|MUTABLE` is required) or `@position("x")` (string
    /// where a non-negative integer is required). The parser used to swallow
    /// these as `Ok(None)` / `unwrap_or(0)`, silently generating code with a
    /// default value. They are now a hard error surfaced by the semantic
    /// gate.
    WrongAnnotationArgument {
        /// Annotation name (without the leading `@`).
        annotation: String,
        /// Human-readable description of the accepted argument form.
        expected: String,
    },
}

impl core::fmt::Display for LowerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidIdArgument => {
                write!(f, "annotation argument must be a non-negative integer")
            }
            Self::UnknownExtensibilityKind(k) => write!(
                f,
                "@extensibility({k}) is not a valid kind (expected FINAL, APPENDABLE or MUTABLE)"
            ),
            Self::UnknownAutoidKind(k) => write!(
                f,
                "@autoid({k}) is not a valid kind (expected SEQUENTIAL or HASH)"
            ),
            Self::UnknownTryConstructKind(k) => write!(
                f,
                "@try_construct({k}) is not a valid kind (expected DISCARD, USE_DEFAULT or TRIM)"
            ),
            Self::WrongArgumentCount {
                annotation,
                expected,
                got,
            } => write!(f, "@{annotation} expects {expected} argument(s), got {got}"),
            Self::PositionOutOfShortRange { value } => write!(
                f,
                "@position({value}) is out of range (unsigned short: 0..=65535)"
            ),
            Self::WrongAnnotationArgument {
                annotation,
                expected,
            } => write!(
                f,
                "@{annotation} has a wrong-typed argument (expected {expected})"
            ),
        }
    }
}

impl std::error::Error for LowerError {}

/// Result of a lowering — separate lists for recognized builtins
/// and unknown (passed-through) annotations.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lowered {
    /// Typed standard annotations.
    pub builtins: Vec<BuiltinAnnotation>,
    /// Unknown / vendor-specific — passed through for the vendor layer.
    pub custom: Vec<Annotation>,
}

impl Lowered {
    /// `true` if `@key` is set.
    #[must_use]
    pub fn has_key(&self) -> bool {
        self.builtins
            .iter()
            .any(|a| matches!(a, BuiltinAnnotation::Key))
    }

    /// Explicit `@id(n)` value, if present.
    #[must_use]
    pub fn explicit_id(&self) -> Option<u32> {
        self.builtins.iter().find_map(|a| match a {
            BuiltinAnnotation::Id(n) => Some(*n),
            _ => None,
        })
    }

    /// First extensibility kind from `@extensibility(...)`, `@final`,
    /// `@appendable`, `@mutable` (first match wins).
    #[must_use]
    pub fn extensibility(&self) -> Option<ExtensibilityKind> {
        self.builtins.iter().find_map(|a| match a {
            BuiltinAnnotation::Extensibility(k) => Some(*k),
            BuiltinAnnotation::Final => Some(ExtensibilityKind::Final),
            BuiltinAnnotation::Appendable => Some(ExtensibilityKind::Appendable),
            BuiltinAnnotation::Mutable => Some(ExtensibilityKind::Mutable),
            _ => None,
        })
    }

    /// Effective `@try_construct(...)` kind for a member, if present
    /// (XTypes 1.3 §7.2.4.2). First match wins. Returns `None` for an
    /// un-annotated member — the caller then applies the default (DISCARD,
    /// §7.2.2.4.4.4.4), which is the same bit pattern the builder emits by
    /// default (`TRY_CONSTRUCT1`).
    #[must_use]
    pub fn try_construct(&self) -> Option<TryConstructKind> {
        self.builtins.iter().find_map(|a| match a {
            BuiltinAnnotation::TryConstruct(k) => Some(*k),
            _ => None,
        })
    }

    /// Returns all `@verbatim` specs whose `language` field matches the
    /// desired codegen (XTypes 1.3 §7.2.2.4.8 +
    /// IDL 4.2 §8.3.5.1).
    ///
    /// Match rule:
    /// 1. The spec wildcard `"*"` always matches.
    /// 2. The language tag is compared case-insensitively.
    /// 3. `lang_aliases` matches additional accepted tags
    ///    (e.g. `&["c++", "cpp", "cxx"]` for the C++ codegen).
    ///
    /// Order: stable by appearance in the source.
    #[must_use]
    pub fn verbatims_for_language<'a>(&'a self, lang_aliases: &[&str]) -> Vec<&'a VerbatimSpec> {
        self.builtins
            .iter()
            .filter_map(|a| match a {
                BuiltinAnnotation::Verbatim(v) => Some(v),
                _ => None,
            })
            .filter(|v| {
                v.language == "*"
                    || lang_aliases
                        .iter()
                        .any(|alias| alias.eq_ignore_ascii_case(&v.language))
            })
            .collect()
    }
}

fn name_tail(a: &Annotation) -> &str {
    a.name
        .parts
        .last()
        .map(|p| p.text.as_str())
        .unwrap_or_default()
}

/// Central extensibility normalization (broad-audit P0-4).
///
/// Reads the effective [`ExtensibilityKind`] from a raw annotation list,
/// honoring BOTH the short forms (`@final` / `@appendable` / `@mutable`)
/// AND the long form `@extensibility(FINAL|APPENDABLE|MUTABLE)`
/// (XTypes 1.3 §7.3.3). First match wins. Returns `None` for an
/// un-annotated aggregate — the caller then applies the default
/// (APPENDABLE, XTypes 1.3 §7.3.3.1).
///
/// Every backend (Rust/C++/C/TS/…) MUST read the extensibility through this
/// one function so the wire form agrees: MUTABLE → PL_CDR / EMHEADER,
/// APPENDABLE → DHEADER-delimited, FINAL → plain. Scanning only the short
/// forms silently downgrades `@extensibility(MUTABLE)` to the default and
/// drifts the wire between backends.
#[must_use]
pub fn extensibility_of(annotations: &[Annotation]) -> Option<ExtensibilityKind> {
    annotations.iter().find_map(extensibility_of_single)
}

/// Extensibility carried by a single annotation, if any. Recognizes both the
/// short forms and the `@extensibility(...)` long form; an unknown/malformed
/// long-form argument yields `None` (the type falls through to the default),
/// matching the lenient read of the per-backend emitters.
fn extensibility_of_single(a: &Annotation) -> Option<ExtensibilityKind> {
    match name_tail(a) {
        "final" => Some(ExtensibilityKind::Final),
        "appendable" => Some(ExtensibilityKind::Appendable),
        "mutable" => Some(ExtensibilityKind::Mutable),
        "extensibility" => match &a.params {
            AnnotationParams::Single(ConstExpr::Scoped(s)) => {
                match s.parts.last().map(|p| p.text.as_str()).unwrap_or("") {
                    "FINAL" => Some(ExtensibilityKind::Final),
                    "APPENDABLE" => Some(ExtensibilityKind::Appendable),
                    "MUTABLE" => Some(ExtensibilityKind::Mutable),
                    _ => None,
                }
            }
            _ => None,
        },
        _ => None,
    }
}

fn const_to_u32(expr: &ConstExpr) -> Option<u32> {
    if let ConstExpr::Literal(l) = expr {
        if matches!(l.kind, LiteralKind::Integer) {
            return l.raw.parse::<u32>().ok();
        }
    }
    None
}

/// First `@…(value)` parameter of an annotation, if it carries exactly one.
fn first_param(a: &Annotation) -> Option<&ConstExpr> {
    match &a.params {
        AnnotationParams::Single(e) => Some(e),
        AnnotationParams::Named(named) => named.first().map(|np| &np.value),
        AnnotationParams::None | AnnotationParams::Empty => None,
    }
}

/// Effective `@bit_bound(N)` of an enum — the integer that selects the wire
/// holder width (XTypes 1.3 §7.3.1.2.1.9 `@bit_bound` + §7.4.5.1 enum
/// serialization). The DEFAULT enum bit_bound is **32**; an explicit
/// `@bit_bound(N)` with N in 1..=32 narrows the holder. Out-of-range or
/// malformed values fall back to the 32-bit default (the lowering path in
/// [`lower_single`] separately rejects a non-integer argument as a hard error).
///
/// This is the ONE place every backend reads the enum bound from, so the wire
/// width agrees across all 17 emitters instead of each defaulting to 4 octets.
#[must_use]
pub fn enum_bit_bound(annotations: &[Annotation]) -> u32 {
    annotations
        .iter()
        .find(|a| name_tail(a) == "bit_bound")
        .and_then(first_param)
        .and_then(const_to_u32)
        .filter(|&v| (1..=32).contains(&v))
        .unwrap_or(32)
}

/// Wire width in octets (1/2/4) for an enum `@bit_bound` (XTypes 1.3
/// §7.3.1.2.1.9 + §7.4.5.1). The enum is serialized as a SIGNED integer whose
/// holder is picked by the bound:
///
/// | bit_bound | octets | signed holder |
/// |-----------|--------|---------------|
/// | 1..=8     | 1      | int8          |
/// | 9..=16    | 2      | int16         |
/// | 17..=32   | 4      | int32         |
///
/// The default bound (32, no `@bit_bound`) therefore stays 4 octets. The
/// in-memory representation may remain a wider integer; only the wire cast
/// narrows. Cyclone DDS honours this width; matching it is spec-faithful.
#[must_use]
pub fn enum_wire_octets(bit_bound: u32) -> u8 {
    if bit_bound <= 8 {
        1
    } else if bit_bound <= 16 {
        2
    } else {
        4
    }
}

fn const_to_string(expr: &ConstExpr) -> Option<String> {
    if let ConstExpr::Literal(l) = expr {
        let s = l.raw.as_str();
        if matches!(l.kind, LiteralKind::String) {
            // Remove only ONE quote pair at the edge. `trim_matches` would
            // swallow all quotes for `"""x"""` — but we only want the
            // outer pair delimiter.
            return Some(
                s.strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .unwrap_or(s)
                    .to_string(),
            );
        }
        return Some(s.to_string());
    }
    None
}

/// Lower a generic annotation to its typed form.
/// Returns `None` if it is not recognized as a builtin.
///
/// # Errors
/// `LowerError` on invalid arguments (e.g. `@id("abc")`).
pub fn lower_single(ann: &Annotation) -> Result<Option<BuiltinAnnotation>, LowerError> {
    let name = name_tail(ann);
    let params = &ann.params;
    Ok(Some(match name {
        "key" => BuiltinAnnotation::Key,
        "id" => {
            let v = match params {
                AnnotationParams::Single(e) => {
                    const_to_u32(e).ok_or(LowerError::InvalidIdArgument)?
                }
                _ => return Err(LowerError::InvalidIdArgument),
            };
            BuiltinAnnotation::Id(v)
        }
        "optional" => BuiltinAnnotation::Optional,
        "shared" => BuiltinAnnotation::Shared,
        "must_understand" => BuiltinAnnotation::MustUnderstand,
        "external" => BuiltinAnnotation::External,
        "non_serialized" => BuiltinAnnotation::NonSerialized,
        "ignore_literal_names" => BuiltinAnnotation::IgnoreLiteralNames,
        "default" => match params {
            AnnotationParams::Single(e) => {
                BuiltinAnnotation::Default(const_to_string(e).unwrap_or_default())
            }
            _ => return Ok(None),
        },
        // `@extensibility(FINAL|APPENDABLE|MUTABLE)` (XTypes 1.3 §7.3.3). The
        // value is an enum kind, never a number. A bare `@extensibility` (no
        // argument) is left for the default; an argument of the wrong TYPE
        // (`@extensibility(1)`) is a hard error (broad-audit P1) instead of
        // silently downgrading to the default extensibility.
        "extensibility" => match params {
            AnnotationParams::Single(ConstExpr::Scoped(s)) => {
                let ident = s.parts.last().map(|p| p.text.as_str()).unwrap_or("");
                let kind = match ident {
                    "FINAL" => ExtensibilityKind::Final,
                    "APPENDABLE" => ExtensibilityKind::Appendable,
                    "MUTABLE" => ExtensibilityKind::Mutable,
                    other => {
                        return Err(LowerError::UnknownExtensibilityKind(other.to_string()));
                    }
                };
                BuiltinAnnotation::Extensibility(kind)
            }
            AnnotationParams::None | AnnotationParams::Empty => return Ok(None),
            _ => {
                return Err(LowerError::WrongAnnotationArgument {
                    annotation: "extensibility".to_string(),
                    expected: "FINAL, APPENDABLE or MUTABLE".to_string(),
                });
            }
        },
        "final" => BuiltinAnnotation::Final,
        "appendable" => BuiltinAnnotation::Appendable,
        "mutable" => BuiltinAnnotation::Mutable,
        // `@autoid(SEQUENTIAL|HASH)` (XTypes 1.3 §7.3.1.2.1.10). The value is
        // an enum kind, never a number. A bare `@autoid` (no argument) is left
        // for the member-id layer to default (§7.3.1.2.1.1); an argument of the
        // wrong TYPE (`@autoid(1)`) is a hard error (broad-audit P1) instead of
        // being silently dropped into `custom`.
        "autoid" => match params {
            AnnotationParams::None | AnnotationParams::Empty => return Ok(None),
            AnnotationParams::Single(ConstExpr::Scoped(s)) => {
                let ident = s.parts.last().map(|p| p.text.as_str()).unwrap_or("");
                let kind = match ident {
                    "SEQUENTIAL" => AutoidKind::Sequential,
                    "HASH" => AutoidKind::Hash,
                    other => return Err(LowerError::UnknownAutoidKind(other.to_string())),
                };
                BuiltinAnnotation::Autoid(kind)
            }
            _ => {
                return Err(LowerError::WrongAnnotationArgument {
                    annotation: "autoid".to_string(),
                    expected: "SEQUENTIAL or HASH".to_string(),
                });
            }
        },
        // `@try_construct(DISCARD|USE_DEFAULT|TRIM)` (XTypes 1.3 §7.2.4.2). The
        // value is an enum kind, never a number. A bare `@try_construct` (no
        // argument) is left for the default (DISCARD, §7.2.2.4.4.4.4); an
        // argument of the wrong TYPE (`@try_construct(1)`) is a hard error
        // (broad-audit P1d) instead of silently downgrading to the default.
        "try_construct" => match params {
            AnnotationParams::None | AnnotationParams::Empty => return Ok(None),
            AnnotationParams::Single(ConstExpr::Scoped(s)) => {
                let ident = s.parts.last().map(|p| p.text.as_str()).unwrap_or("");
                let kind = match ident {
                    "DISCARD" => TryConstructKind::Discard,
                    "USE_DEFAULT" => TryConstructKind::UseDefault,
                    "TRIM" => TryConstructKind::Trim,
                    other => return Err(LowerError::UnknownTryConstructKind(other.to_string())),
                };
                BuiltinAnnotation::TryConstruct(kind)
            }
            _ => {
                return Err(LowerError::WrongAnnotationArgument {
                    annotation: "try_construct".to_string(),
                    expected: "DISCARD, USE_DEFAULT or TRIM".to_string(),
                });
            }
        },
        "topic" => BuiltinAnnotation::Topic,
        "nested" => BuiltinAnnotation::Nested,
        "unit" => match params {
            AnnotationParams::Single(e) => {
                BuiltinAnnotation::Unit(const_to_string(e).unwrap_or_default())
            }
            _ => return Ok(None),
        },
        "hashid" => match params {
            AnnotationParams::None | AnnotationParams::Empty => BuiltinAnnotation::HashId(None),
            AnnotationParams::Single(e) => BuiltinAnnotation::HashId(const_to_string(e)),
            _ => return Ok(None),
        },
        "min" => match params {
            AnnotationParams::Single(e) => {
                BuiltinAnnotation::Min(const_to_string(e).unwrap_or_default())
            }
            _ => return Ok(None),
        },
        "max" => match params {
            AnnotationParams::Single(e) => {
                BuiltinAnnotation::Max(const_to_string(e).unwrap_or_default())
            }
            _ => return Ok(None),
        },
        "value" => match params {
            AnnotationParams::Single(e) => {
                BuiltinAnnotation::Value(const_to_string(e).unwrap_or_default())
            }
            _ => return Ok(None),
        },
        // `@position(n)` (§8.3.1.4: `unsigned short value`) — a non-negative
        // integer in 0..=65535. A non-integer argument (`@position("x")`) is a
        // hard error (broad-audit P1) instead of silently defaulting to 0.
        "position" => match params {
            AnnotationParams::Single(e) => {
                let value = const_to_u32(e).ok_or_else(|| LowerError::WrongAnnotationArgument {
                    annotation: "position".to_string(),
                    expected: "a non-negative integer (0..=65535)".to_string(),
                })?;
                // §8.3.1.4: range 0..=65535 (u16). Values above that are a spec
                // violation.
                if value > u32::from(u16::MAX) {
                    return Err(LowerError::PositionOutOfShortRange { value });
                }
                BuiltinAnnotation::Position(value)
            }
            _ => return Ok(None),
        },
        "bit_bound" => match params {
            AnnotationParams::Single(e) => {
                // Bugfix (#30): an invalid argument (e.g. @bit_bound("x"))
                // is a hard error, not a silent default.
                let n = const_to_u32(e).ok_or(LowerError::InvalidIdArgument)?;
                BuiltinAnnotation::BitBound(n as u16)
            }
            _ => return Ok(None),
        },
        "default_literal" => BuiltinAnnotation::DefaultLiteral,
        "verbatim" => BuiltinAnnotation::Verbatim(lower_verbatim(params)?),
        "ami" => BuiltinAnnotation::Ami(lower_ami(params)),
        "range" => match params {
            AnnotationParams::Named(named) => {
                let mut min = None;
                let mut max = None;
                for np in named {
                    match np.name.text.as_str() {
                        "min" => min = const_to_string(&np.value),
                        "max" => max = const_to_string(&np.value),
                        _ => {}
                    }
                }
                BuiltinAnnotation::Range { min, max }
            }
            _ => return Ok(None),
        },
        "service" => match params {
            AnnotationParams::None | AnnotationParams::Empty => {
                BuiltinAnnotation::Service("*".to_string())
            }
            AnnotationParams::Single(e) => {
                BuiltinAnnotation::Service(const_to_string(e).unwrap_or_else(|| "*".into()))
            }
            AnnotationParams::Named(named) => {
                let platform = named
                    .iter()
                    .find_map(|np| {
                        if np.name.text == "platform" {
                            const_to_string(&np.value)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "*".into());
                BuiltinAnnotation::Service(platform)
            }
        },
        "oneway" => match params {
            AnnotationParams::None | AnnotationParams::Empty => BuiltinAnnotation::OnewayAnno(true),
            AnnotationParams::Single(ConstExpr::Literal(l))
                if matches!(l.kind, LiteralKind::Boolean) =>
            {
                BuiltinAnnotation::OnewayAnno(l.raw == "TRUE" || l.raw == "true")
            }
            AnnotationParams::Single(ConstExpr::Scoped(s))
                if s.parts.len() == 1
                    && matches!(
                        s.parts[0].text.as_str(),
                        "TRUE" | "FALSE" | "true" | "false"
                    ) =>
            {
                BuiltinAnnotation::OnewayAnno(matches!(s.parts[0].text.as_str(), "TRUE" | "true"))
            }
            _ => return Ok(None),
        },
        _ => return Ok(None),
    }))
}

/// §8.3.6.3 — `@ami(boolean default TRUE)`. The compact form `@ami` without
/// parens takes the default `true`. The single form with a bool-literal/scoped
/// `TRUE`/`FALSE` sets the value.
fn lower_ami(params: &AnnotationParams) -> bool {
    match params {
        AnnotationParams::None | AnnotationParams::Empty => true,
        AnnotationParams::Single(ConstExpr::Literal(l))
            if matches!(l.kind, LiteralKind::Boolean) =>
        {
            l.raw == "TRUE" || l.raw == "true"
        }
        AnnotationParams::Single(ConstExpr::Scoped(s)) => {
            let ident = s.parts.last().map(|p| p.text.as_str()).unwrap_or("");
            matches!(ident, "TRUE" | "true")
        }
        _ => true, // Default fallback for non-trivial args.
    }
}

fn lower_verbatim(params: &AnnotationParams) -> Result<VerbatimSpec, LowerError> {
    // Spec §8.3.5.1: all three members have defaults, or are interpreted via
    // the single form as `text` (compact form `@verbatim("...")`).
    match params {
        AnnotationParams::None | AnnotationParams::Empty => Ok(VerbatimSpec {
            language: "*".to_string(),
            placement: PlacementKind::AfterDeclaration,
            text: String::new(),
        }),
        AnnotationParams::Single(e) => Ok(VerbatimSpec {
            language: "*".to_string(),
            placement: PlacementKind::AfterDeclaration,
            text: const_to_string(e).unwrap_or_default(),
        }),
        AnnotationParams::Named(params) => {
            let mut spec = VerbatimSpec {
                language: "*".to_string(),
                placement: PlacementKind::AfterDeclaration,
                text: String::new(),
            };
            for p in params {
                match p.name.text.as_str() {
                    "language" => {
                        if let Some(s) = const_to_string(&p.value) {
                            spec.language = s;
                        }
                    }
                    "placement" => {
                        if let ConstExpr::Scoped(s) = &p.value {
                            let ident = s.parts.last().map(|p| p.text.as_str()).unwrap_or("");
                            if let Some(k) = PlacementKind::from_ident(ident) {
                                spec.placement = k;
                            }
                        }
                    }
                    "text" => {
                        if let Some(s) = const_to_string(&p.value) {
                            spec.text = s;
                        }
                    }
                    _ => {} // ignore unknown param names (spec lax).
                }
            }
            Ok(spec)
        }
    }
}

/// Lower an annotation list.
///
/// # Errors
/// `LowerError` on semantically invalid annotations (e.g. `@id("abc")`).
pub fn lower_annotations(anns: &[Annotation]) -> Result<Lowered, LowerError> {
    let mut out = Lowered::default();
    for a in anns {
        match lower_single(a)? {
            Some(b) => out.builtins.push(b),
            None => out.custom.push(a.clone()),
        }
    }
    Ok(out)
}

/// §8.3.3 — annotations on a typedef are inherited by every use
/// (e.g. a member whose `type_spec` points to the typedef symbol).
/// This function collects the effective annotations of a
/// member: member annotations + possibly typedef annotations (transitively,
/// if a typedef again points to a typedef).
///
/// On a member annotation and typedef annotation with the same name
/// the member annotation wins (the local spec statement takes precedence).
#[must_use]
pub fn effective_member_annotations(
    member: &crate::ast::Member,
    spec: &crate::ast::Specification,
) -> Vec<Annotation> {
    use crate::ast::TypeSpec;
    let mut out = member.annotations.clone();
    let mut current = &member.type_spec;
    let mut seen: Vec<String> = Vec::new();
    loop {
        let name = match current {
            TypeSpec::Scoped(s) => s.parts.last().map(|p| p.text.as_str()),
            _ => None,
        };
        let Some(name) = name else { break };
        if seen.iter().any(|n| n == name) {
            break; // cycle stop (defensive).
        }
        seen.push(name.to_string());
        let Some((td_anns, next_spec)) = lookup_typedef(spec, name) else {
            break;
        };
        for ann in td_anns {
            let ann_name = name_tail(ann);
            if !out.iter().any(|existing| name_tail(existing) == ann_name) {
                out.push(ann.clone());
            }
        }
        current = next_spec;
    }
    out
}

/// Returns `(annotations, target_type_spec)` of a top-level typedef
/// with the given name. Only simple (`Simple`) declarators are
/// considered; array declarators are their own new types and inherit
/// no annotations.
fn lookup_typedef<'a>(
    spec: &'a crate::ast::Specification,
    name: &str,
) -> Option<(&'a [Annotation], &'a crate::ast::TypeSpec)> {
    use crate::ast::{Declarator, Definition, TypeDecl};
    for def in &spec.definitions {
        if let Definition::Type(TypeDecl::Typedef(td)) = def {
            for d in &td.declarators {
                if let Declarator::Simple(ident) = d {
                    if ident.text == name {
                        return Some((&td.annotations, &td.type_spec));
                    }
                }
            }
        }
    }
    None
}

/// Convenience: lower type-level annotations. Currently identical to
/// [`lower_annotations`]; a placeholder for scope-specific
/// validation (e.g. `@key` only on members, not on types).
///
/// # Errors
/// Like [`lower_annotations`].
pub fn lower_type_annotations(anns: &[Annotation]) -> Result<Lowered, LowerError> {
    lower_annotations(anns)
}

#[cfg(test)]
extern crate alloc;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;
    use crate::parser::parse;

    fn parse_to_ast(src: &str) -> crate::ast::Specification {
        parse(src, &ParserConfig::default()).expect("parse")
    }

    fn find_struct_def(ast: &crate::ast::Specification) -> Option<&crate::ast::StructDef> {
        for def in &ast.definitions {
            if let crate::ast::Definition::Type(crate::ast::TypeDecl::Constr(
                crate::ast::ConstrTypeDecl::Struct(crate::ast::StructDcl::Def(s)),
            )) = def
            {
                return Some(s);
            }
        }
        None
    }

    fn struct_with_annotations(src: &str) -> Vec<Annotation> {
        let ast = parse_to_ast(src);
        find_struct_def(&ast)
            .map(|s| s.annotations.clone())
            .unwrap_or_default()
    }

    fn first_member_annotations(src: &str) -> Vec<Annotation> {
        let ast = parse_to_ast(src);
        find_struct_def(&ast)
            .and_then(|s| s.members.first())
            .map(|m| m.annotations.clone())
            .unwrap_or_default()
    }

    #[test]
    fn key_lowers_correctly() {
        let anns = first_member_annotations("struct S { @key long id; };");
        let lowered = lower_annotations(&anns).unwrap();
        assert!(lowered.has_key());
    }

    #[test]
    fn id_lowers_with_u32_value() {
        let anns = first_member_annotations("struct S { @id(7) long x; };");
        let lowered = lower_annotations(&anns).unwrap();
        assert_eq!(lowered.explicit_id(), Some(7));
    }

    #[test]
    fn appendable_shorthand_lowers_to_extensibility() {
        let anns = struct_with_annotations("@appendable struct S { long x; };");
        let lowered = lower_annotations(&anns).unwrap();
        assert_eq!(lowered.extensibility(), Some(ExtensibilityKind::Appendable));
    }

    #[test]
    fn mutable_shorthand_lowers() {
        let anns = struct_with_annotations("@mutable struct S { long x; };");
        let lowered = lower_annotations(&anns).unwrap();
        assert_eq!(lowered.extensibility(), Some(ExtensibilityKind::Mutable));
    }

    #[test]
    fn final_shorthand_lowers() {
        let anns = struct_with_annotations("@final struct S { long x; };");
        let lowered = lower_annotations(&anns).unwrap();
        assert_eq!(lowered.extensibility(), Some(ExtensibilityKind::Final));
    }

    // --- P0-4: central `extensibility_of` (short AND long form) ------------

    #[test]
    fn extensibility_of_long_form_mutable() {
        let anns = struct_with_annotations("@extensibility(MUTABLE) struct S { long x; };");
        assert_eq!(extensibility_of(&anns), Some(ExtensibilityKind::Mutable));
    }

    #[test]
    fn extensibility_of_long_form_final() {
        let anns = struct_with_annotations("@extensibility(FINAL) struct S { long x; };");
        assert_eq!(extensibility_of(&anns), Some(ExtensibilityKind::Final));
    }

    #[test]
    fn extensibility_of_long_form_appendable() {
        let anns = struct_with_annotations("@extensibility(APPENDABLE) struct S { long x; };");
        assert_eq!(extensibility_of(&anns), Some(ExtensibilityKind::Appendable));
    }

    #[test]
    fn extensibility_of_short_and_long_form_agree() {
        // The whole point of P0-4: the long form and the matching short form
        // resolve to the same ExtensibilityKind.
        for (long, short, kind) in [
            (
                "@extensibility(MUTABLE) struct S { long x; };",
                "@mutable struct S { long x; };",
                ExtensibilityKind::Mutable,
            ),
            (
                "@extensibility(FINAL) struct S { long x; };",
                "@final struct S { long x; };",
                ExtensibilityKind::Final,
            ),
            (
                "@extensibility(APPENDABLE) struct S { long x; };",
                "@appendable struct S { long x; };",
                ExtensibilityKind::Appendable,
            ),
        ] {
            let l = extensibility_of(&struct_with_annotations(long));
            let s = extensibility_of(&struct_with_annotations(short));
            assert_eq!(l, Some(kind));
            assert_eq!(l, s, "long and short form drift");
        }
    }

    #[test]
    fn extensibility_of_unannotated_is_none() {
        let anns = struct_with_annotations("struct S { long x; };");
        assert_eq!(extensibility_of(&anns), None);
    }

    #[test]
    fn unknown_annotation_preserved_in_custom() {
        let anns = struct_with_annotations("@my_vendor_tag struct S { long x; };");
        let lowered = lower_annotations(&anns).unwrap();
        assert_eq!(lowered.custom.len(), 1);
    }

    #[test]
    fn multiple_annotations_combine() {
        let anns = first_member_annotations("struct S { @key @id(1) long id; };");
        let lowered = lower_annotations(&anns).unwrap();
        assert!(lowered.has_key());
        assert_eq!(lowered.explicit_id(), Some(1));
    }

    // ---- Helpers for synthetic Annotation construction -------------------

    use crate::ast::{Identifier, Literal, NamedParam, ScopedName};
    use crate::errors::Span;

    fn sp() -> Span {
        Span::SYNTHETIC
    }

    fn ident(t: &str) -> Identifier {
        Identifier::new(t, sp())
    }

    fn scoped(parts: &[&str]) -> ScopedName {
        ScopedName {
            absolute: false,
            parts: parts.iter().map(|p| ident(p)).collect(),
            span: sp(),
        }
    }

    fn lit(kind: LiteralKind, raw: &str) -> ConstExpr {
        ConstExpr::Literal(Literal {
            kind,
            raw: raw.to_string(),
            span: sp(),
        })
    }

    fn ann(name: &str, params: AnnotationParams) -> Annotation {
        Annotation {
            name: scoped(&[name]),
            params,
            span: sp(),
        }
    }

    fn lower_one(name: &str, params: AnnotationParams) -> Option<BuiltinAnnotation> {
        lower_single(&ann(name, params)).unwrap()
    }

    // ---- Per-Builtin coverage --------------------------------------------

    #[test]
    fn optional_lowers_to_optional() {
        assert_eq!(
            lower_one("optional", AnnotationParams::None),
            Some(BuiltinAnnotation::Optional)
        );
    }

    #[test]
    fn must_understand_lowers() {
        assert_eq!(
            lower_one("must_understand", AnnotationParams::None),
            Some(BuiltinAnnotation::MustUnderstand)
        );
    }

    #[test]
    fn external_lowers() {
        assert_eq!(
            lower_one("external", AnnotationParams::None),
            Some(BuiltinAnnotation::External)
        );
    }

    #[test]
    fn default_lowers_with_integer_literal() {
        let a = lower_one(
            "default",
            AnnotationParams::Single(lit(LiteralKind::Integer, "42")),
        );
        assert_eq!(a, Some(BuiltinAnnotation::Default("42".into())));
    }

    #[test]
    fn default_lowers_with_string_literal_strips_quotes() {
        // const_to_string Edge: string literal should have its outer
        // double-quotes stripped so downstream consumers see the raw value.
        let a = lower_one(
            "default",
            AnnotationParams::Single(lit(LiteralKind::String, "\"foo\"")),
        );
        assert_eq!(a, Some(BuiltinAnnotation::Default("foo".into())));
    }

    #[test]
    fn default_without_single_is_ignored() {
        let a = lower_one("default", AnnotationParams::None);
        assert_eq!(a, None);
    }

    #[test]
    fn extensibility_final_lowers() {
        let a = lower_one(
            "extensibility",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["FINAL"]))),
        );
        assert_eq!(
            a,
            Some(BuiltinAnnotation::Extensibility(ExtensibilityKind::Final))
        );
    }

    #[test]
    fn extensibility_appendable_scoped_lowers() {
        let a = lower_one(
            "extensibility",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["APPENDABLE"]))),
        );
        assert_eq!(
            a,
            Some(BuiltinAnnotation::Extensibility(
                ExtensibilityKind::Appendable
            ))
        );
    }

    #[test]
    fn extensibility_mutable_scoped_lowers() {
        let a = lower_one(
            "extensibility",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["MUTABLE"]))),
        );
        assert_eq!(
            a,
            Some(BuiltinAnnotation::Extensibility(ExtensibilityKind::Mutable))
        );
    }

    #[test]
    fn extensibility_unknown_returns_error() {
        let err = lower_single(&ann(
            "extensibility",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["BAR"]))),
        ))
        .unwrap_err();
        assert_eq!(err, LowerError::UnknownExtensibilityKind("BAR".into()));
    }

    #[test]
    fn extensibility_wrong_typed_argument_is_error() {
        // Broad-audit P1: `@extensibility(1)` — an integer where an enum kind
        // is required — is a hard error, not a silent drop to the default.
        let err = lower_single(&ann(
            "extensibility",
            AnnotationParams::Single(lit(LiteralKind::Integer, "1")),
        ))
        .unwrap_err();
        assert_eq!(
            err,
            LowerError::WrongAnnotationArgument {
                annotation: "extensibility".into(),
                expected: "FINAL, APPENDABLE or MUTABLE".into(),
            }
        );
    }

    #[test]
    fn autoid_sequential_lowers() {
        let a = lower_one(
            "autoid",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["SEQUENTIAL"]))),
        );
        assert_eq!(a, Some(BuiltinAnnotation::Autoid(AutoidKind::Sequential)));
    }

    #[test]
    fn autoid_hash_lowers() {
        let a = lower_one(
            "autoid",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["HASH"]))),
        );
        assert_eq!(a, Some(BuiltinAnnotation::Autoid(AutoidKind::Hash)));
    }

    #[test]
    fn autoid_unknown_is_error() {
        let err = lower_single(&ann(
            "autoid",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["BAR"]))),
        ))
        .unwrap_err();
        assert_eq!(err, LowerError::UnknownAutoidKind("BAR".into()));
    }

    #[test]
    fn autoid_wrong_typed_argument_is_error() {
        // Broad-audit P1: `@autoid(1)` — an integer where SEQUENTIAL|HASH is
        // required — is a hard error, not a silent drop to `custom`.
        let err = lower_single(&ann(
            "autoid",
            AnnotationParams::Single(lit(LiteralKind::Integer, "1")),
        ))
        .unwrap_err();
        assert_eq!(
            err,
            LowerError::WrongAnnotationArgument {
                annotation: "autoid".into(),
                expected: "SEQUENTIAL or HASH".into(),
            }
        );
    }

    #[test]
    fn autoid_bare_is_left_for_default() {
        // A bare `@autoid` (no argument) is not a wrong-typed argument; it is
        // left for the member-id layer to default (§7.3.1.2.1.1), so lowering
        // must not treat it as an error.
        assert_eq!(lower_one("autoid", AnnotationParams::None), None);
        assert_eq!(lower_one("autoid", AnnotationParams::Empty), None);
    }

    // ---- @try_construct (XTypes 1.3 §7.2.4.2) ----------------------------

    #[test]
    fn try_construct_discard_lowers() {
        let a = lower_one(
            "try_construct",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["DISCARD"]))),
        );
        assert_eq!(
            a,
            Some(BuiltinAnnotation::TryConstruct(TryConstructKind::Discard))
        );
    }

    #[test]
    fn try_construct_use_default_lowers() {
        let a = lower_one(
            "try_construct",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["USE_DEFAULT"]))),
        );
        assert_eq!(
            a,
            Some(BuiltinAnnotation::TryConstruct(
                TryConstructKind::UseDefault
            ))
        );
    }

    #[test]
    fn try_construct_trim_lowers() {
        let a = lower_one(
            "try_construct",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["TRIM"]))),
        );
        assert_eq!(
            a,
            Some(BuiltinAnnotation::TryConstruct(TryConstructKind::Trim))
        );
    }

    #[test]
    fn try_construct_unknown_is_error() {
        let err = lower_single(&ann(
            "try_construct",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["BOGUS"]))),
        ))
        .unwrap_err();
        assert_eq!(err, LowerError::UnknownTryConstructKind("BOGUS".into()));
    }

    #[test]
    fn try_construct_wrong_typed_argument_is_error() {
        // Broad-audit P1d: `@try_construct(1)` — an integer where an enum kind
        // is required — is a hard error, not a silent drop to the default.
        let err = lower_single(&ann(
            "try_construct",
            AnnotationParams::Single(lit(LiteralKind::Integer, "1")),
        ))
        .unwrap_err();
        assert_eq!(
            err,
            LowerError::WrongAnnotationArgument {
                annotation: "try_construct".into(),
                expected: "DISCARD, USE_DEFAULT or TRIM".into(),
            }
        );
    }

    #[test]
    fn try_construct_bare_is_left_for_default() {
        // A bare `@try_construct` (no argument) is not an error; it is left for
        // the TypeObject layer to default to DISCARD (§7.2.2.4.4.4.4).
        assert_eq!(lower_one("try_construct", AnnotationParams::None), None);
        assert_eq!(lower_one("try_construct", AnnotationParams::Empty), None);
    }

    #[test]
    fn lowered_try_construct_reads_first_match() {
        let anns = alloc::vec![ann(
            "try_construct",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["TRIM"]))),
        )];
        let lowered = lower_annotations(&anns).unwrap();
        assert_eq!(lowered.try_construct(), Some(TryConstructKind::Trim));
    }

    #[test]
    fn try_construct_absent_is_none() {
        let lowered = Lowered::default();
        assert_eq!(lowered.try_construct(), None);
    }

    #[test]
    fn topic_and_nested_lower() {
        assert_eq!(
            lower_one("topic", AnnotationParams::None),
            Some(BuiltinAnnotation::Topic)
        );
        assert_eq!(
            lower_one("nested", AnnotationParams::None),
            Some(BuiltinAnnotation::Nested)
        );
    }

    #[test]
    fn unit_lowers_with_string() {
        let a = lower_one(
            "unit",
            AnnotationParams::Single(lit(LiteralKind::String, "\"meters\"")),
        );
        assert_eq!(a, Some(BuiltinAnnotation::Unit("meters".into())));
    }

    #[test]
    fn unit_without_single_is_ignored() {
        assert_eq!(lower_one("unit", AnnotationParams::None), None);
    }

    #[test]
    fn hashid_no_params_lowers_with_none_hint() {
        assert_eq!(
            lower_one("hashid", AnnotationParams::None),
            Some(BuiltinAnnotation::HashId(None))
        );
    }

    #[test]
    fn hashid_empty_params_lowers_with_none_hint() {
        assert_eq!(
            lower_one("hashid", AnnotationParams::Empty),
            Some(BuiltinAnnotation::HashId(None))
        );
    }

    #[test]
    fn hashid_with_string_hint_lowers() {
        let a = lower_one(
            "hashid",
            AnnotationParams::Single(lit(LiteralKind::String, "\"abc\"")),
        );
        assert_eq!(a, Some(BuiltinAnnotation::HashId(Some("abc".into()))));
    }

    #[test]
    fn hashid_named_params_ignored() {
        let a = lower_one(
            "hashid",
            AnnotationParams::Named(alloc::vec![NamedParam {
                name: ident("hint"),
                value: lit(LiteralKind::String, "\"x\""),
                span: sp(),
            }]),
        );
        assert_eq!(a, None);
    }

    #[test]
    fn min_and_max_lower_with_integer_literal() {
        assert_eq!(
            lower_one(
                "min",
                AnnotationParams::Single(lit(LiteralKind::Integer, "0"))
            ),
            Some(BuiltinAnnotation::Min("0".into()))
        );
        assert_eq!(
            lower_one(
                "max",
                AnnotationParams::Single(lit(LiteralKind::Integer, "100"))
            ),
            Some(BuiltinAnnotation::Max("100".into()))
        );
    }

    #[test]
    fn min_and_max_without_single_are_ignored() {
        assert_eq!(lower_one("min", AnnotationParams::None), None);
        assert_eq!(lower_one("max", AnnotationParams::Empty), None);
    }

    #[test]
    fn value_lowers_with_literal() {
        assert_eq!(
            lower_one(
                "value",
                AnnotationParams::Single(lit(LiteralKind::Integer, "7"))
            ),
            Some(BuiltinAnnotation::Value("7".into()))
        );
    }

    #[test]
    fn value_without_single_is_ignored() {
        assert_eq!(lower_one("value", AnnotationParams::None), None);
    }

    #[test]
    fn position_lowers_with_u32() {
        assert_eq!(
            lower_one(
                "position",
                AnnotationParams::Single(lit(LiteralKind::Integer, "5"))
            ),
            Some(BuiltinAnnotation::Position(5))
        );
    }

    #[test]
    fn position_non_integer_is_error() {
        // Broad-audit P1: `@position("foo")` — a string where a non-negative
        // integer is required — is a hard error, not a silent fallback to 0.
        let err = lower_single(&ann(
            "position",
            AnnotationParams::Single(lit(LiteralKind::String, "\"foo\"")),
        ))
        .unwrap_err();
        assert_eq!(
            err,
            LowerError::WrongAnnotationArgument {
                annotation: "position".into(),
                expected: "a non-negative integer (0..=65535)".into(),
            }
        );
    }

    #[test]
    fn position_without_single_is_ignored() {
        assert_eq!(lower_one("position", AnnotationParams::None), None);
    }

    #[test]
    fn bit_bound_lowers_with_u16() {
        assert_eq!(
            lower_one(
                "bit_bound",
                AnnotationParams::Single(lit(LiteralKind::Integer, "16"))
            ),
            Some(BuiltinAnnotation::BitBound(16))
        );
    }

    #[test]
    fn bit_bound_non_integer_rejects_with_error() {
        // Bugfix #30: @bit_bound("oops") is a LowerError, not a silent
        // default to 32.
        let err = lower_single(&ann(
            "bit_bound",
            AnnotationParams::Single(lit(LiteralKind::String, "\"oops\"")),
        ))
        .unwrap_err();
        assert_eq!(err, LowerError::InvalidIdArgument);
    }

    #[test]
    fn bit_bound_without_single_is_ignored() {
        assert_eq!(lower_one("bit_bound", AnnotationParams::None), None);
    }

    #[test]
    fn enum_wire_octets_bucket_boundaries() {
        // XTypes 1.3 §7.4.5.1: 1..=8 → 1, 9..=16 → 2, 17..=32 → 4.
        assert_eq!(enum_wire_octets(1), 1);
        assert_eq!(enum_wire_octets(8), 1);
        assert_eq!(enum_wire_octets(9), 2);
        assert_eq!(enum_wire_octets(16), 2);
        assert_eq!(enum_wire_octets(17), 4);
        assert_eq!(enum_wire_octets(32), 4);
    }

    #[test]
    fn enum_bit_bound_reads_annotation_and_defaults_to_32() {
        // No @bit_bound → default 32 → 4 octets.
        assert_eq!(enum_bit_bound(&[]), 32);
        assert_eq!(enum_wire_octets(enum_bit_bound(&[])), 4);
        // @bit_bound(8) → 8 → 1 octet.
        let a8 = [ann(
            "bit_bound",
            AnnotationParams::Single(lit(LiteralKind::Integer, "8")),
        )];
        assert_eq!(enum_bit_bound(&a8), 8);
        assert_eq!(enum_wire_octets(enum_bit_bound(&a8)), 1);
        // @bit_bound(16) → 16 → 2 octets.
        let a16 = [ann(
            "bit_bound",
            AnnotationParams::Single(lit(LiteralKind::Integer, "16")),
        )];
        assert_eq!(enum_wire_octets(enum_bit_bound(&a16)), 2);
        // Out-of-range bound (0) falls back to the 32-bit default.
        let a0 = [ann(
            "bit_bound",
            AnnotationParams::Single(lit(LiteralKind::Integer, "0")),
        )];
        assert_eq!(enum_bit_bound(&a0), 32);
    }

    #[test]
    fn default_literal_lowers() {
        assert_eq!(
            lower_one("default_literal", AnnotationParams::None),
            Some(BuiltinAnnotation::DefaultLiteral)
        );
    }

    #[test]
    fn id_with_string_argument_is_error() {
        let err = lower_single(&ann(
            "id",
            AnnotationParams::Single(lit(LiteralKind::String, "\"x\"")),
        ))
        .unwrap_err();
        assert_eq!(err, LowerError::InvalidIdArgument);
    }

    #[test]
    fn id_with_non_single_params_is_error() {
        let err = lower_single(&ann("id", AnnotationParams::None)).unwrap_err();
        assert_eq!(err, LowerError::InvalidIdArgument);
    }

    #[test]
    fn const_to_string_with_scoped_returns_none() {
        // Scoped const-expr is not a Literal — `const_to_string` returns None.
        // `@min(Foo::Bar)` → Min("") because `unwrap_or_default()`.
        let a = lower_one(
            "min",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["NS", "V"]))),
        );
        assert_eq!(a, Some(BuiltinAnnotation::Min(String::new())));
    }

    #[test]
    fn unknown_annotation_returns_none_from_single() {
        assert_eq!(
            lower_one("completely_unknown", AnnotationParams::None),
            None
        );
    }

    #[test]
    fn lowered_extensibility_picks_first_of_many() {
        let anns = alloc::vec![
            ann(
                "extensibility",
                AnnotationParams::Single(ConstExpr::Scoped(scoped(&["FINAL"])))
            ),
            ann("mutable", AnnotationParams::None),
        ];
        let lowered = lower_annotations(&anns).unwrap();
        assert_eq!(lowered.extensibility(), Some(ExtensibilityKind::Final));
    }

    #[test]
    fn lower_type_annotations_delegates() {
        let anns = alloc::vec![ann("final", AnnotationParams::None)];
        let lowered = lower_type_annotations(&anns).unwrap();
        assert_eq!(lowered.extensibility(), Some(ExtensibilityKind::Final));
    }

    #[test]
    fn has_key_false_when_absent() {
        let lowered = Lowered::default();
        assert!(!lowered.has_key());
        assert_eq!(lowered.explicit_id(), None);
        assert_eq!(lowered.extensibility(), None);
    }

    // -----------------------------------------------------------------
    // §8.3.5.1 — @verbatim PlacementKind fully modeled (§8.2 open list)
    // -----------------------------------------------------------------

    #[test]
    fn verbatim_no_params_uses_defaults() {
        let v = lower_one("verbatim", AnnotationParams::None);
        assert_eq!(
            v,
            Some(BuiltinAnnotation::Verbatim(VerbatimSpec {
                language: "*".into(),
                placement: PlacementKind::AfterDeclaration,
                text: String::new(),
            }))
        );
    }

    #[test]
    fn verbatim_compact_form_takes_text() {
        // @verbatim("// comment") — single form interpreted as text.
        let v = lower_one(
            "verbatim",
            AnnotationParams::Single(lit(LiteralKind::String, "\"// hello\"")),
        );
        match v {
            Some(BuiltinAnnotation::Verbatim(spec)) => {
                assert_eq!(spec.language, "*");
                assert_eq!(spec.placement, PlacementKind::AfterDeclaration);
                assert_eq!(spec.text, "// hello");
            }
            other => panic!("expected Verbatim, got {other:?}"),
        }
    }

    #[test]
    fn verbatim_named_params_full() {
        let v = lower_one(
            "verbatim",
            AnnotationParams::Named(alloc::vec![
                NamedParam {
                    name: ident("language"),
                    value: lit(LiteralKind::String, "\"cpp\""),
                    span: sp(),
                },
                NamedParam {
                    name: ident("placement"),
                    value: ConstExpr::Scoped(scoped(&["BEGIN_FILE"])),
                    span: sp(),
                },
                NamedParam {
                    name: ident("text"),
                    value: lit(LiteralKind::String, "\"#include <vector>\""),
                    span: sp(),
                },
            ]),
        );
        match v {
            Some(BuiltinAnnotation::Verbatim(spec)) => {
                assert_eq!(spec.language, "cpp");
                assert_eq!(spec.placement, PlacementKind::BeginFile);
                assert_eq!(spec.text, "#include <vector>");
            }
            other => panic!("expected Verbatim, got {other:?}"),
        }
    }

    #[test]
    fn verbatim_all_placement_kinds() {
        for (ident_str, expected) in [
            ("BEGIN_FILE", PlacementKind::BeginFile),
            ("BEFORE_DECLARATION", PlacementKind::BeforeDeclaration),
            ("BEGIN_DECLARATION", PlacementKind::BeginDeclaration),
            ("END_DECLARATION", PlacementKind::EndDeclaration),
            ("AFTER_DECLARATION", PlacementKind::AfterDeclaration),
            ("END_FILE", PlacementKind::EndFile),
        ] {
            assert_eq!(PlacementKind::from_ident(ident_str), Some(expected));
        }
    }

    // -----------------------------------------------------------------
    // §8.3.6.3 — @ami(boolean default TRUE) (§8.1 Open-List)
    // -----------------------------------------------------------------

    #[test]
    fn ami_no_params_defaults_to_true() {
        let v = lower_one("ami", AnnotationParams::None);
        assert_eq!(v, Some(BuiltinAnnotation::Ami(true)));
    }

    #[test]
    fn ami_empty_parens_defaults_to_true() {
        let v = lower_one("ami", AnnotationParams::Empty);
        assert_eq!(v, Some(BuiltinAnnotation::Ami(true)));
    }

    #[test]
    fn ami_with_true_keyword_lowers_true() {
        let v = lower_one(
            "ami",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["TRUE"]))),
        );
        assert_eq!(v, Some(BuiltinAnnotation::Ami(true)));
    }

    #[test]
    fn ami_with_false_keyword_lowers_false() {
        let v = lower_one(
            "ami",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["FALSE"]))),
        );
        assert_eq!(v, Some(BuiltinAnnotation::Ami(false)));
    }

    #[test]
    fn verbatim_unknown_placement_falls_back_to_default() {
        let v = lower_one(
            "verbatim",
            AnnotationParams::Named(alloc::vec![NamedParam {
                name: ident("placement"),
                value: ConstExpr::Scoped(scoped(&["INVALID_KIND"])),
                span: sp(),
            }]),
        );
        match v {
            Some(BuiltinAnnotation::Verbatim(spec)) => {
                assert_eq!(spec.placement, PlacementKind::AfterDeclaration);
            }
            other => panic!("expected Verbatim, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------
    // Phase 4 — new annotations §8.3.3.2/§8.3.6.1/§8.3.6.2
    // -----------------------------------------------------------------

    fn int_lit(raw: &str) -> ConstExpr {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw: raw.to_string(),
            span: sp(),
        })
    }

    fn str_lit(raw: &str) -> ConstExpr {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::String,
            raw: format!("\"{raw}\""),
            span: sp(),
        })
    }

    // Phase 4.1 — @range

    #[test]
    fn lowers_range_annotation_with_min_max() {
        let v = lower_one(
            "range",
            AnnotationParams::Named(alloc::vec![
                NamedParam {
                    name: ident("min"),
                    value: int_lit("10"),
                    span: sp(),
                },
                NamedParam {
                    name: ident("max"),
                    value: int_lit("20"),
                    span: sp(),
                },
            ]),
        );
        assert_eq!(
            v,
            Some(BuiltinAnnotation::Range {
                min: Some("10".into()),
                max: Some("20".into()),
            })
        );
    }

    // Phase 4.2 — @service

    #[test]
    fn lowers_service_annotation_with_default_platform() {
        let v = lower_one("service", AnnotationParams::None);
        assert_eq!(v, Some(BuiltinAnnotation::Service("*".into())));
    }

    #[test]
    fn lowers_service_annotation_with_platform_string() {
        let v = lower_one("service", AnnotationParams::Single(str_lit("CORBA")));
        assert_eq!(v, Some(BuiltinAnnotation::Service("CORBA".into())));
    }

    #[test]
    fn lowers_service_annotation_with_named_platform() {
        let v = lower_one(
            "service",
            AnnotationParams::Named(alloc::vec![NamedParam {
                name: ident("platform"),
                value: str_lit("DDS"),
                span: sp(),
            }]),
        );
        assert_eq!(v, Some(BuiltinAnnotation::Service("DDS".into())));
    }

    // Phase 4.3 — @oneway (annotation, not keyword)

    #[test]
    fn lowers_oneway_annotation_with_default_true() {
        let v = lower_one("oneway", AnnotationParams::None);
        assert_eq!(v, Some(BuiltinAnnotation::OnewayAnno(true)));
    }

    #[test]
    fn lowers_oneway_annotation_with_false() {
        let v = lower_one(
            "oneway",
            AnnotationParams::Single(ConstExpr::Scoped(scoped(&["FALSE"]))),
        );
        assert_eq!(v, Some(BuiltinAnnotation::OnewayAnno(false)));
    }

    // Phase 4.4 — §8.3.1.4 Position-u16-Range-Validation

    #[test]
    fn position_at_short_max_is_ok() {
        // 65535 is the highest valid value for `unsigned short`.
        let v = lower_single(&ann(
            "position",
            AnnotationParams::Single(lit(LiteralKind::Integer, "65535")),
        ));
        assert_eq!(v, Ok(Some(BuiltinAnnotation::Position(65535))));
    }

    #[test]
    fn position_over_short_max_is_error() {
        // §8.3.1.4: position > 65535 violates the unsigned-short range.
        let v = lower_single(&ann(
            "position",
            AnnotationParams::Single(lit(LiteralKind::Integer, "65536")),
        ));
        assert_eq!(v, Err(LowerError::PositionOutOfShortRange { value: 65536 }));
    }

    // Phase 4.5 — §8.3.3 Range-Inheritance-Through-Typedef

    #[test]
    fn range_annotation_on_typedef_inherited_by_member() {
        // §8.3.3: annotations on a typedef are inherited by members that
        // reference this typedef.
        let ast = parse_to_ast(
            "@range(min=0, max=100)\n\
             typedef long MyInt;\n\
             struct S { MyInt x; };\n",
        );
        let s = find_struct_def(&ast).expect("struct");
        let member = s.members.first().expect("member x");
        let effective = effective_member_annotations(member, &ast);
        let lowered = lower_annotations(&effective).expect("lower ok");
        let has_range = lowered.builtins.iter().any(|b| {
            matches!(
                b,
                BuiltinAnnotation::Range { min, max }
                    if min.as_deref() == Some("0") && max.as_deref() == Some("100")
            )
        });
        assert!(has_range, "expected Range from typedef, got {lowered:?}");
    }

    #[test]
    fn member_annotation_overrides_inherited_typedef_annotation() {
        // On a name conflict between a member and typedef annotation
        // the local member statement wins (spec precedence).
        let ast = parse_to_ast(
            "@range(min=0, max=100)\n\
             typedef long MyInt;\n\
             struct S { @range(min=10, max=20) MyInt x; };\n",
        );
        let s = find_struct_def(&ast).expect("struct");
        let member = s.members.first().expect("member x");
        let effective = effective_member_annotations(member, &ast);
        let lowered = lower_annotations(&effective).expect("lower ok");
        let range_count = lowered
            .builtins
            .iter()
            .filter(|b| matches!(b, BuiltinAnnotation::Range { .. }))
            .count();
        assert_eq!(range_count, 1, "duplicate Range entries: {lowered:?}");
        let has_local = lowered.builtins.iter().any(|b| {
            matches!(
                b,
                BuiltinAnnotation::Range { min, max }
                    if min.as_deref() == Some("10") && max.as_deref() == Some("20")
            )
        });
        assert!(has_local, "member-local Range must win: {lowered:?}");
    }
}

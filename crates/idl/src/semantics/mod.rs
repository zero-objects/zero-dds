// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Semantische Annotation-Lowering + AST-Validierung (WP 1.5 T-IDL1/2).
//!
//! Der Parser produziert generische `Annotation { name, params }`-
//! Knoten. Fuer XTypes-Tests brauchen wir typisierte Varianten (`@key`,
//! `@id(n)`, `@mutable`, ...). Dieses Modul liefert das Lowering.

pub mod annotations;
pub mod bitfield_validation;
pub mod const_eval;
pub mod dynamic_apply;
pub mod fixed_validation;
pub mod map_validation;
pub mod resolver;
pub mod spec_validators;
pub mod to_typeobject;
pub mod union_validation;

pub use annotations::{
    AutoidKind, BuiltinAnnotation, ExtensibilityKind, LowerError, Lowered, lower_annotations,
    lower_type_annotations,
};
pub use bitfield_validation::{BitfieldValidationError, validate_bitfields};
pub use const_eval::{ConstValue, EvalError, Symbol, SymbolTable, concat_strings, evaluate};
pub use dynamic_apply::{MemberApplyReport, TypeApplyReport, apply_to_member, apply_to_type};
pub use fixed_validation::{FixedValidationError, validate_fixed_types};
pub use map_validation::{MapValidationError, validate_maps};
pub use resolver::{
    CaseInsensitiveIdent, ResolvedSymbol, Resolver, ResolverError, Scope, SymbolKind,
};
pub use spec_validators::{SpecValidationError, validate_all as validate_spec_constraints};
pub use to_typeobject::{MapError, lower_struct_to_minimal, map_type_spec};
pub use union_validation::{UnionValidationError, validate_unions};

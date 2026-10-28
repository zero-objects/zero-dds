// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! FlatData and zero-copy properties. Spec §4.5.

/// FlatData information attached to a node. Spec §4.5.
///
/// `is_plain` is recorded rather than derived on the spot because a
/// frontend may assert it — a vendor extension can declare a type plain
/// — and because the derivation walks the whole reachable graph.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlatDataFacet {
    /// Whether the type is plain: fixed size, no indirection, loanable
    /// without serialisation.
    pub is_plain: bool,
    /// Alignment the zero-copy loan path requires.
    pub loan_alignment: Option<u8>,
    /// Whether the frontend asked for fixed-size layout explicitly
    /// rather than the property being inferred.
    pub explicit_fixed_layout: bool,
}

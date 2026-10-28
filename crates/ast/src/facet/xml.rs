// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML provenance. Spec §4.4.

use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::XmlRefKey;

/// DDS-XML information attached to a node. Spec §4.4.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XmlFacet {
    /// The element name the node was parsed from.
    pub element: String,
    /// Attributes as written, in document order.
    pub attributes: Vec<(String, String)>,
    /// `idref` targets that have not been resolved yet.
    pub unresolved_idrefs: Vec<XmlRefKey>,
    /// XSD type the element was validated against.
    pub xsd_type: Option<String>,
    /// Library and profile nesting the element appeared in, outermost
    /// first.
    pub library_path: Vec<String>,
}

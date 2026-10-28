// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Vendor-specific directives. Spec §4.6.
//!
//! Each vendor gets its own struct rather than a shared bag, and vendor
//! directives are never normalised into core constructs: normalising
//! loses exactly what a round-trip back to that vendor's dialect needs.

use alloc::string::String;
use alloc::vec::Vec;

/// Which vendor a facet belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VendorKind {
    /// RTI Connext.
    Rti,
    /// OpenDDS.
    OpenDds,
    /// eProsima Fast DDS.
    FastDds,
    /// Twin Oaks CoreDX.
    CoreDx,
}

impl VendorKind {
    /// Every vendor the model carries a facet for.
    pub const ALL: [Self; 4] = [Self::Rti, Self::OpenDds, Self::FastDds, Self::CoreDx];

    /// Vendor name, for diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rti => "RTI Connext",
            Self::OpenDds => "OpenDDS",
            Self::FastDds => "Fast DDS",
            Self::CoreDx => "CoreDX",
        }
    }
}

/// RTI Connext directives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RtiFacet {
    /// `//@copy` and `//@copy-declaration` text, in declaration order.
    pub copy_directives: Vec<String>,
    /// `//@resolve-name` setting.
    pub resolve_name: Option<bool>,
    /// `//@top-level` setting, which governs whether a type gets its own
    /// support class.
    pub top_level: Option<bool>,
    /// `keylist T (a, b);` / `#pragma keylist T a b` field names, in the
    /// order written.
    ///
    /// Kept as written for a round trip back to the dialect that wrote it.
    /// What the directive means is on the members: lowering also sets
    /// `@key` on each named member, and on each member along a path
    /// `a.b` (`idl-lower`'s `apply_keylists`).
    pub keylist: Vec<String>,
    /// Directives the frontend recognised but the model has no typed
    /// field for. Retained verbatim so a round-trip stays lossless;
    /// never consulted by layout or validation.
    pub unrecognised: Vec<(String, String)>,
}

/// OpenDDS directives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenDdsFacet {
    /// `#pragma DCPS_DATA_TYPE`.
    pub dcps_data_type: bool,
    /// `#pragma DCPS_DATA_KEY` field names, in declaration order.
    pub dcps_data_keys: Vec<String>,
    /// `#pragma DCPS_SUPPORT_ZERO_COPY`.
    pub supports_zero_copy: Option<bool>,
    /// Directives with no typed field. See [`RtiFacet::unrecognised`].
    pub unrecognised: Vec<(String, String)>,
}

/// eProsima Fast DDS directives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FastDdsFacet {
    /// `@eprosima::xtypes` extension flags.
    pub xtypes_extensions: Vec<String>,
    /// Whether the type was declared for the zero-copy data path.
    pub plain_data_type: Option<bool>,
    /// Directives with no typed field. See [`RtiFacet::unrecognised`].
    pub unrecognised: Vec<(String, String)>,
}

/// Twin Oaks CoreDX directives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoreDxFacet {
    /// `//@key` field names declared the CoreDX way.
    pub key_fields: Vec<String>,
    /// Directives with no typed field. See [`RtiFacet::unrecognised`].
    pub unrecognised: Vec<(String, String)>,
}

/// Vendor information attached to a node. Spec §4.6.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VendorFacets {
    /// RTI Connext.
    pub rti: Option<RtiFacet>,
    /// OpenDDS.
    pub open_dds: Option<OpenDdsFacet>,
    /// Fast DDS.
    pub fast_dds: Option<FastDdsFacet>,
    /// CoreDX.
    pub core_dx: Option<CoreDxFacet>,
}

impl VendorFacets {
    /// Whether any vendor facet is present.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rti.is_none()
            && self.open_dds.is_none()
            && self.fast_dds.is_none()
            && self.core_dx.is_none()
    }

    /// Which vendors have a facet on this node.
    #[must_use]
    pub fn present(&self) -> Vec<VendorKind> {
        let mut out = Vec::new();
        if self.rti.is_some() {
            out.push(VendorKind::Rti);
        }
        if self.open_dds.is_some() {
            out.push(VendorKind::OpenDds);
        }
        if self.fast_dds.is_some() {
            out.push(VendorKind::FastDds);
        }
        if self.core_dx.is_some() {
            out.push(VendorKind::CoreDx);
        }
        out
    }
}

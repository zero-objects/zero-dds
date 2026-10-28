// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Per-source facets. Spec §4.
//!
//! This is where the superset is carried without a stringly-typed bag.
//! Every facet is a `struct`; a node from IDL, XTypes, CORBA, ROS 2,
//! DDS-XML or the FlatData model keeps everything its frontend knew,
//! and a node may carry several at once — a ROS 2 message lowered to
//! IDL and bridged to CORBA carries three.
//!
//! An absent facet is `None`, never a defaulted struct: a node with no
//! XTypes information must not be taken to claim `Final` extensibility.

use alloc::vec::Vec;

pub mod applied;
pub mod corba;
pub mod flatdata;
pub mod ros2;
pub mod source;
pub mod vendor;
pub mod xml;
pub mod xtypes;

pub use applied::{AppliedAnnotation, AppliedArgument};
pub use corba::{
    ComponentRole, CorbaFacet, HomeManages, RepositoryIdOrigin, StateVisibility, TemplateArg,
    TemplateParam, TemplateParamKind,
};
pub use flatdata::FlatDataFacet;
pub use ros2::{Ros2Facet, RosBoundedOrigin, RosInterfaceRole};
pub use source::{ByteSpan, MacroExpansion, SourceFacet, SourceOrigin};
pub use vendor::{CoreDxFacet, FastDdsFacet, OpenDdsFacet, RtiFacet, VendorFacets, VendorKind};
pub use xml::XmlFacet;
pub use xtypes::{
    AutoId, DataRepresentationMask, Extensibility, MemberIdAssignment, MemberIdOrigin,
    RangeConstraint, TopicAnnotation, TryConstruct, VerbatimDirective, VerbatimPlacement,
    XTypesFacet,
};

/// Everything a node knows about where it came from and what its source
/// vocabulary said about it. Spec §4.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facets {
    /// Provenance. Always present.
    pub source: SourceFacet,
    /// XTypes annotations.
    pub xtypes: Option<XTypesFacet>,
    /// CORBA and component-model information.
    pub corba: Option<CorbaFacet>,
    /// ROS 2 interface information.
    pub ros2: Option<Ros2Facet>,
    /// DDS-XML provenance.
    pub xml: Option<XmlFacet>,
    /// FlatData and zero-copy properties.
    pub flatdata: Option<FlatDataFacet>,
    /// Vendor-specific directives.
    pub vendor: VendorFacets,
    /// Annotations with no typed field — the ones a user declared with
    /// `@annotation`, in the order they were written.
    ///
    /// The specified annotations are not repeated here: `@key` is
    /// [`XTypesFacet::is_key`] and nothing else, so a consumer never has
    /// to reconcile two answers.
    pub annotations: Vec<AppliedAnnotation>,
}

impl Facets {
    /// Facets for a node created through the model API.
    #[must_use]
    pub fn synthetic(reason: &'static str) -> Self {
        Self {
            source: SourceFacet::synthetic(reason),
            ..Self::default()
        }
    }

    /// Facets for a built-in declaration.
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            source: SourceFacet::builtin(),
            ..Self::default()
        }
    }

    /// The XTypes facet, creating an empty one if the node had none.
    ///
    /// Used by the edit surface when setting an XTypes property; a read
    /// path must use the field directly, so that absence stays visible.
    pub fn xtypes_mut(&mut self) -> &mut XTypesFacet {
        self.xtypes.get_or_insert_with(XTypesFacet::default)
    }

    /// The CORBA facet, creating an empty one if the node had none.
    pub fn corba_mut(&mut self) -> &mut CorbaFacet {
        self.corba.get_or_insert_with(CorbaFacet::default)
    }

    /// The ROS 2 facet, creating an empty one if the node had none.
    pub fn ros2_mut(&mut self) -> &mut Ros2Facet {
        self.ros2.get_or_insert_with(Ros2Facet::default)
    }

    /// The DDS-XML facet, creating an empty one if the node had none.
    pub fn xml_mut(&mut self) -> &mut XmlFacet {
        self.xml.get_or_insert_with(XmlFacet::default)
    }

    /// The FlatData facet, creating an empty one if the node had none.
    pub fn flatdata_mut(&mut self) -> &mut FlatDataFacet {
        self.flatdata.get_or_insert_with(FlatDataFacet::default)
    }

    /// The extensibility declared on this node, if any.
    #[must_use]
    pub fn extensibility(&self) -> Option<Extensibility> {
        self.xtypes.as_ref().and_then(|x| x.extensibility)
    }

    /// Whether this node is annotated `@key`.
    #[must_use]
    pub fn is_key(&self) -> bool {
        self.xtypes.as_ref().is_some_and(|x| x.is_key)
    }

    /// Whether this node is annotated `@optional`.
    #[must_use]
    pub fn is_optional(&self) -> bool {
        self.xtypes.as_ref().is_some_and(|x| x.is_optional)
    }

    /// Whether this node is annotated `@must_understand`.
    #[must_use]
    pub fn is_must_understand(&self) -> bool {
        self.xtypes.as_ref().is_some_and(|x| x.is_must_understand)
    }

    /// The effective member id, if one has been assigned.
    #[must_use]
    pub fn effective_member_id(&self) -> Option<u32> {
        self.xtypes
            .as_ref()
            .and_then(XTypesFacet::effective_member_id)
    }
}

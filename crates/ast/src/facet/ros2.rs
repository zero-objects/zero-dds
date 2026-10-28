// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ROS 2 interface information. Spec §4.3.

use crate::node::constant::ConstExprNode;
use zerodds_ast_core::{Id, RosTypeName};

/// Which part of a ROS 2 interface a declaration belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RosInterfaceRole {
    /// A `.msg` message.
    Msg,
    /// The request half of a `.srv`.
    Request,
    /// The response half of a `.srv`.
    Response,
    /// The goal part of an `.action`.
    Goal,
    /// The result part of an `.action`.
    Result,
    /// The feedback part of an `.action`.
    Feedback,
}

impl RosInterfaceRole {
    /// Every role a ROS 2 interface file produces.
    pub const ALL: [Self; 6] = [
        Self::Msg,
        Self::Request,
        Self::Response,
        Self::Goal,
        Self::Result,
        Self::Feedback,
    ];
}

/// How a bound was written in the ROS 2 source.
///
/// ROS spells bounded arrays and strings differently from IDL, and the
/// difference has to survive a round-trip back to `.msg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RosBoundedOrigin {
    /// `type[<=N]` — bounded array.
    BoundedArray,
    /// `type[N]` — fixed-size array.
    FixedArray,
    /// `string<=N` — bounded string.
    BoundedString,
    /// `type[]` — unbounded array.
    UnboundedArray,
}

/// ROS 2 information attached to a node. Spec §4.3.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ros2Facet {
    /// Default value, which ROS allows on any field and IDL does not.
    pub default_value: Option<Id<ConstExprNode>>,
    /// How the bound was written, when the field is a collection.
    pub bounded_origin: Option<RosBoundedOrigin>,
    /// Which part of a service or action triple this belongs to.
    pub interface_role: Option<RosInterfaceRole>,
    /// The ROS type name, alongside the IDL scoped name.
    pub ros_name: Option<RosTypeName>,
    /// Whether the declaration was a ROS constant rather than a field.
    pub was_constant: bool,
}

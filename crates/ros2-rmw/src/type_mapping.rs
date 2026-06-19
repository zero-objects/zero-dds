// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! REP-2008 Type-System-Mapping — `rosidl` → DDS-XTypes-1.3.
//!
//! Spec REP-2008 §4 + ROS 2 IDL-Subset (`design.ros2.org/articles/
//! idl_interface_definition.html`).
//!
//! ROS-2 IDL is a subset of OMG-IDL 4.2 with additional
//! annotations (`@verbatim`, `@key`, `@default`). The type names
//! are encoded in the DDS wire format with the pattern
//! `<package>::<sub-namespace>::dds_::<TypeName>_`
//! (convention from `rosidl_typesupport_fastrtps_cpp`).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// ROS 2 IDL Sub-Namespace (Spec REP-2008 §3.3 + Convention).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RosNamespace {
    /// `msg/` — Message types.
    Msg,
    /// `srv/` — Service types (Request/Response).
    Srv,
    /// `action/` — Action types (Goal/Result/Feedback).
    Action,
}

impl RosNamespace {
    /// Wire sub-namespace string per the convention.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Msg => "msg",
            Self::Srv => "srv",
            Self::Action => "action",
        }
    }
}

/// ROS-2-Type-Reference (z.B. `geometry_msgs/msg/Twist`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosTypeRef {
    /// ROS-Package-Name.
    pub package: String,
    /// Sub-Namespace.
    pub namespace: RosNamespace,
    /// Type-Name (CamelCase per ROS-Convention).
    pub type_name: String,
}

impl RosTypeRef {
    /// Constructor.
    #[must_use]
    pub fn new(
        package: impl Into<String>,
        namespace: RosNamespace,
        type_name: impl Into<String>,
    ) -> Self {
        Self {
            package: package.into(),
            namespace,
            type_name: type_name.into(),
        }
    }

    /// Render to the ROS 2 slash form (`package/namespace/Type`).
    #[must_use]
    pub fn to_ros_form(&self) -> String {
        format!(
            "{}/{}/{}",
            self.package,
            self.namespace.as_str(),
            self.type_name
        )
    }

    /// Renders to the DDS wire type-name form for Cyclone-DDS / FastDDS
    /// compat: `<package>::<namespace>::dds_::<TypeName>_`. Spec
    /// convention from `rosidl_typesupport_fastrtps_cpp`.
    #[must_use]
    pub fn to_dds_type_name(&self) -> String {
        format!(
            "{}::{}::dds_::{}_",
            self.package,
            self.namespace.as_str(),
            self.type_name
        )
    }

    /// Parses a ROS-2 slash form (`package/namespace/Type`) back.
    ///
    /// # Errors
    /// Static string if not 3 slash-separated segments.
    pub fn from_ros_form(s: &str) -> Result<Self, &'static str> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 3 {
            return Err("expected `package/namespace/Type`");
        }
        let namespace = match parts[1] {
            "msg" => RosNamespace::Msg,
            "srv" => RosNamespace::Srv,
            "action" => RosNamespace::Action,
            _ => return Err("unknown namespace (expected msg/srv/action)"),
        };
        Ok(Self {
            package: parts[0].to_string(),
            namespace,
            type_name: parts[2].to_string(),
        })
    }
}

/// XTypes mapping for a ROS-2 builtin type (rosidl type token).
/// Spec REP-2008 §4.4: ROS builtins ↔ IDL builtins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RosBuiltinType {
    /// `bool` ↔ IDL `boolean`.
    Bool,
    /// `byte` ↔ IDL `octet`.
    Byte,
    /// `char` ↔ IDL `wchar`.
    Char,
    /// `float32` ↔ IDL `float`.
    Float32,
    /// `float64` ↔ IDL `double`.
    Float64,
    /// `int8` ↔ IDL `int8`.
    Int8,
    /// `uint8` ↔ IDL `uint8`.
    Uint8,
    /// `int16` ↔ IDL `int16`.
    Int16,
    /// `uint16` ↔ IDL `uint16`.
    Uint16,
    /// `int32` ↔ IDL `int32`.
    Int32,
    /// `uint32` ↔ IDL `uint32`.
    Uint32,
    /// `int64` ↔ IDL `int64`.
    Int64,
    /// `uint64` ↔ IDL `uint64`.
    Uint64,
    /// `string` ↔ IDL `string` (bounded or unbounded).
    String,
    /// `wstring` ↔ IDL `wstring`.
    WString,
}

impl RosBuiltinType {
    /// IDL name per REP-2008 §4.4 Tab.
    #[must_use]
    pub const fn idl_name(self) -> &'static str {
        match self {
            Self::Bool => "boolean",
            Self::Byte => "octet",
            Self::Char => "wchar",
            Self::Float32 => "float",
            Self::Float64 => "double",
            Self::Int8 => "int8",
            Self::Uint8 => "uint8",
            Self::Int16 => "int16",
            Self::Uint16 => "uint16",
            Self::Int32 => "int32",
            Self::Uint32 => "uint32",
            Self::Int64 => "int64",
            Self::Uint64 => "uint64",
            Self::String => "string",
            Self::WString => "wstring",
        }
    }

    /// Wire-Size in CDR (Spec OMG CDR-2 §10.5).
    #[must_use]
    pub const fn cdr_size(self) -> Option<usize> {
        match self {
            Self::Bool | Self::Byte | Self::Int8 | Self::Uint8 => Some(1),
            Self::Char | Self::Int16 | Self::Uint16 => Some(2),
            Self::Float32 | Self::Int32 | Self::Uint32 => Some(4),
            Self::Float64 | Self::Int64 | Self::Uint64 => Some(8),
            Self::String | Self::WString => None, // variable
        }
    }

    /// Parses a ROS-IDL token string to `RosBuiltinType`.
    ///
    /// # Errors
    /// Static string if unknown.
    pub fn from_ros_token(s: &str) -> Result<Self, &'static str> {
        match s {
            "bool" => Ok(Self::Bool),
            "byte" => Ok(Self::Byte),
            "char" => Ok(Self::Char),
            "float32" => Ok(Self::Float32),
            "float64" => Ok(Self::Float64),
            "int8" => Ok(Self::Int8),
            "uint8" => Ok(Self::Uint8),
            "int16" => Ok(Self::Int16),
            "uint16" => Ok(Self::Uint16),
            "int32" => Ok(Self::Int32),
            "uint32" => Ok(Self::Uint32),
            "int64" => Ok(Self::Int64),
            "uint64" => Ok(Self::Uint64),
            "string" => Ok(Self::String),
            "wstring" => Ok(Self::WString),
            _ => Err("unknown ROS-IDL builtin type"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn ros_form_round_trip() {
        let t = RosTypeRef::new("geometry_msgs", RosNamespace::Msg, "Twist");
        assert_eq!(t.to_ros_form(), "geometry_msgs/msg/Twist");
        let back = RosTypeRef::from_ros_form("geometry_msgs/msg/Twist").unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn dds_wire_form_uses_dds_dunder() {
        let t = RosTypeRef::new("std_msgs", RosNamespace::Msg, "String");
        assert_eq!(t.to_dds_type_name(), "std_msgs::msg::dds_::String_");
    }

    #[test]
    fn srv_namespace_mapped_correctly() {
        let t = RosTypeRef::new("example_interfaces", RosNamespace::Srv, "AddTwoInts");
        assert_eq!(
            t.to_dds_type_name(),
            "example_interfaces::srv::dds_::AddTwoInts_"
        );
    }

    #[test]
    fn action_namespace_mapped_correctly() {
        let t = RosTypeRef::new("action_msgs", RosNamespace::Action, "GoalStatus");
        assert_eq!(
            t.to_dds_type_name(),
            "action_msgs::action::dds_::GoalStatus_"
        );
    }

    #[test]
    fn from_ros_form_rejects_wrong_segment_count() {
        assert!(RosTypeRef::from_ros_form("only_two/parts").is_err());
        assert!(RosTypeRef::from_ros_form("a/b/c/d").is_err());
    }

    #[test]
    fn from_ros_form_rejects_unknown_namespace() {
        assert!(RosTypeRef::from_ros_form("pkg/topic/Type").is_err());
    }

    #[test]
    fn builtin_idl_names_match_omg_idl() {
        // rosidl builtin → OMG IDL 4.2 primitive spellings (DDS-XTypes 1.3
        // §7.2.2.4.1.1 primitive kinds): the `.idl` names DDS sees on the wire.
        assert_eq!(RosBuiltinType::Bool.idl_name(), "boolean");
        assert_eq!(RosBuiltinType::Byte.idl_name(), "octet");
        assert_eq!(RosBuiltinType::Float32.idl_name(), "float");
        assert_eq!(RosBuiltinType::Int64.idl_name(), "int64");
    }

    #[test]
    fn cdr_size_matches_omg_cdr2() {
        assert_eq!(RosBuiltinType::Bool.cdr_size(), Some(1));
        assert_eq!(RosBuiltinType::Int16.cdr_size(), Some(2));
        assert_eq!(RosBuiltinType::Float64.cdr_size(), Some(8));
        assert!(RosBuiltinType::String.cdr_size().is_none());
    }

    #[test]
    fn from_ros_token_round_trip() {
        for (t, token) in [
            (RosBuiltinType::Bool, "bool"),
            (RosBuiltinType::Byte, "byte"),
            (RosBuiltinType::Float32, "float32"),
            (RosBuiltinType::Int64, "int64"),
            (RosBuiltinType::String, "string"),
        ] {
            assert_eq!(RosBuiltinType::from_ros_token(token).unwrap(), t);
        }
    }

    #[test]
    fn from_ros_token_rejects_unknown() {
        assert!(RosBuiltinType::from_ros_token("complex").is_err());
    }

    #[test]
    fn namespace_str_repr() {
        assert_eq!(RosNamespace::Msg.as_str(), "msg");
        assert_eq!(RosNamespace::Srv.as_str(), "srv");
        assert_eq!(RosNamespace::Action.as_str(), "action");
    }
}

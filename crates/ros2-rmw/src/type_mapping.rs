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

/// ROS-2-Type-Reference (e.g. `geometry_msgs/msg/Twist`).
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
    /// `char` ↔ IDL `uint8` (REP-2008 §4.4: ROS `char` is a deprecated
    /// alias for an unsigned 8-bit integer, 1 byte — NOT a 2-byte wide
    /// char; verified against native rclpy/Cyclone+FastDDS: `char{65}`
    /// serializes to a single byte `0x41`).
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
            Self::Char => "uint8",
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
            Self::Bool | Self::Byte | Self::Int8 | Self::Uint8 | Self::Char => Some(1),
            Self::Int16 | Self::Uint16 => Some(2),
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
    //! The oracle: the design article "Interface definition and language
    //! mapping" (design.ros2.org/articles/idl_interface_definition.html)
    //! and the type names rosidl_typesupport_fastrtps_cpp and
    //! rmw_cyclonedds_cpp put on the wire, `<package>::<msg|srv|action>::
    //! dds_::<Type>_`.
    use super::*;

    #[test]
    fn ros2_idl_a_type_is_package_namespace_name_and_dds_names_it_dds_underscore() {
        for (ns, word, ros, dds) in [
            (
                RosNamespace::Msg,
                "msg",
                "std_msgs/msg/String",
                "std_msgs::msg::dds_::String_",
            ),
            (
                RosNamespace::Srv,
                "srv",
                "example_interfaces/srv/AddTwoInts",
                "example_interfaces::srv::dds_::AddTwoInts_",
            ),
            (
                RosNamespace::Action,
                "action",
                "action_msgs/action/GoalStatus",
                "action_msgs::action::dds_::GoalStatus_",
            ),
        ] {
            let (package, rest) = ros.split_once('/').unwrap();
            let name = rest.rsplit('/').next().unwrap();
            let t = RosTypeRef::new(package, ns, name);
            assert_eq!(ns.as_str(), word);
            assert_eq!(t.to_ros_form(), ros);
            assert_eq!(t.to_dds_type_name(), dds);
            assert_eq!(RosTypeRef::from_ros_form(ros).unwrap(), t);
        }
    }

    #[test]
    fn ros2_idl_a_name_without_three_parts_or_with_another_namespace_is_refused() {
        assert!(RosTypeRef::from_ros_form("only_two/parts").is_err());
        assert!(RosTypeRef::from_ros_form("a/b/c/d").is_err());
        assert!(RosTypeRef::from_ros_form("pkg/topic/Type").is_err());
    }

    /// The design article's table of the built-in types and the IDL types
    /// they map to; `char` is the 8-bit unsigned type (REP-2008-era
    /// rosidl, measured against rclpy's octets).
    #[test]
    fn ros2_idl_the_builtin_types_map_to_their_idl_types() {
        for (token, t, idl, size) in [
            ("bool", RosBuiltinType::Bool, "boolean", Some(1)),
            ("byte", RosBuiltinType::Byte, "octet", Some(1)),
            ("char", RosBuiltinType::Char, "uint8", Some(1)),
            ("float32", RosBuiltinType::Float32, "float", Some(4)),
            ("float64", RosBuiltinType::Float64, "double", Some(8)),
            ("int8", RosBuiltinType::Int8, "int8", Some(1)),
            ("uint8", RosBuiltinType::Uint8, "uint8", Some(1)),
            ("int16", RosBuiltinType::Int16, "int16", Some(2)),
            ("uint16", RosBuiltinType::Uint16, "uint16", Some(2)),
            ("int32", RosBuiltinType::Int32, "int32", Some(4)),
            ("uint32", RosBuiltinType::Uint32, "uint32", Some(4)),
            ("int64", RosBuiltinType::Int64, "int64", Some(8)),
            ("uint64", RosBuiltinType::Uint64, "uint64", Some(8)),
            ("string", RosBuiltinType::String, "string", None),
            ("wstring", RosBuiltinType::WString, "wstring", None),
        ] {
            assert_eq!(RosBuiltinType::from_ros_token(token).unwrap(), t);
            assert_eq!(t.idl_name(), idl, "{token}");
            assert_eq!(t.cdr_size(), size, "{token}");
        }
        assert!(RosBuiltinType::from_ros_token("complex").is_err());
    }
}

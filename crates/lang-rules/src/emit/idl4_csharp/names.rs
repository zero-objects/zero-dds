// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Names: the two naming schemes of IDL4 to C# Language Mapping 1.0
//! §7.1.1, the transformations of §7.1.1.2.1 and §7.1.1.2.2, Table 8.1,
//! and the reserved names of §7.1.2.

pub use crate::emit::idl4_java::names::{camel_case, pascal_case};

/// The naming scheme in force, §7.1.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Naming {
    /// §7.1.1.1: names as they are in IDL.
    #[default]
    Idl,
    /// §7.1.1.2: names after the .NET Framework Design Guidelines, Table
    /// 8.1.
    DotNet,
}

impl Naming {
    /// The name the command line gives the scheme.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idl => "idl",
            Self::DotNet => "dotnet",
        }
    }

    /// The scheme a command-line name or an `apply_naming_convention`
    /// value selects.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "idl" | "IDL_NAMING_CONVENTION" => Some(Self::Idl),
            "dotnet" | "DOTNET_NAMING_CONVENTION" => Some(Self::DotNet),
            _ => None,
        }
    }
}

/// The keywords of C#, ECMA-334 6th edition §7.4.4, which §7.1.2
/// reserves.
pub const CSHARP_KEYWORDS: [&str; 77] = [
    "abstract",
    "as",
    "base",
    "bool",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "checked",
    "class",
    "const",
    "continue",
    "decimal",
    "default",
    "delegate",
    "do",
    "double",
    "else",
    "enum",
    "event",
    "explicit",
    "extern",
    "false",
    "finally",
    "fixed",
    "float",
    "for",
    "foreach",
    "goto",
    "if",
    "implicit",
    "in",
    "int",
    "interface",
    "internal",
    "is",
    "lock",
    "long",
    "namespace",
    "new",
    "null",
    "object",
    "operator",
    "out",
    "override",
    "params",
    "private",
    "protected",
    "public",
    "readonly",
    "ref",
    "return",
    "sbyte",
    "sealed",
    "short",
    "sizeof",
    "stackalloc",
    "static",
    "string",
    "struct",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "uint",
    "ulong",
    "unchecked",
    "unsafe",
    "ushort",
    "using",
    "virtual",
    "void",
    "volatile",
    "while",
];

/// The class of the Constants Container Mapping, §7.2.3.2, which §7.1.2
/// reserves in every namespace.
pub const CONSTANTS_CLASS: &str = "Constants";

/// §7.1.1: "the collision shall be resolved by prepending the "@"
/// character to the mapped name when the name collides with a C#
/// language keyword".
#[must_use]
pub fn escape(mapped: String) -> String {
    if CSHARP_KEYWORDS.contains(&mapped.as_str()) {
        format!("@{mapped}")
    } else {
        mapped
    }
}

/// A type name: a keyword takes `@`, `Constants` takes `_` (§7.1.2: "a
/// prepended "_" character for other conflicts").
#[must_use]
pub fn escape_type(mapped: String) -> String {
    if mapped == CONSTANTS_CLASS {
        format!("_{mapped}")
    } else {
        escape(mapped)
    }
}

/// The names one IDL name takes under a scheme, Table 8.1.
impl Naming {
    fn apply(self, name: &str, dotnet: fn(&str) -> String) -> String {
        match self {
            Self::Idl => name.to_string(),
            Self::DotNet => dotnet(name),
        }
    }

    /// A module: a namespace segment (Pascal Case).
    #[must_use]
    pub fn namespace(self, name: &str) -> String {
        escape(self.apply(name, pascal_case))
    }

    /// A structure, union, enumeration, exception, bit set or value type
    /// (Pascal Case).
    #[must_use]
    pub fn type_name(self, name: &str) -> String {
        escape_type(self.apply(name, pascal_case))
    }

    /// An interface: `I` and its name (Pascal Case), Table 8.1.
    #[must_use]
    pub fn interface_name(self, name: &str) -> String {
        format!("I{}", self.apply(name, pascal_case))
    }

    /// A bit mask's enum: its name and `Flags` (§7.14.3.3).
    #[must_use]
    pub fn bitmask_name(self, name: &str) -> String {
        format!("{}Flags", self.apply(name, pascal_case))
    }

    /// An annotation: its name, and `Attribute` under the .NET scheme
    /// (§7.16.1).
    #[must_use]
    pub fn annotation_name(self, name: &str) -> String {
        match self {
            Self::Idl => escape_type(name.to_string()),
            Self::DotNet => format!("{}Attribute", pascal_case(name)),
        }
    }

    /// A constant's class or a constant in a container (Pascal Case).
    #[must_use]
    pub fn constant(self, name: &str) -> String {
        escape_type(self.apply(name, pascal_case))
    }

    /// A member, attribute or bit field property, an enumerator, an
    /// operation (Pascal Case).
    #[must_use]
    pub fn member(self, name: &str) -> String {
        escape(self.apply(name, pascal_case))
    }

    /// A parameter of an operation, of a constructor or of a modifier
    /// method (Camel Case).
    #[must_use]
    pub fn parameter(self, name: &str) -> String {
        escape(self.apply(name, camel_case))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Table 8.1, the clause's examples of §7.2.2, §7.2.3.1 and §7.4.
    #[test]
    fn table_8_1_gives_each_construct_its_name() {
        assert_eq!(Naming::Idl.namespace("my_math"), "my_math");
        assert_eq!(Naming::DotNet.namespace("my_math"), "MyMath");
        assert_eq!(Naming::DotNet.constant("e"), "E");
        assert_eq!(Naming::DotNet.constant("my_string"), "MyString");
        assert_eq!(Naming::Idl.interface_name("AnInterface"), "IAnInterface");
        assert_eq!(Naming::DotNet.member("ro_attr"), "RoAttr");
        assert_eq!(Naming::DotNet.parameter("i_param"), "iParam");
        assert_eq!(Naming::Idl.bitmask_name("MyBitMask"), "MyBitMaskFlags");
        assert_eq!(
            Naming::DotNet.annotation_name("MyAnnotation"),
            "MyAnnotationAttribute"
        );
        assert_eq!(Naming::Idl.annotation_name("MyAnnotation"), "MyAnnotation");
    }

    /// §7.1.2: a keyword takes `@`, `Constants` takes `_`.
    #[test]
    fn reserved_names_take_their_prefix() {
        assert_eq!(Naming::Idl.member("class"), "@class");
        assert_eq!(Naming::Idl.parameter("out"), "@out");
        assert_eq!(Naming::Idl.type_name("Constants"), "_Constants");
        assert_eq!(Naming::DotNet.type_name("object"), "Object");
        assert_eq!(Naming::Idl.type_name("object"), "@object");
    }
}

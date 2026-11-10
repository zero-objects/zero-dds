// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The annotations the IDL4 language mappings define for themselves.
//!
//! IDL4 to Java 1.0 §8.1 defines `@java_mapping`, IDL4 to C# 1.0 §8.1
//! `@csharp_mapping` and IDL4 to C++ 1.0 §8.1 `@cpp_mapping`.
//! `@rust_mapping` is ZeroDDS's own, shaped like the three: no IDL4 to
//! Rust mapping is adopted, and the Rust generator's naming scheme is
//! chosen the way the Java and C# ones are. Each
//! steers one language's generator — a naming scheme, the class constants
//! are collected in, the width of an unsigned integer — and none changes
//! the type: the model keeps the settings as written, and the generator of
//! the language reads them.

use alloc::string::String;

/// Whose annotation a setting comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MappingLanguage {
    /// `@java_mapping`, IDL4 to Java 1.0 §8.1.
    Java,
    /// `@csharp_mapping`, IDL4 to C# 1.0 §8.1.
    CSharp,
    /// `@cpp_mapping`, IDL4 to C++ 1.0 §8.1.
    Cpp,
    /// `@rust_mapping`, ZeroDDS's annotation for its Rust generator.
    Rust,
}

impl MappingLanguage {
    /// The annotation's name.
    #[must_use]
    pub const fn annotation(self) -> &'static str {
        match self {
            Self::Java => "java_mapping",
            Self::CSharp => "csharp_mapping",
            Self::Cpp => "cpp_mapping",
            Self::Rust => "rust_mapping",
        }
    }

    /// The language whose annotation `name` is.
    #[must_use]
    pub fn of_annotation(name: &str) -> Option<Self> {
        match name {
            "java_mapping" => Some(Self::Java),
            "csharp_mapping" => Some(Self::CSharp),
            "cpp_mapping" => Some(Self::Cpp),
            "rust_mapping" => Some(Self::Rust),
            _ => None,
        }
    }

    /// The tag a snapshot stores.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Java => 0,
            Self::CSharp => 1,
            Self::Cpp => 2,
            Self::Rust => 3,
        }
    }

    /// The language of a snapshot tag.
    #[must_use]
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Java),
            1 => Some(Self::CSharp),
            2 => Some(Self::Cpp),
            3 => Some(Self::Rust),
            _ => None,
        }
    }
}

/// The value one parameter was given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingValue {
    /// An enumerator of the annotation's own enumeration, such as
    /// `JAVA_NAMING_CONVENTION`.
    Name(String),
    /// A string, such as the name of a constants container.
    Text(String),
    /// A boolean, such as `promote_integer_width`.
    Flag(bool),
}

/// One parameter of one language-mapping annotation on a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingSetting {
    /// Whose annotation.
    pub language: MappingLanguage,
    /// The parameter, as the annotation's declaration names it.
    pub parameter: String,
    /// What it was given.
    pub value: MappingValue,
}

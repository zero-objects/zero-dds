// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `XmlError` — error enum for the DDS-XML loader.
//!
//! Spec references see the doc comment per variant.

use alloc::string::String;
use core::fmt;

/// Error while parsing or resolving a DDS-XML document.
///
/// Spec source: OMG DDS-XML 1.0 §7.1 (XML Representation Syntax) and
/// §7.2 (XML Representation of DDS IDL PSM).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlError {
    /// XML is not well-formed per [XML] §2.1.
    ///
    /// Spec ref: DDS-XML 1.0 §7.1.1 ("XML shall be well-formed").
    InvalidXml(String),

    /// An element mandatory per spec table 7.2 / 7.3.x is missing.
    ///
    /// Spec ref: DDS-XML 1.0 §7.1.4 Tab.7.1, §7.2.x.
    MissingRequiredElement(String),

    /// An element not present in the element table 7.1/7.2/7.3
    /// was found. Strict mode rejects; lax mode ignores.
    ///
    /// Spec ref: DDS-XML 1.0 §7.1.4 (element value table).
    UnknownElement(String),

    /// Enum string does not fit the DCPS-IDL whitelist
    /// (Spec §7.1.4 Tab.7.1 — `enum` values are string literals, *not*
    /// numeric).
    ///
    /// Spec ref: DDS-XML 1.0 §7.1.4 Tab.7.1 (enum), §7.2.1.
    BadEnum(String),

    /// `base_name` inheritance forms a cycle (A inherits from B inherits from A).
    ///
    /// Spec ref: DDS-XML 1.0 §7.3.2.4.2 (QoS Profile Inheritance —
    /// "shall only inherit from previously defined profiles"). Naive
    /// implementations can create cycles across library boundaries;
    /// the loader therefore performs DAG checking.
    CircularInheritance(String),

    /// Element value outside the spec value range (e.g. `long` >
    /// `0x7fffffff` without symbol aliasing).
    ///
    /// Spec ref: DDS-XML 1.0 §7.1.4 Tab.7.1 (value ranges), §7.2.2
    /// (`LENGTH_UNLIMITED`, `DURATION_INFINITE_*`).
    ValueOutOfRange(String),

    /// DoS cap hit — list/string exceeds the upper bound
    /// configured in the loader (default: 1024 list elements, 64 KiB
    /// strings).
    ///
    /// No direct spec reference; follows the ZeroDDS security posture
    /// (`docs/spec-coverage/zerodds-xml-1.0.open.md` risks section).
    LimitExceeded(String),

    /// A cross-reference (e.g. `base_name` of a QoS profile
    /// inheritance) could not be resolved because the referenced
    /// item does not exist.
    ///
    /// Spec ref: DDS-XML 1.0 §7.3.2.4.2 (QoS profile inheritance).
    UnresolvedReference(String),
}

impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidXml(msg) => write!(f, "invalid XML: {msg}"),
            Self::MissingRequiredElement(name) => {
                write!(f, "missing required element <{name}>")
            }
            Self::UnknownElement(name) => write!(f, "unknown element <{name}>"),
            Self::BadEnum(value) => write!(f, "invalid enum value `{value}`"),
            Self::CircularInheritance(chain) => {
                write!(f, "circular base_name inheritance: {chain}")
            }
            Self::ValueOutOfRange(msg) => write!(f, "value out of range: {msg}"),
            Self::LimitExceeded(msg) => write!(f, "DoS limit exceeded: {msg}"),
            Self::UnresolvedReference(name) => write!(f, "unresolved reference `{name}`"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for XmlError {}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn display_invalid_xml() {
        let e = XmlError::InvalidXml("unexpected token".into());
        assert_eq!(e.to_string(), "invalid XML: unexpected token");
    }

    #[test]
    fn display_missing_required() {
        let e = XmlError::MissingRequiredElement("qos_profile".into());
        assert_eq!(e.to_string(), "missing required element <qos_profile>");
    }

    #[test]
    fn display_unknown_element() {
        let e = XmlError::UnknownElement("foo".into());
        assert_eq!(e.to_string(), "unknown element <foo>");
    }

    #[test]
    fn display_bad_enum() {
        let e = XmlError::BadEnum("WRONG".into());
        assert_eq!(e.to_string(), "invalid enum value `WRONG`");
    }

    #[test]
    fn display_circular() {
        let e = XmlError::CircularInheritance("A -> B -> A".into());
        assert_eq!(e.to_string(), "circular base_name inheritance: A -> B -> A");
    }

    #[test]
    fn display_value_out_of_range() {
        let e = XmlError::ValueOutOfRange("long > 0x7fffffff".into());
        assert_eq!(e.to_string(), "value out of range: long > 0x7fffffff");
    }

    #[test]
    fn display_limit_exceeded() {
        let e = XmlError::LimitExceeded("seq > 1024".into());
        assert_eq!(e.to_string(), "DoS limit exceeded: seq > 1024");
    }

    #[test]
    fn equality_and_clone() {
        let a = XmlError::InvalidXml("x".into());
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn display_unresolved_reference() {
        let e = XmlError::UnresolvedReference("MissingProfile".into());
        assert_eq!(e.to_string(), "unresolved reference `MissingProfile`");
    }
}

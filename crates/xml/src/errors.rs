// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `XmlError` — Fehler-Enum fuer den DDS-XML-Loader.
//!
//! Spec-Referenzen siehe Doc-Comment pro Variante.

use alloc::string::String;
use core::fmt;

/// Fehler beim Parsen oder Aufloesen eines DDS-XML-Dokuments.
///
/// Spec-Quelle: OMG DDS-XML 1.0 §7.1 (XML Representation Syntax) und
/// §7.2 (XML Representation of DDS IDL PSM).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlError {
    /// XML ist nicht wohlgeformt gemaess [XML] §2.1.
    ///
    /// Spec-Ref: DDS-XML 1.0 §7.1.1 ("XML shall be well-formed").
    InvalidXml(String),

    /// Ein gemaess Spec-Tabelle 7.2 / 7.3.x verpflichtendes Element fehlt.
    ///
    /// Spec-Ref: DDS-XML 1.0 §7.1.4 Tab.7.1, §7.2.x.
    MissingRequiredElement(String),

    /// Ein Element, das nicht in der Element-Tabelle 7.1/7.2/7.3 steht,
    /// wurde gefunden. Strict-Modus rejected; Lax-Modus ignoriert.
    ///
    /// Spec-Ref: DDS-XML 1.0 §7.1.4 (Element-Werte-Tabelle).
    UnknownElement(String),

    /// Enum-String passt nicht in die DCPS-IDL-Whitelist
    /// (Spec §7.1.4 Tab.7.1 — `enum`-Werte sind String-Literale, *nicht*
    /// numerisch).
    ///
    /// Spec-Ref: DDS-XML 1.0 §7.1.4 Tab.7.1 (enum), §7.2.1.
    BadEnum(String),

    /// `base_name`-Inheritance bildet einen Zyklus (A erbt von B erbt von A).
    ///
    /// Spec-Ref: DDS-XML 1.0 §7.3.2.4.2 (QoS Profile Inheritance —
    /// "shall only inherit from previously defined profiles"). Naive
    /// Implementierungen koennen Zyklen ueber Bibliotheks-Grenzen
    /// erzeugen; der Loader fuehrt darum DAG-Pruefung durch.
    CircularInheritance(String),

    /// Element-Wert ausserhalb des Spec-Wertebereichs (z.B. `long` >
    /// `0x7fffffff` ohne Symbol-Aliasing).
    ///
    /// Spec-Ref: DDS-XML 1.0 §7.1.4 Tab.7.1 (Wertebereiche), §7.2.2
    /// (`LENGTH_UNLIMITED`, `DURATION_INFINITE_*`).
    ValueOutOfRange(String),

    /// DoS-Cap getroffen — Liste/String ueberschreitet die im Loader
    /// konfigurierte Obergrenze (Default: 1024 Listen-Elemente, 64 KiB
    /// Strings).
    ///
    /// Kein direkter Spec-Bezug; folgt der ZeroDDS-Security-Posture
    /// (`docs/spec-coverage/zerodds-xml-1.0.open.md` Risiken-Abschnitt).
    LimitExceeded(String),

    /// Eine Cross-Reference (z.B. `base_name` einer QoS-Profile-
    /// Inheritance) konnte nicht aufgeloest werden, weil das referenzierte
    /// Item nicht existiert.
    ///
    /// Spec-Ref: DDS-XML 1.0 §7.3.2.4.2 (QoS-Profile-Inheritance).
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

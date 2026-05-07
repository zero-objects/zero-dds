// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Cross-Library-Reference-Resolver fuer DDS-XML 1.0 §7.3.4-7.3.6.
//!
//! Mehrere Building-Blocks (Domain, DomainParticipant, Application,
//! QoS-Profile) erlauben qualifizierte Verweise der Form
//! `library::name`. Dieses Modul stellt die generische Pfad-Zerlegung +
//! Lookup-Logik bereit, damit die einzelnen Decoder konsistent denselben
//! Format-Vertrag erfuellen.
//!
//! Das DDS-XML 1.0 Spec-Beispiel in Annex C verwendet konsequent das
//! 2-Segment-Format (`my_lib::MyDomain`), und die OMG-Schema-Definition in
//! §7.3 erlaubt diese Form als XSD-`token`-Attribut. Single-Segment-Refs
//! (`MyDomain`) sind in der Spec nicht explizit verboten — wir lassen sie
//! als Convenience-Form zu, falls das uebergebene Default-Library-Argument
//! Some(_) ist.

use alloc::format;
use alloc::string::{String, ToString};

use crate::errors::XmlError;

/// Aufgeloestes 2-Segment-Library-Verweis-Tupel `(library, name)`.
///
/// `library = ""` heisst "kein Library-Prefix"; in diesem Fall muss der
/// Aufrufer einen Default-Library-Scope mitliefern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRef {
    /// Name der Library (`""` wenn nicht qualifiziert).
    pub library: String,
    /// Name des Items innerhalb der Library.
    pub name: String,
}

impl LibraryRef {
    /// `true` wenn der Verweis qualifiziert ist (`library::name`).
    #[must_use]
    pub fn is_qualified(&self) -> bool {
        !self.library.is_empty()
    }
}

/// Zerlegt einen Verweis-String in `(library, name)`.
///
/// Akzeptierte Formen:
/// 1. `"library::name"` — qualifiziert.
/// 2. `"name"` — unqualifiziert (`library` leer).
///
/// # Errors
/// * [`XmlError::UnresolvedReference`] — leerer String, oder `"::name"`,
///   oder `"library::"`, oder mehr als zwei `::`-Segmente.
pub fn parse_library_ref(s: &str) -> Result<LibraryRef, XmlError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(XmlError::UnresolvedReference("empty reference".into()));
    }
    if let Some((lib, rest)) = trimmed.split_once("::") {
        if lib.is_empty() {
            return Err(XmlError::UnresolvedReference(format!(
                "empty library segment in `{trimmed}`"
            )));
        }
        if rest.contains("::") {
            return Err(XmlError::UnresolvedReference(format!(
                "more than two segments in `{trimmed}`"
            )));
        }
        if rest.is_empty() {
            return Err(XmlError::UnresolvedReference(format!(
                "empty name segment in `{trimmed}`"
            )));
        }
        Ok(LibraryRef {
            library: lib.to_string(),
            name: rest.to_string(),
        })
    } else {
        Ok(LibraryRef {
            library: String::new(),
            name: trimmed.to_string(),
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn qualified() {
        let r = parse_library_ref("lib::name").expect("ok");
        assert_eq!(r.library, "lib");
        assert_eq!(r.name, "name");
        assert!(r.is_qualified());
    }

    #[test]
    fn unqualified() {
        let r = parse_library_ref("name").expect("ok");
        assert_eq!(r.library, "");
        assert_eq!(r.name, "name");
        assert!(!r.is_qualified());
    }

    #[test]
    fn empty_rejected() {
        assert!(matches!(
            parse_library_ref(""),
            Err(XmlError::UnresolvedReference(_))
        ));
        assert!(matches!(
            parse_library_ref("   "),
            Err(XmlError::UnresolvedReference(_))
        ));
    }

    #[test]
    fn empty_segment_rejected() {
        assert!(matches!(
            parse_library_ref("::name"),
            Err(XmlError::UnresolvedReference(_))
        ));
        assert!(matches!(
            parse_library_ref("lib::"),
            Err(XmlError::UnresolvedReference(_))
        ));
    }

    #[test]
    fn three_segments_rejected() {
        assert!(matches!(
            parse_library_ref("a::b::c"),
            Err(XmlError::UnresolvedReference(_))
        ));
    }
}

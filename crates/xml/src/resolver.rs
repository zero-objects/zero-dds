// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Cross-library reference resolver for DDS-XML 1.0 §7.3.4-7.3.6.
//!
//! Several building blocks (domain, domain participant, application,
//! QoS profiles) allow qualified references of the form
//! `library::name`. This module provides the generic path splitting +
//! lookup logic so that the individual decoders consistently fulfill the
//! same format contract.
//!
//! The DDS-XML 1.0 spec example in Annex C consistently uses the
//! 2-segment format (`my_lib::MyDomain`), and the OMG schema definition in
//! §7.3 allows this form as an XSD `token` attribute. Single-segment refs
//! (`MyDomain`) are not explicitly forbidden in the spec — we allow them
//! as a convenience form if the supplied default library argument
//! is Some(_).

use alloc::format;
use alloc::string::{String, ToString};

use crate::errors::XmlError;

/// Resolved 2-segment library reference tuple `(library, name)`.
///
/// `library = ""` means "no library prefix"; in this case the
/// caller must supply a default library scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRef {
    /// Name of the library (`""` if not qualified).
    pub library: String,
    /// Name of the item within the library.
    pub name: String,
}

impl LibraryRef {
    /// `true` if the reference is qualified (`library::name`).
    #[must_use]
    pub fn is_qualified(&self) -> bool {
        !self.library.is_empty()
    }
}

/// Splits a reference string into `(library, name)`.
///
/// Accepted forms:
/// 1. `"library::name"` — qualified.
/// 2. `"name"` — unqualified (`library` empty).
///
/// # Errors
/// * [`XmlError::UnresolvedReference`] — empty string, or `"::name"`,
///   or `"library::"`, or more than two `::` segments.
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

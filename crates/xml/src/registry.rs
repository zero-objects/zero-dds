// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! QoS profile registry — DDS-XML 1.0 §7.3 profile resolution.
//!
//! Loads one or more `<qos_library>`/`<qos_profile>` definitions from XML
//! (RTI / Cyclone / FastDDS style) and resolves a profile reference
//! `"Library::Profile"` (or unqualified `"Profile"` against the first
//! library) under **full `base_name` inheritance** (§7.3.2.4.2) into a
//! materialized [`zerodds_qos::WriterQos`] / [`zerodds_qos::ReaderQos`].
//!
//! This makes the documented migration path real: an existing
//! RTI/Cyclone/FastDDS QoS XML is read and applied by profile name to
//! entity creation — without translating the settings into Rust code.
//!
//! ```ignore
//! let reg = QosProfileRegistry::from_xml(xml)?;
//! let qos = reg.writer_qos("MyLib::HighPerf")?; // zerodds_qos::WriterQos
//! // then: publisher.create_datawriter(&topic, qos.into())  (see zerodds-dcps From)
//! ```

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_qos::{ReaderQos, WriterQos};

use crate::errors::XmlError;
use crate::inheritance::resolve_chain;
use crate::qos::{EntityQos, QosLibrary, QosProfile};
use crate::qos_parser::parse_qos_libraries;
use crate::resolver::parse_library_ref;

/// An in-memory set of DDS-XML QoS libraries, resolvable by `"Lib::Profile"`.
#[derive(Debug, Clone, Default)]
pub struct QosProfileRegistry {
    libraries: Vec<QosLibrary>,
}

impl QosProfileRegistry {
    /// Parses all `<qos_library>` definitions from a `<dds>` document.
    ///
    /// # Errors
    /// [`XmlError`] on malformed XML or an unexpected root element.
    pub fn from_xml(xml: &str) -> Result<Self, XmlError> {
        Ok(Self {
            libraries: parse_qos_libraries(xml)?,
        })
    }

    /// Reads + parses a QoS-profile XML file. `std` only.
    ///
    /// # Errors
    /// I/O error or [`XmlError`] on a malformed document.
    #[cfg(feature = "std")]
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, XmlError> {
        let xml = std::fs::read_to_string(path)
            .map_err(|e| XmlError::InvalidXml(alloc::format!("cannot read profile file: {e}")))?;
        Self::from_xml(&xml)
    }

    /// Number of loaded libraries.
    #[must_use]
    pub fn library_count(&self) -> usize {
        self.libraries.len()
    }

    /// Resolves `"Lib::Profile"` (or unqualified `"Profile"` → first library) to
    /// a [`WriterQos`], applying the full `base_name` inheritance chain.
    ///
    /// # Errors
    /// [`XmlError::UnresolvedReference`] (unknown library/profile) /
    /// [`XmlError::CircularInheritance`] / [`XmlError::InvalidXml`].
    pub fn writer_qos(&self, profile_ref: &str) -> Result<WriterQos, XmlError> {
        Ok(self
            .resolve(profile_ref, |p| p.datawriter_qos.as_ref())?
            .into_writer_qos())
    }

    /// Like [`Self::writer_qos`] but materializes a [`ReaderQos`].
    ///
    /// # Errors
    /// As [`Self::writer_qos`].
    pub fn reader_qos(&self, profile_ref: &str) -> Result<ReaderQos, XmlError> {
        Ok(self
            .resolve(profile_ref, |p| p.datareader_qos.as_ref())?
            .into_reader_qos())
    }

    /// Resolves a profile's `<datawriter_qos>`/`<datareader_qos>` container
    /// (selected by `pick`) with full inheritance, base→derived.
    fn resolve<F>(&self, profile_ref: &str, pick: F) -> Result<EntityQos, XmlError>
    where
        F: Fn(&QosProfile) -> Option<&EntityQos>,
    {
        // Resolve the starting profile to its actual (library, profile) so the
        // inheritance chain is built from qualified names (cross-library bases).
        let r = parse_library_ref(profile_ref)?;
        let (start_lib, _) = self.find(&r.library, &r.name)?;
        let start = qualify(start_lib, &r.name);

        // Chain (derived → base) with cycle detection from `resolve_chain`.
        let chain = resolve_chain(&start, |qname| {
            let lr = parse_library_ref(qname)?;
            let (lib, prof) = self.find(&lr.library, &lr.name)?;
            // A simple base_name resolves within the same library; a qualified
            // `Lib::Base` keeps its library.
            Ok(prof.base_name.as_ref().map(|b| {
                let br = parse_library_ref(b).unwrap_or(crate::resolver::LibraryRef {
                    library: String::new(),
                    name: b.clone(),
                });
                if br.is_qualified() {
                    b.clone()
                } else {
                    qualify(lib, &br.name)
                }
            }))
        })?;

        // `resolve_chain` already returns the chain most-base-first, so we fold
        // in order: each more-derived profile overrides its base where set.
        let mut acc = EntityQos::default();
        for qname in &chain {
            let lr = parse_library_ref(qname)?;
            let (_, prof) = self.find(&lr.library, &lr.name)?;
            if let Some(eq) = pick(prof) {
                acc = acc.merge(eq);
            }
        }
        Ok(acc)
    }

    /// Looks up `(actual_library_name, profile)` for a (possibly empty) library
    /// name + profile name. An empty library name selects the first library.
    fn find(&self, lib_name: &str, prof_name: &str) -> Result<(&str, &QosProfile), XmlError> {
        let lib = if lib_name.is_empty() {
            self.libraries.first()
        } else {
            self.libraries.iter().find(|l| l.name == lib_name)
        }
        .ok_or_else(|| XmlError::UnresolvedReference(qualify(lib_name, prof_name)))?;
        let prof = lib
            .profile(prof_name)
            .ok_or_else(|| XmlError::UnresolvedReference(qualify(&lib.name, prof_name)))?;
        Ok((lib.name.as_str(), prof))
    }
}

/// `Lib::Name` (or just `Name` when `lib` is empty).
fn qualify(lib: &str, name: &str) -> String {
    if lib.is_empty() {
        name.to_string()
    } else {
        let mut s = String::with_capacity(lib.len() + 2 + name.len());
        s.push_str(lib);
        s.push_str("::");
        s.push_str(name);
        s
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_qos::{HistoryKind, ReliabilityKind};

    // RTI / FastDDS style: a Base profile + a Derived profile that inherits it
    // and overrides only the history depth.
    const XML: &str = r#"
<dds>
  <qos_library name="MyLib">
    <qos_profile name="Base">
      <datawriter_qos>
        <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
        <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>64</depth></history>
      </datawriter_qos>
      <datareader_qos>
        <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
      </datareader_qos>
    </qos_profile>
    <qos_profile name="Derived" base_name="Base">
      <datawriter_qos>
        <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>128</depth></history>
      </datawriter_qos>
    </qos_profile>
  </qos_library>
</dds>
"#;

    #[test]
    fn resolves_base_writer_qos() {
        let reg = QosProfileRegistry::from_xml(XML).expect("parse");
        assert_eq!(reg.library_count(), 1);
        let q = reg.writer_qos("MyLib::Base").expect("base");
        assert_eq!(q.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(q.history.kind, HistoryKind::KeepLast);
        assert_eq!(q.history.depth, 64);
    }

    #[test]
    fn inheritance_derived_overrides_base() {
        let reg = QosProfileRegistry::from_xml(XML).expect("parse");
        let q = reg.writer_qos("MyLib::Derived").expect("derived");
        // reliability inherited from Base, history depth overridden by Derived.
        assert_eq!(q.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(q.history.depth, 128);
    }

    #[test]
    fn unqualified_ref_uses_first_library() {
        let reg = QosProfileRegistry::from_xml(XML).expect("parse");
        let q = reg.writer_qos("Base").expect("unqualified");
        assert_eq!(q.history.depth, 64);
    }

    #[test]
    fn reader_qos_resolves() {
        let reg = QosProfileRegistry::from_xml(XML).expect("parse");
        let q = reg.reader_qos("MyLib::Base").expect("reader");
        assert_eq!(q.reliability.kind, ReliabilityKind::Reliable);
    }

    #[test]
    fn missing_profile_is_unresolved_reference() {
        let reg = QosProfileRegistry::from_xml(XML).expect("parse");
        match reg.writer_qos("MyLib::Nope") {
            Err(XmlError::UnresolvedReference(_)) => {}
            other => panic!("expected UnresolvedReference, got {other:?}"),
        }
    }
}

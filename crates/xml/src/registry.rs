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
use crate::options::{Parsed, ReadOptions};
use crate::qos::{
    EntityQos, FilteredEntityQos, QosLibrary, QosProfile, default_qos, select_qos_for_topic,
};
use crate::qos_inheritance::{ResolvedQos, resolve_profile};
use crate::qos_parser::{parse_qos_libraries, parse_qos_libraries_with};
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

    /// As [`Self::from_xml`], with reading options: under
    /// [`UnknownElements::ReportAndSkip`](crate::options::UnknownElements)
    /// what the QoS reader does not model (an RTI file's own elements) is
    /// left out and returned as [`Parsed::diagnostics`].
    ///
    /// # Errors
    /// As [`Self::from_xml`].
    pub fn from_xml_with(xml: &str, options: &ReadOptions) -> Result<Parsed<Self>, XmlError> {
        let parsed = parse_qos_libraries_with(xml, options)?;
        Ok(Parsed {
            value: Self::from_libraries(parsed.value),
            diagnostics: parsed.diagnostics,
        })
    }

    /// A registry over libraries already parsed.
    #[must_use]
    pub fn from_libraries(libraries: Vec<QosLibrary>) -> Self {
        Self { libraries }
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

    /// As [`Self::from_file`], with reading options (see
    /// [`Self::from_xml_with`]).
    ///
    /// # Errors
    /// As [`Self::from_file`].
    #[cfg(feature = "std")]
    pub fn from_file_with<P: AsRef<std::path::Path>>(
        path: P,
        options: &ReadOptions,
    ) -> Result<Parsed<Self>, XmlError> {
        let xml = std::fs::read_to_string(path)
            .map_err(|e| XmlError::InvalidXml(alloc::format!("cannot read profile file: {e}")))?;
        Self::from_xml_with(&xml, options)
    }

    /// Number of loaded libraries.
    #[must_use]
    pub fn library_count(&self) -> usize {
        self.libraries.len()
    }

    /// Resolves `"Lib::Profile"` (or unqualified `"Profile"` → first library) to
    /// a [`WriterQos`], applying the full `base_name` inheritance chain.
    ///
    /// No topic name is known here, so the selection of §7.3.2.4.3 takes
    /// the container without a `topic_filter`, else the first one.
    ///
    /// # Errors
    /// [`XmlError::UnresolvedReference`] (unknown library/profile) /
    /// [`XmlError::CircularInheritance`] / [`XmlError::InvalidXml`].
    pub fn writer_qos(&self, profile_ref: &str) -> Result<WriterQos, XmlError> {
        let slots = self.resolve(profile_ref, |p| p.datawriter_qos.as_slice())?;
        Ok(default_qos(&slots)
            .cloned()
            .unwrap_or_default()
            .into_writer_qos())
    }

    /// Like [`Self::writer_qos`], but selects the container by the
    /// `topic_filter` attribute of DDS-XML 1.0 §7.3.2.4.3 for
    /// `topic_name`: explicit filters in order, else the filterless
    /// container (the assumed "*").
    ///
    /// # Errors
    /// As [`Self::writer_qos`], plus [`XmlError::UnresolvedReference`]
    /// when no container of the chain matches `topic_name`.
    pub fn writer_qos_for_topic(
        &self,
        profile_ref: &str,
        topic_name: &str,
    ) -> Result<WriterQos, XmlError> {
        let slots = self.resolve(profile_ref, |p| p.datawriter_qos.as_slice())?;
        select_qos_for_topic(&slots, topic_name)
            .ok_or_else(|| {
                XmlError::UnresolvedReference(alloc::format!(
                    "datawriter_qos of profile `{profile_ref}` matching topic `{topic_name}`"
                ))
            })
            .map(|qos| qos.into_writer_qos())
    }

    /// Like [`Self::writer_qos`] but materializes a [`ReaderQos`].
    ///
    /// # Errors
    /// As [`Self::writer_qos`].
    pub fn reader_qos(&self, profile_ref: &str) -> Result<ReaderQos, XmlError> {
        let slots = self.resolve(profile_ref, |p| p.datareader_qos.as_slice())?;
        Ok(default_qos(&slots)
            .cloned()
            .unwrap_or_default()
            .into_reader_qos())
    }

    /// Like [`Self::reader_qos`], but selects the container by
    /// `topic_name` per §7.3.2.4.3.
    ///
    /// # Errors
    /// As [`Self::writer_qos_for_topic`].
    pub fn reader_qos_for_topic(
        &self,
        profile_ref: &str,
        topic_name: &str,
    ) -> Result<ReaderQos, XmlError> {
        let slots = self.resolve(profile_ref, |p| p.datareader_qos.as_slice())?;
        select_qos_for_topic(&slots, topic_name)
            .ok_or_else(|| {
                XmlError::UnresolvedReference(alloc::format!(
                    "datareader_qos of profile `{profile_ref}` matching topic `{topic_name}`"
                ))
            })
            .map(|qos| qos.into_reader_qos())
    }

    /// Materializes an inline `<datawriter_qos>` (§7.3.5.4.4) as a
    /// [`WriterQos`]. When the container names a `base_name`, the matching
    /// `<datawriter_qos>` of that profile is the base — selected for
    /// `topic_name` per §7.3.2.4.3 — and the inline policies override it.
    ///
    /// # Errors
    /// As [`Self::writer_qos`] for the `base_name` profile, plus
    /// [`XmlError::UnresolvedReference`] when none of its containers
    /// matches `topic_name`.
    pub fn writer_qos_for_inline(
        &self,
        inline: &EntityQos,
        topic_name: &str,
    ) -> Result<WriterQos, XmlError> {
        Ok(self
            .inline_over_base(inline, topic_name, |p| p.datawriter_qos.as_slice())?
            .into_writer_qos())
    }

    /// The [`ReaderQos`] counterpart of [`Self::writer_qos_for_inline`].
    ///
    /// # Errors
    /// See [`Self::writer_qos_for_inline`].
    pub fn reader_qos_for_inline(
        &self,
        inline: &EntityQos,
        topic_name: &str,
    ) -> Result<ReaderQos, XmlError> {
        Ok(self
            .inline_over_base(inline, topic_name, |p| p.datareader_qos.as_slice())?
            .into_reader_qos())
    }

    fn inline_over_base<F>(
        &self,
        inline: &EntityQos,
        topic_name: &str,
        pick: F,
    ) -> Result<EntityQos, XmlError>
    where
        F: Fn(&ResolvedQos) -> &[FilteredEntityQos],
    {
        let Some(base) = inline.base_name.as_deref() else {
            return Ok(inline.clone());
        };
        let slots = self.resolve(base, pick)?;
        let base_qos = select_qos_for_topic(&slots, topic_name).ok_or_else(|| {
            XmlError::UnresolvedReference(alloc::format!(
                "container of profile `{base}` matching topic `{topic_name}`"
            ))
        })?;
        let mut merged = base_qos.clone().merge(inline);
        merged.base_name = None;
        Ok(merged)
    }

    /// Resolves a profile's containers of one kind (selected by `pick`)
    /// with full inheritance, base→derived: containers with the same
    /// `topic_filter` merge per policy, new filters are appended. The
    /// resolution is [`resolve_profile`]'s, so the `base_name` chain, the
    /// order of definition and a container's own `base_name` count as
    /// there.
    fn resolve<F>(&self, profile_ref: &str, pick: F) -> Result<Vec<FilteredEntityQos>, XmlError>
    where
        F: Fn(&ResolvedQos) -> &[FilteredEntityQos],
    {
        // Resolve the starting profile to its actual (library, profile): an
        // unqualified reference selects the first library.
        let r = parse_library_ref(profile_ref)?;
        let (start_lib, _) = self.find(&r.library, &r.name)?;
        let start = qualify(start_lib, &r.name);
        Ok(pick(&resolve_profile(&self.libraries, &start)?).to_vec())
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

    /// §7.3.2.4.2: the base must be defined before the inheriting profile.
    #[test]
    fn forward_base_name_rejected() {
        let xml = r#"<dds><qos_library name="L">
            <qos_profile name="Child" base_name="Base"/>
            <qos_profile name="Base"/>
        </qos_library></dds>"#;
        let reg = QosProfileRegistry::from_xml(xml).expect("parse");
        let err = reg.writer_qos("L::Child").expect_err("forward");
        assert!(matches!(err, XmlError::InvalidXml(_)), "got {err:?}");
    }

    #[test]
    fn missing_profile_is_unresolved_reference() {
        let reg = QosProfileRegistry::from_xml(XML).expect("parse");
        match reg.writer_qos("MyLib::Nope") {
            Err(XmlError::UnresolvedReference(_)) => {}
            other => panic!("expected UnresolvedReference, got {other:?}"),
        }
    }

    /// §7.3.2.4.3: the topic name selects between the profile's repeated
    /// containers; explicit filters take precedence over the filterless
    /// one.
    #[test]
    fn writer_qos_for_topic_selects_by_filter() {
        let xml = r#"<dds><qos_library name="L">
            <qos_profile name="P">
                <datawriter_qos topic_filter="A*">
                    <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
                </datawriter_qos>
                <datawriter_qos>
                    <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>7</depth></history>
                </datawriter_qos>
            </qos_profile>
        </qos_library></dds>"#;
        let reg = QosProfileRegistry::from_xml(xml).expect("parse");
        let a = reg.writer_qos_for_topic("L::P", "A1").expect("A*");
        assert_eq!(a.history.kind, HistoryKind::KeepAll);
        let other = reg.writer_qos_for_topic("L::P", "B").expect("fallback");
        assert_eq!(other.history.kind, HistoryKind::KeepLast);
        assert_eq!(other.history.depth, 7);
    }

    /// §7.3.5.4.4: an inline container inherits through `base_name`; the
    /// inline policies override the base's.
    #[test]
    fn inline_container_inherits_base_name_profile() {
        let reg = QosProfileRegistry::from_xml(XML).expect("parse");
        let xml = r#"<datawriter_qos base_name="MyLib::Base">
            <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>5</depth></history>
        </datawriter_qos>"#;
        let doc = crate::parser::parse_xml_tree(xml).expect("xml");
        let inline = crate::qos_parser::parse_entity_qos_public(&doc.root).expect("inline");
        let q = reg.writer_qos_for_inline(&inline, "T").expect("resolve");
        assert_eq!(q.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(q.history.depth, 5);
        // Without a base_name the container stands for itself.
        let plain = EntityQos::default();
        let q = reg.writer_qos_for_inline(&plain, "T").expect("plain");
        assert_eq!(q.reliability, WriterQos::default().reliability);
        // An unknown base is an error, not a dropped base.
        let missing = EntityQos {
            base_name: Some("MyLib::Nope".into()),
            ..EntityQos::default()
        };
        assert!(matches!(
            reg.reader_qos_for_inline(&missing, "T"),
            Err(XmlError::UnresolvedReference(_))
        ));
    }

    /// §7.3.2.4.3: with only filtered containers, a topic matching
    /// none of them has no QoS — that is an error, not a silent
    /// default.
    #[test]
    fn writer_qos_for_topic_without_match_errors() {
        let xml = r#"<dds><qos_library name="L">
            <qos_profile name="P">
                <datawriter_qos topic_filter="A*">
                    <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
                </datawriter_qos>
            </qos_profile>
        </qos_library></dds>"#;
        let reg = QosProfileRegistry::from_xml(xml).expect("parse");
        let err = reg.writer_qos_for_topic("L::P", "B").expect_err("no match");
        assert!(
            matches!(err, XmlError::UnresolvedReference(_)),
            "got {err:?}"
        );
    }
}

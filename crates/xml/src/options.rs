// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Reading options and diagnostics of the DDS-XML readers.
//!
//! The readers follow the schemas of the specification: an element or an
//! attribute the schema does not define is an error that names it
//! ([`UnknownElements::Refuse`], the default). A document written for
//! another tool — an RTI `USER_QOS_PROFILES.xml` carries elements of its
//! own — can be read with [`UnknownElements::ReportAndSkip`]: what the
//! reader does not model is left out of the result and each omission comes
//! back as a [`Diagnostic`] with the path of the element. Nothing is left
//! out without a diagnostic.
//!
//! The second setting is [`VendorForms`]. Other DDS products write a few
//! things differently from the schemas (a `<topic_filter>` child element, an
//! `appendable` extensibility, the IDL names `long` and `octet` in `type`,
//! and others). The default reading refuses such a form with an error that
//! names it and the form the specification uses;
//! [`VendorForms::Accept`] reads it and adds a [`Diagnostic`] of kind
//! [`DiagnosticKind::VendorForm`] for each one. The two settings are
//! independent: [`UnknownElements::ReportAndSkip`] does not accept vendor
//! forms, and accepting them does not skip unknown elements.

use alloc::string::String;
use alloc::vec::Vec;

/// What the readers do with an element or attribute the specification's
/// schema does not define at that place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnknownElements {
    /// Stop with [`crate::XmlError::UnknownElement`] naming the path.
    #[default]
    Refuse,
    /// Leave the element (with everything below it) or the attribute out
    /// of the result and add a [`Diagnostic`] for it.
    ReportAndSkip,
}

/// What the readers do with a form that is not in the specification's
/// schemas but that other DDS products write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VendorForms {
    /// Stop with [`crate::XmlError::VendorForm`], which names the form and
    /// the specification's form.
    #[default]
    Refuse,
    /// Read the form and add a [`Diagnostic`] of kind
    /// [`DiagnosticKind::VendorForm`] for it.
    Accept,
}

/// Options of the `*_with` loader functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReadOptions {
    /// See [`UnknownElements`].
    pub unknown: UnknownElements,
    /// See [`VendorForms`].
    pub vendor_forms: VendorForms,
}

impl ReadOptions {
    /// The reading that skips what it does not model and reports it.
    #[must_use]
    pub const fn report_and_skip() -> Self {
        Self {
            unknown: UnknownElements::ReportAndSkip,
            vendor_forms: VendorForms::Refuse,
        }
    }

    /// The reading that accepts the vendor forms and reports each one.
    #[must_use]
    pub const fn accept_vendor_forms() -> Self {
        Self {
            unknown: UnknownElements::Refuse,
            vendor_forms: VendorForms::Accept,
        }
    }

    /// These options with [`VendorForms::Accept`].
    #[must_use]
    pub const fn with_vendor_forms(mut self) -> Self {
        self.vendor_forms = VendorForms::Accept;
        self
    }
}

/// What a [`Diagnostic`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// An element the schema does not define at that place.
    UnknownElement,
    /// An element that is defined, but not in this kind of container
    /// (for example `ownership_strength` in a `datareader_qos`).
    MisplacedElement,
    /// An attribute the schema does not define on that element.
    UnknownAttribute,
    /// A form other DDS products write, read under
    /// [`VendorForms::Accept`].
    VendorForm,
}

/// One thing the reader left out of its result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Path of the element from the document root, with the `name`
    /// attribute in brackets where there is one:
    /// `dds/qos_library[L]/qos_profile[P]/datawriter_qos/publish_mode`.
    /// For an attribute, the path of its element plus `@name`.
    pub path: String,
    /// What was left out.
    pub kind: DiagnosticKind,
    /// Why, in words.
    pub message: String,
}

impl core::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// A result together with the diagnostics of the reading that made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed<T> {
    /// The result.
    pub value: T,
    /// What the reading left out or read as a vendor form; empty under the
    /// default options.
    pub diagnostics: Vec<Diagnostic>,
}

/// State of one reading.
#[derive(Debug, Default)]
pub(crate) struct Ctx {
    options: ReadOptions,
    diagnostics: Vec<Diagnostic>,
}

impl Ctx {
    pub(crate) fn new(options: ReadOptions) -> Self {
        Self {
            options,
            diagnostics: Vec::new(),
        }
    }

    pub(crate) fn finish<T>(self, value: T) -> Parsed<T> {
        Parsed {
            value,
            diagnostics: self.diagnostics,
        }
    }

    /// A child element the schema does not define in `parent`.
    ///
    /// # Errors
    /// [`crate::XmlError::UnknownElement`] under [`UnknownElements::Refuse`].
    pub(crate) fn unknown_child(
        &mut self,
        child: &crate::parser::XmlElement,
    ) -> Result<(), crate::XmlError> {
        self.omit(
            &child.path,
            DiagnosticKind::UnknownElement,
            "unknown element",
        )
    }

    /// A child element that exists, but not in this container.
    ///
    /// # Errors
    /// [`crate::XmlError::UnknownElement`] under [`UnknownElements::Refuse`].
    pub(crate) fn misplaced_child(
        &mut self,
        child: &crate::parser::XmlElement,
        reason: &str,
    ) -> Result<(), crate::XmlError> {
        self.omit(&child.path, DiagnosticKind::MisplacedElement, reason)
    }

    /// Checks the attributes of `el` against the ones its schema defines.
    ///
    /// # Errors
    /// [`crate::XmlError::UnknownElement`] under [`UnknownElements::Refuse`].
    pub(crate) fn check_attributes(
        &mut self,
        el: &crate::parser::XmlElement,
        allowed: &[&str],
    ) -> Result<(), crate::XmlError> {
        for name in el.attributes.keys() {
            if !allowed.contains(&name.as_str()) {
                let path = alloc::format!("{}@{name}", el.path);
                self.omit(&path, DiagnosticKind::UnknownAttribute, "unknown attribute")?;
            }
        }
        Ok(())
    }

    /// A vendor form found at `path`: `found` is what the document has,
    /// `spec_form` what DDS-XML 1.0 writes.
    ///
    /// # Errors
    /// [`crate::XmlError::VendorForm`] under [`VendorForms::Refuse`].
    pub(crate) fn vendor_form(
        &mut self,
        path: &str,
        found: &str,
        spec_form: &str,
    ) -> Result<(), crate::XmlError> {
        match self.options.vendor_forms {
            VendorForms::Refuse => Err(crate::XmlError::VendorForm(alloc::format!(
                "{path}: {found} is not a DDS-XML 1.0 form; the specification writes \
                 {spec_form} (ReadOptions::vendor_forms accepts the vendor form)"
            ))),
            VendorForms::Accept => {
                self.diagnostics.push(Diagnostic {
                    path: String::from(path),
                    kind: DiagnosticKind::VendorForm,
                    message: alloc::format!(
                        "{found} is a vendor form, read as given; the specification writes \
                         {spec_form}"
                    ),
                });
                Ok(())
            }
        }
    }

    fn omit(
        &mut self,
        path: &str,
        kind: DiagnosticKind,
        message: &str,
    ) -> Result<(), crate::XmlError> {
        match self.options.unknown {
            UnknownElements::Refuse => Err(crate::XmlError::UnknownElement(alloc::format!(
                "{path}: {message}"
            ))),
            UnknownElements::ReportAndSkip => {
                self.diagnostics.push(Diagnostic {
                    path: String::from(path),
                    kind,
                    message: String::from(message),
                });
                Ok(())
            }
        }
    }
}

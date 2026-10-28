// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Source provenance. Spec §4.7.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A byte range in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ByteSpan {
    /// Inclusive start offset.
    pub start: u32,
    /// Exclusive end offset.
    pub end: u32,
}

/// One step of preprocessor expansion.
///
/// The chain lets a diagnostic point at the macro that produced a
/// construct as well as at the place the construct textually appears.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroExpansion {
    /// Macro name.
    pub name: String,
    /// Where the macro was invoked, as a range of the file in `path`.
    pub invoked_at: Option<ByteSpan>,
    /// The file the invocation is written in, as the frontend saw it: an
    /// inner expansion of a chain sits in the body of the macro around
    /// it, which may come from another file than the node.
    pub path: Option<String>,
}

/// Where a node came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceOrigin {
    /// Parsed from a file.
    File {
        /// Path as the frontend saw it.
        path: String,
    },
    /// Created programmatically, with no source text.
    Synthetic {
        /// Why it was synthesised.
        ///
        /// Owned rather than `&'static str`: a snapshot that replaced
        /// every reason with one fixed sentence would answer "where did
        /// this come from" with the loader's answer, not the model's.
        reason: String,
    },
    /// Part of the built-in set the model always carries.
    Builtin,
}

/// Provenance, always present on every node.
///
/// A node created programmatically records a synthetic origin rather
/// than none, so "where did this come from" always has an answer.
/// Spec §4.7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFacet {
    /// Where the node came from.
    pub origin: SourceOrigin,
    /// Byte range in the source file, when there is one.
    pub span: Option<ByteSpan>,
    /// Preprocessor expansions that produced this construct, outermost
    /// first.
    pub macro_chain: Vec<MacroExpansion>,
    /// Documentation comments attached to the declaration.
    pub doc_comments: Vec<String>,
}

impl SourceFacet {
    /// Provenance for a programmatically created node.
    #[must_use]
    pub fn synthetic(reason: &str) -> Self {
        Self {
            origin: SourceOrigin::Synthetic {
                reason: reason.to_string(),
            },
            span: None,
            macro_chain: Vec::new(),
            doc_comments: Vec::new(),
        }
    }

    /// Provenance for a built-in declaration.
    #[must_use]
    pub const fn builtin() -> Self {
        Self {
            origin: SourceOrigin::Builtin,
            span: None,
            macro_chain: Vec::new(),
            doc_comments: Vec::new(),
        }
    }

    /// Provenance for a node parsed from a file.
    #[must_use]
    pub fn file(path: String, span: Option<ByteSpan>) -> Self {
        Self {
            origin: SourceOrigin::File { path },
            span,
            macro_chain: Vec::new(),
            doc_comments: Vec::new(),
        }
    }

    /// Whether the node came from parsed source text.
    #[must_use]
    pub const fn is_from_file(&self) -> bool {
        matches!(self.origin, SourceOrigin::File { .. })
    }
}

impl Default for SourceFacet {
    fn default() -> Self {
        Self::synthetic("created through the model API")
    }
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Macro definitions and their use sites. Spec §3.4.
//!
//! Both kinds ISO/IEC 14882:2003 §16.3 defines: object-like, and
//! function-like with a parameter list. The replacement list is held as
//! preprocessing tokens, which is the form §16.3 states its rules in.

use super::pptoken::{Piece, is_identifier, tokenize};
use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_idl_cst::Span;

/// One place a macro was used.
///
/// Recorded per expanded token so a diagnostic inside an expansion can
/// name the macro as well as the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroUse {
    /// Which macro.
    pub name: String,
    /// Where it was written.
    pub at: Span,
    /// Which file that was, as a raw id.
    pub file: u32,
}

/// One macro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Macro {
    /// The parameters of a function-like macro; `None` for an
    /// object-like one. `Some(vec![])` is `F()`, which is function-like
    /// with no parameter.
    pub params: Option<Vec<String>>,
    /// The replacement list.
    pub(crate) body: Vec<Piece>,
}

/// The names §16.3/8 and §16.8/4 put out of reach of `#define`:
/// `defined`, and the predefined macros this preprocessor answers.
pub const RESERVED: [&str; 3] = ["defined", "__LINE__", "__FILE__"];

/// The macros in scope.
#[derive(Debug, Clone, Default)]
pub struct MacroTable {
    entries: Vec<(String, Macro)>,
}

impl MacroTable {
    /// An empty table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Define `name` as the object-like macro `body`, replacing any
    /// previous definition.
    ///
    /// # Errors
    /// A replacement list §16.3.3/1 forbids, or one that does not
    /// tokenize, in one sentence.
    pub fn define(&mut self, name: &str, body: &str) -> Result<(), String> {
        self.insert(name, None, body)
    }

    /// Define `name` as a function-like macro.
    ///
    /// # Errors
    /// As [`Self::define`], and a parameter list §16.3/4 forbids — a
    /// name that is not an identifier, or one written twice — or a `#`
    /// not followed by a parameter (§16.3.2/1).
    pub fn define_function(
        &mut self,
        name: &str,
        params: Vec<String>,
        body: &str,
    ) -> Result<(), String> {
        for (index, param) in params.iter().enumerate() {
            if !is_identifier(param) {
                return Err(format!("`{param}` is not a parameter name"));
            }
            if params[..index].contains(param) {
                return Err(format!("parameter `{param}` is written twice"));
            }
        }
        self.insert(name, Some(params), body)
    }

    fn insert(
        &mut self,
        name: &str,
        params: Option<Vec<String>>,
        body: &str,
    ) -> Result<(), String> {
        if RESERVED.contains(&name) {
            return Err(format!("`{name}` cannot be defined"));
        }
        if !is_identifier(name) {
            return Err(format!("`{name}` is not a macro name"));
        }
        let body = tokenize(body)?;
        // §16.3.3/1: "A ## preprocessing token shall not occur at the
        // beginning or at the end of a replacement list".
        if body.first().is_some_and(|p| p.text == "##")
            || body.last().is_some_and(|p| p.text == "##")
        {
            return Err("`##` at either end of a replacement list has nothing to paste".to_owned());
        }
        // §16.3.2/1: in a function-like macro, "each # preprocessing
        // token in the replacement list shall be followed by a parameter".
        if let Some(params) = &params {
            for (index, piece) in body.iter().enumerate() {
                if piece.text == "#"
                    && !body
                        .get(index + 1)
                        .is_some_and(|next| params.contains(&next.text))
                {
                    return Err(
                        "`#` in a function-like macro is not followed by a parameter".to_owned(),
                    );
                }
            }
        }
        self.undefine(name);
        self.entries.push((name.to_owned(), Macro { params, body }));
        Ok(())
    }

    /// Remove `name`, if it was defined.
    pub fn undefine(&mut self, name: &str) {
        self.entries.retain(|(defined, _)| defined != name);
    }

    /// Whether `name` is defined, whatever its body.
    ///
    /// An include guard defines a name with an empty body, so emptiness
    /// is not absence.
    #[must_use]
    pub fn is_defined(&self, name: &str) -> bool {
        self.entries.iter().any(|(defined, _)| defined == name)
    }

    /// The macro called `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Macro> {
        self.entries
            .iter()
            .find(|(defined, _)| defined == name)
            .map(|(_, found)| found)
    }

    /// How many macros are defined.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether none are.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

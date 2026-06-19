// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Stub code-generation helper — spec Annex-A.1 operation mapping.
//!
//! A **stub** is the client-side proxy that marshals an IDL operation
//! into a GIOP request, sends it over IIOP, and demarshals the reply.
//! We provide a language-agnostic data structure that the codegen
//! processes per target language.

use alloc::string::String;
use alloc::vec::Vec;

use crate::special_types::TargetLanguage;

/// Parameter direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamDirection {
    /// `in`.
    In,
    /// `out`.
    Out,
    /// `inout`.
    InOut,
}

/// Operation parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubParam {
    /// IDL name.
    pub name: String,
    /// IDL type spec (caller representation).
    pub type_spec: String,
    /// Direction.
    pub direction: ParamDirection,
}

/// Operation definition for stub generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubOp {
    /// Operation name.
    pub name: String,
    /// Return-type spec (`void` if none).
    pub return_type: String,
    /// Parameters.
    pub params: Vec<StubParam>,
    /// `oneway` (spec §7.4.16) — no reply expected.
    pub oneway: bool,
}

/// Renders a textual stub-skeleton preamble with the concrete
/// language names. This is a **helper** for tests and documentation;
/// the real codegen happens in `crates/idl-cpp/` etc., which consumes
/// this data structure.
#[must_use]
pub fn render_stub_op(op: &StubOp, lang: TargetLanguage) -> String {
    let lang_label = match lang {
        TargetLanguage::Cpp => "C++",
        TargetLanguage::CSharp => "C#",
        TargetLanguage::Java => "Java",
        TargetLanguage::Rust => "Rust",
    };
    let oneway_marker = if op.oneway { "[oneway]" } else { "" };
    let mut s = alloc::format!("// {lang_label} stub for {oneway_marker}{}", op.name);
    s.push_str(&alloc::format!("\n//   return: {}", op.return_type));
    for p in &op.params {
        let dir = match p.direction {
            ParamDirection::In => "in",
            ParamDirection::Out => "out",
            ParamDirection::InOut => "inout",
        };
        s.push_str(&alloc::format!("\n//   {dir} {} {}", p.type_spec, p.name));
    }
    s
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn echo_op() -> StubOp {
        StubOp {
            name: "echo".into(),
            return_type: "string".into(),
            params: alloc::vec![StubParam {
                name: "msg".into(),
                type_spec: "string".into(),
                direction: ParamDirection::In,
            }],
            oneway: false,
        }
    }

    #[test]
    fn render_stub_lists_params_per_language() {
        let s = render_stub_op(&echo_op(), TargetLanguage::Cpp);
        assert!(s.contains("C++ stub"));
        assert!(s.contains("in string msg"));
    }

    #[test]
    fn oneway_marker_present_for_oneway_ops() {
        let mut op = echo_op();
        op.oneway = true;
        op.return_type = "void".into();
        let s = render_stub_op(&op, TargetLanguage::Java);
        assert!(s.contains("[oneway]"));
    }

    #[test]
    fn out_and_inout_params_render() {
        let op = StubOp {
            name: "swap".into(),
            return_type: "void".into(),
            params: alloc::vec![
                StubParam {
                    name: "a".into(),
                    type_spec: "long".into(),
                    direction: ParamDirection::InOut,
                },
                StubParam {
                    name: "b".into(),
                    type_spec: "long".into(),
                    direction: ParamDirection::Out,
                },
            ],
            oneway: false,
        };
        let s = render_stub_op(&op, TargetLanguage::CSharp);
        assert!(s.contains("inout long a"));
        assert!(s.contains("out long b"));
    }
}

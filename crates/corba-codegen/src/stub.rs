// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Stub-Code-Generation-Helper — Spec Annex-A.1 Operation-Mapping.
//!
//! Ein **Stub** ist der Client-Side-Proxy, der eine IDL-Operation
//! in einen GIOP-Request marshalt, ueber IIOP sendet und die Reply
//! demarshalt. Wir liefern eine Sprache-agnostische Datenstruktur,
//! die das Codegen pro Ziel-Sprache verarbeitet.

use alloc::string::String;
use alloc::vec::Vec;

use crate::special_types::TargetLanguage;

/// Parameter-Direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamDirection {
    /// `in`.
    In,
    /// `out`.
    Out,
    /// `inout`.
    InOut,
}

/// Operation-Parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubParam {
    /// IDL-Name.
    pub name: String,
    /// IDL-Type-Spec (Caller-Repraesentation).
    pub type_spec: String,
    /// Direction.
    pub direction: ParamDirection,
}

/// Operation-Definition fuer Stub-Generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubOp {
    /// Operation-Name.
    pub name: String,
    /// Return-Type-Spec (`void` wenn keine).
    pub return_type: String,
    /// Parameter.
    pub params: Vec<StubParam>,
    /// `oneway` (Spec §7.4.16) — keine Reply erwartet.
    pub oneway: bool,
}

/// Rendert eine textuelle Stub-Skeleton-Praeambel mit den
/// konkreten Sprach-Namen. Das ist ein **Helper** fuer Tests
/// und Dokumentation; das richtige Codegen passiert in
/// `crates/idl-cpp/`/etc., das diese Datenstruktur konsumiert.
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

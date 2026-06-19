// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Skeleton code-generation helper — server-side dispatch.
//!
//! A **skeleton** dispatches a GIOP request to the concrete servant
//! method based on the operation name and marshals the reply.

use alloc::string::String;

use crate::special_types::TargetLanguage;

/// An operation that the skeleton must dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonOp {
    /// Operation name (wire form).
    pub name: String,
    /// Servant method name (language form, e.g. `getStock` in Java).
    pub method_name: String,
}

/// Renders an operation-name switch for the server skeleton.
///
/// This is a simple `switch operation_name` construct that the codegen
/// emits in C++ / C# / Java. The actual method body (marshalling) is
/// language-specific and lives in the `crates/idl-*` crates; here we
/// only provide the switch body as a helper.
#[must_use]
pub fn render_skeleton_dispatch(
    class_name: &str,
    ops: &[SkeletonOp],
    lang: TargetLanguage,
) -> String {
    match lang {
        TargetLanguage::Cpp => render_cpp(class_name, ops),
        TargetLanguage::CSharp => render_csharp(class_name, ops),
        TargetLanguage::Java => render_java(class_name, ops),
        TargetLanguage::Rust => render_rust(class_name, ops),
    }
}

fn render_rust(class_name: &str, ops: &[SkeletonOp]) -> String {
    let mut s = alloc::format!(
        "// Rust skeleton dispatch for {class_name} (used as template by zerodds-corba-rust)\nmatch op {{\n"
    );
    for op in ops {
        s.push_str(&alloc::format!(
            "    \"{}\" => self.{}(req),\n",
            op.name,
            op.method_name,
        ));
    }
    s.push_str("    _ => Err(CorbaException::BadOperation),\n}\n");
    s
}

fn render_cpp(class_name: &str, ops: &[SkeletonOp]) -> String {
    let mut s = alloc::format!("// C++ skeleton dispatch for {class_name}\n");
    s.push_str("if (false) {}\n");
    for op in ops {
        s.push_str(&alloc::format!(
            "else if (strcmp(op, \"{}\") == 0) return this->{}(req);\n",
            op.name,
            op.method_name,
        ));
    }
    s.push_str("else throw CORBA::BAD_OPERATION();\n");
    s
}

fn render_csharp(class_name: &str, ops: &[SkeletonOp]) -> String {
    let mut s = alloc::format!("// C# skeleton dispatch for {class_name}\nswitch (op) {{\n");
    for op in ops {
        s.push_str(&alloc::format!(
            "    case \"{}\": return this.{}(req);\n",
            op.name,
            op.method_name,
        ));
    }
    s.push_str("    default: throw new omg.org.CORBA.BAD_OPERATION();\n}\n");
    s
}

fn render_java(class_name: &str, ops: &[SkeletonOp]) -> String {
    let mut s = alloc::format!("// Java skeleton dispatch for {class_name}\nswitch (op) {{\n");
    for op in ops {
        s.push_str(&alloc::format!(
            "    case \"{}\": return this.{}(req);\n",
            op.name,
            op.method_name,
        ));
    }
    s.push_str("    default: throw new org.omg.CORBA.BAD_OPERATION();\n}\n");
    s
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn ops() -> alloc::vec::Vec<SkeletonOp> {
        alloc::vec![
            SkeletonOp {
                name: "echo".into(),
                method_name: "echo".into(),
            },
            SkeletonOp {
                name: "ping".into(),
                method_name: "Ping".into(),
            },
        ]
    }

    #[test]
    fn cpp_dispatch_uses_strcmp_and_bad_operation() {
        let s = render_skeleton_dispatch("EchoImpl", &ops(), TargetLanguage::Cpp);
        assert!(s.contains("strcmp(op, \"echo\")"));
        assert!(s.contains("CORBA::BAD_OPERATION"));
    }

    #[test]
    fn csharp_dispatch_uses_switch_and_omg_org_namespace() {
        let s = render_skeleton_dispatch("EchoImpl", &ops(), TargetLanguage::CSharp);
        assert!(s.contains("switch (op)"));
        assert!(s.contains("omg.org.CORBA.BAD_OPERATION"));
    }

    #[test]
    fn java_dispatch_uses_org_omg_namespace() {
        let s = render_skeleton_dispatch("EchoImpl", &ops(), TargetLanguage::Java);
        assert!(s.contains("org.omg.CORBA.BAD_OPERATION"));
    }
}

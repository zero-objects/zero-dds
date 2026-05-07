// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Skeleton-Code-Generation-Helper — Server-Side-Dispatch.
//!
//! Ein **Skeleton** dispatched einen GIOP-Request anhand des
//! Operation-Names auf die konkrete Servant-Methode und marshallt
//! die Reply.

use alloc::string::String;

use crate::special_types::TargetLanguage;

/// Eine Operation, die das Skeleton dispatchen muss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonOp {
    /// Operation-Name (Wire-Form).
    pub name: String,
    /// Servant-Method-Name (Sprach-Form, z.B. `getStock` in Java).
    pub method_name: String,
}

/// Rendert einen Operation-Name-Switch fuer das Server-Skeleton.
///
/// Das ist ein einfaches `switch operation_name`-Konstrukt, das
/// das Codegen in C++ / C# / Java emittiert. Der echte Method-Body
/// (Marshalling) ist Sprach-spezifisch und liegt in den
/// `crates/idl-*`-Crates; hier ist der Switch-Body als Helper.
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

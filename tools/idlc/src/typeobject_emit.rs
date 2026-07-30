// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Backend dispatch for TypeObject rendering.
//!
//! The actual lowering (`type_object_blobs`) and per-language rendering
//! (`render_rust`/`render_c`/...) live in `zerodds-idl-compose` — shared
//! with `zerodds-build` and any other non-CLI consumer, so the emitted
//! byte blocks cannot drift between the CLI and a build.rs. This module
//! only maps idlc's own [`Backend`] enum onto the shared per-language
//! renderers.

pub use zerodds_idl_compose::TypeObjectBlob;
pub use zerodds_idl_compose::typeobject::{type_object_blobs, type_objects_class_name};

use zerodds_idl_compose::typeobject;

use crate::Backend;

/// Renders the TypeObject constant block for `backend`. `spec`/`base`
/// derive and disambiguate the synthetic Rust module / C#-Java holder class
/// so multi-file compose into one crate/namespace/package does not collide
/// and no user symbol is shadowed (other backends ignore them). Empty string
/// for backends without a wired-up TypeObject renderer yet
/// (Go/Ada/Zig/Nim/D/Elixir/OCaml/Julia/Lua/Swift emit self-contained wire
/// code).
#[must_use]
pub fn render(
    backend: Backend,
    spec: &zerodds_idl::ast::Specification,
    base: &str,
    blobs: &[TypeObjectBlob],
) -> String {
    match backend {
        Backend::Rust => typeobject::render_rust(blobs, spec, base),
        Backend::C => typeobject::render_c(blobs),
        Backend::Cpp => typeobject::render_cpp(blobs),
        Backend::CSharp => typeobject::render_csharp(blobs, spec, base),
        Backend::Java => typeobject::render_java(blobs, spec, base),
        Backend::Python => typeobject::render_python(blobs),
        Backend::Ts => typeobject::render_ts(blobs),
        Backend::Go
        | Backend::Ada
        | Backend::Zig
        | Backend::Nim
        | Backend::D
        | Backend::Elixir
        | Backend::OCaml
        | Backend::Julia
        | Backend::Lua
        | Backend::Swift => String::new(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn empty_spec() -> zerodds_idl::ast::Specification {
        zerodds_idl::parser::parse("", &zerodds_idl::config::ParserConfig::default())
            .expect("parse empty spec")
    }

    fn sample() -> Vec<TypeObjectBlob> {
        vec![TypeObjectBlob {
            fqn: "Robot::Pose".to_string(),
            bytes: vec![0x01, 0x02, 0xff],
        }]
    }

    #[test]
    fn dispatch_covers_wired_backends() {
        let s = empty_spec();
        assert!(render(Backend::Rust, &s, "chat", &sample()).contains("pub mod type_objects"));
        assert!(render(Backend::C, &s, "chat", &sample()).contains("static const unsigned char"));
        assert!(render(Backend::Cpp, &s, "chat", &sample()).contains("inline constexpr"));
        assert!(
            render(Backend::CSharp, &s, "chat", &sample())
                .contains("public static class TypeObjects_chat")
        );
        assert!(
            render(Backend::Java, &s, "chat", &sample())
                .contains("public final class TypeObjects_chat")
        );
        assert!(render(Backend::Python, &s, "chat", &sample()).contains("bytes(["));
        assert!(render(Backend::Ts, &s, "chat", &sample()).contains("Uint8Array"));
    }

    #[test]
    fn dispatch_base_disambiguates_csharp_java_holder() {
        // Two different base names must yield two different holder classes
        // for C# and Java, so composing them into one namespace/package is
        // collision-free.
        let s = empty_spec();
        let common = render(Backend::CSharp, &s, "common", &sample());
        let metrics = render(Backend::CSharp, &s, "metrics", &sample());
        assert!(common.contains("public static class TypeObjects_common"));
        assert!(metrics.contains("public static class TypeObjects_metrics"));
        assert!(!common.contains("class TypeObjects_metrics"));

        let jc = render(Backend::Java, &s, "common", &sample());
        let jm = render(Backend::Java, &s, "metrics", &sample());
        assert!(jc.contains("public final class TypeObjects_common"));
        assert!(jm.contains("public final class TypeObjects_metrics"));
        assert_eq!(type_objects_class_name(&s, "common"), "TypeObjects_common");
    }

    #[test]
    fn dispatch_is_empty_for_unwired_backends() {
        let s = empty_spec();
        for b in [
            Backend::Go,
            Backend::Ada,
            Backend::Zig,
            Backend::Nim,
            Backend::D,
            Backend::Elixir,
            Backend::OCaml,
            Backend::Julia,
            Backend::Lua,
            Backend::Swift,
        ] {
            assert!(render(b, &s, "chat", &sample()).is_empty());
        }
    }
}

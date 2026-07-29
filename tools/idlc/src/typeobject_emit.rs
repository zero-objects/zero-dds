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
pub use zerodds_idl_compose::typeobject::type_object_blobs;

use zerodds_idl_compose::typeobject;

use crate::Backend;

/// Renders the TypeObject constant block for `backend`. Empty string for
/// backends without a wired-up TypeObject renderer yet (Go/Ada/Zig/Nim/D/
/// Elixir/OCaml/Julia/Lua/Swift emit self-contained wire code).
#[must_use]
pub fn render(backend: Backend, blobs: &[TypeObjectBlob]) -> String {
    match backend {
        Backend::Rust => typeobject::render_rust(blobs),
        Backend::C => typeobject::render_c(blobs),
        Backend::Cpp => typeobject::render_cpp(blobs),
        Backend::CSharp => typeobject::render_csharp(blobs),
        Backend::Java => typeobject::render_java(blobs),
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
    use super::*;

    fn sample() -> Vec<TypeObjectBlob> {
        vec![TypeObjectBlob {
            fqn: "Robot::Pose".to_string(),
            bytes: vec![0x01, 0x02, 0xff],
        }]
    }

    #[test]
    fn dispatch_covers_wired_backends() {
        assert!(render(Backend::Rust, &sample()).contains("pub mod type_objects"));
        assert!(render(Backend::C, &sample()).contains("static const unsigned char"));
        assert!(render(Backend::Cpp, &sample()).contains("inline constexpr"));
        assert!(render(Backend::CSharp, &sample()).contains("public static class TypeObjects"));
        assert!(render(Backend::Java, &sample()).contains("public final class TypeObjects"));
        assert!(render(Backend::Python, &sample()).contains("bytes(["));
        assert!(render(Backend::Ts, &sample()).contains("Uint8Array"));
    }

    #[test]
    fn dispatch_is_empty_for_unwired_backends() {
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
            assert!(render(b, &sample()).is_empty());
        }
    }
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Generates the CORBA Rust code from the feature-matrix IDL at build time
//! → `$OUT_DIR/corba_gen.rs`, wrapped in `pub mod corba_gen { … }`.
//!
//! Two-stage composition into ONE module:
//!   1. **Data types** (struct/enum/union) and their `encode`/`decode`
//!      via `zerodds-lang-rules`, which replaced `zerodds-idl-rust`.
//!      Its `cdr_only` flag has no counterpart and needs none: the
//!      materialiser emits the declaration and the codec and nothing
//!      else, so there is no `DdsType` impl to switch off.
//!   2. **Interfaces** (stubs/skeletons) via `zerodds-corba-rust`.
//!
//! Both outputs are placed into the same `corba_gen` module so that the
//! interface operations see the generated types directly in scope.
//! The `#![…]` inner attributes of the individual outputs are stripped (invalid
//! in the middle of a module) and replaced by a single module-top attribute.

use std::path::PathBuf;

use zerodds_ast::TypeKind;
use zerodds_ast_layout::key::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_rules::adapter::cst::{nest_by_module, rust_items};
use zerodds_lang_rules::encode::rust_codec;
use zerodds_lang_rules::rule::rust;
use zerodds_lang_rules::{ModelGraph, apply};

const FEATURE_MATRIX_IDL: &str = r#"
        interface Echo {
            string ping(in string msg);
        };

        // Cross-ORB interop interface (identical in competitors/interop.idl):
        // a compact matrix implemented by ZeroDDS AND omniORB/TAO/JacORB.
        // Deliberately contains double + long long (8-byte-aligned) — the hardest
        // GIOP-1.2 alignment test.
        exception RangeError { long requested; long limit; };

        interface Bench {
            long           add(in long a, in long b);
            double         scale(in double x, in double factor);
            long long      add64(in long long a, in long long b);
            char           next_char(in char c);
            string         concat(in string a, in string b);
            // wstring (UTF-16 wire with BOM §15.3.1.6 + codeset negotiation §13.10):
            // cross-ORB against omniORB/TAO/JacORB.
            wstring        wecho(in wstring w);
            // any echo (struct/sequence inside the any, §15.3.5 TypeCode) — cross-ORB.
            any            aecho(in any a);
            sequence<long> reverse(in sequence<long> xs);
            void           divmod(in long a, in long b, out long quotient, out long remainder);
            void           increment(inout long x);
            // Object reference as in + return (wire = IOR, §15.3.3).
            Object         echo_ref(in Object o);
            // Typed user exception (raises) — wire = UserException reply.
            long           checked(in long idx, in long limit) raises(RangeError);
        };

        interface Features {
            // --- Primitive types ---
            long          add(in long a, in long b);
            double        scale(in double x, in double factor);
            boolean       toggle(in boolean b);
            octet         xor_byte(in octet a, in octet b);
            short         neg_short(in short s);
            unsigned long uadd(in unsigned long a, in unsigned long b);
            long long     add64(in long long a, in long long b);

            // --- String + Sequence ---
            string           concat(in string a, in string b);
            wstring          wecho(in wstring w);
            any              aecho(in any a);
            sequence<long>   reverse(in sequence<long> xs);
            sequence<string> echo_seq(in sequence<string> xs);

            // --- out / inout ---
            void divmod(in long a, in long b, out long quotient, out long remainder);
            void increment(inout long x);
            void swap(inout long a, inout long b);

            // --- oneway ---
            oneway void fire(in long signal);

            // --- Attribute ---
            readonly attribute long  answer;
            attribute        string  label;
        };

        // --- Constructed types (Phase B) ---
        enum Color { RED, GREEN, BLUE };

        struct Point { long x; long y; };

        union Variant switch (long) {
            case 0: long    as_long;
            case 1: string  as_string;
            default: boolean as_flag;
        };

        exception OutOfRange { long requested; long limit; };

        interface Shapes {
            // struct as in + return
            Point   translate(in Point p, in long dx, in long dy);
            long    sum_point(in Point p);
            // struct out / inout
            void    split(in Point p, out long x, out long y);
            void    bump(inout Point p);
            // enum
            Color   next_color(in Color c);
            // union
            Variant wrap_long(in long v);
            long    unwrap_long(in Variant v);
            // user-exception
            long    checked(in long idx, in long limit) raises(OutOfRange);
        };

        // Live name service (CosNaming wired live over GIOP): uses
        // object references (IOR) + typed raises — both newly wired.
        exception NameError { string reason; };

        interface Naming {
            void   bind(in string name, in Object obj) raises(NameError);
            Object resolve(in string name) raises(NameError);
        };

        // Live event channel (CosEvent live over GIOP): pull model, uses `any`.
        interface EventBus {
            void    push(in any event);
            boolean try_pull(out any event);
        };

        // Live notification (CosNotification live over GIOP): StructuredEvent
        // (domain/type_name + any payload) through the cos-notify channel.
        interface NotifyBus {
            void    push_structured(in string domain, in string type_name, in any body);
            boolean try_pull_structured(out string domain, out string type_name, out any body);
        };

        // Live interface repository (corba-ir live over GIOP).
        interface TypeRepo {
            boolean          contains(in string repo_id);
            sequence<string> ids();
        };

        // Provided facet of a CCM component (live over GIOP, lifecycle-gated).
        interface Calculator {
            long add(in long a, in long b);
        };

        // REAL OMG CosNaming (formal/2004-10-03), module-nested with
        // typeprefix → exact standard RepositoryIds including interface-nested
        // exceptions: IDL:omg.org/CosNaming/NamingContext:1.0 +
        // IDL:omg.org/CosNaming/NamingContext/NotFound:1.0 (cross-ORB NotFound).
        // The module merge (#4(4)) joins the two codegen passes; idl-rust
        // emits the interface-nested exception structs (#4(3) completion);
        // interface returns→ObjectReference (#4(1)); real per-op raises (#4(2)).
        typeprefix CosNaming "omg.org";
        module CosNaming {
            struct NameComponent { string id; string kind; };
            typedef sequence<NameComponent> CosName;
            enum NotFoundReason { missing_node, not_context, not_object };
            interface NamingContext {
                exception NotFound { NotFoundReason why; CosName rest_of_name; };
                exception InvalidName { };
                exception AlreadyBound { };
                exception NotEmpty { };
                void          bind(in CosName n, in Object obj) raises(NotFound, InvalidName, AlreadyBound);
                void          rebind(in CosName n, in Object obj) raises(NotFound, InvalidName);
                Object        resolve(in CosName n) raises(NotFound, InvalidName);
                void          unbind(in CosName n) raises(NotFound, InvalidName);
                NamingContext new_context();
                NamingContext bind_new_context(in CosName n) raises(NotFound, AlreadyBound, InvalidName);
                void          destroy() raises(NotEmpty);
            };
        };
    "#;

/// Merges identically named top-level `pub mod NAME { … }` blocks that the
/// two-pass codegen (idl-rust types + corba-rust interfaces) emits separately.
/// Non-module items stay unchanged; nested modules are merged recursively.
/// Precondition: inner attrs are already stripped (the two bodies concatenated).
fn merge_modules(code: &str) -> String {
    let mut passthrough = String::new();
    let mut mods: Vec<(String, String)> = Vec::new();
    let mut rest = code;
    while let Some((pre, name, body, after)) = find_top_mod(rest) {
        passthrough.push_str(pre);
        match mods.iter_mut().find(|(nm, _)| *nm == name) {
            Some(e) => {
                e.1.push('\n');
                e.1.push_str(body);
            }
            None => mods.push((name, body.to_string())),
        }
        rest = after;
    }
    passthrough.push_str(rest);
    let mut out = passthrough;
    for (name, body) in mods {
        // `use super::*;` at the top of every generated module: the two
        // columns put the runtime import at the file's top level, and a
        // Rust module does not inherit its parent's `use`. Without it
        // an `impl` inside `pub mod CosNaming` names a `Writer` that is
        // not in scope — which is what appeared the moment the data
        // column started nesting.
        out.push_str(&format!(
            "\npub mod {name} {{\nuse super::*;\n{}\n}}\n",
            merge_modules(&body)
        ));
    }
    out
}

/// Finds the next `pub mod NAME { … }`: `(text_before, name, inner_body, text_after)`.
fn find_top_mod(code: &str) -> Option<(&str, String, &str, &str)> {
    const KW: &str = "pub mod ";
    let mut from = 0usize;
    loop {
        let idx = code[from..].find(KW)? + from;
        let after_kw = &code[idx + KW.len()..];
        let name: String = after_kw
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let after_name = after_kw[name.len()..].trim_start();
        if name.is_empty() || !after_name.starts_with('{') {
            from = idx + KW.len();
            continue;
        }
        let brace_pos = code.len() - after_name.len();
        let Some(close) = match_brace(code, brace_pos) else {
            from = idx + KW.len();
            continue;
        };
        return Some((
            &code[..idx],
            name,
            &code[brace_pos + 1..close],
            &code[close + 1..],
        ));
    }
}

/// Index of the `}` matching `code[open]=='{'`, counting braces and skipping
/// `"…"` strings and `//` line comments.
fn match_brace(code: &str, open: usize) -> Option<usize> {
    let b = code.as_bytes();
    let mut depth = 0i32;
    let mut i = open;
    let mut in_str = false;
    while i < b.len() {
        let c = b[i];
        if in_str {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == b'"' {
                in_str = false;
            }
        } else if c == b'"' {
            in_str = true;
        } else if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            // Line comment up to EOL.
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        } else if c == b'{' {
            depth += 1;
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Removes `#![…]` inner-attribute lines (valid only at the start of a module).
fn strip_inner_attrs(code: &str) -> String {
    code.lines()
        .filter(|l| !l.trim_start().starts_with("#!["))
        .collect::<Vec<_>>()
        .join("\n")
}

fn main() {
    // Stage 0: parse, lower, resolve. The resolve pass is the caller's
    // — `zerodds-idl-lower` leaves a name it could not bind on the spot
    // pending — and a model handed to either column without it has an
    // unresolved type for every forward reference.
    let parsed = zerodds_idl_parse::parse(FEATURE_MATRIX_IDL);
    assert!(
        parsed.diagnostics().iter().all(|d| !d.is_error()),
        "the feature-matrix IDL must parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "feature-matrix.idl");
    assert!(
        lowered.diagnostics.iter().all(|d| !d.is_error()),
        "the feature-matrix IDL must lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(
        report.broken.is_empty(),
        "every name in the feature-matrix IDL must resolve: {:?}",
        report.broken
    );

    // Stage 1: the data types and their codecs, from
    // `zerodds-lang-rules`, with the data rules a CORBA target needs
    // (`rust::corba_data_catalogue`).
    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rust::corba_data_catalogue()).expect("apply the Rust rules");
    let mut items = rust_items(&ast, &applied).expect("materialise the data types");

    // The `encode` and `decode` beside the declarations. The key is
    // omniORB's classic CDR, big-endian: that is what a GIOP request
    // body carries (CORBA 3.3 part 2 §15.3) and what the generated
    // stubs write.
    let key = LayoutKey::in_dialect(
        Dialect::OmniOrb,
        Encoding::PlainCdr1,
        Endianness::Big,
        Purpose::Data,
    );
    // The codecs land in the same module as the declaration they are an
    // `impl` of, which is why they are nested rather than appended.
    let mut codecs = Vec::new();
    for (id, node) in ast.types() {
        if node.name.is_empty() {
            continue;
        }
        if !matches!(
            node.kind,
            TypeKind::Structure { .. }
                | TypeKind::Union { .. }
                | TypeKind::Enumeration { .. }
                | TypeKind::Bitmask { .. }
                | TypeKind::Bitset { .. }
        ) {
            continue;
        }
        for item in rust_codec(&ast, id, key).expect("a codec for every declared data type") {
            codecs.push((node.module, item));
        }
    }
    items.extend(nest_by_module(&ast, codecs).expect("a scope for every codec's declaration"));

    let file = zerodds_lang_cst::rust::File {
        inner_attrs: Vec::new(),
        doc: zerodds_lang_cst::common::Doc::none(),
        uses: Vec::new(),
        items,
    };
    let types = zerodds_lang_render::rust::file(&file);

    // Stage 2: the interfaces — traits, stubs, skeletons.
    let ifaces = zerodds_corba_rust::generate_corba_rust_module(
        &ast,
        &zerodds_corba_rust::CorbaRustGenOptions::default(),
    )
    .expect("generate the CORBA service module");

    // Merge identically named `pub mod` from both passes — otherwise a
    // module-nested IDL (CosNaming, Notification) declares the module
    // twice.
    let body = merge_modules(&format!(
        "{}\n{}",
        strip_inner_attrs(&types),
        strip_inner_attrs(&ifaces)
    ));
    let wrapped = format!(
        "#[allow(warnings, clippy::all, clippy::pedantic)]\npub mod corba_gen {{\n#![allow(warnings, clippy::all, clippy::pedantic)]\nuse zerodds_wire::{{Reader, WireError, Writer}};\n{body}\n}}\n"
    );
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("corba_gen.rs");
    std::fs::write(&out, wrapped).expect("write generated module");
    println!("cargo:rerun-if-changed=build.rs");
}

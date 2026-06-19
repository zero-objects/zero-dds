// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Generates the CORBA Rust code from the feature-matrix IDL at build time
//! → `$OUT_DIR/corba_gen.rs`, wrapped in `pub mod corba_gen { … }`.
//!
//! Two-stage composition into ONE module:
//!   1. **Data types** (struct/enum/union/exception) via
//!      `zerodds-idl-rust` in `cdr_only` mode — only type declarations +
//!      the classic `CdrEncode`/`CdrDecode`, without the `DdsType` impl, which
//!      would pull in `zerodds_dcps`/`zerodds_types`/`zerodds_sql_filter`.
//!   2. **Interfaces** (stubs/skeletons) via `zerodds-corba-rust`.
//!
//! Both outputs are placed into the same `corba_gen` module so that the
//! interface operations see the generated types directly in scope.
//! The `#![…]` inner attributes of the individual outputs are stripped (invalid
//! in the middle of a module) and replaced by a single module-top attribute.

use std::path::PathBuf;

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
        out.push_str(&format!(
            "\npub mod {name} {{\n{}\n}}\n",
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
    // Full 4.2 profile: enables all CORBA building blocks (oneway, attribute,
    // interface, union, enum, exception, raises, …) — `default()` gates these.
    let ast = zerodds_idl::parse(
        FEATURE_MATRIX_IDL,
        &zerodds_idl::config::ParserConfig::full_4_2(),
    )
    .expect("parse feature-matrix IDL");

    // Stage 1: data types (cdr_only — no DdsType/dcps).
    let types = zerodds_idl_rust::generate_rust_module(
        &ast,
        &zerodds_idl_rust::RustGenOptions {
            cdr_only: true,
            ..Default::default()
        },
    )
    .expect("generate idl-rust data types");

    // Stage 2: interfaces (stubs/skeletons).
    let ifaces = zerodds_corba_rust::generate_corba_rust_module(
        &ast,
        &zerodds_corba_rust::CorbaRustGenOptions::default(),
    )
    .expect("generate corba-rust module");

    // Merge identically named `pub mod` from both passes (#4(4)) — otherwise
    // double definition for module-nested IDLs (CosNaming, Notification).
    let body = merge_modules(&format!(
        "{}\n{}",
        strip_inner_attrs(&types),
        strip_inner_attrs(&ifaces)
    ));
    let wrapped = format!(
        "#[allow(warnings, clippy::all, clippy::pedantic)]\npub mod corba_gen {{\n#![allow(warnings, clippy::all, clippy::pedantic)]\n{body}\n}}\n"
    );
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("corba_gen.rs");
    std::fs::write(&out, wrapped).expect("write generated module");
    println!("cargo:rerun-if-changed=build.rs");
}

//! Integration-Tests fuer den DDS-RPC Java-PSM-Codegen
//! (Cluster C6.1.D-java).
//!
//! Pro Test: parse eine `@service`-IDL, generiere die fuenf Java-Files
//! (Interface, Async-Interface, Handler-Interface, Requester, Replier)
//! und asserte semantische Anker auf Stringsuche-Basis. Marker-basiertes
//! Snapshot-Format — robust gegen Whitespace-Drift.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_idl::ast::{
    Annotation, AnnotationParams, Definition, Export, Identifier, IntegerType, InterfaceDcl,
    InterfaceDef, InterfaceKind, OpDecl, ParamAttribute, ParamDecl, PrimitiveType, ScopedName,
    Specification, StringType, TypeSpec,
};
use zerodds_idl::errors::Span;
use zerodds_idl_java::{JavaFile, JavaGenOptions, generate_java_files};

fn sp() -> Span {
    Span::SYNTHETIC
}

fn ident(t: &str) -> Identifier {
    Identifier::new(t, sp())
}

fn long_t() -> TypeSpec {
    TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::Long))
}

fn longlong_t() -> TypeSpec {
    TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::LongLong))
}

fn string_t() -> TypeSpec {
    TypeSpec::String(StringType {
        wide: false,
        bound: None,
        span: sp(),
    })
}

fn ann(name: &str) -> Annotation {
    Annotation {
        name: ScopedName {
            absolute: false,
            parts: vec![ident(name)],
            span: sp(),
        },
        params: AnnotationParams::None,
        span: sp(),
    }
}

fn op(
    name: &str,
    oneway: bool,
    ret: Option<TypeSpec>,
    params: Vec<ParamDecl>,
    raises: Vec<ScopedName>,
) -> OpDecl {
    OpDecl {
        name: ident(name),
        oneway,
        return_type: ret,
        params,
        raises,
        annotations: Vec::new(),
        span: sp(),
    }
}

fn p(name: &str, attr: ParamAttribute, ty: TypeSpec) -> ParamDecl {
    ParamDecl {
        attribute: attr,
        type_spec: ty,
        name: ident(name),
        annotations: Vec::new(),
        span: sp(),
    }
}

fn iface(
    name: &str,
    ops: Vec<OpDecl>,
    anns: Vec<Annotation>,
    exports_extra: Vec<Export>,
) -> InterfaceDef {
    let mut exports: Vec<Export> = ops.into_iter().map(Export::Op).collect();
    exports.extend(exports_extra);
    InterfaceDef {
        kind: InterfaceKind::Plain,
        name: ident(name),
        bases: Vec::new(),
        exports,
        annotations: anns,
        span: sp(),
    }
}

fn spec_of(iface_def: InterfaceDef) -> Specification {
    Specification {
        definitions: vec![Definition::Interface(InterfaceDcl::Def(iface_def))],
        span: sp(),
    }
}

fn gen_files(spec: &Specification) -> Vec<JavaFile> {
    generate_java_files(spec, &JavaGenOptions::default()).expect("gen")
}

fn find<'a>(files: &'a [JavaFile], class: &str) -> &'a JavaFile {
    files
        .iter()
        .find(|f| f.class_name == class)
        .unwrap_or_else(|| panic!("no JavaFile named {class}"))
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[test]
fn calculator_emits_five_files_for_single_method() {
    let svc = iface(
        "Calculator",
        vec![op(
            "add",
            false,
            Some(longlong_t()),
            vec![
                p("a", ParamAttribute::In, longlong_t()),
                p("b", ParamAttribute::In, longlong_t()),
            ],
            Vec::new(),
        )],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let names: Vec<&str> = files.iter().map(|f| f.class_name.as_str()).collect();
    assert!(names.contains(&"Calculator"));
    assert!(names.contains(&"CalculatorAsync"));
    assert!(names.contains(&"CalculatorRequester"));
    assert!(names.contains(&"CalculatorReplier"));
    assert!(names.contains(&"CalculatorService"));
    assert_eq!(files.len(), 5);
}

#[test]
fn service_interface_carries_runtime_annotation() {
    let svc = iface(
        "Calc",
        vec![op("noop", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "Calc");
    assert!(f.source.contains("@org.zerodds.rpc.Service(\"Calc\")"));
    assert!(f.source.contains("public interface Calc"));
}

#[test]
fn async_interface_returns_completable_future() {
    let svc = iface(
        "Calc",
        vec![op(
            "add",
            false,
            Some(longlong_t()),
            vec![p("a", ParamAttribute::In, longlong_t())],
            Vec::new(),
        )],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcAsync");
    assert!(
        f.source
            .contains("java.util.concurrent.CompletableFuture<Long> addAsync")
    );
}

#[test]
fn sync_interface_returns_unboxed_primitive() {
    let svc = iface(
        "Calc",
        vec![op(
            "add",
            false,
            Some(longlong_t()),
            vec![p("a", ParamAttribute::In, longlong_t())],
            Vec::new(),
        )],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "Calc");
    // Sync-Variante: long-Return, kein Boxing.
    assert!(f.source.contains("long add(long a)"));
}

#[test]
fn oneway_method_emits_oneway_annotation() {
    let svc = iface(
        "Logger",
        vec![op(
            "log",
            true,
            None,
            vec![p("msg", ParamAttribute::In, string_t())],
            Vec::new(),
        )],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "Logger");
    assert!(f.source.contains("@org.zerodds.rpc.Oneway"));
    assert!(f.source.contains("void log(String msg)"));
}

#[test]
fn oneway_async_returns_void_future() {
    let svc = iface(
        "Logger",
        vec![op(
            "log",
            true,
            None,
            vec![p("msg", ParamAttribute::In, string_t())],
            Vec::new(),
        )],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "LoggerAsync");
    assert!(
        f.source
            .contains("java.util.concurrent.CompletableFuture<Void> logAsync(String msg)")
    );
}

#[test]
fn void_method_returns_void() {
    let svc = iface(
        "S",
        vec![op("noop", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "S");
    assert!(f.source.contains("void noop()"));
}

#[test]
fn out_param_uses_holder_pattern() {
    let svc = iface(
        "S",
        vec![op(
            "result",
            false,
            None,
            vec![p("v", ParamAttribute::Out, long_t())],
            Vec::new(),
        )],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "S");
    assert!(f.source.contains("org.zerodds.rpc.Holder<Integer> v"));
}

#[test]
fn inout_param_uses_holder_pattern() {
    let svc = iface(
        "S",
        vec![op(
            "swap",
            false,
            None,
            vec![p("v", ParamAttribute::InOut, long_t())],
            Vec::new(),
        )],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "S");
    assert!(f.source.contains("org.zerodds.rpc.Holder<Integer> v"));
}

#[test]
fn requester_implements_both_sync_and_async() {
    let svc = iface(
        "Calc",
        vec![op("noop", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcRequester");
    assert!(f.source.contains("public final class CalcRequester"));
    assert!(f.source.contains("implements Calc, CalcAsync"));
}

#[cfg(not(feature = "jni"))]
#[test]
fn requester_holds_runtime_handle() {
    let svc = iface(
        "Calc",
        vec![op("noop", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcRequester");
    assert!(
        f.source
            .contains("org.zerodds.rpc.Requester<Object, Object> requester")
    );
}

#[test]
fn replier_constructor_takes_handler() {
    let svc = iface(
        "Calc",
        vec![op("noop", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcReplier");
    assert!(f.source.contains(
        "public CalcReplier(org.zerodds.rpc.Replier<Object, Object> replier, CalcService handler)"
    ));
}

#[test]
fn replier_dispatch_table_uses_method_ids() {
    let svc = iface(
        "Calc",
        vec![
            op("foo", false, None, Vec::new(), Vec::new()),
            op("bar", false, None, Vec::new(), Vec::new()),
        ],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcReplier");
    assert!(f.source.contains("case 1:"));
    assert!(f.source.contains("case 2:"));
    assert!(f.source.contains("UNKNOWN_OPERATION"));
}

#[test]
fn handler_interface_has_method_signatures() {
    let svc = iface(
        "Calc",
        vec![op(
            "add",
            false,
            Some(longlong_t()),
            vec![p("a", ParamAttribute::In, longlong_t())],
            Vec::new(),
        )],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcService");
    assert!(f.source.contains("public interface CalcService"));
    assert!(f.source.contains("long add(long a)"));
}

#[test]
fn module_scoped_service_lands_in_lowercase_package() {
    use zerodds_idl::ast::ModuleDef;
    let svc = iface(
        "Calc",
        vec![op("noop", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let m = ModuleDef {
        name: ident("Org"),
        definitions: vec![Definition::Module(ModuleDef {
            name: ident("Example"),
            definitions: vec![Definition::Interface(InterfaceDcl::Def(svc))],
            annotations: Vec::new(),
            span: sp(),
        })],
        annotations: Vec::new(),
        span: sp(),
    };
    let spec = Specification {
        definitions: vec![Definition::Module(m)],
        span: sp(),
    };
    let files = gen_files(&spec);
    let f = find(&files, "Calc");
    assert_eq!(f.package_path, "org.example");
    assert!(f.source.contains("package org.example;"));
}

#[test]
fn raises_clause_emits_inner_exception_class() {
    use zerodds_idl::ast::{ExceptDecl, Member};
    let exc = ExceptDecl {
        name: ident("BadInput"),
        members: vec![Member {
            type_spec: string_t(),
            declarators: vec![zerodds_idl::ast::Declarator::Simple(ident("what"))],
            annotations: Vec::new(),
            span: sp(),
        }],
        annotations: Vec::new(),
        span: sp(),
    };
    let svc = iface(
        "Calc",
        vec![op(
            "add",
            false,
            Some(longlong_t()),
            vec![p("a", ParamAttribute::In, longlong_t())],
            vec![ScopedName {
                absolute: false,
                parts: vec![ident("BadInput")],
                span: sp(),
            }],
        )],
        vec![ann("service")],
        vec![Export::Except(exc)],
    );
    let files = gen_files(&spec_of(svc));
    let exc_file = find(&files, "BadInput");
    assert!(
        exc_file
            .source
            .contains("public class BadInput extends RuntimeException")
    );
}

#[test]
fn multi_service_in_one_idl_emits_independent_class_sets() {
    use zerodds_idl::ast::ModuleDef;
    let svc_a = iface(
        "Alpha",
        vec![op("a", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let svc_b = iface(
        "Beta",
        vec![op("b", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let spec = Specification {
        definitions: vec![
            Definition::Interface(InterfaceDcl::Def(svc_a)),
            Definition::Interface(InterfaceDcl::Def(svc_b)),
        ],
        span: sp(),
    };
    let files = gen_files(&spec);
    let class_names: Vec<&str> = files.iter().map(|f| f.class_name.as_str()).collect();
    for cn in [
        "Alpha",
        "AlphaAsync",
        "AlphaRequester",
        "AlphaReplier",
        "AlphaService",
        "Beta",
        "BetaAsync",
        "BetaRequester",
        "BetaReplier",
        "BetaService",
    ] {
        assert!(
            class_names.contains(&cn),
            "missing class {cn}; got {class_names:?}"
        );
    }
    let _ = ModuleDef {
        name: ident(""),
        definitions: Vec::new(),
        annotations: Vec::new(),
        span: sp(),
    };
}

#[test]
fn service_with_empty_method_list_still_emits_all_files() {
    let svc = iface("Empty", Vec::new(), vec![ann("service")], Vec::new());
    let files = gen_files(&spec_of(svc));
    assert_eq!(files.len(), 5);
}

#[cfg(not(feature = "jni"))]
#[test]
fn requester_async_method_uses_send_request() {
    let svc = iface(
        "Calc",
        vec![op(
            "add",
            false,
            Some(longlong_t()),
            vec![p("a", ParamAttribute::In, longlong_t())],
            Vec::new(),
        )],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcRequester");
    assert!(f.source.contains("requester.sendRequest"));
}

#[cfg(not(feature = "jni"))]
#[test]
fn requester_oneway_uses_send_oneway() {
    let svc = iface(
        "Logger",
        vec![op(
            "log",
            true,
            None,
            vec![p("m", ParamAttribute::In, string_t())],
            Vec::new(),
        )],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "LoggerRequester");
    assert!(f.source.contains("requester.sendOneway"));
}

#[test]
fn async_method_name_has_async_suffix() {
    let svc = iface(
        "Calc",
        vec![op("compute", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcAsync");
    assert!(f.source.contains("computeAsync"));
}

#[test]
fn sync_method_signature_in_requester_overrides() {
    let svc = iface(
        "Calc",
        vec![op("compute", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcRequester");
    // Requester implementiert sync (non-Async) interface — Override-
    // Annotation muss vorhanden sein.
    assert!(f.source.contains("@Override"));
    assert!(f.source.contains("public void compute()"));
}

#[test]
fn requester_sync_blocks_via_get() {
    let svc = iface(
        "Calc",
        vec![op(
            "add",
            false,
            Some(longlong_t()),
            vec![p("a", ParamAttribute::In, longlong_t())],
            Vec::new(),
        )],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcRequester");
    assert!(f.source.contains("addAsync(a).get()"));
}

#[test]
fn service_name_annotation_overrides_iface_name() {
    let svc = iface(
        "InternalIface",
        vec![op("noop", false, None, Vec::new(), Vec::new())],
        vec![Annotation {
            name: ScopedName {
                absolute: false,
                parts: vec![ident("service")],
                span: sp(),
            },
            params: AnnotationParams::Named(vec![zerodds_idl::ast::NamedParam {
                name: ident("name"),
                value: zerodds_idl::ast::ConstExpr::Literal(zerodds_idl::ast::Literal {
                    kind: zerodds_idl::ast::LiteralKind::String,
                    raw: "\"PublicName\"".into(),
                    span: sp(),
                }),
                span: sp(),
            }]),
            span: sp(),
        }],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    // Files werden weiterhin nach Annotation-Name benannt.
    let names: Vec<&str> = files.iter().map(|f| f.class_name.as_str()).collect();
    assert!(names.contains(&"PublicName"));
    let f = find(&files, "PublicName");
    assert!(
        f.source
            .contains("@org.zerodds.rpc.Service(\"PublicName\")")
    );
}

#[test]
fn non_service_interface_emits_plain_java_interface() {
    // Frueher Reject; jetzt voll abgedeckt — Spec idl4-java §7.4
    // (CORBA-Migration-Pfad).
    let plain_iface = iface(
        "PlainNonService",
        vec![op("op", false, None, Vec::new(), Vec::new())],
        Vec::new(), // kein @service
        Vec::new(),
    );
    let spec = spec_of(plain_iface);
    let files = generate_java_files(&spec, &JavaGenOptions::default()).expect("ok");
    let combined: String = files.iter().map(|f| f.source.clone()).collect();
    assert!(combined.contains("public interface PlainNonService"));
}

#[test]
fn replier_dispatch_includes_service_handler_field() {
    let svc = iface(
        "Calc",
        vec![op("noop", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcReplier");
    assert!(f.source.contains("private final CalcService handler;"));
}

#[test]
fn each_emitted_file_starts_with_generated_marker() {
    let svc = iface(
        "Calc",
        vec![op("noop", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    for f in &files {
        assert!(f.source.starts_with("// Generated by zerodds idl-java."));
    }
}

#[test]
fn requester_non_oneway_with_no_return_uses_get_for_void() {
    let svc = iface(
        "Calc",
        vec![op("ping", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcRequester");
    // void-return: body ruft `.get()` ohne `return` davor.
    assert!(f.source.contains("pingAsync().get()"));
    assert!(!f.source.contains("return pingAsync().get()"));
}

#[test]
fn service_context_skeleton_referenced_in_runtime() {
    // Sanity: `org.zerodds.rpc.ServiceContext` ist als Runtime-Klasse
    // gemeint; der Codegen referenziert sie nicht direkt, aber das
    // Replier-Dispatch-Skelett ist mit ihr kompatibel.
    let svc = iface(
        "Calc",
        vec![op("noop", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcReplier");
    // Wir erwarten zumindest, dass Replier-Class kompilierbar ist
    // (ServiceContext-Wiring ist Runtime-Aufgabe; das Codegen-Output
    // muss konsistent sein).
    assert!(f.source.contains("public final class CalcReplier"));
}

#[test]
fn sequence_param_uses_list_in_signature() {
    use zerodds_idl::ast::SequenceType;
    let seq_long = TypeSpec::Sequence(SequenceType {
        elem: Box::new(longlong_t()),
        bound: None,
        span: sp(),
    });
    let svc = iface(
        "Calc",
        vec![op(
            "sum",
            false,
            Some(longlong_t()),
            vec![p("xs", ParamAttribute::In, seq_long)],
            Vec::new(),
        )],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "Calc");
    assert!(f.source.contains("java.util.List<Long> xs"));
}

#[test]
fn string_param_signature() {
    let svc = iface(
        "Greeter",
        vec![op(
            "greet",
            false,
            Some(string_t()),
            vec![p("name", ParamAttribute::In, string_t())],
            Vec::new(),
        )],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "Greeter");
    assert!(f.source.contains("String greet(String name)"));
}

#[test]
fn async_string_return_uses_boxed_string() {
    let svc = iface(
        "Greeter",
        vec![op("greet", false, Some(string_t()), Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "GreeterAsync");
    assert!(
        f.source
            .contains("java.util.concurrent.CompletableFuture<String> greetAsync")
    );
}

#[cfg(not(feature = "jni"))]
#[test]
fn requester_implements_close_via_runtime_field() {
    // Skeleton: kein expliziter close()-Code emittiert — Requester ist
    // ein dünner Wrapper, das Runtime-Interface stellt close() bereit.
    let svc = iface(
        "Calc",
        vec![op("noop", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcRequester");
    // Klasse haelt Reference auf Runtime-Requester.
    assert!(f.source.contains("private final org.zerodds.rpc.Requester"));
}

#[test]
fn replier_holds_runtime_replier_reference() {
    let svc = iface(
        "Calc",
        vec![op("noop", false, None, Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcReplier");
    assert!(
        f.source
            .contains("private final org.zerodds.rpc.Replier<Object, Object> replier;")
    );
}

#[test]
fn multiple_methods_async_have_independent_signatures() {
    let svc = iface(
        "Multi",
        vec![
            op("foo", false, Some(longlong_t()), Vec::new(), Vec::new()),
            op(
                "bar",
                false,
                None,
                vec![p("s", ParamAttribute::In, string_t())],
                Vec::new(),
            ),
            op("baz", true, None, Vec::new(), Vec::new()),
        ],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "MultiAsync");
    assert!(
        f.source
            .contains("java.util.concurrent.CompletableFuture<Long> fooAsync()")
    );
    assert!(
        f.source
            .contains("java.util.concurrent.CompletableFuture<Void> barAsync(String s)")
    );
    assert!(
        f.source
            .contains("java.util.concurrent.CompletableFuture<Void> bazAsync()")
    );
}

// ---------------------------------------------------------------------
// JNI-Feature: Codegen emittiert RustRequesterFFI-basierte Klassen
// ---------------------------------------------------------------------

#[cfg(feature = "jni")]
#[test]
fn requester_with_jni_feature_uses_rust_requester_ffi() {
    let svc = iface(
        "Calc",
        vec![op("ping", false, Some(long_t()), Vec::new(), Vec::new())],
        vec![ann("service")],
        Vec::new(),
    );
    let files = gen_files(&spec_of(svc));
    let f = find(&files, "CalcRequester");
    // Ctor + Field referenzieren `RustRequesterFFI`.
    assert!(f.source.contains("org.zerodds.rpc.RustRequesterFFI"));
    // Async-Body ruft sendRequestAsync.
    assert!(f.source.contains("sendRequestAsync"));
}

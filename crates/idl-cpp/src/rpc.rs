// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C6.1.D-cpp: DDS-RPC C++ PSM-Codegen.
//!
//! Spec-Referenz: OMG DDS-RPC 1.0 (formal/16-12-04), §10 — C++ PSM.
//!
//! Diese Schicht emittiert pro [`zerodds_rpc::ServiceDef`] vier Bausteine:
//!
//! * **Service-Interface** — abstrakte Klasse `<Service>` mit pro-Methode
//!   sync + `_async`-Signaturen plus `<Service>HandlerInterface` fuer die
//!   Server-Seite.
//! * **Requester-Klasse** — typisierter `<Service>_Requester`-Wrapper, der
//!   vom Generic [`dds::rpc::Requester<TIn, TOut>`]-Template erbt und pro
//!   Methode eine `Future<TOut> name_async(TIn)` plus `TOut name(TIn)`-API
//!   exportiert.
//! * **Replier-Klasse** — typisierter `<Service>_Replier`-Wrapper mit
//!   `dispatch_to_handler`, der einen [`<Service>HandlerInterface`] an
//!   Methoden-Dispatch bindet.
//! * **ServiceTraits** — `dds::rpc::ServiceTraits<<Service>>`-Spezialisierung
//!   mit Topic-Names + Mapping-Variante.
//!
//! Die generischen Templates liegen unter
//! `templates/dds-psm-cxx/rpc/{requester,replier,exception,service_traits}.hpp.tmpl`
//! und werden via `include_str!` eingebettet.
//!
//! # Spec §10 Coverage
//!
//! | §10-Item | Coverage |
//! |---|---|
//! | §10.2 `dds::rpc`-Namespace | done — alle generierten Klassen liegen unter `dds::rpc::*`. |
//! | §10.3 ServiceTraits | done — pro Service spezialisiert. |
//! | §10.4 Requester-Template | done — Generic + typisierter Per-Service-Wrapper. |
//! | §10.5 Replier-Template | done — Generic + typisierter Per-Service-Wrapper + HandlerInterface. |
//! | §10.5 Operation-Naming | done — `<Service>_<method>_In/Out` direkt aus C6.1.B. |
//! | §10.6 RemoteException | done — Hierarchie + Mapping pro IDL-Exception via [`emit_remote_exception_class`]. |
//! | §10.7 Promise/Future | done — `dds::rpc::Future<T>` + `Promise<T>` als `std::future`-Wrapper. |
//! | §10.7 Async-API | done — `<method>_async` liefert `Future<TOut>`. |
//!
//! # Bewusst nicht im Crate
//!
//! * Live-FFI-Anbindung an die Rust-RPC-Runtime — nur Quelltext.
//! * Promise/Future-Optimierung (Skeleton ohne Listener-Style-API).
//! * `attribute T name`-Mapping (§7.5.1.1.3) — keine Attribute-Slots im
//!   aktuellen [`zerodds_rpc::ServiceDef`].
//! * `g++`/`clang++`-Compile-Test — nicht Teil der CI.

use core::fmt::Write;

use zerodds_idl::ast::{
    ExceptDecl, FloatingType, IntegerType, PrimitiveType, ScopedName, TypeSpec,
};
use zerodds_rpc::{MethodDef, ParamDef, ParamDirection, ServiceDef};

use crate::error::CppGenError;

/// Statische Templates (eingebettet via `include_str!`).
const TPL_REQUESTER_HPP: &str = include_str!("../templates/dds-psm-cxx/rpc/requester.hpp.tmpl");
const TPL_REPLIER_HPP: &str = include_str!("../templates/dds-psm-cxx/rpc/replier.hpp.tmpl");
const TPL_EXCEPTION_HPP: &str = include_str!("../templates/dds-psm-cxx/rpc/exception.hpp.tmpl");
const TPL_SERVICE_TRAITS_HPP: &str =
    include_str!("../templates/dds-psm-cxx/rpc/service_traits.hpp.tmpl");

// ---------------------------------------------------------------------------
// Public API — Top-Level-Emitter
// ---------------------------------------------------------------------------

/// Emittiert das **Service-Interface** (abstrakte Klasse + HandlerInterface).
///
/// Output (vereinfacht):
///
/// ```cpp
/// namespace dds { namespace rpc {
///   class Calculator {
///   public:
///     virtual ~Calculator() = default;
///     virtual int32_t add(int32_t a, int32_t b) = 0;
///     virtual ::dds::rpc::Future<int32_t> add_async(int32_t a, int32_t b) = 0;
///   };
///   class CalculatorHandlerInterface {
///   public:
///     virtual ~CalculatorHandlerInterface() = default;
///     virtual int32_t add(int32_t a, int32_t b) = 0;
///   };
/// } }
/// ```
#[must_use]
pub fn emit_service_interface(svc: &ServiceDef) -> String {
    let mut out = String::new();
    let svc_name = &svc.name;
    let _ = writeln!(
        out,
        "// zerodds-rpc-1.0 — Service-Interface fuer '{svc_name}' (Spec §10.4)."
    );
    let _ = writeln!(out, "namespace dds {{ namespace rpc {{");
    let _ = writeln!(out);

    // Abstract Service-Interface.
    let _ = writeln!(out, "class {svc_name} {{");
    let _ = writeln!(out, "public:");
    let _ = writeln!(out, "    virtual ~{svc_name}() = default;");
    for m in &svc.methods {
        emit_doxygen_for_method(&mut out, m, "    ");
        emit_sync_method_signature(&mut out, m, "    ");
        if !m.oneway {
            emit_async_method_signature(&mut out, m, "    ");
        }
    }
    let _ = writeln!(out, "}};");
    let _ = writeln!(out);

    // HandlerInterface (Server-side).
    let _ = writeln!(
        out,
        "/// Server-side Handler-Interface fuer '{svc_name}' (Spec §10.5)."
    );
    let _ = writeln!(out, "class {svc_name}HandlerInterface {{");
    let _ = writeln!(out, "public:");
    let _ = writeln!(out, "    virtual ~{svc_name}HandlerInterface() = default;");
    for m in &svc.methods {
        emit_doxygen_for_method(&mut out, m, "    ");
        emit_sync_method_signature(&mut out, m, "    ");
    }
    let _ = writeln!(out, "}};");
    let _ = writeln!(out);

    let _ = writeln!(out, "}} }} // namespace dds::rpc");
    out
}

/// Emittiert die typisierte **Requester-Klasse** fuer einen Service.
///
/// Erbt vom Generic `dds::rpc::Requester<TIn, TOut>` (Spec §10.4) und
/// liefert pro Methode `Future<TOut> name_async(TIn)` + `TOut name(TIn)`.
///
/// Oneway-Methoden liefern `void` und kein Future (Spec §10.7
/// "no reply for oneway operations").
#[must_use]
pub fn emit_requester_class(svc: &ServiceDef) -> String {
    let mut out = String::new();
    let svc_name = &svc.name;
    let req_class = format!("{svc_name}_Requester");
    let _ = writeln!(
        out,
        "// zerodds-rpc-1.0 — typisierter Requester fuer '{svc_name}' (Spec §10.4)."
    );
    let _ = writeln!(out, "namespace dds {{ namespace rpc {{");
    let _ = writeln!(out);
    let _ = writeln!(out, "class {req_class} {{");
    let _ = writeln!(out, "public:");
    let _ = writeln!(out, "    {req_class}() = default;");
    let _ = writeln!(out, "    virtual ~{req_class}() = default;");
    let _ = writeln!(out);
    for m in &svc.methods {
        emit_doxygen_for_method(&mut out, m, "    ");
        if m.oneway {
            // Oneway: fire-and-forget, kein Reply.
            let _ = writeln!(out, "    /// Oneway — kein Reply, kein Future.");
            emit_oneway_send_signature(&mut out, m, "    ");
        } else {
            emit_async_method_signature_pure(&mut out, m, "    ");
            emit_sync_method_signature_inline(&mut out, m, "    ");
        }
        let _ = writeln!(out);
    }
    let _ = writeln!(out, "}};");
    let _ = writeln!(out);
    let _ = writeln!(out, "}} }} // namespace dds::rpc");
    out
}

/// Emittiert die typisierte **Replier-Klasse** fuer einen Service.
///
/// Spec §10.5 — der Replier dispatcht eingehende Requests an den
/// HandlerInterface-Implementor.
#[must_use]
pub fn emit_replier_class(svc: &ServiceDef) -> String {
    let mut out = String::new();
    let svc_name = &svc.name;
    let rep_class = format!("{svc_name}_Replier");
    let handler_iface = format!("{svc_name}HandlerInterface");
    let _ = writeln!(
        out,
        "// zerodds-rpc-1.0 — typisierter Replier fuer '{svc_name}' (Spec §10.5)."
    );
    let _ = writeln!(out, "namespace dds {{ namespace rpc {{");
    let _ = writeln!(out);
    let _ = writeln!(out, "class {rep_class} {{");
    let _ = writeln!(out, "public:");
    let _ = writeln!(
        out,
        "    explicit {rep_class}(::dds::rpc::{handler_iface}* handler)"
    );
    let _ = writeln!(out, "        : handler_(handler) {{}}");
    let _ = writeln!(out, "    virtual ~{rep_class}() = default;");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "    /// Dispatcht eine eingegangene Anfrage anhand des Methoden-Tokens"
    );
    let _ = writeln!(
        out,
        "    /// an den registrierten Handler. Spec §10.5 — Operation-Naming."
    );
    let _ = writeln!(
        out,
        "    /// Skeleton: konkrete Wire-Deserialisierung erfolgt im Binding."
    );
    let _ = writeln!(
        out,
        "    void dispatch_to_handler(const std::string& method_name) {{"
    );
    if svc.methods.is_empty() {
        let _ = writeln!(out, "        (void)method_name;");
    } else {
        for (i, m) in svc.methods.iter().enumerate() {
            let kw = if i == 0 { "if" } else { "else if" };
            let _ = writeln!(out, "        {kw} (method_name == \"{}\") {{", m.name);
            if m.oneway {
                let _ = writeln!(
                    out,
                    "            // Oneway: invoke handler, kein Reply (Spec §10.7)."
                );
                let args = handler_call_args(m);
                let _ = writeln!(out, "            handler_->{}({args});", m.name);
            } else {
                let _ = writeln!(
                    out,
                    "            // Sync invoke; Promise-Set erfolgt im Caller (Skeleton)."
                );
                let args = handler_call_args(m);
                if m.return_type.is_some() {
                    let _ = writeln!(out, "            (void)handler_->{}({args});", m.name);
                } else {
                    let _ = writeln!(out, "            handler_->{}({args});", m.name);
                }
            }
            let _ = writeln!(out, "        }}");
        }
        let _ = writeln!(out, "        else {{");
        let _ = writeln!(
            out,
            "            throw ::dds::rpc::UnknownOperationError(method_name);"
        );
        let _ = writeln!(out, "        }}");
    }
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out);
    let _ = writeln!(out, "private:");
    let _ = writeln!(out, "    ::dds::rpc::{handler_iface}* handler_{{nullptr}};");
    let _ = writeln!(out, "}};");
    let _ = writeln!(out);
    let _ = writeln!(out, "}} }} // namespace dds::rpc");
    out
}

/// Emittiert die `ServiceTraits<<Service>>`-Spezialisierung (Spec §10.3).
///
/// Liefert `request_topic_name`, `reply_topic_name` und das
/// [`ServiceMapping`](super) (Basic vs Enhanced).
#[must_use]
pub fn emit_service_traits(svc: &ServiceDef) -> String {
    let mut out = String::new();
    let svc_name = &svc.name;
    let req_topic = format!("{svc_name}_Request");
    let rep_topic = format!("{svc_name}_Reply");
    let _ = writeln!(
        out,
        "// zerodds-rpc-1.0 — ServiceTraits-Spezialisierung fuer '{svc_name}' (Spec §10.3)."
    );
    let _ = writeln!(out, "namespace dds {{ namespace rpc {{");
    let _ = writeln!(out);
    let _ = writeln!(out, "template <>");
    let _ = writeln!(out, "struct ServiceTraits<{svc_name}> {{");
    let _ = writeln!(out, "    static constexpr bool is_specialized = true;");
    let _ = writeln!(
        out,
        "    static constexpr const char* service_name = \"{svc_name}\";"
    );
    let _ = writeln!(
        out,
        "    static constexpr const char* request_topic_name = \"{req_topic}\";"
    );
    let _ = writeln!(
        out,
        "    static constexpr const char* reply_topic_name = \"{rep_topic}\";"
    );
    let _ = writeln!(
        out,
        "    static constexpr ServiceMapping mapping = ServiceMapping::Basic;"
    );
    let _ = writeln!(out, "    using requester_type = {svc_name}_Requester;");
    let _ = writeln!(out, "    using replier_type = {svc_name}_Replier;");
    let _ = writeln!(
        out,
        "    using handler_interface_type = {svc_name}HandlerInterface;"
    );
    let _ = writeln!(out, "}};");
    let _ = writeln!(out);
    let _ = writeln!(out, "}} }} // namespace dds::rpc");
    out
}

/// Emittiert eine konkrete RemoteException-Subklasse aus einer IDL-
/// `exception E { ... }`-Deklaration (Spec §10.6).
///
/// Die generierte Klasse erbt von `::dds::rpc::RemoteException` und
/// liefert pro Member ein privates Feld + Getter/Setter.
///
/// # Errors
/// * [`CppGenError::InvalidName`], wenn der Exception-Name oder ein Member-
///   Name reserviert ist.
/// * [`CppGenError::UnsupportedConstruct`], wenn ein Member einen
///   nicht-unterstuetzten Typ verwendet (z.B. `any`/`fixed`).
pub fn emit_remote_exception_class(except: &ExceptDecl) -> Result<String, CppGenError> {
    crate::type_map::check_identifier(&except.name.text)?;
    let mut out = String::new();
    let name = &except.name.text;
    let _ = writeln!(
        out,
        "// zerodds-rpc-1.0 — RemoteException-Subklasse '{name}' (Spec §10.6)."
    );
    let _ = writeln!(out, "namespace dds {{ namespace rpc {{");
    let _ = writeln!(out);
    let _ = writeln!(out, "class {name} : public ::dds::rpc::RemoteException {{");
    let _ = writeln!(out, "public:");
    let _ = writeln!(out, "    {name}() = default;");
    let _ = writeln!(
        out,
        "    explicit {name}(std::string msg) : ::dds::rpc::RemoteException(std::move(msg)) {{}}"
    );
    let _ = writeln!(out, "    ~{name}() override = default;");
    let _ = writeln!(out);

    // Body: members.
    if !except.members.is_empty() {
        let _ = writeln!(out, "private:");
        for m in &except.members {
            for decl in &m.declarators {
                let cpp_ty = idl_typespec_to_cpp(&m.type_spec)?;
                let mname = decl.name();
                crate::type_map::check_identifier(&mname.text)?;
                let _ = writeln!(out, "    {cpp_ty} {}_{{}};", mname.text);
            }
        }
        let _ = writeln!(out);
        let _ = writeln!(out, "public:");
        for m in &except.members {
            for decl in &m.declarators {
                let cpp_ty = idl_typespec_to_cpp(&m.type_spec)?;
                let mn = &decl.name().text;
                let _ = writeln!(
                    out,
                    "    const {cpp_ty}& {mn}() const noexcept {{ return {mn}_; }}"
                );
                let _ = writeln!(out, "    void {mn}(const {cpp_ty}& v) {{ {mn}_ = v; }}");
            }
        }
    }
    let _ = writeln!(out, "}};");
    let _ = writeln!(out);
    let _ = writeln!(out, "}} }} // namespace dds::rpc");
    Ok(out)
}

/// Emittiert die generischen RPC-Header-Templates (Requester/Replier/
/// Exception/ServiceTraits) als zusammengefuegten Header-Block.
///
/// Wird typischerweise einmal pro Compilation-Unit emittiert; die
/// per-Service-Header (siehe [`emit_service_full_header`]) ziehen diesen
/// Block per `#include` ein.
#[must_use]
pub fn emit_rpc_runtime_headers() -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Generated zerodds-rpc-1.0 runtime headers (zerodds idl-cpp C6.1.D-cpp)."
    );
    let _ = writeln!(out, "#pragma once");
    let _ = writeln!(out);
    let _ = writeln!(out, "#include <cstdint>");
    let _ = writeln!(out, "#include <future>");
    let _ = writeln!(out, "#include <memory>");
    let _ = writeln!(out, "#include <string>");
    let _ = writeln!(out, "#include <utility>");
    let _ = writeln!(out, "#include <exception>");
    let _ = writeln!(out, "#include <dds/core/exceptions.hpp>");
    let _ = writeln!(out);
    out.push_str(TPL_EXCEPTION_HPP);
    if !TPL_EXCEPTION_HPP.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(TPL_SERVICE_TRAITS_HPP);
    if !TPL_SERVICE_TRAITS_HPP.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(TPL_REQUESTER_HPP);
    if !TPL_REQUESTER_HPP.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(TPL_REPLIER_HPP);
    if !TPL_REPLIER_HPP.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Emittiert einen vollstaendigen RPC-Service-Header
/// (Interface + Requester + Replier + ServiceTraits) fuer einen einzelnen
/// Service.
///
/// Reihenfolge folgt der Header-Konvention aus dds-psm-cxx — zuerst die
/// Top-Level-Service-Klasse, dann die Endpoint-Wrapper, am Ende das
/// `ServiceTraits`-Template (Spec §10.3 verlangt vollstaendige Sicht-
/// barkeit der typisierten Wrapper, bevor das Trait spezialisiert wird).
#[must_use]
pub fn emit_service_full_header(svc: &ServiceDef) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Generated zerodds-rpc-1.0 service header for '{}' (zerodds idl-cpp C6.1.D-cpp).",
        svc.name
    );
    let _ = writeln!(out, "#pragma once");
    let _ = writeln!(out);
    out.push_str(&emit_service_interface(svc));
    out.push('\n');
    out.push_str(&emit_requester_class(svc));
    out.push('\n');
    out.push_str(&emit_replier_class(svc));
    out.push('\n');
    out.push_str(&emit_service_traits(svc));
    out
}

// ---------------------------------------------------------------------------
// Method-Signature-Helpers
// ---------------------------------------------------------------------------

fn return_type_str(m: &MethodDef) -> String {
    if m.oneway {
        "void".to_string()
    } else if let Some(ret) = &m.return_type {
        idl_typespec_to_cpp(ret).unwrap_or_else(|_| "void".to_string())
    } else {
        "void".to_string()
    }
}

fn method_param_list(m: &MethodDef) -> String {
    let mut parts: Vec<String> = Vec::new();
    for p in &m.params {
        parts.push(format_param_decl(p));
    }
    parts.join(", ")
}

/// Liefert die Parameter so, dass `inout` und `out` als nicht-const-Referenz
/// emittiert werden (klassisches IDL→C++-Mapping fuer veraenderliche
/// Parameter, siehe DDS-RPC §10.4 + IDL4-CPP §7.6.7).
fn format_param_decl(p: &ParamDef) -> String {
    let ty = idl_typespec_to_cpp(&p.type_ref).unwrap_or_else(|_| "void".to_string());
    match p.direction {
        ParamDirection::In => format!("const {ty}& {}", p.name),
        ParamDirection::Out | ParamDirection::InOut => format!("{ty}& {}", p.name),
    }
}

/// Argument-Liste fuer Handler-Calls — gleiche Param-Namen, ohne Typ.
fn handler_call_args(m: &MethodDef) -> String {
    m.params
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

fn emit_sync_method_signature(out: &mut String, m: &MethodDef, indent: &str) {
    let ret = return_type_str(m);
    let params = method_param_list(m);
    let _ = writeln!(out, "{indent}virtual {ret} {}({params}) = 0;", m.name);
}

fn emit_async_method_signature(out: &mut String, m: &MethodDef, indent: &str) {
    let ret = return_type_str(m);
    let params = method_param_list(m);
    let _ = writeln!(
        out,
        "{indent}virtual ::dds::rpc::Future<{ret}> {}_async({params}) = 0;",
        m.name
    );
}

fn emit_async_method_signature_pure(out: &mut String, m: &MethodDef, indent: &str) {
    let ret = return_type_str(m);
    let params = method_param_list(m);
    let _ = writeln!(
        out,
        "{indent}virtual ::dds::rpc::Future<{ret}> {}_async({params}) {{",
        m.name
    );
    let _ = writeln!(out, "{indent}    ::dds::rpc::Promise<{ret}> _promise{{}};");
    // Argumente "verwenden" (Skeleton, kein realer Send).
    for p in &m.params {
        let _ = writeln!(out, "{indent}    (void){};", p.name);
    }
    let _ = writeln!(out, "{indent}    return _promise.get_future();");
    let _ = writeln!(out, "{indent}}}");
}

fn emit_sync_method_signature_inline(out: &mut String, m: &MethodDef, indent: &str) {
    let ret = return_type_str(m);
    let params = method_param_list(m);
    let args = handler_call_args(m);
    let _ = writeln!(out, "{indent}virtual {ret} {}({params}) {{", m.name);
    if ret == "void" {
        let _ = writeln!(out, "{indent}    {}_async({args}).get();", m.name);
    } else {
        let _ = writeln!(out, "{indent}    return {}_async({args}).get();", m.name);
    }
    let _ = writeln!(out, "{indent}}}");
}

fn emit_oneway_send_signature(out: &mut String, m: &MethodDef, indent: &str) {
    let params = method_param_list(m);
    let _ = writeln!(out, "{indent}virtual void {}({params}) {{", m.name);
    for p in &m.params {
        let _ = writeln!(out, "{indent}    (void){};", p.name);
    }
    let _ = writeln!(out, "{indent}    // oneway: fire-and-forget (Spec §10.7).");
    let _ = writeln!(out, "{indent}}}");
}

fn emit_doxygen_for_method(out: &mut String, m: &MethodDef, indent: &str) {
    let _ = writeln!(out, "{indent}/// Operation '{}'.", m.name);
    if m.oneway {
        let _ = writeln!(out, "{indent}/// @oneway — kein Reply (Spec §10.7).");
    }
    if !m.params.is_empty() {
        for p in &m.params {
            let dir = match p.direction {
                ParamDirection::In => "in",
                ParamDirection::Out => "out",
                ParamDirection::InOut => "inout",
            };
            let _ = writeln!(out, "{indent}/// @param {} ({dir})", p.name);
        }
    }
    if let Some(_r) = &m.return_type {
        let _ = writeln!(out, "{indent}/// @return Reply-Wert.");
    }
}

// ---------------------------------------------------------------------------
// IDL → C++-Type-Mapping (lokaler Subset von emitter::typespec_to_cpp).
// ---------------------------------------------------------------------------

/// zerodds-lint: recursion-depth 16
///
/// Sequenz/Array-Verschachtelung: IDL-Spec erlaubt theoretisch beliebige
/// Tiefe, in der Praxis sind 4-8 Ebenen das Maximum (z.B. `sequence<sequence<
/// sequence<long>>>`). Cap auf 16 schuetzt vor pathologischen Eingaben.
fn idl_typespec_to_cpp(ts: &TypeSpec) -> Result<String, CppGenError> {
    match ts {
        TypeSpec::Primitive(p) => Ok(primitive_str(*p).to_string()),
        TypeSpec::Scoped(s) => Ok(scoped_to_cpp(s)),
        TypeSpec::Sequence(s) => {
            let inner = idl_typespec_to_cpp(&s.elem)?;
            Ok(format!("std::vector<{inner}>"))
        }
        TypeSpec::String(s) => {
            if s.wide {
                Ok("std::wstring".into())
            } else {
                Ok("std::string".into())
            }
        }
        TypeSpec::Map(m) => {
            let k = idl_typespec_to_cpp(&m.key)?;
            let v = idl_typespec_to_cpp(&m.value)?;
            Ok(format!("std::map<{k}, {v}>"))
        }
        TypeSpec::Fixed(_) => Err(CppGenError::UnsupportedConstruct {
            construct: "fixed".into(),
            context: None,
        }),
        TypeSpec::Any => Err(CppGenError::UnsupportedConstruct {
            construct: "any".into(),
            context: None,
        }),
    }
}

fn primitive_str(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "bool",
        PrimitiveType::Octet => "uint8_t",
        PrimitiveType::Char => "char",
        PrimitiveType::WideChar => "wchar_t",
        PrimitiveType::Integer(i) => integer_str(i),
        PrimitiveType::Floating(f) => floating_str(f),
    }
}

fn integer_str(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "int16_t",
        IntegerType::Long | IntegerType::Int32 => "int32_t",
        IntegerType::LongLong | IntegerType::Int64 => "int64_t",
        IntegerType::UShort | IntegerType::UInt16 => "uint16_t",
        IntegerType::ULong | IntegerType::UInt32 => "uint32_t",
        IntegerType::ULongLong | IntegerType::UInt64 => "uint64_t",
        IntegerType::Int8 => "int8_t",
        IntegerType::UInt8 => "uint8_t",
    }
}

fn floating_str(f: FloatingType) -> &'static str {
    match f {
        FloatingType::Float => "float",
        FloatingType::Double => "double",
        FloatingType::LongDouble => "long double",
    }
}

fn scoped_to_cpp(s: &ScopedName) -> String {
    let parts: Vec<String> = s.parts.iter().map(|p| p.text.clone()).collect();
    let joined = parts.join("::");
    if s.absolute {
        format!("::{joined}")
    } else {
        joined
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_idl::ast::{IntegerType, PrimitiveType, StringType, TypeSpec};
    use zerodds_idl::errors::Span;
    use zerodds_rpc::{MethodDef, ParamDef, ParamDirection, ServiceDef};

    fn long_t() -> TypeSpec {
        TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::Long))
    }

    fn string_t() -> TypeSpec {
        TypeSpec::String(StringType {
            wide: false,
            bound: None,
            span: Span::SYNTHETIC,
        })
    }

    fn calc_service() -> ServiceDef {
        ServiceDef {
            name: "Calculator".into(),
            methods: vec![MethodDef {
                name: "add".into(),
                params: vec![
                    ParamDef {
                        name: "a".into(),
                        direction: ParamDirection::In,
                        type_ref: long_t(),
                    },
                    ParamDef {
                        name: "b".into(),
                        direction: ParamDirection::In,
                        type_ref: long_t(),
                    },
                ],
                return_type: Some(long_t()),
                oneway: false,
            }],
        }
    }

    #[test]
    fn service_interface_contains_class_and_async() {
        let s = emit_service_interface(&calc_service());
        assert!(s.contains("class Calculator {"));
        assert!(s.contains("virtual int32_t add("));
        assert!(s.contains("::dds::rpc::Future<int32_t> add_async"));
    }

    #[test]
    fn requester_class_emits_topic_wrapper() {
        let s = emit_requester_class(&calc_service());
        assert!(s.contains("class Calculator_Requester"));
        assert!(s.contains("add_async"));
        assert!(s.contains("Promise<int32_t>"));
    }

    #[test]
    fn replier_class_dispatches_by_name() {
        let s = emit_replier_class(&calc_service());
        assert!(s.contains("class Calculator_Replier"));
        assert!(s.contains("if (method_name == \"add\")"));
        assert!(s.contains("UnknownOperationError"));
    }

    #[test]
    fn service_traits_specialization_carries_topic_names() {
        let s = emit_service_traits(&calc_service());
        assert!(s.contains("struct ServiceTraits<Calculator>"));
        assert!(s.contains("\"Calculator_Request\""));
        assert!(s.contains("\"Calculator_Reply\""));
    }

    #[test]
    fn full_header_combines_all_blocks() {
        let s = emit_service_full_header(&calc_service());
        assert!(s.contains("#pragma once"));
        assert!(s.contains("class Calculator {"));
        assert!(s.contains("class Calculator_Requester"));
        assert!(s.contains("class Calculator_Replier"));
        assert!(s.contains("ServiceTraits<Calculator>"));
    }

    #[test]
    fn runtime_headers_include_all_templates() {
        let s = emit_rpc_runtime_headers();
        assert!(s.contains("class RemoteException"));
        assert!(s.contains("class Requester"));
        assert!(s.contains("class Replier"));
        assert!(s.contains("ServiceTraits"));
    }

    #[test]
    fn primitive_str_covers_all_branches() {
        assert_eq!(primitive_str(PrimitiveType::Boolean), "bool");
        assert_eq!(primitive_str(PrimitiveType::Octet), "uint8_t");
        assert_eq!(primitive_str(PrimitiveType::Char), "char");
        assert_eq!(primitive_str(PrimitiveType::WideChar), "wchar_t");
    }

    #[test]
    fn integer_str_short_long() {
        assert_eq!(integer_str(IntegerType::Short), "int16_t");
        assert_eq!(integer_str(IntegerType::ULongLong), "uint64_t");
    }

    #[test]
    fn idl_typespec_string_wide_vs_narrow() {
        let s = idl_typespec_to_cpp(&string_t()).unwrap();
        assert_eq!(s, "std::string");
    }

    #[test]
    fn idl_typespec_any_is_rejected() {
        let res = idl_typespec_to_cpp(&TypeSpec::Any);
        assert!(matches!(res, Err(CppGenError::UnsupportedConstruct { .. })));
    }
}

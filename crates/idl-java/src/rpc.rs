// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL-Service → Java-RPC-Codegen (DDS-RPC 1.0 §7.11.2 — Java-PSM).
//!
//! Dieser Modul ist die Bruecke zwischen dem typisierten RPC-Datenmodell
//! aus `zerodds-rpc` (`ServiceDef`/`MethodDef`/`ParamDef`) und dem
//! Java-Source-Codegen. Pro Service emittieren wir vier Klassen:
//!
//! * `<Service>.java`         — synchrones Interface mit allen Methoden
//!                              (`throws RemoteException` + User-Exceptions).
//! * `<Service>Async.java`    — asynchrones Interface mit
//!                              `CompletableFuture<TOut>`-Returns
//!                              (Spec §7.11.2.2.4 mappt `Future<T>` auf
//!                              `java.util.concurrent.Future<T>`; wir
//!                              nutzen `CompletableFuture` als
//!                              vollstaendige Implementierung).
//! * `<Service>Requester.java` — Client-Side: kapselt einen
//!                              `org.zerodds.rpc.Requester<TIn,TOut>`
//!                              und implementiert `<Service>` +
//!                              `<Service>Async`.
//! * `<Service>Replier.java`   — Server-Side: kapselt einen
//!                              `org.zerodds.rpc.Replier<TIn,TOut>` und
//!                              dispatcht eingehende Requests an einen
//!                              `<Service>Service`-Handler.
//! * `<Service>Service.java`   — Server-Side Handler-Interface: das
//!                              Service-Implementor implementiert dieses
//!                              Interface.
//!
//! # Out-Parameter
//! Java hat kein `out`/`inout`-Konzept. Wir mappen `out`/`inout` ueber das
//! **Holder-Pattern** (Spec §7.11.2.3 / IDL-to-Java 1.3 §1.5):
//!
//! ```java
//! public final class IntHolder {
//!     public int value;
//!     public IntHolder() {}
//!     public IntHolder(int v) { this.value = v; }
//! }
//! ```
//!
//! Holder werden **nicht** pro Service emittiert — wir referenzieren die
//! generischen Holders aus `org.zerodds.rpc.Holder<T>` (siehe
//! `runtime/rpc/Holder.java`). Begruendung: weniger Boilerplate als
//! pro-Type-Holder, vollstaendig kompatibel mit dem Spec-Pattern (der
//! Caller schreibt vor dem Aufruf in `holder.value`, der Reply-Decoder
//! ueberschreibt nach Rueckkehr). Alternative `Object[]`-Wrapper waere
//! type-unsafe und scheidet daher aus.
//!
//! # Exception-Mapping (Spec §7.11.2.1)
//! * IDL `exception E { ... }` → wir delegieren an den existierenden
//!   `emit_exception_file`-Pfad (`E extends RuntimeException`); der RPC-
//!   Codegen ergaenzt nur die `throws E1, E2`-Klausel auf der Methode.
//! * `org.zerodds.rpc.RemoteException` ist eine RuntimeException-
//!   Subclass, die jede RPC-Methode implizit werfen darf — wir setzen
//!   sie deshalb **nicht** explizit in den `throws`-Listen, weil
//!   RuntimeException in Java nicht checked ist.
//!
//! # Was diese Stufe NICHT macht
//! * Keine `javac`/`mvn`-Integration — der Output ist Skeleton-Java
//!   ohne JVM-Build-Verifikation.
//! * Keine JNI-Anbindung — `Requester`/`Replier` halten `Object`-Felder
//!   fuer den Native-Handle (Stub-Form).
//! * Keine Reflection-TypeRep (Stretch in idl4-java §8).

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write;

use zerodds_idl::ast::{IntegerType, PrimitiveType, ScopedName, TypeSpec};
use zerodds_rpc::service_mapping::{MethodDef, ParamDef, ParamDirection, ServiceDef};

use crate::JavaGenOptions;
use crate::emitter::{JavaFile, fmt_err, indent_unit, wrap_compilation_unit_default};
use crate::error::JavaGenError;
use crate::keywords::sanitize_identifier;
use crate::type_map::{
    floating_to_java, floating_to_java_boxed, integer_to_java, integer_to_java_boxed,
    primitive_to_java, primitive_to_java_boxed,
};

// ---------------------------------------------------------------------------
// Public API — siehe Crate-Doc fuer Aufruf-Schema
// ---------------------------------------------------------------------------

/// Emittiert das synchrone Service-Interface (`<Service>.java`).
///
/// # Errors
/// `JavaGenError::InvalidName` bei nicht-sanitisierbarem Service-Namen.
pub fn emit_service_interface(
    svc: &ServiceDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let class = sanitize_identifier(&svc.name)?;
    let ind = indent_unit(opts);
    let mut body = String::new();
    writeln!(body, "/** Synchronous service interface for {class}. */").map_err(fmt_err)?;
    writeln!(body, "@org.zerodds.rpc.Service(\"{}\")", svc.name).map_err(fmt_err)?;
    writeln!(body, "public interface {class} {{").map_err(fmt_err)?;
    for m in &svc.methods {
        emit_sync_method_signature(&mut body, m, &ind)?;
    }
    writeln!(body, "}}").map_err(fmt_err)?;
    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

/// Emittiert das asynchrone Service-Interface (`<Service>Async.java`).
///
/// # Errors
/// Wie [`emit_service_interface`].
pub fn emit_service_interface_async(
    svc: &ServiceDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let svc_class = sanitize_identifier(&svc.name)?;
    let class = format!("{svc_class}Async");
    let ind = indent_unit(opts);
    let mut body = String::new();
    writeln!(
        body,
        "/** Asynchronous service interface for {svc_class}. */"
    )
    .map_err(fmt_err)?;
    writeln!(body, "public interface {class} {{").map_err(fmt_err)?;
    for m in &svc.methods {
        emit_async_method_signature(&mut body, m, &ind)?;
    }
    writeln!(body, "}}").map_err(fmt_err)?;
    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

/// Emittiert die Client-seitige Requester-Klasse (`<Service>Requester.java`).
///
/// # Errors
/// Wie [`emit_service_interface`].
pub fn emit_requester_class(
    svc: &ServiceDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let svc_class = sanitize_identifier(&svc.name)?;
    let class = format!("{svc_class}Requester");
    let ind = indent_unit(opts);
    let mut body = String::new();
    writeln!(
        body,
        "/** Client-side proxy for {svc_class}. Implements both the \
         synchronous {svc_class} interface and the {svc_class}Async \
         interface. */",
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "public final class {class} implements {svc_class}, {svc_class}Async {{",
    )
    .map_err(fmt_err)?;
    if cfg!(feature = "jni") {
        // JNI-Variante: wir delegieren direkt an `RustRequesterFFI` und
        // halten den native Handle dort.
        writeln!(
            body,
            "{ind}private final org.zerodds.rpc.RustRequesterFFI requesterFfi;",
        )
        .map_err(fmt_err)?;
        writeln!(body).map_err(fmt_err)?;
        writeln!(
            body,
            "{ind}public {class}(org.zerodds.rpc.RustRequesterFFI requesterFfi) {{",
        )
        .map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}this.requesterFfi = requesterFfi;").map_err(fmt_err)?;
        writeln!(body, "{ind}}}").map_err(fmt_err)?;
        writeln!(body).map_err(fmt_err)?;
    } else {
        writeln!(
            body,
            "{ind}private final org.zerodds.rpc.Requester<Object, Object> requester;",
        )
        .map_err(fmt_err)?;
        writeln!(body).map_err(fmt_err)?;
        writeln!(
            body,
            "{ind}public {class}(org.zerodds.rpc.Requester<Object, Object> requester) {{",
        )
        .map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}this.requester = requester;").map_err(fmt_err)?;
        writeln!(body, "{ind}}}").map_err(fmt_err)?;
        writeln!(body).map_err(fmt_err)?;
    }

    // Sync-Methoden: rufen die Async-Variante auf und blocken.
    for m in &svc.methods {
        emit_requester_sync_impl(&mut body, m, &ind)?;
    }
    writeln!(body).map_err(fmt_err)?;
    // Async-Methoden: senden Request, geben CompletableFuture zurueck.
    for m in &svc.methods {
        emit_requester_async_impl(&mut body, m, &ind)?;
    }

    writeln!(body, "}}").map_err(fmt_err)?;
    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

/// Emittiert die Server-seitige Replier-Klasse (`<Service>Replier.java`).
///
/// # Errors
/// Wie [`emit_service_interface`].
pub fn emit_replier_class(
    svc: &ServiceDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let svc_class = sanitize_identifier(&svc.name)?;
    let class = format!("{svc_class}Replier");
    let handler_iface = format!("{svc_class}Service");
    let ind = indent_unit(opts);
    let mut body = String::new();
    writeln!(
        body,
        "/** Server-side replier for {svc_class}. Wires a {handler_iface} \
         implementation to the underlying RPC runtime. */",
    )
    .map_err(fmt_err)?;
    writeln!(body, "public final class {class} {{").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}private final org.zerodds.rpc.Replier<Object, Object> replier;",
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}private final {handler_iface} handler;").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public {class}(org.zerodds.rpc.Replier<Object, Object> replier, {handler_iface} handler) {{",
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}this.replier = replier;").map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}this.handler = handler;").map_err(fmt_err)?;
    writeln!(body, "{ind}}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // Dispatch-Stub: nimmt eine Request entgegen und ruft die richtige
    // Handler-Methode anhand der Method-ID auf.
    writeln!(
        body,
        "{ind}/** Dispatches an incoming request by method id. */",
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public Object dispatch(int methodId, Object args) {{",
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}switch (methodId) {{").map_err(fmt_err)?;
    for (idx, m) in svc.methods.iter().enumerate() {
        let mname = sanitize_identifier(&m.name)?;
        let case_id = idx + 1;
        // void-return (inkl. oneway) → kein `return` vor dem Aufruf;
        // sonst Result als `return`-Wert weiterreichen.
        let void_like = m.oneway
            || (m.return_type.is_none()
                && m.params.iter().all(|p| p.direction == ParamDirection::In));
        let stub = if void_like {
            format!("{ind}{ind}{ind}case {case_id}: handler.{mname}(/* args */); return null;")
        } else {
            format!("{ind}{ind}{ind}case {case_id}: return handler.{mname}(/* args */);")
        };
        writeln!(body, "{stub}").map_err(fmt_err)?;
    }
    writeln!(
        body,
        "{ind}{ind}{ind}default: throw new org.zerodds.rpc.RemoteException(\
         \"unknown method id: \" + methodId, \
         org.zerodds.rpc.RemoteExceptionCode.UNKNOWN_OPERATION);",
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}}}").map_err(fmt_err)?;
    writeln!(body, "{ind}}}").map_err(fmt_err)?;

    writeln!(body, "}}").map_err(fmt_err)?;
    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

/// Emittiert das Server-side Handler-Interface (`<Service>Service.java`).
///
/// Anwender implementieren dieses Interface und uebergeben ihre
/// Implementierung dem [`emit_replier_class`]-Output.
///
/// # Errors
/// Wie [`emit_service_interface`].
pub fn emit_service_handler_interface(
    svc: &ServiceDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let svc_class = sanitize_identifier(&svc.name)?;
    let class = format!("{svc_class}Service");
    let ind = indent_unit(opts);
    let mut body = String::new();
    writeln!(
        body,
        "/** Server-side handler interface for {svc_class}. Implementors \
         provide the actual business logic; a {svc_class}Replier wires \
         them to the RPC runtime. */",
    )
    .map_err(fmt_err)?;
    writeln!(body, "public interface {class} {{").map_err(fmt_err)?;
    for m in &svc.methods {
        emit_handler_method_signature(&mut body, m, &ind)?;
    }
    writeln!(body, "}}").map_err(fmt_err)?;
    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

/// Convenience-Wrapper: emittiert alle 5 Java-Files fuer ein Service.
///
/// Reihenfolge: Interface, Async-Interface, Handler-Interface, Requester,
/// Replier.
///
/// # Errors
/// Wie [`emit_service_interface`].
pub fn emit_service_files(
    svc: &ServiceDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<Vec<JavaFile>, JavaGenError> {
    Ok(alloc::vec![
        emit_service_interface(svc, pkg, opts)?,
        emit_service_interface_async(svc, pkg, opts)?,
        emit_service_handler_interface(svc, pkg, opts)?,
        emit_requester_class(svc, pkg, opts)?,
        emit_replier_class(svc, pkg, opts)?,
    ])
}

// ---------------------------------------------------------------------------
// Method-Signature-Emitter
// ---------------------------------------------------------------------------

fn emit_sync_method_signature(
    out: &mut String,
    m: &MethodDef,
    ind: &str,
) -> Result<(), JavaGenError> {
    let name = sanitize_identifier(&m.name)?;
    let ret = sync_return_type(m)?;
    let params = render_method_params(m)?;
    let throws = String::new();
    if m.oneway {
        writeln!(out, "{ind}@org.zerodds.rpc.Oneway").map_err(fmt_err)?;
    }
    writeln!(out, "{ind}{ret} {name}({params}){throws};").map_err(fmt_err)?;
    Ok(())
}

fn emit_async_method_signature(
    out: &mut String,
    m: &MethodDef,
    ind: &str,
) -> Result<(), JavaGenError> {
    let name = sanitize_identifier(&m.name)?;
    let async_name = format!("{name}Async");
    let ret = async_return_type(m)?;
    let params = render_method_params_async(m)?;
    if m.oneway {
        writeln!(out, "{ind}@org.zerodds.rpc.Oneway").map_err(fmt_err)?;
    }
    writeln!(out, "{ind}{ret} {async_name}({params});").map_err(fmt_err)?;
    Ok(())
}

fn emit_handler_method_signature(
    out: &mut String,
    m: &MethodDef,
    ind: &str,
) -> Result<(), JavaGenError> {
    // Handler-Signatur ist identisch zur sync-Signatur — der
    // Replier-Dispatch ruft sie direkt auf.
    emit_sync_method_signature(out, m, ind)
}

fn emit_requester_sync_impl(
    out: &mut String,
    m: &MethodDef,
    ind: &str,
) -> Result<(), JavaGenError> {
    let name = sanitize_identifier(&m.name)?;
    let async_name = format!("{name}Async");
    let ret_ty = sync_return_type(m)?;
    let params = render_method_params(m)?;
    let arg_list = render_call_arglist(m)?;
    writeln!(out, "{ind}@Override").map_err(fmt_err)?;
    writeln!(out, "{ind}public {ret_ty} {name}({params}) {{").map_err(fmt_err)?;
    if m.oneway {
        writeln!(out, "{ind}{ind}{async_name}({arg_list});").map_err(fmt_err)?;
        writeln!(out, "{ind}}}").map_err(fmt_err)?;
        return Ok(());
    }
    if m.return_type.is_none() && m.params.iter().all(|p| p.direction == ParamDirection::In) {
        writeln!(
            out,
            "{ind}{ind}try {{ {async_name}({arg_list}).get(); }} catch (Exception e) {{ \
             throw new org.zerodds.rpc.RemoteException(e); }}",
        )
        .map_err(fmt_err)?;
    } else {
        writeln!(
            out,
            "{ind}{ind}try {{ return {async_name}({arg_list}).get(); }} catch (Exception e) {{ \
             throw new org.zerodds.rpc.RemoteException(e); }}",
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    Ok(())
}

fn emit_requester_async_impl(
    out: &mut String,
    m: &MethodDef,
    ind: &str,
) -> Result<(), JavaGenError> {
    let name = sanitize_identifier(&m.name)?;
    let async_name = format!("{name}Async");
    let ret_ty = async_return_type(m)?;
    let params = render_method_params_async(m)?;
    writeln!(out, "{ind}@Override").map_err(fmt_err)?;
    writeln!(out, "{ind}public {ret_ty} {async_name}({params}) {{").map_err(fmt_err)?;

    // JNI-Skeleton: Wenn das `jni`-Feature aktiv ist, zeigen wir
    // direkt den Rust-FFI-Pfad. Der Marshalling-Code ist bewusst
    // als Kommentar markiert — pro Service waere ein dedizierter
    // XCDR2-Encoder noetig, der in einer spaeteren Codegen-Stufe
    // emittiert wird (idl4-java §8 / zerodds-rpc §7.11).
    if cfg!(feature = "jni") {
        if m.oneway {
            writeln!(
                out,
                "{ind}{ind}// JNI: oneway -> Requester.sendRequest blocking + ignore reply.",
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{ind}{ind}requesterFfi.sendRequest(/* xcdr2_encode(args) */ new byte[0], 0L);",
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{ind}{ind}return java.util.concurrent.CompletableFuture.completedFuture(null);",
            )
            .map_err(fmt_err)?;
        } else {
            writeln!(
                out,
                "{ind}{ind}// JNI: async-Path via RustRequesterFFI.sendRequestAsync.",
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{ind}{ind}return requesterFfi.sendRequestAsync(/* xcdr2_encode(args) */ new byte[0])",
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{ind}{ind}{ind}.thenApply(reply -> /* xcdr2_decode<TOut>(reply) */ null);",
            )
            .map_err(fmt_err)?;
        }
        writeln!(out, "{ind}}}").map_err(fmt_err)?;
        return Ok(());
    }

    if m.oneway {
        writeln!(
            out,
            "{ind}{ind}requester.sendOneway(new Object[] {{ /* args */ }});",
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "{ind}{ind}return java.util.concurrent.CompletableFuture.completedFuture(null);",
        )
        .map_err(fmt_err)?;
    } else {
        writeln!(
            out,
            "{ind}{ind}return requester.sendRequest(new Object[] {{ /* args */ }});",
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Type-Mapping-Helpers
// ---------------------------------------------------------------------------

/// Liefert den Java-Return-Type fuer die synchrone Methoden-Signatur.
/// `oneway` und `void` → `void`. `out`-Parameter sind im Holder-Pattern
/// abgebildet — sie bleiben Teil der Parameterliste.
fn sync_return_type(m: &MethodDef) -> Result<String, JavaGenError> {
    if m.oneway {
        return Ok("void".to_string());
    }
    match &m.return_type {
        None => Ok("void".to_string()),
        Some(ts) => typespec_to_java_unboxed(ts),
    }
}

/// Liefert den Java-Async-Return-Type. `oneway` ergibt
/// `CompletableFuture<Void>` (Caller darf trotzdem auf "complete"
/// warten, auch wenn Reply ueberspringen wird), `void` → `Void`,
/// sonst `CompletableFuture<TBoxed>`.
fn async_return_type(m: &MethodDef) -> Result<String, JavaGenError> {
    if m.oneway {
        return Ok("java.util.concurrent.CompletableFuture<Void>".to_string());
    }
    match &m.return_type {
        None => Ok("java.util.concurrent.CompletableFuture<Void>".to_string()),
        Some(ts) => Ok(format!(
            "java.util.concurrent.CompletableFuture<{}>",
            typespec_to_java_boxed(ts)?
        )),
    }
}

/// Param-Liste fuer die sync-Variante. `out`/`inout` werden als
/// Holder gerendert.
fn render_method_params(m: &MethodDef) -> Result<String, JavaGenError> {
    let mut parts: Vec<String> = Vec::new();
    for p in &m.params {
        parts.push(render_param(p)?);
    }
    Ok(parts.join(", "))
}

/// Param-Liste fuer die async-Variante; Holder-Pattern bleibt erhalten.
fn render_method_params_async(m: &MethodDef) -> Result<String, JavaGenError> {
    render_method_params(m)
}

fn render_param(p: &ParamDef) -> Result<String, JavaGenError> {
    let name = sanitize_identifier(&p.name)?;
    let ty = match p.direction {
        ParamDirection::In => typespec_to_java_unboxed(&p.type_ref)?,
        ParamDirection::Out | ParamDirection::InOut => {
            // Holder-Pattern (Spec §7.11.2 / IDL2Java 1.3 §1.5).
            format!(
                "org.zerodds.rpc.Holder<{}>",
                typespec_to_java_boxed(&p.type_ref)?
            )
        }
    };
    Ok(format!("{ty} {name}"))
}

fn render_call_arglist(m: &MethodDef) -> Result<String, JavaGenError> {
    let mut parts: Vec<String> = Vec::new();
    for p in &m.params {
        parts.push(sanitize_identifier(&p.name)?);
    }
    Ok(parts.join(", "))
}

// ---------------------------------------------------------------------------
// TypeSpec → Java
// ---------------------------------------------------------------------------

fn typespec_to_java_unboxed(ts: &TypeSpec) -> Result<String, JavaGenError> {
    match ts {
        TypeSpec::Primitive(p) => Ok(primitive_to_java(*p).to_string()),
        TypeSpec::Scoped(s) => Ok(scoped_to_java(s)),
        TypeSpec::String(_) => Ok("String".to_string()),
        TypeSpec::Sequence(s) => Ok(format!(
            "java.util.List<{}>",
            typespec_to_java_boxed(&s.elem)?
        )),
        TypeSpec::Map(mm) => Ok(format!(
            "java.util.Map<{}, {}>",
            typespec_to_java_boxed(&mm.key)?,
            typespec_to_java_boxed(&mm.value)?,
        )),
        TypeSpec::Fixed(_) => Err(JavaGenError::UnsupportedConstruct {
            construct: "fixed".into(),
            context: None,
        }),
        TypeSpec::Any => Err(JavaGenError::UnsupportedConstruct {
            construct: "any".into(),
            context: None,
        }),
    }
}

fn typespec_to_java_boxed(ts: &TypeSpec) -> Result<String, JavaGenError> {
    match ts {
        TypeSpec::Primitive(p) => Ok(primitive_to_java_boxed(*p).to_string()),
        TypeSpec::Scoped(s) => Ok(scoped_to_java(s)),
        TypeSpec::String(_) => Ok("String".to_string()),
        TypeSpec::Sequence(s) => Ok(format!(
            "java.util.List<{}>",
            typespec_to_java_boxed(&s.elem)?
        )),
        TypeSpec::Map(mm) => Ok(format!(
            "java.util.Map<{}, {}>",
            typespec_to_java_boxed(&mm.key)?,
            typespec_to_java_boxed(&mm.value)?,
        )),
        TypeSpec::Fixed(_) => Err(JavaGenError::UnsupportedConstruct {
            construct: "fixed".into(),
            context: None,
        }),
        TypeSpec::Any => Err(JavaGenError::UnsupportedConstruct {
            construct: "any".into(),
            context: None,
        }),
    }
}

fn scoped_to_java(s: &ScopedName) -> String {
    s.parts
        .iter()
        .map(|p| p.text.clone())
        .collect::<Vec<_>>()
        .join(".")
}

// Marker, damit der Linter die "vielleicht-spaeter"-Helpers nicht
// als unused meldet.
#[allow(dead_code)]
fn _unused_marker(_i: IntegerType) {
    let _ = integer_to_java;
    let _ = floating_to_java;
    let _ = integer_to_java_boxed;
    let _ = floating_to_java_boxed;
    let _ = PrimitiveType::Boolean;
}

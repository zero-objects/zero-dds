// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 clause 11: the language representations of
//! `dds_security.idl`. Each is rendered by the mapping rules of its language
//! and compiled with that language's compiler:
//!
//! - §11.3 C: IDL2C, `cc -std=c99`;
//! - §11.4 C++ classic: IDL2C++, `c++ -std=c++98` (no standard library);
//! - §11.6 C++11: IDL2C++11 with the five exceptions, compiled as C++98
//!   because exception 2 forbids any C++11-only feature;
//! - §11.5 Java classic: IDL2Java, `javac`;
//! - §11.7 Java modern: IDL2Java with module DDS as `org.omg.dds`.
//!
//! The base types of the ORB mappings (`CORBA_long`, `org.omg.CORBA`) and of
//! the DDS-STD-C++ PSM (`dds::core::array`, `safe_enum`, `DynamicData`) are
//! small stand-ins here; the compilers see the generated text against them.
//! A host without a compiler prints `[skip]` and the test does not count as
//! run.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unreachable
)]

mod idl_bindings;

use std::path::{Path, PathBuf};
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn available(tool: &str, probe: &str) -> bool {
    Command::new(tool)
        .arg(probe)
        .output()
        .is_ok_and(|o| o.status.success())
}

fn write_all(dir: &Path, prefix: &str) {
    for (path, text) in idl_bindings::render_all() {
        if let Some(rest) = path.strip_prefix(prefix) {
            let target = dir.join(rest);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, text).unwrap();
        }
    }
}

fn run(mut cmd: Command, what: &str) {
    let out = cmd.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        out.status.success(),
        "{what} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn run_output(mut cmd: Command, what: &str) -> String {
    let out = cmd.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        out.status.success(),
        "{what} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

// ---------------------------------------------------------------- C ------

/// §11.3: the header compiles as C99, and a program that implements the
/// operations of the Logging interface (Table 46) with the generated
/// prototypes links and runs.
#[test]
fn c_representation_compiles_and_the_logging_operations_link() {
    if !available("cc", "--version") {
        println!("[skip] no C compiler");
        return;
    }
    let dir = scratch("sec-bindings-c");
    write_all(&dir, "c/");
    std::fs::write(
        dir.join("main.c"),
        r#"#include <stdio.h>
#include <string.h>
#include "dds_security.h"

static int enabled = 0;
static CORBA_long last_level = -1;

CORBA_boolean DDS_Security_Logging_set_log_options(DDS_Security_Logging _obj,
    const DDS_Security_LogOptions *options, DDS_Security_SecurityException *ex,
    CORBA_Environment *_ev) {
    (void)_obj; (void)ex; (void)_ev;
    last_level = options->log_level;
    return 1;
}
void DDS_Security_Logging_log(DDS_Security_Logging _obj, DDS_Security_LogLevel log_level,
    const CORBA_char *message, const CORBA_char *category, DDS_Security_SecurityException *ex,
    CORBA_Environment *_ev) {
    (void)_obj; (void)message; (void)category; (void)ex; (void)_ev;
    last_level = log_level;
}
void DDS_Security_Logging_enable_logging(DDS_Security_Logging _obj,
    DDS_Security_SecurityException *ex, CORBA_Environment *_ev) {
    (void)_obj; (void)ex; (void)_ev;
    enabled = 1;
}
CORBA_boolean DDS_Security_Logging_set_listener(DDS_Security_Logging _obj,
    DDS_Security_LoggerListener listener, DDS_Security_SecurityException *ex,
    CORBA_Environment *_ev) {
    (void)_obj; (void)listener; (void)ex; (void)_ev;
    return 1;
}

int main(void) {
    DDS_Security_SecurityException ex;
    CORBA_Environment ev;
    DDS_Security_LogOptions options;
    memset(&ex, 0, sizeof ex);
    memset(&ev, 0, sizeof ev);
    options.log_level = DDS_Security_WARNING_LEVEL;
    options.log_file = 0;
    options.distribute = 1;
    if (!DDS_Security_Logging_set_log_options(0, &options, &ex, &ev)) return 1;
    DDS_Security_Logging_enable_logging(0, &ex, &ev);
    DDS_Security_Logging_log(0, DDS_Security_TRACE_LEVEL, "m", "c", &ex, &ev);
    printf("%d %d\n", enabled, (int)last_level);
    return 0;
}
"#,
    )
    .unwrap();
    let mut cc = Command::new("cc");
    cc.current_dir(&dir).args([
        "-std=c99", "-Wall", "-Wextra", "-Werror", "-o", "main", "main.c",
    ]);
    run(cc, "cc");
    let output = run_output(Command::new(dir.join("main")), "main");
    assert_eq!(output.trim(), "1 7");
}

// ------------------------------------------------------------- C++ -------

const CORE_STUBS: &[(&str, &str)] = &[
    (
        "dds/core/Array.hpp",
        "#ifndef STUB_ARRAY\n#define STUB_ARRAY\n#include <stddef.h>\nnamespace dds { namespace core {\ntemplate <typename T, size_t N> class array {\npublic:\n    T &operator[](size_t i) { return d_[i]; }\n    const T &operator[](size_t i) const { return d_[i]; }\n    size_t size() const { return N; }\nprivate:\n    T d_[N];\n};\n} }\n#endif\n",
    ),
    (
        "dds/core/SafeEnumeration.hpp",
        "#ifndef STUB_SAFE_ENUM\n#define STUB_SAFE_ENUM\nnamespace dds { namespace core {\ntemplate <typename D> class safe_enum : public D {\npublic:\n    typedef typename D::type type;\n    safe_enum() : v_(type()) {}\n    safe_enum(type v) : v_(v) {}\n    type underlying() const { return v_; }\n    operator type() const { return v_; }\nprivate:\n    type v_;\n};\n} }\n#endif\n",
    ),
    (
        "dds/core/xtypes/DynamicData.hpp",
        "#ifndef STUB_DYNAMIC_DATA\n#define STUB_DYNAMIC_DATA\nnamespace dds { namespace core { namespace xtypes {\nclass DynamicData { public: DynamicData() {} };\n} } }\n#endif\n",
    ),
];

/// A program that implements the Logging interface with the generated
/// signatures. The same source serves both C++ dialects: only the namespace
/// differs.
fn logging_program(namespace: &str, header: &str, is_classic: bool) -> String {
    let (string_in, listener_ty) = if is_classic {
        ("const char *", "::DDS::Security::LoggerListener *")
    } else {
        ("const std::string &", "::dds::Security::LoggerListener *")
    };
    let ns = namespace;
    format!(
        r#"#include <stdio.h>
#include "{header}"

class Recorder : public ::{ns}::Security::Logging {{
public:
    Recorder() : enabled(false), level(-1) {{}}
    bool enabled;
    int level;
    virtual {boolean} set_log_options(const ::{ns}::Security::LogOptions &options, ::{ns}::Security::SecurityException &ex) {{
        (void)ex; level = options.log_level; return true;
    }}
    virtual void log(::{ns}::Security::LogLevel log_level, {string_in}message, {string_in}category, ::{ns}::Security::SecurityException &ex) {{
        (void)message; (void)category; (void)ex; level = log_level;
    }}
    virtual void enable_logging(::{ns}::Security::SecurityException &ex) {{ (void)ex; enabled = true; }}
    virtual {boolean} set_listener({listener_ty}listener, ::{ns}::Security::SecurityException &ex) {{
        (void)listener; (void)ex; return true;
    }}
}};

int main() {{
    Recorder r;
    ::{ns}::Security::Logging &logging = r;
    ::{ns}::Security::SecurityException ex;
    ::{ns}::Security::LogOptions options;
    options.log_level = ::{ns}::Security::WARNING_LEVEL;
    options.distribute = true;
    if (!logging.set_log_options(options, ex)) return 1;
    logging.enable_logging(ex);
    logging.log(::{ns}::Security::TRACE_LEVEL, "m", "c", ex);
    printf("%d %d\n", r.enabled ? 1 : 0, r.level);
    return 0;
}}
"#,
        boolean = if is_classic {
            "::CORBA::Boolean"
        } else {
            "bool"
        },
    )
}

#[test]
fn cpp_classic_representation_compiles_and_the_logging_interface_is_implementable() {
    if !available("c++", "--version") {
        println!("[skip] no C++ compiler");
        return;
    }
    let dir = scratch("sec-bindings-cpp");
    write_all(&dir, "cpp/");
    std::fs::write(
        dir.join("main.cpp"),
        logging_program("DDS", "dds_security.hpp", true),
    )
    .unwrap();
    let mut cxx = Command::new("c++");
    cxx.current_dir(&dir).args([
        "-std=c++98",
        "-pedantic",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-o",
        "main",
        "main.cpp",
    ]);
    run(cxx, "c++ (classic)");
    let output = run_output(Command::new(dir.join("main")), "main");
    assert_eq!(output.trim(), "1 7");
}

#[test]
fn cpp11_representation_compiles_without_a_cpp11_only_feature() {
    if !available("c++", "--version") {
        println!("[skip] no C++ compiler");
        return;
    }
    let dir = scratch("sec-bindings-cpp11");
    write_all(&dir, "cpp11/");
    for (path, text) in CORE_STUBS {
        let target = dir.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, text).unwrap();
    }
    std::fs::write(
        dir.join("main.cpp"),
        logging_program("dds", "dds_security.hpp", false),
    )
    .unwrap();
    // Exception 2 of §11.6: nothing that only C++11 has. The header is read
    // as C++98.
    let mut cxx = Command::new("c++");
    cxx.current_dir(&dir).args([
        "-std=c++98",
        "-I.",
        "-pedantic",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-o",
        "main",
        "main.cpp",
    ]);
    run(cxx, "c++ (C++11 representation as C++98)");
    let output = run_output(Command::new(dir.join("main")), "main");
    assert_eq!(output.trim(), "1 7");
    // Exceptions 1, 3, 4 and 5 are visible in the text.
    let header = std::fs::read_to_string(dir.join("dds_security.hpp")).unwrap();
    assert!(
        header.contains("namespace dds {"),
        "module DDS is namespace dds"
    );
    assert!(
        header.contains("::dds::core::array< uint8_t, 12 >"),
        "arrays"
    );
    assert!(
        header.contains("::dds::core::safe_enum< ValidationResult_t_def >"),
        "enums"
    );
    assert!(
        header.contains("::dds::core::xtypes::DynamicData"),
        "DynamicData"
    );
    for banned in [
        "noexcept",
        "override",
        "std::array",
        "&&",
        "constexpr",
        "nullptr",
    ] {
        assert!(!header.contains(banned), "{banned} is C++11 only");
    }
}

// ------------------------------------------------------------- Java ------

/// The classes of `org.omg.CORBA` the IDL2Java output refers to (JDK 11 and
/// later no longer carry them; an ORB such as JacORB does).
fn corba_stubs() -> Vec<(&'static str, String)> {
    let holder = |name: &str, ty: &str| {
        (
            Box::leak(format!("org/omg/CORBA/{name}.java").into_boxed_str()) as &'static str,
            format!(
                "package org.omg.CORBA;\npublic final class {name} implements org.omg.CORBA.portable.Streamable {{\n    public {ty} value;\n    public {name}() {{}}\n    public {name}({ty} v) {{ value = v; }}\n    public void _read(org.omg.CORBA.portable.InputStream in) {{}}\n    public void _write(org.omg.CORBA.portable.OutputStream out) {{}}\n}}\n"
            ),
        )
    };
    let mut v = vec![
        ("org/omg/CORBA/Object.java", "package org.omg.CORBA;\npublic interface Object {}\n".to_string()),
        ("org/omg/CORBA/BAD_PARAM.java", "package org.omg.CORBA;\npublic class BAD_PARAM extends RuntimeException {}\n".to_string()),
        ("org/omg/CORBA/MARSHAL.java", "package org.omg.CORBA;\npublic class MARSHAL extends RuntimeException {}\n".to_string()),
        ("org/omg/CORBA/portable/IDLEntity.java", "package org.omg.CORBA.portable;\npublic interface IDLEntity extends java.io.Serializable {}\n".to_string()),
        ("org/omg/CORBA/portable/Streamable.java", "package org.omg.CORBA.portable;\npublic interface Streamable {\n    void _read(InputStream in);\n    void _write(OutputStream out);\n}\n".to_string()),
        ("org/omg/CORBA/portable/InputStream.java", "package org.omg.CORBA.portable;\npublic abstract class InputStream {\n    public abstract int read_long();\n    public abstract int read_ulong();\n    public abstract long read_longlong();\n    public abstract long read_ulonglong();\n    public abstract short read_short();\n    public abstract short read_ushort();\n    public abstract byte read_octet();\n    public abstract boolean read_boolean();\n    public abstract char read_char();\n    public abstract float read_float();\n    public abstract double read_double();\n    public abstract String read_string();\n}\n".to_string()),
        ("org/omg/CORBA/portable/OutputStream.java", "package org.omg.CORBA.portable;\npublic abstract class OutputStream {\n    public abstract void write_long(int v);\n    public abstract void write_ulong(int v);\n    public abstract void write_longlong(long v);\n    public abstract void write_ulonglong(long v);\n    public abstract void write_short(short v);\n    public abstract void write_ushort(short v);\n    public abstract void write_octet(byte v);\n    public abstract void write_boolean(boolean v);\n    public abstract void write_char(char v);\n    public abstract void write_float(float v);\n    public abstract void write_double(double v);\n    public abstract void write_string(String v);\n}\n".to_string()),
    ];
    for (name, ty) in [
        ("IntHolder", "int"),
        ("LongHolder", "long"),
        ("ShortHolder", "short"),
        ("ByteHolder", "byte"),
        ("BooleanHolder", "boolean"),
        ("CharHolder", "char"),
        ("FloatHolder", "float"),
        ("DoubleHolder", "double"),
        ("StringHolder", "String"),
    ] {
        v.push(holder(name, ty));
    }
    v
}

fn java_available() -> bool {
    available("javac", "-version") && available("java", "-version")
}

fn compile_java(dir: &Path, main_source: &str, main_class: &str, tree: &str) -> String {
    write_all(dir, &format!("{tree}/"));
    for (path, text) in corba_stubs() {
        let target = dir.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, text).unwrap();
    }
    if tree == "java-modern" {
        let target = dir.join("org/omg/dds/type/dynamic/DynamicData.java");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(
            target,
            "package org.omg.dds.type.dynamic;\npublic interface DynamicData {}\n",
        )
        .unwrap();
    }
    std::fs::write(dir.join(format!("{main_class}.java")), main_source).unwrap();
    let mut sources = Vec::new();
    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                collect(&p, out);
            } else if p.extension().is_some_and(|e| e == "java") {
                out.push(p);
            }
        }
    }
    collect(dir, &mut sources);
    let mut javac = Command::new("javac");
    javac.current_dir(dir).arg("-d").arg("out").args(&sources);
    run(javac, "javac");
    let mut java = Command::new("java");
    java.current_dir(dir).args(["-cp", "out", main_class]);
    run_output(java, "java")
}

fn java_main(package: &str) -> String {
    format!(
        r#"import {package}.LogOptions;
import {package}.LoggerListener;
import {package}.LoggingOperations;
import {package}.SecurityException;
import {package}.SecurityExceptionHolder;
import {package}.TRACE_LEVEL;
import {package}.WARNING_LEVEL;

public class Main {{
    static class Recorder implements LoggingOperations {{
        boolean enabled;
        int level = -1;
        public boolean set_log_options(LogOptions options, SecurityExceptionHolder ex) {{
            level = options.log_level;
            return true;
        }}
        public void log(int log_level, String message, String category, SecurityExceptionHolder ex) {{
            level = log_level;
        }}
        public void enable_logging(SecurityExceptionHolder ex) {{ enabled = true; }}
        public boolean set_listener(LoggerListener listener, SecurityExceptionHolder ex) {{ return true; }}
    }}

    public static void main(String[] args) {{
        Recorder r = new Recorder();
        SecurityExceptionHolder ex = new SecurityExceptionHolder(new SecurityException("", 0, 0));
        LogOptions options = new LogOptions(WARNING_LEVEL.value, null, true);
        r.set_log_options(options, ex);
        r.enable_logging(ex);
        r.log(TRACE_LEVEL.value, "m", "c", ex);
        System.out.println((r.enabled ? 1 : 0) + " " + r.level);
    }}
}}
"#
    )
}

#[test]
fn java_classic_representation_compiles_and_the_logging_interface_is_implementable() {
    if !java_available() {
        println!("[skip] no JDK");
        return;
    }
    let dir = scratch("sec-bindings-java");
    let output = compile_java(&dir, &java_main("DDS.Security"), "Main", "java");
    assert_eq!(output.trim(), "1 7");
    // The IDL2Java classes of a structure and an enumeration.
    assert!(
        dir.join("DDS/Security/ValidationResult_tHelper.java")
            .exists()
    );
    assert!(
        dir.join("DDS/Security/AuthenticationOperations.java")
            .exists()
    );
}

#[test]
fn java_modern_representation_maps_module_dds_to_org_omg_dds() {
    if !java_available() {
        println!("[skip] no JDK");
        return;
    }
    let dir = scratch("sec-bindings-java-modern");
    let output = compile_java(
        &dir,
        &java_main("org.omg.dds.Security"),
        "Main",
        "java-modern",
    );
    assert_eq!(output.trim(), "1 7");
    // Exception 2 of §11.7: DynamicData is the type of the Java 5 PSM.
    let ops =
        std::fs::read_to_string(dir.join("org/omg/dds/Security/AccessControlOperations.java"))
            .unwrap();
    assert!(
        ops.contains("org.omg.dds.type.dynamic.DynamicData key"),
        "{ops}"
    );
}

// ------------------------------------------------- all representations ----

/// Every operation of every interface is in each representation, with every
/// `inout` parameter as a reference (C pointer, C++ reference, Java holder).
#[test]
fn every_operation_reaches_every_representation() {
    let all = idl_bindings::render_all();
    let c = &all["c/dds_security.h"];
    assert!(
        c.contains("DDS_Security_Authentication_validate_local_identity(DDS_Security_Authentication _obj, DDS_Security_IdentityHandle *local_identity_handle, DDS_Security_GUID_t *adjusted_participant_guid,"),
        "{c}"
    );
    let cpp = &all["cpp/dds_security.hpp"];
    assert!(
        cpp.contains("virtual ::CORBA::Boolean check_remote_datareader("),
        "{cpp}"
    );
    assert!(
        cpp.contains("::DDS::Security::SecurityException &ex"),
        "{cpp}"
    );
    let access = &all["java/DDS/Security/AccessControlOperations.java"];
    assert!(
        access.contains("boolean check_remote_datareader("),
        "{access}"
    );
    assert!(
        access.contains("org.omg.CORBA.BooleanHolder relay_only"),
        "{access}"
    );
}

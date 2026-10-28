// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `idlc dlrl`'s generated text, through each language's own compiler —
//! the counterpart of `crates/lang-rules/tests/{cpp,csharp,java,typescript}_corpus_compiles.rs`
//! for the `generate` columns: a snapshot says the emitter produced the
//! text it produced last time, not that the text is C++/C#/Java/
//! TypeScript, that every name in it resolves, or that a member's type
//! is complete where the language requires it to be.
//!
//! ## The stub types are test fixtures, not a runtime
//!
//! `tools/idlc/src/dlrl.rs`'s module doc and every generated file's own
//! header comment say plainly that ZeroDDS ships no DLRL runtime for
//! C++, C# or Java — `DLRL::ObjectRoot`/`ObjectHome`/`RefList`,
//! `org.omg.dds.dlrl.ObjectRoot`/`ObjectHome` and the C# `ObjectRoot`/
//! `ObjectHome`/`[DlrlKey]`/`[DlrlMonoAttribute]`/`[DlrlMultiAttribute]`/`[DlrlRelation]` mapping this codebase
//! invented all resolve against nothing this tree provides. What
//! follows is the minimal stand-in each compiler needs to typecheck the
//! generated class against *some* definition of its base type and
//! attributes — proving the emitted text is syntactically and
//! type-legal C++/C#/Java, not that a real DLRL runtime accepts it.
//! TypeScript needs none of this: its two generated forms name no
//! external type at all.
//!
//! ## Structured and multi-valued attributes
//!
//! An attribute of an IDL enumeration or structure has the type `idlc
//! generate` declares for it. The second test per language compiles the
//! DLRL classes together with that output and the language's wire runtime
//! from `runtime/`, so a name the DLRL backend spells differently from the
//! `generate` backend fails here.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::Path;
use std::process::Command;

fn idlc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const TRADE_LEG_DLRL: &str = "struct Trade { long id; string venue; double price; string trader; };\n\
struct Leg { long id; };\n\
#pragma DCPS_DATA_TYPE \"Trade\"\n\
#pragma DCPS_DATA_KEY \"Trade id\"\n\
#pragma DCPS_DATA_KEY \"Trade venue\"\n\
#pragma DCPS_DLRL_RELATION \"Trade legs Leg\"\n\
#pragma DCPS_DATA_TYPE \"Leg\"\n\
#pragma DCPS_DATA_KEY \"Leg id\"\n";

/// Run `idlc dlrl --lang <language> --out <dir>/out` over the fixture
/// above and answer the output directory.
fn generate(label: &str, language: &str) -> std::path::PathBuf {
    let dir = scratch(label);
    std::fs::write(dir.join("trade.idl"), TRADE_LEG_DLRL).unwrap();
    let status = idlc()
        .args(["dlrl", "--lang", language, "--out"])
        .arg(dir.join("out"))
        .arg(dir.join("trade.idl"))
        .status()
        .unwrap();
    assert!(status.success(), "idlc dlrl --lang {language} failed");
    dir.join("out")
}

// -- C++ -------------------------------------------------------------------

/// The minimal stand-in for the DLRL C++ PSM's own base types (DDS 1.2
/// §B.6/§B.7) — see the module doc.
const CPP_STUB: &str = "#pragma once\n\
namespace DLRL {\n\
class ObjectRoot {};\n\
class ObjectHome {};\n\
template <class T> class RefList {};\n\
template <class T> class List {};\n\
template <class T> class StrMap {};\n\
template <class K, class T> class IntMap {};\n\
}\n\
namespace DDS {\n\
  using Long = long;\n\
  using LongLong = long long;\n\
  using Double = double;\n\
  // The classic C++ mapping's managed string, as far as the generated\n\
  // getter and setter use it.\n\
  struct String_var { const char* p = \"\"; const char* in() const { return p; } String_var& operator=(const char* s) { p = s; return *this; } };\n\
}\n";

#[test]
fn cpp_output_compiles_against_a_stub_of_the_missing_runtime() {
    let cxx = ["c++", "g++", "clang++"].iter().find(|candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
    });
    let Some(cxx) = cxx else {
        eprintln!("SKIPPED: no C++ compiler on the path.");
        return;
    };
    let out = generate("dlrl-compile-cpp", "cpp");
    std::fs::write(out.join("dlrl_stub.hpp"), CPP_STUB).unwrap();
    let unit = out.join("tu.cpp");
    // The stub ahead of the generated header: `trade_dlrl.hpp` writes no
    // `#include` for the runtime it derives from, since that runtime does
    // not exist for this tool to name — see the module doc.
    std::fs::write(
        &unit,
        "#include \"dlrl_stub.hpp\"\n#include \"trade_dlrl.hpp\"\n",
    )
    .unwrap();
    let output = Command::new(cxx)
        .args(["-std=c++20", "-fsyntax-only", "-Wall", "-Wextra", "-Werror"])
        .arg(format!("-I{}", out.display()))
        .arg(&unit)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// -- C# ----------------------------------------------------------------

/// The minimal stand-in for the C#-only DLRL mapping this codebase
/// invented — see the module doc.
const CSHARP_STUB: &str = "public class ObjectRoot {}\n\
public class ObjectHome {}\n\
[System.AttributeUsage(System.AttributeTargets.Class)]\n\
public class DlrlObjectAttribute : System.Attribute {}\n\
[System.AttributeUsage(System.AttributeTargets.Property)]\n\
public class DlrlKeyAttribute : System.Attribute { public DlrlKeyAttribute(string name) {} }\n\
[System.AttributeUsage(System.AttributeTargets.Property)]\n\
public class DlrlMonoAttributeAttribute : System.Attribute { public DlrlMonoAttributeAttribute(string name) {} }\n\
[System.AttributeUsage(System.AttributeTargets.Property)]\n\
public class DlrlMultiAttributeAttribute : System.Attribute { public DlrlMultiAttributeAttribute(string name) {} }\n\
[System.AttributeUsage(System.AttributeTargets.Property)]\n\
public class DlrlRelationAttribute : System.Attribute { public DlrlRelationAttribute(string name) {} }\n";

/// Whether `command` answers `--version` (or `arg`) with success — the
/// same presence check every sibling compile test in this tree uses.
fn available(command: &str, arg: &str) -> bool {
    Command::new(command)
        .arg(arg)
        .output()
        .is_ok_and(|out| out.status.success())
}

#[test]
fn csharp_output_compiles_against_a_stub_of_the_invented_mapping() {
    if !available("dotnet", "--version") {
        eprintln!("SKIPPED: no .NET SDK on the path.");
        return;
    }
    let out = generate("dlrl-compile-csharp", "csharp");
    std::fs::write(out.join("Stub.cs"), CSHARP_STUB).unwrap();
    std::fs::write(
        out.join("project.csproj"),
        "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    \
         <OutputType>Library</OutputType>\n    <TargetFramework>net9.0</TargetFramework>\n    \
         <Nullable>disable</Nullable>\n  </PropertyGroup>\n</Project>\n",
    )
    .unwrap();
    let output = Command::new("dotnet")
        .args(["build", "-v", "quiet", "--nologo"])
        .current_dir(&out)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

// -- Java --------------------------------------------------------------

const JAVA_OBJECT_ROOT_STUB: &str = "package org.omg.dds.dlrl;\npublic class ObjectRoot {}\n";
const JAVA_OBJECT_HOME_STUB: &str = "package org.omg.dds.dlrl;\npublic class ObjectHome {}\n";

#[test]
fn java_output_compiles_against_a_stub_of_the_missing_runtime() {
    if !available("javac", "-version") {
        eprintln!("SKIPPED: `javac` is not usable.");
        return;
    }
    let out = generate("dlrl-compile-java", "java");
    let stub_pkg = out.join("org/omg/dds/dlrl");
    std::fs::create_dir_all(&stub_pkg).unwrap();
    std::fs::write(stub_pkg.join("ObjectRoot.java"), JAVA_OBJECT_ROOT_STUB).unwrap();
    std::fs::write(stub_pkg.join("ObjectHome.java"), JAVA_OBJECT_HOME_STUB).unwrap();
    let classes = out.join("classes");
    std::fs::create_dir_all(&classes).unwrap();
    let mut sources: Vec<std::path::PathBuf> = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
        .collect();
    sources.push(stub_pkg.join("ObjectRoot.java"));
    sources.push(stub_pkg.join("ObjectHome.java"));
    sources.sort();
    let output = Command::new("javac")
        .args(["-encoding", "UTF-8"])
        .arg("-d")
        .arg(&classes)
        .args(&sources)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// -- TypeScript ----------------------------------------------------------

/// No stub: DDS 1.2 Annex B has no TypeScript PSM, so `dlrl-codegen`'s
/// TypeScript column emits a plain `interface` plus a class implementing
/// it, naming no external type at all — the exception the module doc
/// names.
#[test]
fn typescript_output_typechecks_with_no_stub_needed() {
    if !available("tsc", "--version") {
        eprintln!("SKIPPED: no TypeScript compiler on the path.");
        return;
    }
    let out = generate("dlrl-compile-ts", "typescript");
    let output = Command::new("tsc")
        .args(["--noEmit", "--strict", "--target", "ES2022"])
        .arg(out.join("trade_dlrl.ts"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

// -- structured and multi-valued attributes ------------------------------

/// A structure and an enumeration inside a module and at the top level,
/// each as a mono-valued attribute, and a sequence, an array and both
/// maps as multi-valued ones.
const STRUCTURED_DLRL: &str = "enum Color { RED, GREEN };\n\
struct Point { long x; long y; };\n\
module demo {\n\
  enum Side { BUY, SELL };\n\
  struct Money { double amount; string currency; Color tint; };\n\
  struct Leg { long id; };\n\
  struct Trade {\n\
    long id; Side side_of_trade; Money price; Point at; Color colour;\n\
    sequence<string> tags; long fills[3]; sequence<Money> history;\n\
    map<string, Money> fees; map<long long, Point> marks;\n\
  };\n\
};\n\
#pragma DCPS_DATA_TYPE \"demo::Trade\"\n\
#pragma DCPS_DATA_KEY \"demo::Trade id\"\n\
#pragma DCPS_DLRL_RELATION \"demo::Trade legs demo::Leg\"\n\
#pragma DCPS_DATA_TYPE \"demo::Leg\"\n\
#pragma DCPS_DATA_KEY \"demo::Leg id\"\n";

fn repository_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `idlc generate` and `idlc dlrl` for `language` over the fixture above,
/// into one directory.
fn generate_with_types(label: &str, language: &str) -> std::path::PathBuf {
    let dir = scratch(label);
    std::fs::write(dir.join("trade.idl"), STRUCTURED_DLRL).unwrap();
    for subcommand in ["generate", "dlrl"] {
        let output = idlc()
            .args([subcommand, "--lang", language, "--out"])
            .arg(dir.join("out"))
            .arg(dir.join("trade.idl"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "idlc {subcommand} --lang {language}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    dir.join("out")
}

#[test]
fn cpp_structured_attributes_compile_against_the_generated_types() {
    let cxx = ["c++", "g++", "clang++"]
        .iter()
        .find(|candidate| available(candidate, "--version"));
    let Some(cxx) = cxx else {
        eprintln!("SKIPPED: no C++ compiler on the path.");
        return;
    };
    let out = generate_with_types("dlrl-structured-cpp", "cpp");
    std::fs::write(out.join("dlrl_stub.hpp"), CPP_STUB).unwrap();
    let unit = out.join("tu.cpp");
    std::fs::write(
        &unit,
        "#include \"dlrl_stub.hpp\"\n#include \"trade_dlrl.hpp\"\n",
    )
    .unwrap();
    let output = Command::new(cxx)
        .args(["-std=c++20", "-fsyntax-only", "-Wall", "-Wextra", "-Werror"])
        .arg(format!("-I{}", out.display()))
        .arg(format!(
            "-I{}",
            repository_root().join("runtime/cpp/include").display()
        ))
        // `trade.hpp` specialises `dds::topic::topic_type_support<T>` from
        // the C++ DCPS API's `TopicTraits.hpp`, which is header-only.
        .arg(format!(
            "-I{}",
            repository_root().join("crates/cpp/include").display()
        ))
        .arg(&unit)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn csharp_structured_attributes_compile_against_the_generated_types() {
    if !available("dotnet", "--version") {
        eprintln!("SKIPPED: no .NET SDK on the path.");
        return;
    }
    let out = generate_with_types("dlrl-structured-csharp", "csharp");
    std::fs::write(out.join("Stub.cs"), CSHARP_STUB).unwrap();
    // `trade.cs` holds the data types; `trade.TypeSupport.cs` binds them to
    // the DCPS API, which a DLRL class does not name. The wire runtime is
    // compiled from source so that nothing is built inside `runtime/`.
    let runtime = repository_root().join("runtime/csharp/src");
    std::fs::write(
        out.join("project.csproj"),
        format!(
            "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    \
             <OutputType>Library</OutputType>\n    <TargetFramework>net9.0</TargetFramework>\n    \
             <Nullable>disable</Nullable>\n    <EnableDefaultCompileItems>false</EnableDefaultCompileItems>\n  \
             </PropertyGroup>\n  <ItemGroup>\n    \
             <Compile Include=\"trade.cs;trade_dlrl.cs;Stub.cs\" />\n    \
             <Compile Include=\"{}/*.cs\" />\n  </ItemGroup>\n</Project>\n",
            runtime.display()
        ),
    )
    .unwrap();
    let output = Command::new("dotnet")
        .args(["build", "-v", "quiet", "--nologo"])
        .current_dir(&out)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn java_structured_attributes_compile_against_the_generated_types() {
    if !available("javac", "-version") {
        eprintln!("SKIPPED: `javac` is not usable.");
        return;
    }
    let out = generate_with_types("dlrl-structured-java", "java");
    let stub_pkg = out.join("org/omg/dds/dlrl");
    std::fs::create_dir_all(&stub_pkg).unwrap();
    std::fs::write(stub_pkg.join("ObjectRoot.java"), JAVA_OBJECT_ROOT_STUB).unwrap();
    std::fs::write(stub_pkg.join("ObjectHome.java"), JAVA_OBJECT_HOME_STUB).unwrap();
    let classes = out.join("classes");
    std::fs::create_dir_all(&classes).unwrap();
    // `ZdTypes.java` holds the data types and `ZdCodec.java` their codec;
    // `ZdTypeSupports.java` binds them to the DCPS API (`org.zerodds.cdr`),
    // which a DLRL class does not name.
    let mut sources: Vec<std::path::PathBuf> = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
        .filter(|path| !path.ends_with("ZdTypeSupports.java"))
        .collect();
    sources.push(stub_pkg.join("ObjectRoot.java"));
    sources.push(stub_pkg.join("ObjectHome.java"));
    sources.sort();
    let output = Command::new("javac")
        .args(["-encoding", "UTF-8"])
        .arg("-sourcepath")
        .arg(repository_root().join("runtime/java/src/main/java"))
        .arg("-d")
        .arg(&classes)
        .args(&sources)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn typescript_structured_attributes_typecheck_against_the_generated_types() {
    if !available("tsc", "--version") {
        eprintln!("SKIPPED: no TypeScript compiler on the path.");
        return;
    }
    let out = generate_with_types("dlrl-structured-ts", "typescript");
    // `trade.ts` imports `@zerodds/wire`; the path mapping points it at the
    // runtime's source, and `type: module` makes both files ES modules, as
    // `NodeNext` resolution requires for the `./trade.js` import.
    std::fs::write(out.join("package.json"), "{ \"type\": \"module\" }\n").unwrap();
    std::fs::write(
        out.join("tsconfig.json"),
        format!(
            "{{\n  \"compilerOptions\": {{\n    \"target\": \"ES2022\",\n    \
             \"module\": \"NodeNext\",\n    \"moduleResolution\": \"NodeNext\",\n    \
             \"strict\": true,\n    \"noEmit\": true,\n    \"skipLibCheck\": true,\n    \
             \"paths\": {{ \"@zerodds/wire\": [\"{}\"] }}\n  }},\n  \
             \"files\": [\"trade.ts\", \"trade_dlrl.ts\"]\n}}\n",
            repository_root()
                .join("runtime/typescript/src/index.ts")
                .display()
        ),
    )
    .unwrap();
    let output = Command::new("tsc")
        .arg("-p")
        .arg(out.join("tsconfig.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

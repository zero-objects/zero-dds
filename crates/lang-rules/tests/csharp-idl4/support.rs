// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to the C# of IDL4 to C# 1.0, compiled with Roslyn and run with
//! `dotnet` against a check program written from the specification's text.

#![allow(dead_code, clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use zerodds_lang_rules::emit::idl4_csharp::{CsFile, Options, generate};

/// The model of `idl`, front-end diagnostics refused. Composed as
/// `zerodds-idlc` composes a file: preprocessed with the built-in
/// `orb.idl`, lowered and resolved.
pub fn model(idl: &str) -> zerodds_ast::Ast {
    let loader = zerodds_idl_parse::MemoryLoader::new();
    let composed = zerodds_idl_compose::compose_source_with(
        "test.idl",
        idl,
        &loader,
        zerodds_idl_compose::ComposeOptions::default(),
    );
    let errors: Vec<_> = composed
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .collect();
    assert!(errors.is_empty(), "the source does not compose: {errors:?}");
    composed.ast
}

/// The C# files of `idl` under `options`.
pub fn cs(idl: &str, options: &Options) -> Vec<CsFile> {
    generate(&model(idl), options).unwrap_or_else(|error| panic!("{error}"))
}

/// The text of the generated file at `path`.
pub fn file<'a>(files: &'a [CsFile], path: &str) -> &'a str {
    files
        .iter()
        .find(|file| file.path == path)
        .map(|file| file.text.as_str())
        .unwrap_or_else(|| {
            panic!(
                "no file {path}; generated: {:?}",
                files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>()
            )
        })
}

/// The IDL of the CORBA module: the pseudo IDL the ORB implements
/// (`runtime/corba/CORBA_PIDL.idl`), which includes `orb.idl`.
pub fn corba_idl() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/corba/CORBA_PIDL.idl"),
    )
    .expect("runtime/corba/CORBA_PIDL.idl")
}

/// The C# of the CORBA module on the CORBA platform (Annex A.1).
pub fn corba_module() -> Vec<CsFile> {
    let options = Options {
        platform: zerodds_lang_rules::emit::idl4_csharp::Platform::Corba,
        corba_module: true,
        ..Options::default()
    };
    cs(&corba_idl(), &options)
}

/// Roslyn and the framework reference pack, found off `dotnet
/// --list-sdks` as `csharp_corpus_compiles.rs` finds them.
struct Roslyn {
    csc: PathBuf,
    references: PathBuf,
}

fn roslyn() -> Option<&'static Roslyn> {
    static ROSLYN: OnceLock<Option<Roslyn>> = OnceLock::new();
    ROSLYN
        .get_or_init(|| {
            let listed = Command::new("dotnet").arg("--list-sdks").output().ok()?;
            if !listed.status.success() {
                return None;
            }
            let text = String::from_utf8_lossy(&listed.stdout).into_owned();
            let mut lines: Vec<&str> = text.lines().filter(|line| line.contains('[')).collect();
            lines.sort_unstable();
            let line = lines.last()?;
            let (version, rest) = line.split_once(' ')?;
            let directory = rest.trim().trim_start_matches('[').trim_end_matches(']');
            let sdk = PathBuf::from(directory);
            let csc = sdk.join(version).join("Roslyn/bincore/csc.dll");
            if !csc.is_file() {
                return None;
            }
            let root = sdk.parent()?.join("packs/Microsoft.NETCore.App.Ref");
            let mut versions: Vec<PathBuf> = std::fs::read_dir(&root)
                .ok()?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect();
            versions.sort();
            let refs = versions.last()?.join("ref");
            let mut frameworks: Vec<PathBuf> = std::fs::read_dir(&refs)
                .ok()?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect();
            frameworks.sort();
            let pack = frameworks.last()?.clone();
            let mut response = String::new();
            let mut assemblies: Vec<PathBuf> = std::fs::read_dir(&pack)
                .ok()?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.extension().is_some_and(|ext| ext == "dll"))
                .collect();
            assemblies.sort();
            for assembly in assemblies {
                response.push_str(&format!("-r:{}\n", assembly.display()));
            }
            let references = std::env::temp_dir()
                .join(format!("zd-csharp-idl4-refs-{}.rsp", std::process::id()));
            std::fs::write(&references, response).ok()?;
            Some(Roslyn { csc, references })
        })
        .as_ref()
}

fn runtime_sources(corba: bool) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../idl-csharp/runtime");
    let mut out = vec![root.join("Omg.Types.cs")];
    if corba {
        out.push(root.join("Corba.cs"));
    }
    out
}

/// Compile `files` with the runtime and the check program `check` (a
/// class `Check` with a `Main`), run it, and fail on a compile error, a
/// failed check or an exception.
///
/// `-warnaserror+ -warn:4 -langversion:12.0`: a warning in generated code
/// is a question nobody answered. Without a .NET SDK it says so and
/// returns; the codegen job of the C# column runs it with one
/// (ci/integration_scenarios.py).
pub fn compile_and_run(tag: &str, files: &[CsFile], check: &str) {
    let Some(roslyn) = roslyn() else {
        eprintln!("NOT RUN: no .NET SDK with a csc.dll");
        return;
    };
    let dir = std::env::temp_dir().join(format!("zd-csharp-idl4-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let src = dir.join("src");
    std::fs::create_dir_all(&src).expect("a source directory");
    let corba = files.iter().any(|file| file.path.starts_with("Corba/"));
    let mut sources = runtime_sources(corba);
    for file in files {
        let path = src.join(&file.path);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        std::fs::write(&path, &file.text).expect("a source file");
        sources.push(path);
    }
    let check_path = src.join("Check.cs");
    std::fs::write(&check_path, check).expect("the check program");
    sources.push(check_path);
    let exe = dir.join("Check.dll");
    let output = Command::new("dotnet")
        .arg(&roslyn.csc)
        .args([
            "-nologo",
            "-nostdlib",
            "-noconfig",
            "-langversion:12.0",
            "-warnaserror+",
            "-warn:4",
            "-target:exe",
            "-main:Check",
        ])
        .arg(format!("-out:{}", exe.display()))
        .arg(format!("@{}", roslyn.references.display()))
        .args(&sources)
        .output()
        .expect("csc runs");
    if !output.status.success() {
        let listing: String = files
            .iter()
            .map(|f| format!("==== {}\n{}\n", f.path, f.text))
            .collect();
        panic!(
            "csc refused the generated code:\n{}{}\n{listing}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::write(
        dir.join("Check.runtimeconfig.json"),
        "{\n  \"runtimeOptions\": {\n    \"tfm\": \"net8.0\",\n    \"rollForward\": \"LatestMajor\",\n    \
         \"framework\": { \"name\": \"Microsoft.NETCore.App\", \"version\": \"8.0.0\" }\n  }\n}\n",
    )
    .expect("the runtime configuration");
    let output = Command::new("dotnet")
        .arg(&exe)
        .output()
        .expect("dotnet runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("CHECKED"),
        "the check program failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The C# helpers every check program starts with.
pub const CHECK_PRELUDE: &str = r#"
    static int failures = 0;

    static void Ok(bool condition, string what)
    {
        if (!condition)
        {
            System.Console.WriteLine("FAILED: " + what);
            failures++;
        }
    }

    static void Throws<TException>(System.Action action, string what) where TException : System.Exception
    {
        try
        {
            action();
            System.Console.WriteLine("FAILED (no exception): " + what);
            failures++;
        }
        catch (TException)
        {
        }
        catch (System.Exception other)
        {
            System.Console.WriteLine("FAILED (" + other.GetType() + "): " + what);
            failures++;
        }
    }

    static System.Reflection.MethodInfo Method(System.Type type, string name, params System.Type[] parameters)
    {
        var method = type.GetMethod(name, parameters);
        if (method == null)
        {
            System.Console.WriteLine("FAILED (no method " + name + "): " + type);
            failures++;
        }
        return method;
    }

    static void Returns(System.Type type, string name, System.Type result, params System.Type[] parameters)
    {
        var method = Method(type, name, parameters);
        if (method != null)
        {
            Ok(method.ReturnType == result, type + "." + name + " returns " + method.ReturnType + ", not " + result);
        }
    }

    static void Property(System.Type type, string name, System.Type propertyType, bool settable)
    {
        var property = type.GetProperty(name);
        if (property == null)
        {
            System.Console.WriteLine("FAILED (no property " + name + "): " + type);
            failures++;
            return;
        }
        Ok(property.PropertyType == propertyType, type + "." + name + " is " + property.PropertyType + ", not " + propertyType);
        bool hasSetter = property.SetMethod != null && property.SetMethod.IsPublic;
        Ok(hasSetter == settable, type + "." + name + (settable ? " has no public setter" : " has a public setter"));
    }

    static void Absent(System.Type type, string name)
    {
        if (type.GetMember(name).Length != 0)
        {
            System.Console.WriteLine("FAILED (member present): " + type + "." + name);
            failures++;
        }
    }

    static void Done()
    {
        if (failures > 0)
        {
            System.Console.WriteLine(failures + " FAILED");
            System.Environment.Exit(1);
        }
        System.Console.WriteLine("CHECKED");
    }
"#;

/// A check program: the prelude, then `body` as the statements of `Main`.
pub fn check_program(body: &str) -> String {
    format!(
        "public static class Check\n{{\n{CHECK_PRELUDE}\n    public static void Main()\n    {{\n{body}\n        Done();\n    }}\n}}\n"
    )
}

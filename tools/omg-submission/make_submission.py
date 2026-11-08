#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Writes ZeroDDS's submission into a checkout of an OMG interoperability
repository: the client as a crate of its own, built against a published
ZeroDDS version, and the entries the repository's automation reads.

    make_submission.py --suite dds-rtps --repo <checkout> --version 1.0.0-rc.7.3
    make_submission.py --suite dds-rtps --repo <checkout> --version 1.0.0-rc.7 \\
        --patch-root <zerodds checkout>

The clients live in `tools/omg-shape-main`, `tools/omg-xtypes-test-main` and
`tools/omg-security-main`; their sources are copied, nothing is kept twice.
What the precedents put into the repository is what this writes
(`internal/release/rc8-plan.md` F3): int2DDS (dds-rtps #111, dds-xtypes #21),
Dust DDS (dds-xtypes #13), HDDS (dds-rtps #83, dds-xtypes #18).

`--patch-root` is the pre-test: the dependencies are taken from that checkout
(`[patch.crates-io]`) instead of crates.io, so the submission builds and runs
against `main` before the version it names is released. A crate written with
it is not submitted. Its `--version` has to be the checkout's own, because a
patch only replaces a version the requirement accepts.

Needs Python 3.11 or newer (`tomllib`).
"""

import argparse
import re
import shutil
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# Per repository: the client in this tree, where its crate goes, the name the
# repository's harness reads a product from, and the suite's own IDL the
# client generates its type from (the suite's file, not our copy in
# `interop/shapes`, so that the submission measures our generator on the text
# the other vendors compile): its include directory, the file, what the
# generator names its output, and the name the client's source includes.
SUITES = {
    "dds-rtps": {
        "tool": "tools/omg-shape-main",
        "dest": "srcRs/ZeroDDS",
        "package": "zerodds_shape_main_linux",
        "suffix": "shape_main_linux",
        "idl": ("../../srcCxx", "../../srcCxx/shape.idl", "shape.rs", "omg-dds-rtps.rs"),
        "workflow_lists": ".github/workflows/1_run_interoperability_tests.yml",
        "report": "generate_xlsx_report.py",
    },
    "dds-xtypes": {
        "tool": "tools/omg-xtypes-test-main",
        "dest": "src/rs/ZeroDDS",
        "package": "zerodds_test_main_linux",
        "suffix": "test_main_linux",
        "idl": None,
        "workflow_lists": ".github/workflows/1_run_interoperability_tests.yml",
        "report": "generate_xlsx_report.py",
    },
    "dds-security": {
        "tool": "tools/omg-security-main",
        "dest": "srcRs/ZeroDDS",
        "package": "zerodds_shapetype_main_linux",
        "suffix": "linux",
        "idl": ("../../srcCxx", "../../srcCxx/ShapeType.idl", "ShapeType.rs", "omg-dds-security.rs"),
        # No automation in that repository: its matrix is
        # `compatibility_matrix.xlsx`, kept by hand.
        "workflow_lists": None,
        "report": None,
    },
}

PRODUCT = "zerodds"
COMPANY = "ZeroDDS"


def workspace_dependency(name):
    """A `{ workspace = true }` dependency, written out as the workspace has it."""
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["dependencies"]
    spec = workspace[name]
    return {"version": spec} if isinstance(spec, str) else dict(spec)


def crate_dirs():
    """Package name -> directory, for every crate under `crates/`."""
    dirs = {}
    for manifest in (ROOT / "crates").glob("*/Cargo.toml"):
        package = tomllib.loads(manifest.read_text()).get("package", {})
        if "name" in package:
            dirs[package["name"]] = manifest.parent
    return dirs


def toml_value(value):
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, str):
        return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'
    if isinstance(value, list):
        return "[" + ", ".join(toml_value(v) for v in value) + "]"
    if isinstance(value, dict):
        return "{ " + ", ".join(f"{k} = {toml_value(v)}" for k, v in value.items()) + " }"
    raise TypeError(value)


def dependencies(table, version):
    """The tool's dependencies with every ZeroDDS crate taken from crates.io
    at `version` and every workspace one written out."""
    out = {}
    for name, spec in table.items():
        if isinstance(spec, dict) and spec.get("workspace"):
            spec = {**workspace_dependency(name), **{k: v for k, v in spec.items() if k != "workspace"}}
        elif isinstance(spec, dict) and "path" in spec:
            spec = {k: v for k, v in spec.items() if k != "path"}
            spec["version"] = version
        out[name] = spec
    return out


def manifest(suite, tool, version, patch_root):
    config = SUITES[suite]
    source = tomllib.loads((ROOT / tool / "Cargo.toml").read_text())
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]
    lines = [
        "# The ZeroDDS client of this repository's interoperability tests, built",
        f"# against ZeroDDS {version} from crates.io. Its source is",
        f"# `{tool}` in https://github.com/zero-objects/zero-dds.",
        "[workspace]",
        "",
        "[package]",
        f"name = {toml_value(config['package'])}",
        f"version = {toml_value(version)}",
        f"description = {toml_value('OMG ' + suite + ' interoperability: the ZeroDDS client')}",
        f"edition = {toml_value(workspace['edition'])}",
        f"rust-version = {toml_value(workspace['rust-version'])}",
        f"license = {toml_value(workspace['license'])}",
        'repository = "https://github.com/zero-objects/zero-dds"',
        "publish = false",
        "",
    ]
    if "lib" in source:
        lines += ["[lib]", f"name = {toml_value(source['lib']['name'])}", 'path = "src/lib.rs"', ""]
    lines += ["[[bin]]", f"name = {toml_value(config['package'])}", 'path = "src/main.rs"', ""]
    used = set()
    for section in ("build-dependencies", "dependencies"):
        if section not in source:
            continue
        table = dependencies(source[section], version)
        lines.append(f"[{section}]")
        for name, spec in table.items():
            lines.append(f"{name} = {toml_value(spec)}")
            used.add(name)
        lines.append("")
    if patch_root:
        dirs = crate_dirs()
        lines += ["# Pre-test only: the dependencies from a ZeroDDS checkout, not crates.io.",
                  "[patch.crates-io]"]
        for name in sorted(used):
            if name in dirs:
                path = Path(patch_root) / dirs[name].relative_to(ROOT)
                lines.append(f"{name} = {toml_value({'path': str(path)})}")
        lines.append("")
    return "\n".join(lines)


def build_script(suite):
    include, idl, generated, included = SUITES[suite]["idl"]
    return f'''// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ShapeType`, generated by ZeroDDS from this repository's own IDL, so that a
//! run measures what ZeroDDS generates from the text the other vendors compile.
//! An absent extensibility annotation is APPENDABLE (XTypes 1.3 §7.3.1.2.1).

fn main() -> Result<(), zerodds_build::Error> {{
    println!("cargo:rerun-if-changed={idl}");
    zerodds_build::Config::new()
        .include_dir("{include}")
        .default_extensibility(zerodds_build::DefaultExt::Appendable)
        .dds_type_impls(true)
        .compile(&["{idl}"])?;
    // The client's source includes the generated code under the name it has
    // in the ZeroDDS tree.
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("set by cargo"));
    std::fs::copy(out.join("{generated}"), out.join("{included}")).expect("generated code");
    Ok(())
}}
'''


def build_workflow(suite):
    config = SUITES[suite]
    dest, package, suffix = config["dest"], config["package"], config["suffix"]
    return f"""name: CI ZeroDDS

on:
  workflow_dispatch:
  pull_request:
    paths:
    - '{dest}/**'

jobs:
  create_bin_release:
    name: Create binary release
    runs-on: ubuntu-latest
    # Debian 11 has GLIBC 2.31, the oldest the README asks for; ZeroDDS needs
    # Rust 1.97 or newer.
    container: rust:1.97-bullseye
    defaults:
      run:
        working-directory: {dest}
    steps:
    - name: Checkout sources
      uses: actions/checkout@v4
    - name: Install zip
      run: apt-get update && apt-get install -y zip
    - name: Build executable
      run: cargo build --package {package} --release --locked
    - name: Rename executable
      run: |
        version=$( cargo tree --package zerodds-dcps --depth 0 --prefix none | awk '{{print $2}}' | tr -d 'v' )
        cp ./target/release/{package} ./{PRODUCT}-${{version}}_{suffix}
        mkdir artifacts
        zip --junk-paths artifacts/{PRODUCT}-${{version}}_{suffix}.zip ./{PRODUCT}-${{version}}_{suffix}
    - name: Upload executable artifact
      uses: actions/upload-artifact@v4
      with:
        name: interoperability_executable
        path: {dest}/artifacts/
"""


def add_to_lists(path):
    """`"zerodds"` into the default publisher and subscriber lists."""
    text = path.read_text()
    lines = []
    for line in text.splitlines(keepends=True):
        if re.match(r"\s*default: '\[.*\]'\s*$", line) and f'"{PRODUCT}"' not in line:
            line = re.sub(r"\]'(\s*)$", lambda m: f', "{PRODUCT}"]\'' + m.group(1), line)
        lines.append(line)
    path.write_text("".join(lines))


def add_to_report(path):
    """ZeroDDS into `get_company_name` and `get_product_name`, before their
    `else`, in the form the other vendors have."""
    text = path.read_text()
    if f"'{PRODUCT}' in product.lower()" in text:
        return
    out, function = [], None
    for line in text.splitlines(keepends=True):
        # The functions of a product string; dds-xtypes also has a method
        # `get_product_name(self)`, which is not one of them.
        found = re.match(r"\s*def (get_company_name|get_product_name)\(product", line)
        if found:
            function = found.group(1)
        else_line = re.match(r"(\s*)else:\s*$", line)
        if function and else_line:
            indent = else_line.group(1)
            body = indent + "    "
            out.append(f"{indent}elif '{PRODUCT}' in product.lower():\n")
            if function == "get_company_name":
                out.append(f"{body}return '{COMPANY}'\n")
            else:
                out.append(f"{body}return 'ZeroDDS ' + re.search(r'{PRODUCT}-(.+)', product, re.IGNORECASE).group(1)\n")
            function = None
        out.append(line)
    path.write_text("".join(out))


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--suite", required=True, choices=sorted(SUITES))
    parser.add_argument("--repo", required=True, type=Path, help="checkout of the OMG repository")
    parser.add_argument("--version", required=True, help="the ZeroDDS version the client builds against")
    parser.add_argument("--patch-root", type=Path, help="pre-test: take the crates from this ZeroDDS checkout")
    args = parser.parse_args()
    config = SUITES[args.suite]
    repo = args.repo.resolve()
    if not (repo / "CONTRIBUTING.md").is_file():
        sys.exit(f"{repo}: not a checkout of an OMG interoperability repository")
    tool = config["tool"]
    dest = repo / config["dest"]
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(ROOT / tool / "src", dest / "src")
    (dest / "Cargo.toml").write_text(manifest(args.suite, tool, args.version, args.patch_root))
    (dest / ".gitignore").write_text("target/\n")
    if config["idl"]:
        (dest / "build.rs").write_text(build_script(args.suite))
    if config["workflow_lists"]:
        (repo / ".github/workflows/ci_zerodds.yml").write_text(build_workflow(args.suite))
        add_to_lists(repo / config["workflow_lists"])
    if config["report"]:
        add_to_report(repo / config["report"])
    print(f"{args.suite}: {dest.relative_to(repo)} against ZeroDDS {args.version}"
          + (f", patched from {args.patch_root}" if args.patch_root else ""))


if __name__ == "__main__":
    main()

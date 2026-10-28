#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""The public signatures of the machine-readable Java files of DDS-RPC 1.0.

DDS-RPC 1.0 §7.11.2: "The machine readable files associated with this
specification represent the normative language bindings for Java." The
files are `omg-dds-rpc-java.zip` (https://www.omg.org/spec/DDS-RPC/20160215/),
which the repository does not carry. This script reads them and writes
what a program compiled against them may use, one declaration per line,
with every type erased to the class `java.lang.Class.forName` takes:

    type <fqn> <interface|abstract-class|final-class|class|enum> <type-parameters> <supertype>...
    ctor <fqn> (<parameter>,...)
    method <fqn> <static|instance> <type-parameters> <name> <return> (<parameter>,...)
    constant <fqn> <name>

`SpecSignatureCheck.java` checks the runtime against that list by
reflection. Run from the repository root:

    python3 crates/rpc/tests/java/extract_spec_signatures.py \\
        internal/standards/cache/omg/omg-dds-rpc-java.zip \\
        > crates/rpc/tests/java/omg-dds-rpc-java.signatures
"""

import re
import sys
import zipfile

PRIMITIVES = {"void", "boolean", "byte", "short", "int", "long", "char", "float", "double"}
JAVA_LANG = {"Object", "String", "Void", "Class", "ClassLoader", "Runnable", "Boolean",
             "Integer", "Long"}


def strip_comments(text):
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
    return re.sub(r"//[^\n]*", " ", text)


def split_top(text, sep=","):
    """Split at `sep` outside of <> and ()."""
    parts, depth, cur = [], 0, ""
    for ch in text:
        if ch in "<(":
            depth += 1
        elif ch in ">)":
            depth -= 1
        if ch == sep and depth == 0:
            parts.append(cur)
            cur = ""
        else:
            cur += ch
    if cur.strip():
        parts.append(cur)
    return [p.strip() for p in parts]


class Unit:
    def __init__(self, path, text):
        self.text = strip_comments(text)
        self.package = re.search(r"package\s+([\w.]+)\s*;", self.text).group(1)
        self.imports = {}
        for fqn in re.findall(r"import\s+([\w.]+)\s*;", self.text):
            self.imports[fqn.rsplit(".", 1)[1]] = fqn
        head = re.search(
            r"public\s+((?:abstract\s+|final\s+)*)(interface|class|enum)\s+(\w+)\s*(<[^{]*?>)?\s*"
            r"((?:extends|implements)[^{]*)?\{",
            self.text)
        self.modifiers = head.group(1).split()
        self.kind = head.group(2)
        self.name = head.group(3)
        self.type_params = [p.split()[0] for p in split_top(head.group(4)[1:-1])] if head.group(4) else []
        self.supers = []
        clause = head.group(5) or ""
        for part in re.split(r"\b(?:extends|implements)\b", clause):
            self.supers.extend(t for t in split_top(part) if t)
        self.body = self.text[head.end():self.text.rfind("}")]
        self.fqn = self.package + "." + self.name


def erase(unit, units, type_text, type_vars):
    t = type_text.strip()
    t = re.sub(r"\bfinal\s+", "", t)
    dims = 0
    if t.endswith("..."):
        t, dims = t[:-3].strip(), 1
    while t.endswith("[]"):
        t, dims = t[:-2].strip(), dims + 1
    base = re.sub(r"<.*>$", "", t).strip()
    if base in type_vars or base == "?":
        name = "java.lang.Object"
    elif base in PRIMITIVES:
        name = base
    elif "." in base and base.split(".")[0][0].islower():
        name = base  # fully qualified
    else:
        head, *nested = base.split(".")
        if head in unit.imports:
            outer = unit.imports[head]
        elif head in JAVA_LANG:
            outer = "java.lang." + head
        elif unit.package + "." + head in units:
            outer = unit.package + "." + head
        else:
            raise SystemExit(f"{unit.fqn}: cannot resolve {head!r}")
        for member in nested:
            outer = member_type(units, outer, member)
        name = outer
    return name + "[]" * dims


def member_type(units, outer, member):
    """A member type, declared by `outer` or inherited from its supertypes."""
    if outer in units:
        unit = units[outer]
        if re.search(r"\binterface\s+" + member + r"\b|\bclass\s+" + member + r"\b", unit.body):
            return outer + "$" + member
        for sup in unit.supers:
            sup_fqn = erase(unit, units, sup, unit.type_params)
            try:
                return member_type(units, sup_fqn, member)
            except SystemExit:
                continue
        raise SystemExit(f"{outer} has no member type {member}")
    # Outside the files: the DDS Java PSM. `org.omg.dds.sub.Sample.Iterator`
    # is the only member type the files name there.
    if outer == "org.omg.dds.sub.Sample" and member == "Iterator":
        return outer + "$" + member
    raise SystemExit(f"{outer} has no member type {member} this script knows")


def declarations(unit):
    """Top-level members of the body: the text between the braces at depth 0."""
    out, depth, cur = [], 0, ""
    for ch in unit.body:
        if ch == "{":
            if depth == 0:
                out.append(cur.strip() + " {")
                cur = ""
            depth += 1
            continue
        if ch == "}":
            depth -= 1
            continue
        if depth > 0:
            continue
        if ch == ";":
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    return [d for d in (re.sub(r"\s+", " ", d) for d in out) if d]


def describe(unit, units):
    lines = []
    if unit.kind == "interface":
        kind = "interface"
    elif unit.kind == "enum":
        kind = "enum"
    elif "abstract" in unit.modifiers:
        kind = "abstract-class"
    elif "final" in unit.modifiers:
        kind = "final-class"
    else:
        kind = "class"
    supers = [erase(unit, units, s, unit.type_params) for s in unit.supers]
    lines.append(" ".join(["type", unit.fqn, kind, str(len(unit.type_params))] + supers))
    if unit.kind == "enum":
        constants = unit.body.split(";")[0]
        for name in split_top(constants):
            lines.append(f"constant {unit.fqn} {name}")
        return lines
    has_ctor = False
    for decl in declarations(unit):
        decl = re.sub(r"@\w+\s*", "", decl).strip()
        m = re.match(
            r"((?:public\s+|static\s+|abstract\s+|final\s+|default\s+)*)(<[^()]*?>\s*)?"
            r"([\w.<>\[\], ?]+?)\s+(\w+)\s*\(([^)]*)\)", decl)
        if not m:
            c = re.match(r"(?:public\s+)?" + unit.name + r"\s*\(([^)]*)\)", decl)
            if c:
                has_ctor = True
                params = [erase(unit, units, p.rsplit(" ", 1)[0], unit.type_params)
                          for p in split_top(c.group(1))]
                lines.append(f"ctor {unit.fqn} ({','.join(params)})")
            continue  # a field: package-private in these files
        modifiers, generics, ret, name, params = m.groups()
        method_vars = [p.split()[0] for p in split_top(generics.strip()[1:-1])] if generics else []
        scope = unit.type_params + method_vars
        static = "static" if "static" in modifiers.split() else "instance"
        erased = [erase(unit, units, p.rsplit(" ", 1)[0], scope) for p in split_top(params)]
        lines.append(" ".join(["method", unit.fqn, static, str(len(method_vars)), name,
                               erase(unit, units, ret, scope), "(" + ",".join(erased) + ")"]))
    if unit.kind == "class" or kind == "final-class":
        if not has_ctor:
            lines.append(f"ctor {unit.fqn} ()")  # the default constructor, JLS §8.8.9
    return lines


def main():
    archive = zipfile.ZipFile(sys.argv[1])
    units = {}
    for entry in archive.namelist():
        if entry.endswith(".java"):
            unit = Unit(entry, archive.read(entry).decode("utf-8", "replace"))
            units[unit.fqn] = unit
    print("# The public signatures of omg-dds-rpc-java.zip (DDS-RPC 1.0 §7.11.2),")
    print("# written by extract_spec_signatures.py; checked by SpecSignatureCheck.java.")
    for fqn in sorted(units):
        for line in describe(units[fqn], units):
            print(line)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""roster.py — the denominator of the OMG DDS-XTypes 1.3 negative list.

OMG IDL 4.2 has Annex A: a consolidated, numbered grammar, from which a
complete inventory of constructs falls out with a hole check
(`range(1, 228)` against the parsed numbers). XTypes 1.3 has no grammar.
Its inventory is a set of normative tables and IDL blocks, and every one
of them needs its own extractor and its own completeness argument.

This script builds that inventory from the specification PDF and prints
the check for each axis. The PDF itself is OMG intellectual property and
is not in the tree; it is read from
`internal/standards/cache/omg/dds-xtypes-1.3.pdf` (gitignored) via
`pdftotext -layout`. What is committed is the extracted roster —
constant names, clause numbers, table row labels — and the checks that
say the extraction is complete.

    roster.py                 write roster.tsv, print the checks
    roster.py --check         print the checks, write nothing (exit 1 on
                              a failed check)
    XTYPES_PDF=/path/to.pdf roster.py

Exit status:
    0  every axis extracted and every completeness check held
    1  a completeness check failed — the denominator is not trustworthy
    2  nothing was extracted: no PDF, or no pdftotext
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
DEFAULT_PDF = ROOT / "internal/standards/cache/omg/dds-xtypes-1.3.pdf"
OUT = HERE / "roster.tsv"


def die(msg, code=2):
    print(f"xtypes-roster: {msg}", file=sys.stderr)
    sys.exit(code)


def spec_text():
    pdf = Path(os.environ.get("XTYPES_PDF", DEFAULT_PDF))
    if not pdf.is_file():
        die(f"no specification PDF at {pdf}.\n"
            "  DDS-XTypes 1.3 is OMG intellectual property and is not in the\n"
            "  tree. Put formal/20-02-04 there, or set XTYPES_PDF.\n"
            "  Nothing was extracted, so nothing is confirmed.")
    if shutil.which("pdftotext") is None:
        die("no pdftotext (poppler). Nothing was extracted.")
    with tempfile.TemporaryDirectory() as tmp:
        txt = Path(tmp) / "xtypes.txt"
        subprocess.run(["pdftotext", "-layout", str(pdf), str(txt)],
                       check=True, stdout=subprocess.DEVNULL,
                       stderr=subprocess.DEVNULL)
        raw = txt.read_text(encoding="utf-8", errors="replace")
    # `pdftotext` puts a form feed at every page break, and a clause number
    # that lands at the top of a page then sits behind it — `^7.2.4.4.6`
    # would not match and the hole check would report a hole the document
    # does not have. Six of them do.
    raw = raw.replace("\f", "\n")
    for a, b in (("\u201c", '"'), ("\u201d", '"'), ("\u2018", "'"),
                 ("\u2019", "'"), ("\u2013", "-"), ("\u2014", "-")):
        raw = raw.replace(a, b)
    # The table of contents repeats every caption and every clause number.
    # A search over the whole text finds the entry rather than the table,
    # so the body is taken from the first clause heading at column 0.
    body = re.search(r"^7\.1 Overview\s*$", raw, re.M)
    if not body:
        die("the body of the specification was not found in the extracted "
            "text — is this the DDS-XTypes 1.3 PDF?")
    return raw[body.start():]


CHECKS = []   # (axis, ok, sentence)
ROWS = []     # (axis, item, refs, note)


def check(axis, ok, sentence):
    CHECKS.append((axis, bool(ok), sentence))


def emit(axis, item, refs, note=""):
    ROWS.append((axis, item, refs, note))


# --------------------------------------------------------------- axis 1
def axis_typekind(t):
    """§7.3.4.2 / Annex B — the TypeKind octet constants.

    The completeness argument is a cross-check rather than a hole check:
    Annex B declares the constants twice, once as `const octet TK_x` in
    `dds-xtypes_typeobject.idl` and once as `const TypeKind TK_x` in the
    dynamic-language-binding IDL. Both listings must agree on every name
    and every value, and no value may be used twice.
    """
    a = dict(re.findall(r"const octet\s+(TK_[A-Z0-9_]+)\s*=\s*(0x[0-9A-Fa-f]+);", t))
    b = dict(re.findall(r"const TypeKind\s+(TK_[A-Z0-9_]+)\s*=\s*(0x[0-9A-Fa-f]+);", t))
    check("typekind", a and b and a == b,
          f"the two Annex B listings of the TypeKind constants agree "
          f"({len(a)} and {len(b)} constants)")
    vals = [int(v, 16) for v in a.values()]
    check("typekind", len(set(vals)) == len(vals),
          "no TypeKind value is declared twice")
    for name, v in sorted(a.items(), key=lambda kv: int(kv[1], 16)):
        emit("typekind", name, "7.3.4.2, Annex B", v.lower())
    return a


# --------------------------------------------------------------- axis 2
def axis_typeidentifier(t, tks):
    """§7.3.4.2 Table 29 + the `union TypeIdentifier switch (octet)` of
    Annex B — every discriminator a TypeIdentifier may carry.

    Completeness: every `TI_*` constant Annex B declares must appear as a
    case label of that union, and every case label must be a declared
    constant. The union's primitive arm is commented out in the
    specification ("Unions cannot have cases with no member"), so the
    16 primitive TypeKinds are added from the TypeKind axis.
    """
    ti = dict(re.findall(r"const octet\s+(TI_[A-Z0-9_]+)\s*=\s*(0x[0-9A-Fa-f]+);", t))
    ek = dict(re.findall(r"const octet\s+(EK_[A-Z0-9_]+)\s*=\s*(0x[0-9A-Fa-f]+);", t))
    m = re.search(r"union TypeIdentifier switch \(octet\) \{(.*?)\n\s*\};", t, re.S)
    if not m:
        die("the TypeIdentifier union was not found in the extracted text")
    body = m.group(1)
    labels = set(re.findall(r"case\s+([A-Z0-9_]+)\s*:", body))
    has_default = "default:" in body

    declared_ti = set(ti)
    # `TI_STRONG_COMPONENT` is the short name the prose uses for
    # TI_STRONGLY_CONNECTED_COMPONENT; only the long name is declared.
    check("typeid", declared_ti <= labels,
          f"every TI_* constant Annex B declares is a case of the union "
          f"({len(declared_ti)} constants)")
    unknown = {l for l in labels if not (l in ti or l in ek or l in tks)}
    # The commented-out primitive arm names TK_BYTE_TYPE, TK_INT8_TYPE …,
    # which Annex B never declares. That is a defect of the document, not
    # of the extraction; it is recorded rather than silently dropped.
    check("typeid", unknown <= {n + "_TYPE" for n in tks} | {"TK_BOOLEAN", "TK_NONE"},
          f"every case label resolves to a declared constant, "
          f"modulo the {len(unknown)} `TK_*_TYPE` spellings the union uses "
          f"and Annex B never declares")
    check("typeid", has_default, "the union carries a `default:` arm (TI_EXTENDED)")

    for name, v in sorted(tks.items(), key=lambda kv: int(kv[1], 16)):
        if int(v, 16) <= 0x11:            # primitive TypeKinds only
            emit("typeid", name, "7.3.4.2, Table 29", v.lower())
    for name, v in sorted(ti.items(), key=lambda kv: int(kv[1], 16)):
        emit("typeid", name, "7.3.4.2, Table 29", v.lower())
    for name in ("EK_COMPLETE", "EK_MINIMAL"):
        emit("typeid", name, "7.3.4.2, Table 29", ek[name].lower())
    # Table 29 lists TK_ANNOTATION as a TypeIdentifier discriminator —
    # "Uses Hash of the TypeObject representation of the Annotation
    # declaration" — and the Annex B union has no such case. The roster
    # carries the row Table 29 carries, and the disagreement is asserted
    # rather than smoothed over: if a later revision adds the case, this
    # check fails and the roster is rebuilt.
    t29 = re.search(r"Table 29 - Formats and interpretation of the "
                    r"TypeIdentifier(.*?)^7\.3\.4\.3 Complete TypeObject",
                    t, re.S | re.M)
    if not t29:
        die("Table 29 was not found in the extracted text")
    check("typeid", "TK_ANNOTATION" in t29.group(1)
          and "TK_ANNOTATION" not in labels,
          "Table 29 lists TK_ANNOTATION as a TypeIdentifier discriminator "
          "and the Annex B union has no case for it")
    emit("typeid", "TK_ANNOTATION", "7.3.4.2, Table 29", "0x50")
    emit("typeid", "TI_EXTENDED", "7.3.4.2, Table 29", "default")
    return ti, ek


# --------------------------------------------------------------- axis 3
def axis_annotation(t):
    """§7.3.1.2 — the built-in annotation language.

    Completeness: §7.3.1.2.1 has sixteen numbered subclauses and the hole
    check is `range(1, 17)` against the numbers found. Table 21 then
    names the elements each annotation may be applied to; every name in
    Table 21 must be introduced by one of the sixteen subclauses.
    """
    subs = sorted({int(n) for n in
                   re.findall(r"^7\.3\.1\.2\.1\.(\d+)\b", t, re.M)})
    check("annotation", subs == list(range(1, 17)),
          f"§7.3.1.2.1 has the subclauses 1..16 with no hole "
          f"(found {len(subs)})")

    tbl = re.search(r"Table 21 - IDL Built-in Annotations Usage(.*?)"
                    r"7\.3\.1\.2\.3", t, re.S)
    if not tbl:
        die("Table 21 was not found in the extracted text")
    names = []
    for n in re.findall(r"@([a-z_]+)", tbl.group(1)):
        if n not in names:
            names.append(n)
    check("annotation", len(names) >= 19,
          f"Table 21 names {len(names)} annotations")

    # The subclauses introduce annotations Table 21 does not restrict,
    # because Table 21 only lists those whose applicability is narrowed.
    extra = ["default_nested", "ignore_literal_names", "data_representation",
             "topic", "default"]
    for n in names:
        emit("annotation", "@" + n, "7.3.1.2.1, Table 21", "Table 21")
    for n in extra:
        if n not in names:
            emit("annotation", "@" + n, "7.3.1.2.1", "subclause only")
    return names


# --------------------------------------------------------------- axis 4
def axis_assignability(t):
    """§7.2.4.4 — the assignability rules.

    Completeness: §7.2.4.4 has eight numbered subclauses (hole check
    `range(1, 9)`), and Tables 14 through 19 carry the per-kind
    definitions of is-assignable-from. Both the subclause numbers and the
    six table numbers are checked; the rows of each table are the roster
    entries.
    """
    subs = sorted({int(n) for n in
                   re.findall(r"^7\.2\.4\.4\.(\d+)\b", t, re.M)})
    check("assign", subs == list(range(1, 9)),
          f"§7.2.4.4 has the subclauses 1..8 with no hole (found {len(subs)})")

    want = {
        14: "alias types", 15: "primitive types", 16: "string types",
        17: "collection types", 18: "bitmask and enumerated types",
        19: "aggregated types",
    }
    found = {}
    for num, title in re.findall(
            r"Table (1[4-9]) - Definition of the is-assignable-from "
            r"relationship for ([a-z ]+)", t):
        found[int(num)] = title.strip()
    check("assign", set(found) == set(want),
          f"Tables 14..19 all carry an is-assignable-from definition "
          f"(found {sorted(found)})")
    # The rows of these six tables are prose, not a grammar, so the roster
    # entries come from `anchors.tsv` — one line per rule, each carrying a
    # literal fragment of the specification that must be found in the PDF.
    lines = {re.sub(r"\s+", " ", l).strip() for l in t.split("\n")}
    n_rules, missing = 0, []
    for raw in (HERE / "anchors.tsv").read_text(encoding="utf-8").splitlines():
        if raw.startswith("#") or not raw.strip():
            continue
        rid, axis, refs, anchor = raw.split("\t")
        n_rules += 1
        if not any(anchor in l for l in lines):
            missing.append(rid)
        emit(axis, rid, refs, anchor)
    check("assign", not missing,
          f"every one of the {n_rules} rules in anchors.tsv is a literal "
          f"fragment of the specification"
          + (f" — missing: {missing}" if missing else ""))
    return subs


# --------------------------------------------------------------- axis 5
def axis_typeobject(t):
    """§7.3.4.3/7.3.4.4 — the forms a TypeObject may take.

    Completeness: `MinimalTypeObject` and `CompleteTypeObject` are two
    unions over the same discriminator, and the specification's own
    invariant is that they carry the same set of case labels. That
    equality is the check. `TypeObject` itself switches on the
    EquivalenceKind and must carry exactly EK_COMPLETE and EK_MINIMAL.
    """
    def cases(name):
        m = re.search(r"union %s switch \(octet\)[^{]*\{(.*?)\n\s*\};" % name,
                      t, re.S)
        if not m:
            die(f"the {name} union was not found in the extracted text")
        return set(re.findall(r"case\s+([A-Z0-9_]+)\s*:", m.group(1))), \
            ("default:" in m.group(1))

    mini, mini_d = cases("MinimalTypeObject")
    comp, comp_d = cases("CompleteTypeObject")
    check("typeobject", mini == comp,
          f"MinimalTypeObject and CompleteTypeObject carry the same "
          f"{len(mini)} case labels")
    check("typeobject", mini_d and comp_d,
          "both carry a `default:` arm (the extended type)")

    outer, outer_d = cases("TypeObject")
    check("typeobject", outer == {"EK_COMPLETE", "EK_MINIMAL"} and not outer_d,
          "TypeObject switches on EK_COMPLETE and EK_MINIMAL and nothing else")

    for label in sorted(mini):
        emit("typeobject", label, "7.3.4.3, 7.3.4.4, Annex B", "Minimal+Complete")
    emit("typeobject", "EXTENDED", "7.3.4.3, 7.3.4.4", "default arm")
    emit("typeobject", "EK_MINIMAL", "7.3.4.5", "TypeObject discriminator")
    emit("typeobject", "EK_COMPLETE", "7.3.4.5", "TypeObject discriminator")
    return mini


# --------------------------------------------------------------- axis 6
def axis_typelookup(t):
    """§7.6.3.3 — the built-in TypeLookup service.

    Completeness: §7.6.3.3 has four numbered subclauses (hole check
    `range(1, 5)`), and the two service operations of §7.6.3.3.4 are
    named by their own subclauses. The IDL of §7.6.3.3.3 declares the
    request and reply types; every `_In`/`_Out` struct in it is a roster
    entry.
    """
    subs = sorted({int(n) for n in
                   re.findall(r"^7\.6\.3\.3\.(\d+)\b", t, re.M)})
    check("typelookup", subs == list(range(1, 5)),
          f"§7.6.3.3 has the subclauses 1..4 with no hole (found {len(subs)})")

    ops = re.findall(r"^7\.6\.3\.3\.4\.\d+ Service operation (\w+)", t, re.M)
    check("typelookup", sorted(ops) == ["getTypeDependencies", "getTypes"],
          f"§7.6.3.3.4 defines exactly the two service operations "
          f"{sorted(ops)}")

    block = re.search(r"7\.6\.3\.3\.3\s+TypeLookup Types and Endpoints(.*?)"
                      r"7\.6\.3\.3\.4\s", t, re.S)
    if not block:
        die("§7.6.3.3.3 was not found in the extracted text")
    structs = []
    for s in re.findall(r"(?:struct|union)\s+(TypeLookup_\w+)",
                        block.group(1)):
        if s not in structs:
            structs.append(s)
    check("typelookup", len(structs) == 10,
          f"§7.6.3.3.3 declares the six request/reply structs and the four "
          f"result unions ({len(structs)} TypeLookup_* types)")
    for s in structs:
        emit("typelookup", s, "7.6.3.3.3", "IDL")
    for o in sorted(ops):
        emit("typelookup", o, "7.6.3.3.4", "operation")
    # Table 61/62 add the four built-in endpoints the service runs over.
    for e in ("TypeLookupServiceRequestWriter", "TypeLookupServiceRequestReader",
              "TypeLookupServiceReplyWriter", "TypeLookupServiceReplyReader"):
        emit("typelookup", e, "7.6.3.3.3, Table 61", "built-in endpoint")
    return structs


# --------------------------------------------------------------- axis 7
def axis_datarep(t):
    """§7.6.3.1 Table 59 — the DataRepresentationQosPolicy compatibility
    matrix, and Table 60, the encapsulation identifier it selects.

    Completeness: Table 59 is a 2x2 over {XCDR, XCDR2} offered against
    {XCDR, "XCDR and XCDR2"} requested, and the check is that all four
    cells carry a verdict. Table 60 is the twelve rows of
    representation x extensibility x encoding version x endianness, and
    the check is that twelve `{0x00, 0x..}` identifier values are read
    out of it.
    """
    m = re.search(r"Table 59 - Compatibility matrix for the "
                  r"DataRepresentationQosPolicy(.*?)7\.6\.3\.1\.2", t, re.S)
    if not m:
        die("Table 59 was not found in the extracted text")
    verdicts = re.findall(r"(Not Compatible|Compatible)\.", m.group(1))
    check("datarep", len(verdicts) == 4,
          f"Table 59 carries four verdicts (found {len(verdicts)}: {verdicts})")

    m60 = re.search(r"Table 60 - RTPS encapsulation identifier(.*?)"
                    r"7\.6\.3\.2", t, re.S)
    if not m60:
        die("Table 60 was not found in the extracted text")
    cells = [("XCDR", "XCDR"), ("XCDR", "XCDR+XCDR2"),
             ("XCDR2", "XCDR"), ("XCDR2", "XCDR+XCDR2")]
    for (off, req), verdict in zip(cells, verdicts):
        emit("datarep", f"offered={off} requested={req}", "7.6.3.1, Table 59",
             verdict)
    rows = re.findall(
        r"(XCDR)\s+(FINAL|APPENDABLE|MUTABLE)\s+(\d)\s+"
        r"(Big Endian|Little Endian)\s+(\w+)\s+\{0x00, (0x[0-9A-Fa-f]{2})\}",
        m60.group(1))
    check("datarep", len(rows) == 12,
          f"all twelve Table 60 rows parse as "
          f"representation/extensibility/version/endianness/identifier "
          f"(parsed {len(rows)})")
    for _, ext, ver, end, name, val in rows:
        emit("datarep", f"{name}={val}", "7.6.3.1.2, Table 60",
             f"{ext} v{ver} {end}")
    return verdicts


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    if mode not in ("", "--check"):
        die(f"unknown argument {mode}")

    t = spec_text()
    tks = axis_typekind(t)
    axis_typeidentifier(t, tks)
    axis_annotation(t)
    axis_assignability(t)
    axis_typeobject(t)
    axis_typelookup(t)
    axis_datarep(t)

    per_axis = {}
    for axis, _, _, _ in ROWS:
        per_axis[axis] = per_axis.get(axis, 0) + 1

    bad = 0
    for axis, ok, sentence in CHECKS:
        print(f"  [{'ok ' if ok else 'FAIL'}] {axis:<11} {sentence}")
        if not ok:
            bad += 1
    print()
    for axis in sorted(per_axis):
        print(f"  {axis:<11} {per_axis[axis]:>3} item(s)")
    print(f"  {'total':<11} {len(ROWS):>3} item(s) over {len(per_axis)} axes")

    if bad:
        print(f"\nxtypes-roster: {bad} completeness check(s) failed. "
              f"The denominator is not trustworthy.", file=sys.stderr)
        sys.exit(1)

    if mode != "--check":
        with OUT.open("w", encoding="utf-8") as f:
            f.write("# The denominator of docs/spec-coverage/"
                    "dds-xtypes-1.3.gaps.md.\n")
            f.write("# Written by tools/xtypes-1.3-gap-scan/roster.py from "
                    "the OMG PDF.\n")
            f.write("#axis\titem\trefs\tnote\n")
            for row in ROWS:
                f.write("\t".join(row) + "\n")
        print(f"\nxtypes-roster: wrote {len(ROWS)} item(s) to {OUT}")


if __name__ == "__main__":
    main()

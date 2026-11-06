#!/usr/bin/env python3
"""Write tests/interop-corpus/expected-bytes.tsv from a zerodds-matrix commit.

The matrix (`zerodds-matrix`, the `matrix/` checkout) is the one place where
wire bytes are measured (rc8-test-suite.md, decision 3). This script reads
one commit of it and writes the octets the other implementations put on the
wire for every corpus case, sample, encoding and byte order, keyed by the
ZeroDDS dialect that has to reproduce them. Every producer of ZeroDDS bytes
checks its output against that file in its own crate; CI never clones the
matrix.

Oracles, and only these:

- A vendor dialect (`cyclone`, `rti`, `fastdds`, `opendds`, `opensplice`,
  `hdds`, `omniorb`, `tao`, `jacorb`, `ros-cyclone`, `ros-fastrtps`): the
  octets that vendor's own serialiser wrote in its matrix column. The CORBA
  ORBs speak classic CDR, which is XCDR1 final, so their `cdr-<endian>`
  cells answer the `xcdr1` rows only.
- `specification`: the octets on which every DDS implementation of the
  matrix that wrote the cell agrees, and at least two did (Cyclone, RTI,
  Fast DDS, OpenDDS, OpenSplice, HDDS). No spec vector of XTypes 1.3 is a
  corpus case, so this is the only outside oracle the specification rows
  have; a cell where the vendors disagree, or only one wrote, has no
  `specification` row. Neither has a case in the matrix's `BOUNDARY_CASES`
  (`defects.py`): the specification rules its construct out, so its answer
  is a refusal, whatever the vendors that resolved it wrote. Nor has a cell
  whose construct the specification answers differently from the vendors'
  agreement, or not at all (`SPEC_NOT_THE_VENDORS` below, each with its
  clause): there the vendors' agreement is not the specification's answer.

Never read: any `zerodds-*` column. A ZeroDDS column holds this project's
own earlier output, which is not an oracle (rc8-test-suite.md, Rules).

A matrix case enters only when its files are those of the corpus case of the
same name (IDL, top-level type, sample); otherwise the vendor bytes answer a
different question, and the case is counted under `differs from the corpus`.

The bytes are read from the commit (`git cat-file`), not from the working
tree, so the table is what the named commit holds whatever is checked out.

    tools/expected-bytes/generate.py --matrix ../zerodds/matrix
    tools/expected-bytes/generate.py --matrix ../zerodds/matrix --check
"""
import argparse
import ast
import collections
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# DDS implementations whose agreement is the specification oracle.
DDS = ("cyclone", "rti", "fastdds", "opendds", "opensplice", "hdds")
ORBS = ("omniorb", "tao", "jacorb")
ROS = ("ros-cyclone", "ros-fastrtps")
# The dialect order in a row: the default first, then `Dialect::ALL`'s.
DIALECTS = ("specification", "cyclone", "rti", "fastdds", "omniorb", "tao", "jacorb",
            "opendds", "ros-fastrtps", "ros-cyclone", "opensplice", "hdds")
# Vendors whose alignment gaps carry undefined octets, with the clause or
# source that says so. A ZeroDDS dialect for them writes 0x00 there and the
# check accepts a difference at a padding position only.
GAPS_UNDEFINED = {
    "omniorb": "CORBA 3.3 Part 2 9.3.1.1: the value of octets in alignment gaps is undefined",
    "tao": "CORBA 3.3 Part 2 9.3.1.1: the value of octets in alignment gaps is undefined",
    "jacorb": "CORBA 3.3 Part 2 9.3.1.1: the value of octets in alignment gaps is undefined",
    "opensplice": "sd_cdr.c:4425 clear_padding = 0, buffer from os_malloc (OpenSplice 6.9)",
}
MODES = (("xcdr1", "le"), ("xcdr1", "be"), ("xcdr2", "le"), ("xcdr2", "be"))
# Constructs where the vendors' agreement is not the specification's answer,
# found by the IDL of the sample: (name, regular expression, clause).
SPEC_NOT_THE_VENDORS = (
    ("map", r"\bmap\s*<",
     "XTypes 1.3 7.4.3.5.3 rules (14)-(16) fix no order of a map's entries (the vendors write "
     "their own sorted container's), and its XCDR2 framing is the open issue DDSXTY14-72 "
     "(docs/omg-xtypes-issues.md A1)"),
    ("mutable union", r"@mutable\s+union\b",
     "XTypes 1.3 7.2.2.4.4.4.6 and 7.4.3.4.2 require M_FLAG on a @mutable union's "
     "discriminator; no measured vendor sets it (docs/omg-xtypes-issues.md B2)"),
)
SAMPLES = {"values": "values.json", "old": "old.json"}


def git(matrix, *args, data=None):
    return subprocess.run(["git", "-C", str(matrix), *args], input=data,
                          capture_output=True, check=True).stdout


def tree(matrix, rev, paths):
    """Every blob under `paths` at `rev`: path -> (blob id, size)."""
    listing = git(matrix, "ls-tree", "-r", "--long", "-z", rev, "--", *paths)
    blobs = {}
    for entry in listing.split(b"\0"):
        if not entry:
            continue
        meta, path = entry.split(b"\t", 1)
        _mode, kind, blob, size = meta.split()
        if kind == b"blob":
            blobs[path.decode()] = (blob.decode(), int(size))
    return blobs


def read_blobs(matrix, ids):
    """The contents of every blob in `ids`, in one `cat-file --batch`."""
    ids = sorted(set(ids))
    out = git(matrix, "cat-file", "--batch", data="".join(i + "\n" for i in ids).encode())
    contents, pos = {}, 0
    for blob in ids:
        header_end = out.index(b"\n", pos)
        header = out[pos:header_end].split()
        if header[1] != b"blob":
            raise SystemExit(f"generate.py: {blob} is {header[1].decode()}, not a blob")
        size = int(header[2])
        start = header_end + 1
        contents[blob] = out[start:start + size]
        pos = start + size + 1
    return contents


def boundary_cases(source):
    """The `BOUNDARY_CASES` set literal of the matrix's defects.py."""
    match = re.search(r"^BOUNDARY_CASES = (\{.*?^\})", source, re.S | re.M)
    if not match:
        raise SystemExit("generate.py: no BOUNDARY_CASES in the matrix's defects.py")
    return ast.literal_eval(match[1])


def vendor_optset(dialect, encoding, endian):
    if dialect in ORBS:
        return f"cdr-{endian}" if encoding == "xcdr1" else None
    return f"{encoding}-{endian}"


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--matrix", type=Path, required=True,
                        help="a checkout of zerodds-matrix (read only)")
    parser.add_argument("--rev", default="HEAD", help="the matrix commit (default HEAD)")
    parser.add_argument("--corpus", type=Path, default=ROOT / "tests/interop-corpus")
    parser.add_argument("--out", type=Path, default=None,
                        help="default: <corpus>/expected-bytes.tsv")
    parser.add_argument("--check", action="store_true",
                        help="compare with the file instead of writing it")
    args = parser.parse_args()
    out_path = args.out or args.corpus / "expected-bytes.tsv"

    commit = git(args.matrix, "rev-parse", f"{args.rev}^{{commit}}").decode().strip()
    when = git(args.matrix, "log", "-1", "--format=%cI", commit).decode().strip()
    vendors = sorted(set(DIALECTS) - {"specification"})
    blobs = tree(args.matrix, commit, ["cases", "defects.py", *vendors])
    defects = blobs["defects.py"][0]
    boundary = boundary_cases(read_blobs(args.matrix, [defects])[defects].decode())

    # The matrix cases and their files.
    case_files = collections.defaultdict(dict)
    for path, (blob, _size) in blobs.items():
        parts = path.split("/")
        if parts[0] == "cases" and len(parts) == 3:
            case_files[parts[1]][parts[2]] = blob
    texts = read_blobs(args.matrix, [b for files in case_files.values() for b in files.values()])

    corpus = sorted(p.name for p in args.corpus.iterdir() if (p / "type.idl").is_file())
    counts = collections.Counter()
    excluded = collections.defaultdict(list)
    included = {}
    for case in corpus:
        files = case_files.get(case)
        if files is None:
            excluded["not in the matrix"].append(case)
            continue
        here = args.corpus / case
        same = (texts[files["type.idl"]].decode() == (here / "type.idl").read_text()
                and texts[files["type"]].decode().strip() == (here / "type").read_text().strip())
        samples = []
        for sample, corpus_name in SAMPLES.items():
            in_matrix = f"{sample}.jsn" in files
            in_corpus = (here / corpus_name).is_file()
            if in_matrix != in_corpus:
                same = False
                continue
            if not in_matrix:
                continue
            if json.loads(texts[files[f"{sample}.jsn"]]) != json.loads((here / corpus_name).read_text()):
                same = False
            if sample != "values":
                own_idl = here / f"{sample}.idl"
                if (f"{sample}.idl" in files) != own_idl.is_file() or (
                        own_idl.is_file() and texts[files[f"{sample}.idl"]].decode() != own_idl.read_text()):
                    same = False
            samples.append(sample)
        if not same:
            excluded["differs from the corpus"].append(case)
            continue
        included[case] = samples
    matrix_only = sorted(set(case_files) - set(corpus))

    # The recorded octets, per vendor cell.
    def recorded(path):
        entry = blobs.get(path)
        if entry is None:
            return None
        if entry[1] > 0:
            return entry[0]
        # A zero-length .bin is a recording (`empty-struct`) unless the
        # writer left an error file beside it (a crash).
        return None if path + ".err" in blobs else entry[0]

    wanted = {}
    for case, samples in included.items():
        for sample in samples:
            for encoding, endian in MODES:
                for vendor in vendors:
                    optset = vendor_optset(vendor, encoding, endian)
                    if optset is None:
                        continue
                    blob = recorded(f"{vendor}/{optset}/{case}/{vendor}-{optset}-{case}.{sample}.bin")
                    if blob is not None:
                        wanted[(case, sample, encoding, endian, vendor)] = blob
    octets = read_blobs(args.matrix, wanted.values())

    rows = []
    per_dialect = collections.Counter()
    for case, samples in sorted(included.items()):
        for sample in samples:
            for encoding, endian in MODES:
                groups = collections.defaultdict(list)
                for vendor in vendors:
                    blob = wanted.get((case, sample, encoding, endian, vendor))
                    if blob is not None:
                        groups[octets[blob]].append(vendor)
                dds = {data: [v for v in names if v in DDS] for data, names in groups.items()}
                writers = [data for data, names in dds.items() if names]
                idl_name = f"{sample}.idl" if f"{sample}.idl" in case_files[case] else "type.idl"
                idl_text = texts[case_files[case][idl_name]].decode()
                silent = [name for name, pattern, _ in SPEC_NOT_THE_VENDORS
                          if re.search(pattern, idl_text)]
                if case in boundary:
                    counts["specification: boundary case, a refusal"] += 1
                elif silent:
                    counts[f"specification: excluded, {silent[0]}"] += 1
                elif len(writers) == 1 and len(dds[writers[0]]) >= 2:
                    groups[writers[0]].append("specification")
                    counts["specification: vendors agree"] += 1
                elif len(writers) > 1:
                    counts["specification: vendors disagree"] += 1
                elif len(writers) == 1:
                    counts["specification: one vendor wrote"] += 1
                else:
                    counts["specification: no DDS vendor wrote"] += 1
                for data, names in groups.items():
                    ordered = [d for d in DIALECTS if d in names]
                    per_dialect.update(ordered)
                    rows.append((case, sample, encoding, endian, ",".join(ordered), data.hex()))
    rows.sort()

    lines = [
        "# Expected wire bytes of the interop corpus, per ZeroDDS dialect.",
        "# Generated by tools/expected-bytes/generate.py; do not edit by hand.",
        f"# matrix-commit: {commit} ({when})",
        "# oracle: a vendor dialect expects its own matrix column's octets (the CORBA ORBs'",
        "#   cdr-<endian> cells answer xcdr1 only); `specification` expects the octets every DDS",
        f"#   implementation that wrote the cell agrees on, at least two of {', '.join(DDS)}.",
        "#   No zerodds-* column is read. A case of the matrix's BOUNDARY_CASES has no",
        f"#   specification row (its answer is a refusal): {' '.join(sorted(boundary))}.",
        "# payload: the serialized sample without the encapsulation header.",
    ]
    for name, _pattern, clause in SPEC_NOT_THE_VENDORS:
        lines.append(f"# no specification row ({name}): {clause}")
    for vendor, reason in GAPS_UNDEFINED.items():
        lines.append(f"# gaps-undefined: {vendor}: {reason}")
    lines.append(f"# cases: {len(included)} of {len(corpus)} corpus cases;"
                 + "".join(f" {len(v)} {k}" + ";" for k, v in sorted(excluded.items()))
                 + f" {len(matrix_only)} matrix cases not in the corpus")
    for reason, cases in sorted(excluded.items()):
        lines.append(f"# excluded ({reason}): {' '.join(cases)}")
    lines.append("# cells per dialect: " + ", ".join(f"{d} {per_dialect[d]}" for d in DIALECTS))
    lines.append("# specification cells: " + ", ".join(
        f"{k.split(': ', 1)[1]} {v}" for k, v in sorted(counts.items())))
    lines.append("case\tsample\tencoding\tendian\tdialects\thex")
    lines.extend("\t".join(row) for row in rows)
    text = "\n".join(lines) + "\n"

    if args.check:
        current = out_path.read_text() if out_path.is_file() else ""
        if current != text:
            print(f"{out_path}: not what matrix {commit[:11]} gives; rerun without --check",
                  file=sys.stderr)
            return 1
        print(f"{out_path}: matches matrix {commit[:11]}")
        return 0
    out_path.write_text(text)
    print(f"{out_path}: {len(rows)} rows, {len(text)} bytes, matrix {commit[:11]}; "
          f"{len(included)} cases; matrix-only {len(matrix_only)}: {' '.join(matrix_only)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

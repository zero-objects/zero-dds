#!/usr/bin/env python3
"""Write tests/interop-corpus/cyclone-type-objects.tsv from a zerodds-matrix commit.

CycloneDDS's `idlc` compiles every corpus case in the matrix
(`cyclone/<encoding>/<case>/src/values/gen.c`), and each topic descriptor it
emits carries the type's `TypeInformation` and `TypeMapping` (DDS-XTypes 1.3
7.6.3.2.1 and 7.6.3.2.2) as C byte arrays, `TYPE_INFO_CDR_<T>` and
`TYPE_MAP_CDR_<T>`: the serialized MINIMAL and COMPLETE TypeObjects of the
type and of every type it depends on, each keyed by its TypeIdentifier. They
are Cyclone's own wire form of the case's type, the TypeObjects it sends in a
TypeLookup reply, and the oracle `crates/types/tests/type_object_bridge.rs`
holds this project's TypeObjects to.

The arrays are the same in the four encoding directories of a case; the
script reads `xcdr2-le`. A row per TypeObject of the topic type's mapping:

- `equivalence`: `minimal` or `complete`;
- `role`: `topic` for the topic type's own object (the first identifier of
  that kind in its `TypeInformation`), `dependency` otherwise;
- `hash`: the 14 octets of the TypeIdentifier (`EK_MINIMAL` 0xf1,
  `EK_COMPLETE` 0xf2), which the script checks is `MD5(octets)[0:14]`;
- `octets`: the serialized TypeObject, `DHEADER` included, for the
  `complete` rows; the `minimal` rows carry the hash only.

A case enters only when its IDL and top-level type are the corpus case's
(as in `generate.py`) and Cyclone emitted a mapping for the type; the header
names the cases left out and why.

    tools/expected-bytes/type_objects.py --matrix ../zerodds/matrix
    tools/expected-bytes/type_objects.py --matrix ../zerodds/matrix --check
"""
import argparse
import collections
import hashlib
import re
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from generate import ROOT, git, read_blobs, tree  # noqa: E402

ENCODING_DIR = "xcdr2-le"


def arrays(source):
    """The `TYPE_INFO_CDR_*` and `TYPE_MAP_CDR_*` initialisers of a gen.c."""
    out = {}
    for match in re.finditer(
            r"#define\s+(TYPE_(?:INFO|MAP)_CDR_\w+)\s+\(const unsigned char \[\]\)\{(.*?)\}",
            source, re.S):
        out[match[1]] = bytes(int(x, 16) for x in re.findall(r"0x([0-9a-fA-F]{2})", match[2]))
    return out


def pairs(data, offset):
    """One `TypeIdentifierTypeObjectPairSeq` (XCDR2 little endian) at
    `offset`: its pairs as (discriminator, hash, octets) and the offset after
    it. A pair is the 15-octet TypeIdentifier and the TypeObject, whose
    `DHEADER` is 4-aligned."""
    length = struct.unpack_from("<I", data, offset)[0]
    offset += 4
    end = offset + length
    count = struct.unpack_from("<I", data, offset)[0]
    offset += 4
    out = []
    for _ in range(count):
        discriminator = data[offset]
        if discriminator not in (0xF1, 0xF2):
            raise SystemExit(f"type_objects.py: TypeIdentifier {discriminator:#x} at {offset}")
        digest = data[offset + 1:offset + 15]
        offset = (offset + 15 + 3) & ~3
        size = struct.unpack_from("<I", data, offset)[0]
        octets = data[offset:offset + 4 + size]
        offset += 4 + size
        if hashlib.md5(octets).digest()[:14] != digest:
            raise SystemExit("type_objects.py: a TypeObject whose MD5 is not its identifier")
        out.append((discriminator, digest, octets))
    if offset > end:
        raise SystemExit("type_objects.py: a pair runs past its sequence")
    return out, (end + 3) & ~3


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--matrix", type=Path, required=True,
                        help="a checkout of zerodds-matrix (read only)")
    parser.add_argument("--rev", default="HEAD", help="the matrix commit (default HEAD)")
    parser.add_argument("--corpus", type=Path, default=ROOT / "tests/interop-corpus")
    parser.add_argument("--out", type=Path, default=None,
                        help="default: <corpus>/cyclone-type-objects.tsv")
    parser.add_argument("--check", action="store_true",
                        help="compare with the file instead of writing it")
    args = parser.parse_args()
    out_path = args.out or args.corpus / "cyclone-type-objects.tsv"

    commit = git(args.matrix, "rev-parse", f"{args.rev}^{{commit}}").decode().strip()
    when = git(args.matrix, "log", "-1", "--format=%cI", commit).decode().strip()
    blobs = tree(args.matrix, commit, ["cases", f"cyclone/{ENCODING_DIR}"])
    corpus = sorted(p.name for p in args.corpus.iterdir() if (p / "type.idl").is_file())
    wanted = {}
    for case in corpus:
        for name in ("type.idl", "type"):
            entry = blobs.get(f"cases/{case}/{name}")
            if entry:
                wanted[(case, name)] = entry[0]
        entry = blobs.get(f"cyclone/{ENCODING_DIR}/{case}/src/values/gen.c")
        if entry:
            wanted[(case, "gen.c")] = entry[0]
    texts = read_blobs(args.matrix, wanted.values())

    rows = []
    excluded = collections.defaultdict(list)
    for case in corpus:
        here = args.corpus / case
        idl = wanted.get((case, "type.idl"))
        top = wanted.get((case, "type"))
        if idl is None or top is None:
            excluded["not in the matrix"].append(case)
            continue
        type_name = (here / "type").read_text().strip()
        if (texts[idl].decode() != (here / "type.idl").read_text()
                or texts[top].decode().strip() != type_name):
            excluded["differs from the corpus"].append(case)
            continue
        gen = wanted.get((case, "gen.c"))
        if gen is None:
            excluded["idlc wrote no gen.c"].append(case)
            continue
        found = arrays(texts[gen].decode())
        symbol = type_name.strip(":").replace("::", "_")
        mapping = found.get("TYPE_MAP_CDR_" + symbol)
        information = found.get("TYPE_INFO_CDR_" + symbol)
        if mapping is None or information is None:
            excluded["idlc wrote no TypeInformation for the type"].append(case)
            continue
        minimal, offset = pairs(mapping, 0)
        complete, _ = pairs(mapping, offset)

        def topic(discriminator, objects):
            # The topic type's identifier comes before its dependencies'
            # in its TypeInformation.
            found_at = [(information.find(bytes([discriminator]) + digest), digest)
                        for _, digest, _ in objects]
            return min((at, digest) for at, digest in found_at if at >= 0)[1]

        tops = {topic(0xF1, minimal), topic(0xF2, complete)}
        seen = set()
        for discriminator, digest, octets in minimal + complete:
            if digest in seen:
                continue
            seen.add(digest)
            equivalence = "minimal" if discriminator == 0xF1 else "complete"
            rows.append((case, equivalence, "topic" if digest in tops else "dependency",
                         digest.hex(), octets.hex() if equivalence == "complete" else ""))

    cases = len({row[0] for row in rows})
    lines = [
        "# The TypeObjects CycloneDDS writes for the types of the interop corpus.",
        "# Generated by tools/expected-bytes/type_objects.py; do not edit by hand.",
        f"# matrix-commit: {commit} ({when})",
        f"# source: TYPE_MAP_CDR_<type> and TYPE_INFO_CDR_<type> of cyclone/{ENCODING_DIR}/<case>/"
        "src/values/gen.c (idlc of CycloneDDS, the version in the matrix's PROVENANCE).",
        "# hash: MD5 of the octets, first 14, checked; octets: the serialized TypeObject with its",
        "#   DHEADER, complete rows only.",
        f"# cases: {cases} of {len(corpus)} corpus cases;"
        + "".join(f" {len(v)} {k};" for k, v in sorted(excluded.items())),
    ]
    for reason, names in sorted(excluded.items()):
        lines.append(f"# excluded ({reason}): {' '.join(names)}")
    lines.append("case\tequivalence\trole\thash\toctets")
    lines.extend("\t".join(row) for row in rows)
    text = "\n".join(lines) + "\n"

    if args.check:
        current = out_path.read_text() if out_path.is_file() else ""
        if current != text:
            print(f"{out_path}: not what matrix {commit[:11]} gives; rerun without --check",
                  file=sys.stderr)
            return 1
        print(f"{out_path}: matrix {commit[:11]}, {len(rows)} rows")
        return 0
    out_path.write_text(text)
    print(f"{out_path}: {len(rows)} rows, {cases} cases")
    return 0


if __name__ == "__main__":
    sys.exit(main())

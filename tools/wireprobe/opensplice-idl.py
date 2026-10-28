#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""A corpus IDL file, in the IDL OpenSplice 6.9's `idlpp` reads.

`idlpp` (OpenSplice Community Edition 6.9, OSPL_V6_9_210323OSS_RELEASE)
reads OMG IDL 3.5: every `@annotation` is a parse error ("DDS parse error
appendable undefined"), keys are declared with `#pragma keylist`, and the
IDL 4 integer names (`int32`, `uint8`, ...) do not exist. The product
writes one representation, XCDR1 in the host's byte order, and has no
XTypes at all.

So a corpus type reaches OpenSplice in one of two ways:

* rewritten, when every construct in it means the same bytes under XCDR1
  in OpenSplice's IDL: `@key` becomes `#pragma keylist`, the IDL 4
  integers become their IDL 3.5 names, and the annotations with no effect
  on a `@final` or `@appendable` type's XCDR1 bytes are dropped (member
  ids, nesting, ranges, units, defaults, `@verbatim`, `@appendable`
  itself — XCDR1 has no DHEADER — and `@bit_bound(32)` on an enum, the
  width every OpenSplice enum has);
* refused, with the construct named, when OpenSplice cannot say it:
  `@mutable`, `@optional`, `@external`, `@try_construct`,
  `@must_understand`, an enum's `@bit_bound` below 32 and an
  enumerator's `@value` (both change the XCDR1 bytes: ZeroDDS writes a
  `@bit_bound(8)` enum in one octet and a `@value(1000)` enumerator as
  1000, OpenSplice every enum in four octets as its ordinal), a keyed
  union discriminator, `bitmask`, `bitset`, `map`, and a union as the
  topic type (`#pragma keylist`, and with it the type support, exists
  for structs only). A refusal is exit 3 and one line on stderr, never an
  output that silently lost the construct — `idlpp` itself accepts
  `@optional` and ignores it, which is the shape this script exists to
  rule out.

Usage: opensplice-idl.py <in.idl> <out.idl> [<topic type>]
"""
import re
import sys

# Annotations dropped because they change nothing in a @final/@appendable
# type's XCDR1 bytes.
DROPPED = {
    "final", "appendable", "id", "autoid", "hashid", "nested",
    "default_nested", "topic", "range", "min", "max", "unit", "verbatim",
    "default", "default_literal", "extensibility",
}
# Annotations OpenSplice has no equivalent for: the bytes would differ.
REFUSED = {
    "mutable": "@mutable (PL_CDR1 parameter lists)",
    "optional": "@optional members",
    "external": "@external members",
    "try_construct": "@try_construct",
    "must_understand": "@must_understand",
    "shared": "@shared members",
    "value": "@value (an enumerator's value is its ordinal)",
    "position": "@position",
}
TYPE_WORDS = [
    (r"\buint8\b", "octet"),
    (r"\bint8\b", "octet"),
    (r"\buint16\b", "unsigned short"),
    (r"\bint16\b", "short"),
    (r"\buint32\b", "unsigned long"),
    (r"\bint32\b", "long"),
    (r"\buint64\b", "unsigned long long"),
    (r"\bint64\b", "long long"),
]
ANNOTATION = re.compile(r"@([A-Za-z_][A-Za-z0-9_]*)(\s*\((?:[^()]|\([^()]*\))*\))?")


def refuse(what):
    sys.stderr.write(f"opensplice-idl: OpenSplice 6.9 has no {what}\n")
    sys.exit(3)


def strip_comments(text):
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
    return re.sub(r"//[^\n]*", "", text)


def main():
    if len(sys.argv) not in (3, 4):
        sys.stderr.write(__doc__.split("Usage: ")[1])
        sys.exit(2)
    text = strip_comments(open(sys.argv[1], encoding="utf-8").read())

    for word, what in (("bitmask", "bitmask types"), ("bitset", "bitset types"),
                       ("map", "map<K, V>")):
        if re.search(rf"\b{word}\b\s*[<A-Za-z_]", text):
            refuse(what)
    if re.search(r"\bswitch\s*\(\s*@key", text):
        refuse("keyed union discriminators")
    if len(sys.argv) == 4:
        topic = sys.argv[3].split("::")[-1]
        if re.search(rf"\bunion\s+{re.escape(topic)}\s+switch\b", text):
            refuse("union topic types (`#pragma keylist`, and with it the type support, "
                   "exists for structs only)")

    out = []
    pos = 0
    pending = []  # (struct name, [key members]) waiting for the struct's end
    depth = 0
    struct_depth = None
    struct_name = None
    struct_keys = []
    member_buf = ""
    member_key = False
    token = re.compile(r"@[A-Za-z_]|\bstruct\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?::[^{]*)?\{|\{|\}|;")
    while True:
        m = token.search(text, pos)
        if not m:
            out.append(text[pos:])
            break
        chunk = text[pos:m.start()]
        out.append(chunk)
        if struct_depth is not None:
            member_buf += chunk
        tok = m.group(0)
        if tok.startswith("@"):
            a = ANNOTATION.match(text, m.start())
            name = a.group(1)
            if name in REFUSED:
                refuse(REFUSED[name])
            if name == "bit_bound":
                width = (a.group(2) or "").strip("() \t")
                if width != "32":
                    refuse("@bit_bound below 32 (its enums are 32 bits wide)")
            elif name == "key":
                if struct_depth is None:
                    refuse("@key outside a struct member")
                member_key = True
            elif name not in DROPPED:
                refuse(f"@{name}")
            pos = a.end()
            continue
        if m.group(1):
            if struct_depth is not None:
                refuse("nested struct declarations")
            struct_name = m.group(1)
            struct_depth = depth + 1
            struct_keys = []
            member_buf = ""
            member_key = False
            depth += 1
            out.append(tok)
            pos = m.end()
            continue
        if tok == "{":
            depth += 1
        elif tok == "}":
            if struct_depth is not None and depth == struct_depth:
                pending.append((struct_name, struct_keys))
                struct_depth = None
            depth -= 1
        elif tok == ";":
            if struct_depth is not None and depth == struct_depth:
                if member_key:
                    decl = member_buf.strip()
                    names = [n.strip() for n in re.sub(r"<[^>]*>", "", decl).split(",")]
                    for n in names:
                        mm = re.search(r"([A-Za-z_][A-Za-z0-9_]*)\s*(\[[^\]]*\])*\s*$", n)
                        if not mm:
                            refuse(f"a key member it could not read: `{decl}`")
                        struct_keys.append(mm.group(1))
                member_buf = ""
                member_key = False
            elif pending and struct_depth is None:
                out.append(tok)
                # Every struct, keyed or not: `idlpp` generates the type
                # support (and so the serializer) only for a struct a
                # keylist pragma names, and a pragma with no members is
                # OpenSplice's keyless topic type.
                for name, keys in pending:
                    out.append(f"\n#pragma keylist {' '.join([name] + keys)}\n")
                pending = []
                pos = m.end()
                continue
        out.append(tok)
        pos = m.end()

    result = "".join(out)
    for pattern, word in TYPE_WORDS:
        result = re.sub(pattern, word, result)
    open(sys.argv[2], "w", encoding="utf-8").write(result)


if __name__ == "__main__":
    main()

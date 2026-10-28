#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Lists the names the normative headers of DDS-PSM-Cxx 1.0 declare: for each public header
(outside `detail/`), the namespace and the name of every class, struct, enumeration, typedef
and alias declared at namespace level.

    python3 tools/psm-omg-names.py <omg hpp directory> > crates/cpp/tests/omg_names.txt

One line per name: `<header relative to the include directory> <namespace> <name>`. The test
`every_name_of_the_omg_headers_is_reachable_through_its_header` in
`crates/cpp/tests/public_headers.rs` compiles, for each line, a translation unit that includes
the header and names the identifier.

    python3 tools/psm-omg-names.py --words <omg hpp directory> > crates/cpp/tests/omg_words.txt

Lists every identifier the text of the normative headers contains, one per line, sorted. The test
`no_vendor_name_in_the_normative_namespaces` in `crates/cpp/tests/public_headers.rs` runs the first
form over our headers and requires of every name outside `detail` that the normative headers
contain the identifier or that `vendor_names.txt` lists it with its reason.
"""
import os
import re
import sys


def strip(src):
    src = re.sub(r'/\*.*?\*/', '', src, flags=re.S)
    src = re.sub(r'//[^\n]*', '', src)
    src = re.sub(r'^[ \t]*#[^\n]*(?:\\\n[^\n]*)*', '', src, flags=re.M)
    return src


def names(src):
    out = []
    stack = []  # 'ns:<name>' or 'other'
    i = 0
    n = len(src)
    ns_re = re.compile(r'namespace\s+([A-Za-z_]\w*(?:\s*::\s*[A-Za-z_]\w*)*)?\s*\{')
    while i < n:
        c = src[i]
        if c == '{':
            # what opened it?
            before = src[max(0, i - 300):i + 1]
            m = None
            for m_ in ns_re.finditer(before):
                m = m_
            if m is not None and m.end() == len(before):
                for part in re.split(r'\s*::\s*', m.group(1) or ''):
                    stack.append('ns:' + part)
                # remember how many pushed for the matching close
                stack.append('mark:%d' % len(re.split(r'\s*::\s*', m.group(1) or '')))
            else:
                stack.append('other')
            i += 1
            continue
        if c == '}':
            if stack:
                top = stack.pop()
                if top.startswith('mark:'):
                    for _ in range(int(top[5:])):
                        stack.pop()
            i += 1
            continue
        i += 1
    return out


def declared(src):
    """Walk the text with a namespace stack and report declarations at namespace level."""
    res = []
    stack = []
    ns_open = re.compile(r'namespace\s+((?:[A-Za-z_]\w*\s*::\s*)*[A-Za-z_]\w*)?\s*\{')
    decl = re.compile(
        r'(?:^|[;{}])\s*(?:template\s*<[^;{}]*?>\s*)?'
        r'(?:(?:class|struct|union|enum(?:\s+class)?)\s+([A-Za-z_]\w*)\s*(?::[^;{}]*)?\{'
        r'|typedef\s+[^;{}]*?\b([A-Za-z_]\w*)\s*(?:\[[^\]]*\])?\s*;'
        r'|using\s+([A-Za-z_]\w*)\s*=)',
        re.S)
    i = 0
    n = len(src)
    # tokenise by braces, tracking position
    pos = 0
    chunks = []  # (text before brace, brace char)
    depth_marks = []
    cur_start = 0
    while True:
        m = re.search(r'[{}]', src[pos:])
        if not m:
            chunks.append((src[cur_start:], ''))
            break
        k = pos + m.start()
        chunks.append((src[cur_start:k], src[k]))
        cur_start = k + 1
        pos = k + 1
    ns = []
    marks = []
    for text, brace in chunks:
        at_ns_level = all(not e.startswith('other') for e in marks)
        if at_ns_level:
            # declarations that end in ';' inside the text (typedef/using) and class headers
            for d in re.finditer(r'typedef\s+([^;{}]*?)\b([A-Za-z_]\w*)\s*(?:\[[^\]]*\])?\s*;', text, re.S):
                res.append(('::'.join(ns), d.group(2)))
            for d in re.finditer(r'\busing\s+([A-Za-z_]\w*)\s*=', text):
                res.append(('::'.join(ns), d.group(1)))
        if brace == '{':
            h = re.search(r'namespace\s+((?:[A-Za-z_]\w*\s*::\s*)*[A-Za-z_]\w*)\s*$', text)
            c = re.search(r'\b(?:class|struct|union|enum\s+class|enum)\s+([A-Za-z_]\w*)\s*(?::[^;{}]*)?$', text, re.S)
            if h:
                parts = [p.strip() for p in h.group(1).split('::')]
                ns.extend(parts)
                marks.append('ns%d' % len(parts))
            else:
                if c and at_ns_level and not re.search(r'=\s*$', text):
                    res.append(('::'.join(ns), c.group(1)))
                marks.append('other')
        elif brace == '}':
            if marks:
                top = marks.pop()
                if top.startswith('ns'):
                    del ns[-int(top[2:]):]
    return res


def words(root):
    found = set()
    for dirpath, _, files in os.walk(root):
        for f in files:
            src = strip(open(os.path.join(dirpath, f), errors='ignore').read())
            found.update(re.findall(r'[A-Za-z_]\w*', src))
    for w in sorted(found):
        print(w)


def main():
    if sys.argv[1] == '--words':
        words(sys.argv[2])
        return
    root = sys.argv[1]
    seen = set()
    for dirpath, _, files in sorted(os.walk(root)):
        if '/detail' in dirpath or dirpath.endswith('/detail'):
            continue
        for f in sorted(files):
            if not f.endswith('.hpp'):
                continue
            path = os.path.join(dirpath, f)
            rel = os.path.relpath(path, root)
            src = strip(open(path, errors='ignore').read())
            for ns, name in declared(src):
                if not ns.startswith('dds') or name.startswith('_'):
                    continue
                line = '%s %s %s' % (rel, ns, name)
                if line not in seen:
                    seen.add(line)
                    print(line)


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Checks that every public header of `crates/cpp/include/dds` compiles on its own as C++03.

    python3 tools/psm-cxx03-check.py [--cxx g++] [--std c++03] [--summary] [header ...]

One translation unit per header (`#include <header>`), `-fsyntax-only -pedantic`; with clang the
C++11 extensions it accepts in C++03 mode are errors too. Prints the first lines of the errors
per header, or with `--summary` the count of errors per header. Exit status 1 when a header
fails.
"""
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
INC = os.path.join(ROOT, 'crates/cpp/include')


def headers():
    out = []
    for d, _, fs in os.walk(os.path.join(INC, 'dds')):
        if os.sep + 'detail' + os.sep in d + os.sep:
            continue
        for f in fs:
            if f.endswith('.hpp') and not f.startswith('.'):
                out.append(os.path.relpath(os.path.join(d, f), INC))
    out.append('zerodds/psm/cxx.hpp')
    return sorted(out)


def main():
    args = sys.argv[1:]
    cxx = 'g++'
    std = 'c++03'
    summary = False
    names = []
    i = 0
    while i < len(args):
        if args[i] == '--cxx':
            cxx = args[i + 1]; i += 2
        elif args[i] == '--std':
            std = args[i + 1]; i += 2
        elif args[i] == '--summary':
            summary = True; i += 1
        else:
            names.append(args[i]); i += 1
    hs = names or headers()
    flags = ['-std=' + std, '-fsyntax-only', '-pedantic', '-Wno-long-long', '-I' + INC,
             '-I' + os.path.join(ROOT, 'crates/zerodds-c-api/include'),
             '-I' + os.path.join(ROOT, 'runtime/cpp/include')]
    if 'clang' in cxx or cxx == 'c++' and sys.platform == 'darwin':
        flags += ['-Werror=c++11-extensions', '-Werror=c++11-narrowing', '-Wno-error=c++11-long-long']
    tmp = tempfile.mkdtemp()

    def one(h):
        u = os.path.join(tmp, h.replace('/', '_').replace('.', '_') + '.cpp')
        open(u, 'w').write('#include <%s>\n' % h)
        r = subprocess.run([cxx] + flags + [u], capture_output=True, text=True, errors='replace',
                           env=dict(os.environ, LC_ALL='C'))
        return h, r.returncode, r.stderr

    bad = 0
    with ThreadPoolExecutor(max_workers=8) as ex:
        for h, rc, err in ex.map(one, hs):
            n = sum(1 for l in err.splitlines() if ': error:' in l)
            if rc != 0:
                bad += 1
                if summary:
                    print('%4d %s' % (n, h))
                else:
                    print('== %s (%d errors)' % (h, n))
                    print('\n'.join([l for l in err.splitlines() if ': error:' in l][:6]))
    print('%d of %d headers fail' % (bad, len(hs)))
    sys.exit(1 if bad else 0)


main()

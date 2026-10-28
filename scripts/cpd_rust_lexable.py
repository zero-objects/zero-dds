# SPDX-License-Identifier: Apache-2.0
"""Copies Rust files for PMD CPD in a form its Rust lexer reads.

PMD 7.27/7.28 cannot lex a string literal that ends in an escaped
backslash ("a\\") nor the char literals '\\' and '"': it skips the whole
file. Each char literal is rewritten to the \\u{..} escape of the same
char, and each escaped backslash to \\x5c. Both keep one token per
literal, the token's distinctness and every line number; only what the
lexer cannot read changes.
"""
import os
import re
import sys

ESCAPES = {"n": "\n", "r": "\r", "t": "\t", "0": "\0", "\\": "\\", "'": "'", '"': '"'}
CHAR = re.compile(r"'(\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]{1,6}\}|[nrt0\\'\"])|[^\\'\n])'")


def char_value(body: str) -> int:
    if not body.startswith("\\"):
        return ord(body)
    if body[1] == "x":
        return int(body[2:4], 16)
    if body[1] == "u":
        return int(body[3:-1], 16)
    return ord(ESCAPES[body[1]])


def rewrite(source: str) -> str:
    source = CHAR.sub(lambda m: "'\\u{%x}'" % char_value(m.group(1)), source)
    return source.replace("\\\\", "\\x5c")


def main() -> None:
    src_list, out_dir, out_list = sys.argv[1:4]
    with open(out_list, "w", encoding="utf-8") as out:
        for f in (line.strip() for line in open(src_list, encoding="utf-8")):
            if not f:
                continue
            with open(f, encoding="utf-8") as fh:
                text = rewrite(fh.read())
            dest = os.path.join(out_dir, f)
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            with open(dest, "w", encoding="utf-8") as fh:
                fh.write(text)
            out.write(dest + "\n")


if __name__ == "__main__":
    main()

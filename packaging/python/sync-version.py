#!/usr/bin/env python3
"""Keep the two Python distributions aligned with the Cargo release version."""
import argparse
import pathlib
import re


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    root = pathlib.Path(__file__).resolve().parents[2]
    cargo = (root / "Cargo.toml").read_text()
    section = re.search(r"(?ms)^\[workspace\.package\]\s*\n(.*?)(?=^\[|\Z)", cargo)
    if section is None:
        raise ValueError("workspace.package missing")
    version = re.search(r'^version\s*=\s*"([^"]+)"', section[1], re.M)
    if version is None:
        raise ValueError("workspace package version missing")
    match = re.fullmatch(r"(\d+\.\d+\.\d+)(?:-(alpha|beta|rc)\.(\d+))?", version[1])
    if match is None:
        raise ValueError("Cargo version has no supported unambiguous PEP 440 spelling")
    suffix = {None: "", "alpha": "a", "beta": "b", "rc": "rc"}[match[2]]
    pep440 = match[1] + suffix + (match[3] or "")
    replacements = [
        ("runtime/python/pyproject.toml", r'^version = "[^"]+"$', f'version = "{pep440}"'),
        ("crates/py/pyproject.toml", r'^dependencies = \["zerodds-wire==[^"]+"\]$',
         f'dependencies = ["zerodds-wire=={pep440}"]'),
    ]
    for relative, pattern, replacement in replacements:
        path = root / relative
        old = path.read_text()
        new, count = re.subn(pattern, lambda _: replacement, old, flags=re.M)
        if count != 1:
            raise ValueError(f"expected exactly one version field in {relative}")
        if args.check and old != new:
            raise ValueError(f"version drift in {relative}: expected {pep440}")
        if not args.check and old != new:
            path.write_text(new)
    print(f"Python distributions aligned: {pep440}")


if __name__ == "__main__":
    main()

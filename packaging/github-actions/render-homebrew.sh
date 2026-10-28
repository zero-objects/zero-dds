#!/usr/bin/env bash
# packaging/github-actions/render-homebrew.sh
# Substitutes the artefact SHA-256 placeholders in the Homebrew formulae and
# copies them into the homebrew-tap checkout. The bridge/ROS-2 sub-formulae
# carry no url and no sha256 (they are pure service formulae over the binary
# the meta formula installs), so only a template that names an artifact is
# resolved against the distribution directory. Called by release.yml ->
# publish-homebrew-tap and by the `macos-bundle` job (ci/full.yml).
# Spec: zerodds-deployment-1.0.md §3.2.1.
set -euo pipefail
TAP_DIR="${1:?tap-dir required}"
DIST="${DIST_DIR:-target/distrib}"
# Validate every referenced artifact before touching the tap. Cargo-dist's
# per-binary archives are not interchangeable with these bundle URLs.
python3 - "$TAP_DIR" "$DIST" <<'PY'
import hashlib
import os
from pathlib import Path
import re
import sys
from urllib.parse import urlsplit

destination, distribution = map(Path, sys.argv[1:])
rendered = []
templates = sorted(Path("packaging/macos/homebrew").glob("*.rb"))
if not templates:
    raise SystemExit("No Homebrew formula templates found")
for template in templates:
    source = template.read_text()
    version = re.search(r'^\s*version "([^"]+)"$', source, re.MULTILINE)
    if not version:
        raise SystemExit(f"Missing formula version: {template}")
    if os.environ.get("RELEASE_VERSION", version[1]) != version[1]:
        raise SystemExit(f"Formula version does not match selected release: {template}")
    artifact = None
    hashes = 0
    has_url = False
    lines = []
    for line in source.splitlines(keepends=True):
        url = re.match(r'\s*url "([^"]+)"\s*$', line)
        if url:
            has_url = True
            name = Path(urlsplit(url[1].replace("#{version}", version[1])).path).name
            matches = sorted(distribution.rglob(name)) if distribution.is_dir() else []
            matches = [path for path in matches if path.is_file()]
            if len(matches) != 1 or matches[0].stat().st_size == 0:
                raise SystemExit(f"Need exactly one nonempty release artifact for {template}: {name}")
            artifact = matches[0]
        checksum = re.match(r'(\s*sha256 )"[0-9a-fA-F]{64}"(\s*)$', line)
        if checksum:
            if artifact is None:
                raise SystemExit(f"Checksum without its artifact URL: {template}")
            digest = hashlib.sha256()
            with artifact.open("rb") as stream:
                for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                    digest.update(chunk)
            line = f'{checksum[1]}"{digest.hexdigest()}"{checksum[2]}'
            artifact = None
            hashes += 1
        lines.append(line)
    if artifact is not None:
        raise SystemExit(f"URL without its checksum: {template}")
    # A template that names an artifact must have at least one complete
    # URL/sha256 pair; a pure service formula (no url at all) must have none.
    if has_url and not hashes:
        raise SystemExit(f"Incomplete URL/checksum pairs: {template}")
    if not has_url and hashes:
        raise SystemExit(f"Checksum without its artifact URL: {template}")
    rendered.append((template.name, "".join(lines)))
destination.mkdir(parents=True, exist_ok=True)
for name, content in rendered:
    (destination / name).write_text(content)
print(f"Rendered {len(rendered)} formulae from actual matching artifacts into {destination}")
PY

#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Measure into fresh artifacts. Never let old committed cells mask a failed run.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$HERE/verify_record.py" "$@"

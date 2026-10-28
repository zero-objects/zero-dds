#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# optional-bits — a third byte-gate case set: what a vendor leaves in the
# padding, and what it writes for an `@optional` member of a keyed type.
#
#   tools/optional-bits/gate.sh --cases pad2
#
# It takes every option `tools/byte-gate/gate.sh` takes and runs over
# `cases/` beside this file.
#
# The five cases are `pad0`–`pad3`, which vary how much padding a type
# has and where, and `mut_key`. They exist because two of the three
# vendors leave *uninitialised process memory* in alignment padding —
# so a byte comparison that treats padding as significant reports a
# difference that changes between two runs of one writer. The cases
# pin down exactly which offsets are affected, which is what let
# `vendor_bytes.md` say "RTI and Fast DDS leave whatever was there"
# instead of recording one run's accident as a vendor's answer.
#
# ## What this replaces
#
# `/root/opt-bits/gate` on the Linux test host. `wire/` beside it is the
# live pub/sub half of the same question and has its own README.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$HERE/../byte-gate/gate.sh" --corpus "$HERE/cases" "$@"

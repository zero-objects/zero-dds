#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# wire-oracle — the second byte gate.
#
#   tools/wire-oracle/gate.sh --cases map_val_struct
#   tools/wire-oracle/gate.sh --vendors opendds --endians le,be
#
# It takes every option `tools/byte-gate/gate.sh` takes and runs the
# same drivers and templates over `cases/` beside this file instead.
#
# ## Why there are two case sets and not one
#
# The separation is deliberate and predates this port:
# `/root/wire-oracle/gate` held a second copy of the byte gate so that a
# case added for OpenDDS, omniORB or JacORB could not disturb a run of
# the first three vendors. That property is worth keeping — the first
# set fixes rules the layout planner follows and is re-run against every
# change, the second reaches constructs only one or two implementations
# can express at all.
#
# The seventeen cases here are the `map` family, the `bitset` family,
# `fixed<P,S>`, and the length-code cases that needed a fourth
# implementation to settle. Eight of them carry a `fill.odds.inc` and
# are the whole of OpenDDS's column.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$HERE/../byte-gate/gate.sh" --corpus "$HERE/cases" "$@"

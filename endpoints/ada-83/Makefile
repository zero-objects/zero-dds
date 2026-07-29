# SPDX-License-Identifier: Apache-2.0
# pre-Object Ada 83 endpoint SDK (ADR 0013) — strict -gnat83, oldest legacy.
#
#   cargo run -p zerodds-endpoint-golden -- endpoints/ada-83/build   # goldens
#   make test

GPRBUILD ?= gprbuild
GPRCLEAN ?= gprclean

.PHONY: all test clean

all:
	$(GPRBUILD) -p -P zerodds_ada83.gpr

# The identity test reads build/golden_{le,be}.bin (Ada 83 has no Command_Line),
# so it must run from this directory with the goldens in build/.
test: all
	build/test_ada83_identity
	build/test_ada83_async

clean:
	-$(GPRCLEAN) -P zerodds_ada83.gpr
	rm -rf build

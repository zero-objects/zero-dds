# SPDX-License-Identifier: Apache-2.0
# ZeroDDS pure-Ada endpoint SDK (ADR 0013) Stage 2 -- no C, byte-identical.
#
# Generate the goldens first (from the workspace root):
#   cargo run -p zerodds-endpoint-golden -- endpoints/ada-native/build
# then:
#   make test GOLDEN_DIR=build

GPRBUILD   ?= gprbuild
GPRCLEAN   ?= gprclean
GOLDEN_DIR ?= build

.PHONY: all test clean

all:
	$(GPRBUILD) -p -P zerodds_ada_native.gpr

test: all
	build/test_native_identity $(GOLDEN_DIR)
	build/test_native_roundtrip
	build/test_native_framing $(GOLDEN_DIR)
	build/test_native_reflect $(GOLDEN_DIR)
	build/test_oop_endpoint
	build/test_native_reliable_unit $(GOLDEN_DIR)

clean:
	-$(GPRCLEAN) -P zerodds_ada_native.gpr
	rm -rf build

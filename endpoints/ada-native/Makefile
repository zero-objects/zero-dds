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
#	The range-edge test was built by the project file and run by
#	nothing: `for Main` lists test_native_surface.adb, gprbuild
#	produced the binary on every `make all`, and this target never
#	named it. Its siblings under endpoints/ada and endpoints/ada-83
#	do run theirs, so the gap was this one line.
	build/test_native_surface
	build/test_native_roundtrip
	build/test_native_framing $(GOLDEN_DIR)
	build/test_native_reflect $(GOLDEN_DIR)
	build/test_oop_endpoint
	build/test_native_reliable_unit $(GOLDEN_DIR)
	build/test_native_surface

clean:
	-$(GPRCLEAN) -P zerodds_ada_native.gpr
	rm -rf build

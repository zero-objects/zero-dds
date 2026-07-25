# SPDX-License-Identifier: Apache-2.0
# ZeroDDS native Ada endpoint SDK (ADR 0013) Stage 1 -- bindings over the C89
# wire-core, byte-identical to the Rust core.
#
# Generate the goldens first (from the workspace root):
#   cargo run -p zerodds-endpoint-golden -- endpoints/ada/build
# then:
#   make test GOLDEN_DIR=build

GPRBUILD   ?= gprbuild
GPRCLEAN   ?= gprclean
GOLDEN_DIR ?= build

.PHONY: all test clean

all:
	$(GPRBUILD) -p -P zerodds_ada.gpr

test: all
	build/test_byte_identity $(GOLDEN_DIR)/golden_le.bin $(GOLDEN_DIR)/golden_be.bin
	build/test_udp_loopback

clean:
	-$(GPRCLEAN) -P zerodds_ada.gpr
	rm -rf build

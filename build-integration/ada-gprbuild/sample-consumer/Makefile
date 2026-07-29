# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# GNAT project files (.gpr) have NO native pre-build-command hook — unlike
# Cargo (build.rs), CMake (add_custom_command), SPM (BuildToolPlugin),
# Gradle (Task graph), dub (preGenerateCommands) or Mix
# (Mix.Task.Compiler), gprbuild's project-file language has no primitive
# for "run this external command before compiling, and make its output
# an input". This Makefile is the documented, LOUDLY-FLAGGED workaround:
# a thin wrapper that runs `zerodds-idlc` then `gprbuild`, with `make`'s
# own file-mtime rule (not gprbuild's) providing the incremental-rebuild
# guarantee. See ../README.md for what a "real" integration would need
# (a GNAT Ada-language server plugin, or upstream gprbuild feature work
# — both out of ZeroDDS's control).

IDL          := idl/Robot.idl
GEN_DIR      := generated
GEN_SPEC     := $(GEN_DIR)/robot.ads
ZERODDS_IDLC ?= zerodds-idlc
GPR          := robot_demo.gpr

.PHONY: all generate build run clean

all: build

# make's own mtime rule: only re-invoke zerodds-idlc when idl/Robot.idl is
# newer than the last-generated robot.ads.
generate: $(GEN_SPEC)

$(GEN_SPEC): $(IDL)
	mkdir -p $(GEN_DIR)
	$(ZERODDS_IDLC) generate $(IDL) --ada -o $(GEN_DIR)

build: generate
	gprbuild -P $(GPR)

run: build
	./obj/main

clean:
	-gprclean -P $(GPR) -q
	rm -rf $(GEN_DIR) obj

# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The C++ wire runtime. No external dependencies and no build system beyond
# this file: what an embedded target has to link is four translation units.
#
#   make            build the objects and the two test binaries
#   make test       build and run the self-test
#   make corpus     build and run the interop-corpus byte comparison
#   make check      both, which is what CI runs
#   make clean
#
# The warning set is the gate, not a suggestion. `-Wconversion` and
# `-Wsign-conversion` are in it because every silent narrowing this runtime
# could commit is a byte on the wire that nobody asked for, and `-Werror`
# because a warning nobody reads is not a warning.
#
#   make CXX=g++-16 test
#   make CXX=clang++ test

CXX      ?= c++
CXXSTD   ?= -std=c++17
OPT      ?= -O1
WARNINGS := -Wall -Wextra -Wpedantic -Wconversion -Wsign-conversion -Werror
CXXFLAGS ?= $(CXXSTD) $(OPT) $(WARNINGS)
INCLUDES := -Iinclude

BUILD   := build
SOURCES := src/writer.cpp src/reader.cpp src/float128.cpp src/md5.cpp
OBJECTS := $(patsubst src/%.cpp,$(BUILD)/%.o,$(SOURCES))
LIB     := $(BUILD)/libzerodds_wire.a

# The corpus the byte comparison reads. Overridable so the check can be run
# against a checkout somewhere else.
CORPUS_ROOT ?= ../../tests

.PHONY: all test corpus check clean

all: $(LIB) $(BUILD)/selftest $(BUILD)/corpus_bytes

$(BUILD):
	mkdir -p $(BUILD)

$(BUILD)/%.o: src/%.cpp include/zerodds/wire.hpp include/zerodds/map.hpp | $(BUILD)
	$(CXX) $(CXXFLAGS) $(INCLUDES) -c $< -o $@

$(LIB): $(OBJECTS)
	$(AR) rcs $@ $(OBJECTS)

$(BUILD)/selftest: tests/selftest.cpp $(LIB) include/zerodds/wire.hpp include/zerodds/map.hpp include/dds/types.hpp include/dds/return_code.hpp | $(BUILD)
	$(CXX) $(CXXFLAGS) $(INCLUDES) $< $(LIB) -o $@

$(BUILD)/corpus_bytes: tests/corpus_bytes.cpp $(LIB) include/zerodds/wire.hpp | $(BUILD)
	$(CXX) $(CXXFLAGS) $(INCLUDES) $< $(LIB) -o $@

# Silent and exit 0 is the pass. A self-test that printed a summary would
# make a failure something a reader has to look for.
test: $(BUILD)/selftest
	@$(BUILD)/selftest

corpus: $(BUILD)/corpus_bytes
	@$(BUILD)/corpus_bytes $(CORPUS_ROOT)

check: test corpus

clean:
	rm -rf $(BUILD)

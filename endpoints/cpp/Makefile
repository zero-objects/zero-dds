# SPDX-License-Identifier: Apache-2.0
# ZeroDDS native C++ endpoint SDK (ADR 0013). The conservative facade is C++98;
# the async reactor add-on (zerodds_async.hpp) is C++17. Both are thin facades
# over the C89 wire-core (../c) — byte-identical, no re-implementation.
#
#   cargo run -p zerodds-endpoint-golden -- <dir>   # generate goldens first
#   make test GOLDEN_DIR=<dir>

CXX        ?= c++
CXXFLAGS98 ?= -std=c++98 -pedantic -Wall -Wextra -O2
CXXFLAGS17 ?= -std=c++17 -pedantic -Wall -Wextra -O2
INCLUDE     = -Iinclude -I../c/include
BUILD       = build
GOLDEN_DIR ?= .
RUNNER     ?=

CSRC_WIRE   = ../c/src/zerodds_wire.c
CSRC_EP     = ../c/src/zerodds_endpoint.c
CSRC_REFL   = ../c/src/zerodds_reflect.c
CSRC_ASYNC  = ../c/src/zerodds_async.c

.PHONY: all test examples clean

all: $(BUILD)/test_byte_identity $(BUILD)/test_endpoint $(BUILD)/test_mutable \
     $(BUILD)/test_nested $(BUILD)/test_reflect \
     $(BUILD)/test_async_loopback $(BUILD)/test_async_udp \
     $(BUILD)/test_reliable_async_overflow

# --- deep examples (sync: C++98 poll; async: C++17 reactor) ---
$(BUILD)/example_sync: example_sync.cpp include/zerodds_wire.hpp
	@mkdir -p $(BUILD)
	$(CXX) $(CXXFLAGS98) $(INCLUDE) example_sync.cpp $(CSRC_WIRE) $(CSRC_EP) -o $@

$(BUILD)/example_async: example_async.cpp include/zerodds_async.hpp include/zerodds_wire.hpp
	@mkdir -p $(BUILD)
	$(CXX) $(CXXFLAGS17) $(INCLUDE) example_async.cpp $(CSRC_WIRE) $(CSRC_EP) $(CSRC_ASYNC) -o $@

examples: $(BUILD)/example_sync $(BUILD)/example_async
	$(RUNNER) $(BUILD)/example_sync
	$(RUNNER) $(BUILD)/example_async

# --- conservative C++98 facade tests ---
$(BUILD)/test_byte_identity: test/test_byte_identity.cpp include/zerodds_wire.hpp
	@mkdir -p $(BUILD)
	$(CXX) $(CXXFLAGS98) $(INCLUDE) test/test_byte_identity.cpp $(CSRC_WIRE) -o $@

$(BUILD)/test_endpoint: test/test_endpoint.cpp include/zerodds_wire.hpp
	@mkdir -p $(BUILD)
	$(CXX) $(CXXFLAGS98) $(INCLUDE) test/test_endpoint.cpp $(CSRC_WIRE) $(CSRC_EP) -o $@

$(BUILD)/test_mutable: test/test_mutable.cpp include/zerodds_wire.hpp
	@mkdir -p $(BUILD)
	$(CXX) $(CXXFLAGS98) $(INCLUDE) test/test_mutable.cpp $(CSRC_WIRE) -o $@

$(BUILD)/test_nested: test/test_nested.cpp include/zerodds_wire.hpp
	@mkdir -p $(BUILD)
	$(CXX) $(CXXFLAGS98) $(INCLUDE) test/test_nested.cpp $(CSRC_WIRE) -o $@

$(BUILD)/test_reflect: test/test_reflect.cpp include/zerodds_wire.hpp
	@mkdir -p $(BUILD)
	$(CXX) $(CXXFLAGS98) $(INCLUDE) test/test_reflect.cpp $(CSRC_WIRE) $(CSRC_REFL) -o $@

# --- modern C++17 async reactor add-on ---
$(BUILD)/test_async_loopback: test/test_async_loopback.cpp include/zerodds_async.hpp include/zerodds_wire.hpp
	@mkdir -p $(BUILD)
	$(CXX) $(CXXFLAGS17) $(INCLUDE) test/test_async_loopback.cpp $(CSRC_WIRE) $(CSRC_EP) $(CSRC_ASYNC) -o $@

$(BUILD)/test_async_udp: test/test_async_udp.cpp include/zerodds_async.hpp include/zerodds_wire.hpp
	@mkdir -p $(BUILD)
	$(CXX) $(CXXFLAGS17) $(INCLUDE) test/test_async_udp.cpp $(CSRC_WIRE) $(CSRC_EP) $(CSRC_ASYNC) -o $@

# Reliable async writer regression test (C++17 + pthread).
$(BUILD)/test_reliable_async_overflow: test/test_reliable_async_overflow.cpp include/zerodds_reliable.hpp
	@mkdir -p $(BUILD)
	$(CXX) $(CXXFLAGS17) $(INCLUDE) test/test_reliable_async_overflow.cpp -pthread -o $@

test: all
	$(RUNNER) $(BUILD)/test_byte_identity $(GOLDEN_DIR)/golden_le.bin $(GOLDEN_DIR)/golden_be.bin
	$(RUNNER) $(BUILD)/test_endpoint $(GOLDEN_DIR)/golden_xrce_le.bin $(GOLDEN_DIR)/golden_serial_le.bin
	$(RUNNER) $(BUILD)/test_mutable $(GOLDEN_DIR)/golden_mutable_le.bin $(GOLDEN_DIR)/golden_mutable_be.bin
	$(RUNNER) $(BUILD)/test_nested $(GOLDEN_DIR)/golden_nested_le.bin $(GOLDEN_DIR)/golden_nested_be.bin
	$(RUNNER) $(BUILD)/test_reflect $(GOLDEN_DIR)/golden_mutable_le.bin $(GOLDEN_DIR)/golden_mutable_be.bin
	$(RUNNER) $(BUILD)/test_async_loopback
	$(RUNNER) $(BUILD)/test_async_udp
	$(RUNNER) $(BUILD)/test_reliable_async_overflow

clean:
	rm -rf $(BUILD)

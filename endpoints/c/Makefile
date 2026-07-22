# SPDX-License-Identifier: Apache-2.0
# ZeroDDS native C endpoint SDK (ADR 0013) — wire-core + byte-identity test.
#
# Conservative C89, no external libs. Override CC/CFLAGS to cross-compile:
#   make CC=powerpc-linux-gnu-gcc            # big-endian target
#
# Run the byte-identity test against Rust-generated goldens:
#   make test GOLDEN_DIR=/path/to/goldens
# (generate goldens first: cargo run -p zerodds-endpoint-golden -- <dir>)

CC      ?= cc
CFLAGS  ?= -std=c89 -pedantic -Wall -Wextra -O2
INCLUDE  = -Iinclude
BUILD    = build
GOLDEN_DIR ?= .

SRC_FINAL   = src/zerodds_wire.c test/sample_sensor.c test/test_byte_identity.c
SRC_NESTED  = src/zerodds_wire.c test/sample_nested.c test/test_nested.c
SRC_MUTABLE = src/zerodds_wire.c test/sample_mutable.c test/test_mutable.c
SRC_LOOP    = src/zerodds_wire.c src/zerodds_endpoint.c test/sample_sensor.c test/test_endpoint_loopback.c
SRC_XRCE    = src/zerodds_wire.c src/zerodds_endpoint.c test/sample_sensor.c test/test_xrce_frame.c
SRC_SERIAL  = src/zerodds_wire.c src/zerodds_endpoint.c test/sample_sensor.c test/test_serial_frame.c
SRC_RECV    = src/zerodds_wire.c src/zerodds_endpoint.c test/sample_sensor.c test/test_receive.c
SRC_REL     = src/zerodds_endpoint.c test/test_reliable.c
SRC_REFL    = src/zerodds_wire.c src/zerodds_reflect.c test/test_reflect.c
SRC_REFN    = src/zerodds_wire.c src/zerodds_reflect.c test/test_reflect_nested.c
BIN_FINAL   = $(BUILD)/test_byte_identity
BIN_NESTED  = $(BUILD)/test_nested
BIN_MUTABLE = $(BUILD)/test_mutable
BIN_LOOP    = $(BUILD)/test_endpoint_loopback
BIN_XRCE    = $(BUILD)/test_xrce_frame
BIN_SERIAL  = $(BUILD)/test_serial_frame
BIN_RECV    = $(BUILD)/test_receive
BIN_REL     = $(BUILD)/test_reliable
BIN_REFL    = $(BUILD)/test_reflect
BIN_REFN    = $(BUILD)/test_reflect_nested

.PHONY: all test clean

all: $(BIN_FINAL) $(BIN_NESTED) $(BIN_MUTABLE) $(BIN_LOOP) $(BIN_XRCE) $(BIN_SERIAL) $(BIN_RECV) $(BIN_REL) $(BIN_REFL) $(BIN_REFN)

$(BIN_LOOP): $(SRC_LOOP) include/zerodds_wire.h include/zerodds_endpoint.h test/sample_sensor.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS) $(INCLUDE) $(SRC_LOOP) -o $(BIN_LOOP)

$(BIN_XRCE): $(SRC_XRCE) include/zerodds_wire.h include/zerodds_endpoint.h test/sample_sensor.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS) $(INCLUDE) $(SRC_XRCE) -o $(BIN_XRCE)

$(BIN_SERIAL): $(SRC_SERIAL) include/zerodds_wire.h include/zerodds_endpoint.h test/sample_sensor.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS) $(INCLUDE) $(SRC_SERIAL) -o $(BIN_SERIAL)

$(BIN_RECV): $(SRC_RECV) include/zerodds_wire.h include/zerodds_endpoint.h test/sample_sensor.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS) $(INCLUDE) $(SRC_RECV) -o $(BIN_RECV)

$(BIN_REL): $(SRC_REL) include/zerodds_endpoint.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS) $(INCLUDE) $(SRC_REL) -o $(BIN_REL)

$(BIN_REFL): $(SRC_REFL) include/zerodds_wire.h include/zerodds_reflect.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS) $(INCLUDE) $(SRC_REFL) -o $(BIN_REFL)

$(BIN_REFN): $(SRC_REFN) include/zerodds_reflect.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS) $(INCLUDE) $(SRC_REFN) -o $(BIN_REFN)

$(BIN_FINAL): $(SRC_FINAL) include/zerodds_wire.h test/sample_sensor.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS) $(INCLUDE) $(SRC_FINAL) -o $(BIN_FINAL)

$(BIN_NESTED): $(SRC_NESTED) include/zerodds_wire.h test/sample_nested.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS) $(INCLUDE) $(SRC_NESTED) -o $(BIN_NESTED)

$(BIN_MUTABLE): $(SRC_MUTABLE) include/zerodds_wire.h test/sample_mutable.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS) $(INCLUDE) $(SRC_MUTABLE) -o $(BIN_MUTABLE)

test: all
	$(RUNNER) $(BIN_FINAL) $(GOLDEN_DIR)/golden_le.bin $(GOLDEN_DIR)/golden_be.bin
	$(RUNNER) $(BIN_NESTED) $(GOLDEN_DIR)/golden_nested_le.bin $(GOLDEN_DIR)/golden_nested_be.bin
	$(RUNNER) $(BIN_MUTABLE) $(GOLDEN_DIR)/golden_mutable_le.bin $(GOLDEN_DIR)/golden_mutable_be.bin
	$(RUNNER) $(BIN_LOOP)
	$(RUNNER) $(BIN_XRCE) $(GOLDEN_DIR)/golden_xrce_le.bin
	$(RUNNER) $(BIN_SERIAL) $(GOLDEN_DIR)/golden_serial_le.bin
	$(RUNNER) $(BIN_RECV) $(GOLDEN_DIR)/golden_data_le.bin
	$(RUNNER) $(BIN_REL) $(GOLDEN_DIR)/golden_heartbeat_le.bin $(GOLDEN_DIR)/golden_acknack_le.bin
	$(RUNNER) $(BIN_REFL) $(GOLDEN_DIR)/golden_le.bin $(GOLDEN_DIR)/golden_be.bin $(GOLDEN_DIR)/golden_mutable_le.bin $(GOLDEN_DIR)/golden_mutable_be.bin
	$(RUNNER) $(BIN_REFN) $(GOLDEN_DIR)/golden_nested_le.bin $(GOLDEN_DIR)/golden_nested_be.bin

clean:
	rm -rf $(BUILD)

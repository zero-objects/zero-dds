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
SRC_REL_SM  = src/zerodds_endpoint.c test/test_reliable_sm.c
SRC_EX_REL  = src/zerodds_endpoint.c examples/example_reliable.c
SRC_REFL    = src/zerodds_wire.c src/zerodds_reflect.c test/test_reflect.c
SRC_REFN    = src/zerodds_wire.c src/zerodds_reflect.c test/test_reflect_nested.c
# Modern (C11) async reactor add-on — additive, compiled with -std=c11.
SRC_ASYNC   = src/zerodds_wire.c src/zerodds_endpoint.c src/zerodds_async.c test/sample_sensor.c test/test_async_loopback.c
SRC_ASYNC_UDP = src/zerodds_wire.c src/zerodds_endpoint.c src/zerodds_async.c test/sample_sensor.c test/test_async_udp.c
# Async-decoupled reliable writer + latency bench (C11 + pthread).
SRC_BENCH_REL = src/zerodds_endpoint.c src/zerodds_reliable_async.c test/bench_reliable_async.c
# Async reliable writer window-overflow + no-self-ack test (C11 + pthread).
SRC_REL_OVF = src/zerodds_endpoint.c src/zerodds_reliable_async.c test/test_reliable_async_overflow.c
CFLAGS11    = -std=c11 -pedantic -Wall -Wextra -O2
BIN_ASYNC   = $(BUILD)/test_async_loopback
BIN_ASYNC_UDP = $(BUILD)/test_async_udp
BIN_REL_SM  = $(BUILD)/test_reliable_sm
BIN_EX_REL  = $(BUILD)/example_reliable
BIN_BENCH_REL = $(BUILD)/bench_reliable_async
BIN_REL_OVF = $(BUILD)/test_reliable_async_overflow
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

.PHONY: all test examples clean

# Deep examples (sync: C89 poll loop; async: C11 reactor callback).
SRC_EX_SYNC  = src/zerodds_wire.c src/zerodds_endpoint.c examples/example_sync.c
SRC_EX_ASYNC = src/zerodds_wire.c src/zerodds_endpoint.c src/zerodds_async.c examples/example_async.c
BIN_EX_SYNC  = $(BUILD)/example_sync
BIN_EX_ASYNC = $(BUILD)/example_async

all: $(BIN_FINAL) $(BIN_NESTED) $(BIN_MUTABLE) $(BIN_LOOP) $(BIN_XRCE) $(BIN_SERIAL) $(BIN_RECV) $(BIN_REL) $(BIN_REL_SM) $(BIN_EX_REL) $(BIN_BENCH_REL) $(BIN_REL_OVF) $(BIN_REFL) $(BIN_REFN) $(BIN_ASYNC) $(BIN_ASYNC_UDP)

$(BIN_EX_SYNC): $(SRC_EX_SYNC) include/zerodds_wire.h include/zerodds_endpoint.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS) $(INCLUDE) $(SRC_EX_SYNC) -o $(BIN_EX_SYNC)

$(BIN_EX_ASYNC): $(SRC_EX_ASYNC) include/zerodds_async.h include/zerodds_endpoint.h include/zerodds_wire.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS11) $(INCLUDE) $(SRC_EX_ASYNC) -o $(BIN_EX_ASYNC)

examples: $(BIN_EX_SYNC) $(BIN_EX_ASYNC) $(BIN_EX_REL)
	$(RUNNER) $(BIN_EX_SYNC)
	$(RUNNER) $(BIN_EX_ASYNC)
	$(RUNNER) $(BIN_EX_REL)

$(BIN_ASYNC): $(SRC_ASYNC) include/zerodds_async.h include/zerodds_endpoint.h include/zerodds_wire.h test/sample_sensor.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS11) $(INCLUDE) $(SRC_ASYNC) -o $(BIN_ASYNC)

$(BIN_ASYNC_UDP): $(SRC_ASYNC_UDP) include/zerodds_async.h include/zerodds_endpoint.h include/zerodds_wire.h test/sample_sensor.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS11) $(INCLUDE) $(SRC_ASYNC_UDP) -o $(BIN_ASYNC_UDP)

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

$(BIN_REL_SM): $(SRC_REL_SM) include/zerodds_endpoint.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS) $(INCLUDE) $(SRC_REL_SM) -o $(BIN_REL_SM)

$(BIN_EX_REL): $(SRC_EX_REL) include/zerodds_endpoint.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS) $(INCLUDE) $(SRC_EX_REL) -o $(BIN_EX_REL)

$(BIN_BENCH_REL): $(SRC_BENCH_REL) include/zerodds_endpoint.h include/zerodds_reliable_async.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS11) $(INCLUDE) $(SRC_BENCH_REL) -o $(BIN_BENCH_REL) -lpthread

$(BIN_REL_OVF): $(SRC_REL_OVF) include/zerodds_endpoint.h include/zerodds_reliable_async.h
	@mkdir -p $(BUILD)
	$(CC) $(CFLAGS11) $(INCLUDE) $(SRC_REL_OVF) -o $(BIN_REL_OVF) -lpthread

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
	$(RUNNER) $(BIN_REL_SM) $(GOLDEN_DIR)/golden_heartbeat_le.bin $(GOLDEN_DIR)/golden_acknack_le.bin
	$(RUNNER) $(BIN_EX_REL)
	$(RUNNER) $(BIN_BENCH_REL)
	$(RUNNER) $(BIN_REL_OVF)
	$(RUNNER) $(BIN_REFL) $(GOLDEN_DIR)/golden_le.bin $(GOLDEN_DIR)/golden_be.bin $(GOLDEN_DIR)/golden_mutable_le.bin $(GOLDEN_DIR)/golden_mutable_be.bin
	$(RUNNER) $(BIN_REFN) $(GOLDEN_DIR)/golden_nested_le.bin $(GOLDEN_DIR)/golden_nested_be.bin
	$(RUNNER) $(BIN_ASYNC)
	$(RUNNER) $(BIN_ASYNC_UDP)

clean:
	rm -rf $(BUILD)

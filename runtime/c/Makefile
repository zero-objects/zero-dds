# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The C wire runtime and its self-test. No configure step and no
# dependency: a target that can run `cc` can build this.
#
#   make            the static library
#   make check      the self-test
#   make clean

CC      ?= cc
CFLAGS  ?= -std=c99 -O2 -Wall -Wextra -Wpedantic -Wconversion -Wsign-conversion
AR      ?= ar
BUILD   ?= build

SOURCES := src/writer.c src/reader.c src/md5.c src/float128.c src/float128_decimal.c
OBJECTS := $(SOURCES:src/%.c=$(BUILD)/%.o)
LIB     := $(BUILD)/libzerodds_wire.a

.PHONY: all check clean

all: $(LIB)

$(LIB): $(OBJECTS)
	$(AR) rcs $@ $^

$(BUILD)/%.o: src/%.c include/zerodds_wire.h | $(BUILD)
	$(CC) $(CFLAGS) -Iinclude -c $< -o $@

$(BUILD):
	mkdir -p $(BUILD)

check: $(BUILD)/selftest
	$(BUILD)/selftest

$(BUILD)/selftest: tests/selftest.c $(LIB)
	$(CC) $(CFLAGS) -Iinclude $< $(LIB) -lm -o $@

clean:
	rm -rf $(BUILD)

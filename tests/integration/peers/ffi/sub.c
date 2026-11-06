// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The C subscriber of tests/integration/tests/bindings_ffi.rs: the C ABI of
// zerodds.h itself (zerodds-ffi-loader-1.0 §2.3).
//
//   sub_c <domain> <topic> <type name> <samples>
//
// Prints `EVENT matched` once a publisher matched, `EVENT sample <hex>` for
// every body it takes, `EVENT received n=<count>` when it has the samples or
// 10 s passed, takes once more after the scenario closed its standard input
// (a sample taken then was delivered twice) and ends with `EVENT done`.

#include <zerodds.h>

#include <stdio.h>
#include <stdlib.h>
#include <time.h>

static double seconds_now(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
}

// The reader has no blocking take; between two empty takes the program
// waits 5 ms.
static void backoff(void) {
    struct timespec ts = {0, 5 * 1000 * 1000};
    nanosleep(&ts, NULL);
}

// Takes one sample and prints its body; 1 when there was one.
static int take_one(struct zerodds_ZeroDdsReader *reader) {
    uint8_t *buf = NULL;
    uintptr_t len = 0;
    int rc = zerodds_reader_take(reader, &buf, &len, NULL);
    if (rc != 0) {
        fprintf(stderr, "zerodds_reader_take: %d\n", rc);
        exit(1);
    }
    if (buf == NULL) {
        return 0;
    }
    printf("EVENT sample ");
    for (uintptr_t i = 0; i < len; i++) {
        printf("%02x", buf[i]);
    }
    printf("\n");
    fflush(stdout);
    zerodds_buffer_free(buf, len);
    return 1;
}

int main(int argc, char **argv) {
    if (argc != 5) {
        fprintf(stderr, "usage: %s <domain> <topic> <type name> <samples>\n", argv[0]);
        return 2;
    }
    uint32_t domain = (uint32_t)strtoul(argv[1], NULL, 10);
    int wanted = atoi(argv[4]);
    struct zerodds_ZeroDdsRuntime *runtime = zerodds_runtime_create(domain);
    if (runtime == NULL) {
        fprintf(stderr, "zerodds_runtime_create(%u) returned NULL\n", domain);
        return 1;
    }
    struct zerodds_ZeroDdsReader *reader = zerodds_reader_create(runtime, argv[2], argv[3], 1);
    if (reader == NULL) {
        fprintf(stderr, "zerodds_reader_create returned NULL\n");
        return 1;
    }
    if (zerodds_reader_wait_for_matched(reader, 1, 10000) != 0) {
        printf("EVENT unmatched\n");
        return 1;
    }
    printf("EVENT matched\n");
    fflush(stdout);
    int taken = 0;
    double deadline = seconds_now() + 10.0;
    while (taken < wanted && seconds_now() < deadline) {
        if (take_one(reader)) {
            taken++;
        } else {
            backoff();
        }
    }
    printf("EVENT received n=%d\n", taken);
    fflush(stdout);
    while (getchar() != EOF) {
    }
    while (take_one(reader)) {
    }
    printf("EVENT done\n");
    fflush(stdout);
    zerodds_reader_destroy(reader);
    zerodds_runtime_destroy(runtime);
    return 0;
}

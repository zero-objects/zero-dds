/*
 * Three keyed samples through zerodds_writer_write_typed and
 * zerodds_reader_take_typed, over the TypeSupport table
 * `zerodds-idlc generate --lang c` wrote for `tp::Reading`, in one runtime.
 *
 *   typed_c_roundtrip <domain>
 *
 * Prints "TYPED C ROUNDTRIP OK" when each sample came back as written and
 * was released with the table's sample_free.
 */
#define _POSIX_C_SOURCE 200809L

#include "typed_c_probe_typesupport.h"

#include <stdio.h>
#include <time.h>

static int fail(const char *what) {
    fprintf(stderr, "typed_c_roundtrip: %s\n", what);
    return 1;
}

int main(int argc, char **argv) {
    const zerodds_typesupport_t *ts = &tp_Reading_typesupport;
    struct zerodds_ZeroDdsRuntime *rt;
    struct zerodds_ZeroDdsWriter *writer;
    struct zerodds_ZeroDdsReader *reader;
    static double values[3][2];
    static char names[3][32];
    int seen[3] = {0, 0, 0};
    int taken = 0;
    int i;
    int rc;
    int polls;
    struct timespec pause = {0, 10000000L};

    if (argc != 2) {
        return fail("usage: typed_c_roundtrip <domain>");
    }
    rt = zerodds_runtime_create((uint32_t)strtoul(argv[1], NULL, 10));
    if (rt == NULL) {
        return fail("no runtime");
    }
    writer = zerodds_writer_create_kind(rt, "TypedCProbe", ts->type_name, 1, ts->is_keyed);
    reader = zerodds_reader_create_kind(rt, "TypedCProbe", ts->type_name, 1, ts->is_keyed);
    if (writer == NULL || reader == NULL) {
        return fail("no writer or reader");
    }
    if (zerodds_writer_wait_for_matched(writer, 1, 5000u) != 0) {
        return fail("the writer did not match the reader");
    }

    for (i = 0; i < 3; i++) {
        tp_Reading sample;
        memset(&sample, 0, sizeof sample);
        sample.id = i + 1;
        snprintf(names[i], sizeof names[i], "reading-%d", i + 1);
        sample.name = names[i];
        values[i][0] = (double)(i + 1) * 0.5;
        values[i][1] = -(double)(i + 1);
        sample.values._length = 2u;
        sample.values._buffer = values[i];
        sample.note = ((i % 2) == 1) ? "even" : NULL;
        rc = zerodds_writer_write_typed(writer, ts, &sample);
        if (rc != 0) {
            fprintf(stderr, "write %d: %d\n", i + 1, rc);
            return fail("a typed write failed");
        }
    }

    for (polls = 0; taken < 3 && polls < 500; polls++) {
        tp_Reading got;
        int index;
        memset(&got, 0, sizeof got);
        rc = zerodds_reader_take_typed(reader, ts, &got, NULL);
        if (rc == -7) {
            nanosleep(&pause, NULL);
            continue;
        }
        if (rc != 0) {
            fprintf(stderr, "take: %d\n", rc);
            return fail("a typed take failed");
        }
        index = got.id - 1;
        if (index < 0 || index > 2 || seen[index]) {
            ts->sample_free(&got);
            return fail("a sample with an id nobody wrote, or twice");
        }
        seen[index] = 1;
        if (got.name == NULL || strcmp(got.name, names[index]) != 0 ||
            got.values._length != 2u || got.values._buffer[0] != values[index][0] ||
            got.values._buffer[1] != values[index][1] ||
            (got.note != NULL) != ((index % 2) == 1) ||
            (got.note != NULL && strcmp(got.note, "even") != 0)) {
            ts->sample_free(&got);
            return fail("a sample came back other than it was written");
        }
        ts->sample_free(&got);
        taken++;
    }
    if (taken != 3) {
        return fail("not all three samples arrived");
    }

    zerodds_reader_destroy(reader);
    zerodds_writer_destroy(writer);
    zerodds_runtime_destroy(rt);
    printf("TYPED C ROUNDTRIP OK\n");
    return 0;
}

/*
 * The four zerodds_runtime_for_each_* of zerodds.h, called from C through
 * the header alone.
 *
 *   for_each_c <domain>
 *
 * Runtime `a` has a RELIABLE writer, runtime `b` a RELIABLE reader of the
 * same topic. Once the reader matched the writer, `b` enumerates the
 * publication and `a` the subscription: by topic and type name, and by
 * endpoint with the GUID zerodds_writer_guid / zerodds_reader_guid name and
 * the reliability. Prints "FOR EACH C OK" when each call found its
 * endpoint.
 */
#include "zerodds.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define TOPIC "ForEachCTopic"
#define TYPE "ForEachCType"

struct names_seen {
    int count;
};

struct endpoint_seen {
    uint8_t guid[16];
    int count;
    int reliable;
};

static void on_topic(void *user_data, const char *topic_name, const char *type_name) {
    struct names_seen *seen = (struct names_seen *)user_data;
    if (strcmp(topic_name, TOPIC) == 0 && strcmp(type_name, TYPE) == 0) {
        seen->count += 1;
    }
}

static void on_endpoint(void *user_data, const struct zerodds_ZeroDdsEndpointInfo *info) {
    struct endpoint_seen *seen = (struct endpoint_seen *)user_data;
    if (strcmp(info->topic_name, TOPIC) == 0 && strcmp(info->type_name, TYPE) == 0 &&
        memcmp(info->endpoint_guid, seen->guid, 16) == 0) {
        seen->count += 1;
        seen->reliable = info->reliable;
    }
}

static int fail(const char *what) {
    fprintf(stderr, "for_each_c: %s\n", what);
    return 1;
}

int main(int argc, char **argv) {
    struct zerodds_ZeroDdsRuntime *a;
    struct zerodds_ZeroDdsRuntime *b;
    struct zerodds_ZeroDdsWriter *writer;
    struct zerodds_ZeroDdsReader *reader;
    struct names_seen publications = {0};
    struct names_seen subscriptions = {0};
    struct endpoint_seen writer_seen;
    struct endpoint_seen reader_seen;
    int status = 0;

    if (argc != 2) {
        return fail("usage: for_each_c <domain>");
    }
    memset(&writer_seen, 0, sizeof writer_seen);
    memset(&reader_seen, 0, sizeof reader_seen);
    a = zerodds_runtime_create((uint32_t)strtoul(argv[1], NULL, 10));
    b = zerodds_runtime_create((uint32_t)strtoul(argv[1], NULL, 10));
    if (a == NULL || b == NULL) {
        return fail("zerodds_runtime_create");
    }
    writer = zerodds_writer_create(a, TOPIC, TYPE, 1);
    reader = zerodds_reader_create(b, TOPIC, TYPE, 1);
    if (writer == NULL || reader == NULL) {
        return fail("endpoint create");
    }
    if (zerodds_writer_guid(writer, writer_seen.guid) != 0 ||
        zerodds_reader_guid(reader, reader_seen.guid) != 0) {
        return fail("endpoint guid");
    }
    if (zerodds_reader_wait_for_matched(reader, 1, 15000) != 0) {
        return fail("the reader did not match the writer within 15 s");
    }

    if (zerodds_runtime_for_each_publication(b, on_topic, &publications) != 0 ||
        zerodds_runtime_for_each_subscription(a, on_topic, &subscriptions) != 0 ||
        zerodds_runtime_for_each_publication_endpoint(b, on_endpoint, &writer_seen) != 0 ||
        zerodds_runtime_for_each_subscription_endpoint(a, on_endpoint, &reader_seen) != 0) {
        return fail("a for_each call returned an error");
    }
    if (publications.count != 1) {
        status = fail("for_each_publication of b did not name the writer's topic once");
    }
    if (subscriptions.count != 1) {
        status = fail("for_each_subscription of a did not name the reader's topic once");
    }
    if (writer_seen.count != 1 || writer_seen.reliable != 1) {
        status = fail("for_each_publication_endpoint of b did not give the RELIABLE writer once");
    }
    if (reader_seen.count != 1 || reader_seen.reliable != 1) {
        status = fail("for_each_subscription_endpoint of a did not give the RELIABLE reader once");
    }

    zerodds_reader_destroy(reader);
    zerodds_writer_destroy(writer);
    zerodds_runtime_destroy(b);
    zerodds_runtime_destroy(a);
    if (status == 0) {
        printf("FOR EACH C OK\n");
    }
    return status;
}

/* C smoke test for zerodds.h.
 *
 * Build:
 *   cargo build -p dds-c-api --release
 *   gcc -O2 -I crates/dds-c-api/include \
 *       -L target/release \
 *       -o /tmp/zerodds_c_smoke \
 *       crates/dds-c-api/examples/c_smoke.c \
 *       -lzerodds -lpthread -ldl -lm
 *   ./tmp/zerodds_c_smoke
 *
 * Erwartung (Linux): "OK: <N> samples received".
 * Expectation (macOS): may be 0 (multicast loopback unreliable).
 */

#include "zerodds.h"
#include <stdio.h>
#include <string.h>
#include <unistd.h>

int main(void) {
    printf("zerodds version: %s\n", zerodds_version());

    struct zerodds_ZeroDdsRuntime *rt = zerodds_runtime_create(123);
    if (!rt) { fprintf(stderr, "runtime_create failed\n"); return 1; }

    struct zerodds_ZeroDdsWriter *w =
        zerodds_writer_create(rt, "CSmokeTopic", "RawBytes", 1);
    if (!w) { fprintf(stderr, "writer_create failed\n"); return 2; }

    struct zerodds_ZeroDdsReader *r =
        zerodds_reader_create(rt, "CSmokeTopic", "RawBytes", 1);
    if (!r) { fprintf(stderr, "reader_create failed\n"); return 3; }

    /* Discovery-wait. */
    zerodds_writer_wait_for_matched(w, 1, 5000);

    const uint8_t payload[] = {0xDE, 0xAD, 0xBE, 0xEF};
    for (int i = 0; i < 5; ++i) {
        if (zerodds_writer_write(w, payload, sizeof(payload)) != 0) {
            fprintf(stderr, "write failed\n");
        }
        usleep(10 * 1000); /* 10ms */
    }

    /* Bis zu 2s einsammeln. */
    int received = 0;
    for (int i = 0; i < 100; ++i) {
        uint8_t *buf = NULL;
        size_t len = 0;
        if (zerodds_reader_take(r, &buf, &len, NULL) == 0 && buf && len > 0) {
            received++;
            zerodds_buffer_free(buf, len);
        } else {
            usleep(20 * 1000);
        }
    }

    printf("OK: %d samples received\n", received);

    zerodds_writer_destroy(w);
    zerodds_reader_destroy(r);
    zerodds_runtime_destroy(rt);
    return received > 0 ? 0 : 4;
}

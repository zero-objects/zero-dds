/* Trivial C consumer of the installed zerodds SDK. Includes the public C
 * header and references a real exported symbol so the link against the
 * installed library is genuinely exercised (not just the include path). */
#include <zerodds.h>
#include <stdio.h>

int main(int argc, char **argv) {
    (void)argv;
    /* Guarded by a condition the compiler cannot fold away, so the call is
     * kept and the linker must resolve the symbols against the installed
     * libzerodds -- but the runtime never actually opens a participant. */
    if (argc > 1000000) {
        zerodds_runtime_destroy(zerodds_runtime_create(0));
    }
    puts("zerodds C SDK consumer: linked ok");
    return 0;
}

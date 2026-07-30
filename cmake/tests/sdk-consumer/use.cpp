// Trivial C++ consumer of the installed zerodds SDK. Includes the C++
// convenience header (which transitively pulls the dds/** PSM headers), proving
// the whole C++ header tree is present in the staging tree, and links the C
// core library through zerodds::zerodds-cpp -> zerodds::zerodds-c.
#include <zerodds/dds.hpp>
#include <cstdio>

int main(int argc, char **argv) {
    (void)argv;
    // Compile-time use: the C++ wrapper type must be complete (headers parsed).
    static_assert(sizeof(zerodds::Runtime) > 0,
                  "zerodds::Runtime must be a complete type from the installed headers");
    std::printf("zerodds C++ SDK consumer: headers ok, Runtime size=%zu\n",
                sizeof(zerodds::Runtime));
    // Reference the C core symbol (guarded, never executed) so the link is
    // exercised end to end.
    if (argc > 1000000) {
        zerodds_runtime_destroy(zerodds_runtime_create(0));
    }
    return 0;
}

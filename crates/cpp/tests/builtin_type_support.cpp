// DDS-XTypes 1.3 Annex E: StringTypeSupport, KeyedStringTypeSupport, BytesTypeSupport and
// KeyedBytesTypeSupport: get_type_name and register_type.
#include <dds/dds.hpp>

#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "zerodds/builtin_type_support.hpp"

static int g_failures = 0;
#define EXPECT(cond, what)                                                     \
    do {                                                                       \
        if (!(cond)) {                                                         \
            std::printf("FAIL: %s\n", what);                                   \
            ++g_failures;                                                      \
        }                                                                      \
    } while (0)

int main(int argc, char** argv) {
    const uint32_t domain = argc > 1 ? static_cast<uint32_t>(std::atoi(argv[1])) : 0;
    dds::domain::DomainParticipant participant(domain);

    DDS::StringTypeSupport string_support;
    DDS::KeyedStringTypeSupport keyed_string_support;
    DDS::BytesTypeSupport bytes_support;
    DDS::KeyedBytesTypeSupport keyed_bytes_support;
    const DDS::TypeSupport* all[] = {&string_support, &keyed_string_support, &bytes_support,
                                     &keyed_bytes_support};
    const char* names[] = {"DDS::String", "DDS::KeyedString", "DDS::Bytes", "DDS::KeyedBytes"};

    for (int i = 0; i < 4; ++i) {
        EXPECT(std::strcmp(all[i]->get_type_name(), names[i]) == 0, "get_type_name");
        // Under its own name, twice; under a name of the caller's, twice.
        EXPECT(all[i]->register_type(participant) == DDS::RETCODE_OK, "register_type");
        EXPECT(all[i]->register_type(participant, names[i]) == DDS::RETCODE_OK,
               "register_type again");
        const std::string alias = std::string("Alias") + std::to_string(i);
        EXPECT(all[i]->register_type(participant, alias.c_str()) == DDS::RETCODE_OK,
               "register_type under another name");
        EXPECT(all[i]->register_type(participant, alias.c_str()) == DDS::RETCODE_OK,
               "register_type under another name again");
    }
    // Another type under a taken name, and a name that is no name.
    EXPECT(keyed_string_support.register_type(participant, "Alias0") ==
               DDS::RETCODE_PRECONDITION_NOT_MET,
           "another type under a taken name");
    EXPECT(string_support.register_type(participant, "") == DDS::RETCODE_BAD_PARAMETER,
           "an empty name");

    if (g_failures == 0) {
        std::printf("BUILTIN TYPE SUPPORT OK\n");
    }
    return g_failures == 0 ? 0 : 1;
}

#include <iostream>
#include <cstdio>
#include <vector>
#include <string>
#include <rti/topic/cdr.hpp>
#include "gapprobe.hpp"

static void run(const ::GapProbe &s, const char *label,
                dds::core::policy::DataRepresentationId rep)
{
    std::vector<char> probe;
    rti::topic::to_cdr_buffer(probe, s, rep);
    std::vector<char> poisoned(probe.size(), (char)0xAA);
    rti::topic::to_cdr_buffer(poisoned, s, rep);
    printf("%s len=%zu ", label, poisoned.size());
    for (size_t i = 0; i < poisoned.size(); i++)
        printf("%02x", (unsigned char)poisoned[i]);
    printf("\n");
}

int main()
{
    ::GapProbe a;
    a.s = std::string(20, 0x41); a.b = 0x11223344; a.c = 0x55;
    a.d = 1.5; a.e = 0x66; a.f = 0x7788;
    run(a, "static-xcdr1", dds::core::policy::DataRepresentation::xcdr());
    run(a, "static-xcdr2", dds::core::policy::DataRepresentation::xcdr2());
    return 0;
}

#include <cstdio>
#include <fastcdr/Cdr.h>
#include <fastcdr/FastBuffer.h>
#include "gen.hpp"
#include "genCdrAux.hpp"

using eprosima::fastcdr::Cdr;
using eprosima::fastcdr::CdrVersion;
using eprosima::fastcdr::FastBuffer;

static void dump(const char *label, CdrVersion version, const @TYPE@ &sample) {
    char raw[65536];
    FastBuffer buffer(raw, sizeof(raw));
    Cdr ser(buffer, Cdr::LITTLE_ENDIANNESS, version);
    try {
        ser.serialize_encapsulation();
        ser << sample;
    } catch (const std::exception &e) {
        printf("%s REFUSED %s\n", label, e.what());
        return;
    }
    const size_t length = ser.get_serialized_data_length();
    printf("%s ", label);
    for (size_t i = 4; i < length; i++)
        printf("%02x", static_cast<unsigned char>(raw[i]));
    printf("\n");
}

int main() {
    @TYPE@ sample;
#include "fill.inc"
    dump("xcdr1", CdrVersion::XCDRv1, sample);
    dump("xcdr2", CdrVersion::XCDRv2, sample);
    return 0;
}

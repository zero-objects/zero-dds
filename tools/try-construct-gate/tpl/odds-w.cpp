#include <cstdio>
#include <dds/DCPS/Serializer.h>
#include <ace/Message_Block.h>
#include "genTypeSupportImpl.h"

using OpenDDS::DCPS::Encoding;
using OpenDDS::DCPS::Serializer;
using OpenDDS::DCPS::ENDIAN_LITTLE;

static void dump(const char *label, Encoding::Kind kind, const @TYPE@ &sample) {
    Encoding encoding(kind, ENDIAN_LITTLE);
    ACE_Message_Block mb(65536);
    Serializer ser(&mb, encoding);
    if (!(ser << sample)) { printf("%s REFUSED\n", label); return; }
    const size_t length = mb.length();
    printf("%s ", label);
    for (size_t i = 0; i < length; i++)
        printf("%02x", static_cast<unsigned char>(mb.rd_ptr()[i]));
    printf("\n");
}

int main() {
    @TYPE@ sample;
#include "fill.inc"
    dump("xcdr1", Encoding::KIND_XCDR1, sample);
    dump("xcdr2", Encoding::KIND_XCDR2, sample);
    return 0;
}

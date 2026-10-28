#include <cstdio>
#include <cstdlib>
#include <vector>
#include <dds/DCPS/Serializer.h>
#include <ace/Message_Block.h>
#include "genTypeSupportImpl.h"

using OpenDDS::DCPS::Encoding;
using OpenDDS::DCPS::Serializer;
using OpenDDS::DCPS::ENDIAN_LITTLE;

static std::vector<unsigned char> read_hex() {
    std::vector<unsigned char> out;
    int hi, lo;
    while ((hi = std::getchar()) != EOF) {
        if (hi == '\n' || hi == ' ' || hi == '\r') continue;
        lo = std::getchar();
        if (lo == EOF) break;
        char pair[3] = { static_cast<char>(hi), static_cast<char>(lo), 0 };
        out.push_back(static_cast<unsigned char>(std::strtoul(pair, nullptr, 16)));
    }
    return out;
}

int main() {
    std::vector<unsigned char> body = read_hex();
    Encoding encoding(Encoding::@KIND@, ENDIAN_LITTLE);
    ACE_Message_Block mb(body.size() + 16);
    for (unsigned char byte : body) mb.copy(reinterpret_cast<const char *>(&byte), 1);
    Serializer ser(&mb, encoding);
    @TYPE@ sample;
    if (!(ser >> sample)) { printf("REFUSED deserialize\n"); return 0; }
#include "dump.inc"
    return 0;
}

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include <fastcdr/Cdr.h>
#include <fastcdr/CdrEncoding.hpp>
#include "gen.hpp"
#include "genCdrAux.hpp"
#include "genPubSubTypes.hpp"

using eprosima::fastdds::rtps::SerializedPayload_t;

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
    @TYPE@ sample;
    @TYPE@PubSubType support;

    SerializedPayload_t probe(65536);
    if (!support.serialize(&sample, probe, @REPRESENTATION@) || probe.length < 4) {
        printf("REFUSED no-header\n");
        return 0;
    }
    const unsigned flag = static_cast<unsigned>(probe.data[1]) & 0xFEu;

    std::vector<unsigned char> body = read_hex();
    SerializedPayload_t payload(static_cast<std::uint32_t>(body.size() + 4));
    payload.data[0] = 0;
    payload.data[1] = static_cast<unsigned char>(flag | 1u);
    payload.data[2] = 0;
    payload.data[3] = 0;
    std::memcpy(payload.data + 4, body.data(), body.size());
    payload.length = static_cast<std::uint32_t>(body.size() + 4);
    payload.pos = 0;

    try {
        if (!support.deserialize(payload, &sample)) {
            printf("REFUSED deserialize\n");
            return 0;
        }
    } catch (const std::exception &error) {
        printf("REFUSED exception %s\n", error.what());
        return 0;
    }
#include "dump.inc"
    return 0;
}

#include <cstdio>
#include <cstring>
#include <vector>
#include "genPubSubTypes.hpp"
int main() {
    setvbuf(stdout, nullptr, _IONBF, 0);
    BPubSubType pst;
    for (unsigned r = 0; r < 3; ++r) {
        unsigned char raw[12] = { 0x00, 0x07, 0, 0, (unsigned char)r, 0, 0, 0, 0x07, 0, 0, 0 };
        eprosima::fastdds::rtps::SerializedPayload_t payload(sizeof raw);
        memcpy(payload.data, raw, sizeof raw); payload.length = sizeof raw;
        B out;
        bool ok = false;
        try { ok = pst.deserialize(payload, static_cast<void*>(&out)); }
        catch (const std::exception& e) { printf("fastdds boolean=0x%02x threw %s\n", r, e.what()); continue; }
        printf("fastdds boolean=0x%02x  read=%s  flag=%u  n=%d\n", r,
               ok ? "accepted" : "REFUSED", (unsigned)out.flag(), out.n());
    }
    printf("fastdds sizeof(B::flag)=%zu sizeof(C::letter)=%zu\n", sizeof(bool), sizeof(char));
    return 0;
}

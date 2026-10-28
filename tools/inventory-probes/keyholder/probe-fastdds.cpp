#include <cstdio>
#include <cstring>
#include <exception>
#include "khPubSubTypes.hpp"
using eprosima::fastdds::rtps::InstanceHandle_t;
static void show(const char* label, bool ok, const InstanceHandle_t& h) {
    printf("  %-38s ret=%-5s handle=", label, ok ? "true" : "false");
    for (int i = 0; i < 16; ++i) printf("%02x", h.value[i]);
    printf("\n");
}
template <typename PST, typename T> static void probe(const char* name, const T& sample) {
    PST pst; printf("--- %s ---\n", name);
    printf("  max_serialized_type_size %u\n", pst.max_serialized_type_size);
    InstanceHandle_t h;
    try {
        memset(h.value, 0xAA, 16);
        show("compute_key(force_md5=false)", pst.compute_key((const void*)&sample, h, false), h);
        memset(h.value, 0xAA, 16);
        show("compute_key(force_md5=true) ", pst.compute_key((const void*)&sample, h, true), h);
    } catch (const std::exception& e) { printf("  threw %s\n", e.what()); }
    printf("\n");
}
int main() {
    setvbuf(stdout, nullptr, _IONBF, 0);
    KeyOrder ko; ko.b(2); ko.a(1);
    KeyBig kb; kb.a(1); kb.b(2); kb.c(3); kb.d(4); kb.e(5);
    KeyMixed km; km.b(2); km.a(1);
    probe<KeyOrderPubSubType>("KeyOrder{@id(5)@key b=2, @id(2)@key a=1}", ko);
    probe<KeyBigPubSubType>("KeyBig{a..e=1..5}", kb);
    probe<KeyMixedPubSubType>("KeyMixed{b=2,a=1}", km);
    return 0;
}

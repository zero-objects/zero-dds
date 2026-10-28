#include <cstdio>
#include <omniORB4/CORBA.h>
#include "gen.hh"

static void dump(const char *label, bool big_endian, const FixedAll &sample) {
    cdrMemoryStream ms(0, false);
    ms.setByteSwapFlag(big_endian ? 0 : 1);
    sample >>= ms;
    const size_t length = ms.bufSize();
    const unsigned char *p = (const unsigned char *)ms.bufPtr();
    printf("%s ", label);
    for (size_t i = 0; i < length; i++) printf("%02x", p[i]);
    printf(" (%zu)\n", length);
}

int main() {
    FixedAll sample;
    sample.head = 0x77;
    sample.pos = D5S2("123.45");
    sample.neg = D5S2("-123.45");
    sample.zero = D5S2("0.00");
    sample.even = D4S2("12.34");
    sample.allfrac = D3S3("0.123");
    sample.one = D1S0("7");
    sample.two = D2S0("42");
    sample.big = D31S0("1234567890123456789012345678901");
    sample.tail = 0xAB;
    dump("omniorb-cdr-le", false, sample);
    dump("omniorb-cdr-be", true, sample);
    return 0;
}

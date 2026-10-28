#include <cstdio>
#include <omniORB4/CORBA.h>
#include "gen.hh"

int main() {
    @TYPE@ sample;
#include "fill.inc"
    cdrMemoryStream ms(0, false);
    sample >>= ms;
    const size_t length = ms.bufSize();
    const unsigned char *p = (const unsigned char *)ms.bufPtr();
    printf("cdr ");
    for (size_t i = 0; i < length; i++) printf("%02x", p[i]);
    printf("\n");
    return 0;
}

#include <cstdio>
#include <cstdlib>
#include <vector>
#include <omniORB4/CORBA.h>
#include "gen.hh"

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

int main(int argc, char **argv) {
    /* omniORB's unmarshalString reaches into ORB state, so the ORB has
       to exist before a stream is read; without it the read segfaults. */
    CORBA::ORB_var orb = CORBA::ORB_init(argc, argv);
    std::vector<unsigned char> body = read_hex();
    /* A read-only stream over the payload: the buffer is the message,
       so nothing this program writes can be mistaken for it. */
    cdrMemoryStream ms((void *)body.data(), body.size());
    ms.setByteSwapFlag(1);
    @TYPE@ sample;
    try {
        sample <<= ms;
    } catch (const CORBA::Exception &e) {
        printf("REFUSED %s\n", e._name());
        return 0;
    } catch (...) {
        printf("REFUSED unknown\n");
        return 0;
    }
#include "dump.inc"
    return 0;
}

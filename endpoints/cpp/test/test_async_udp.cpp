// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Live-UDP async E2E (C++17, POSIX): a real non-blocking UDP socket is the
// transport. An AsyncWriter sends N samples over the loopback; the AsyncReader
// drains them via a std::function callback (recvfrom -> EAGAIN -> ZDW_T_AGAIN).

#include <arpa/inet.h>
#include <cerrno>
#include <csignal>
#include <cstdio>
#include <cstring>
#include <fcntl.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <unistd.h>

#include <vector>

#include "zerodds_async.hpp"
#include "zerodds_wire.hpp"

namespace {

constexpr int kN = 5;

struct Udp {
    int                fd;
    struct sockaddr_in peer;
};

extern "C" int udp_deliver(void* ctx, const unsigned char* frame, std::size_t len) {
    auto* u = static_cast<Udp*>(ctx);
    ssize_t n = sendto(u->fd, frame, len, 0,
                       reinterpret_cast<struct sockaddr*>(&u->peer), sizeof u->peer);
    return (n == static_cast<ssize_t>(len)) ? ZDW_T_OK : ZDW_T_ERROR;
}

extern "C" int udp_receive(void* ctx, unsigned char* out, std::size_t cap, std::size_t* len) {
    auto* u = static_cast<Udp*>(ctx);
    ssize_t n = recvfrom(u->fd, out, cap, 0, nullptr, nullptr);
    if (n < 0) return (errno == EAGAIN || errno == EWOULDBLOCK) ? ZDW_T_AGAIN : ZDW_T_ERROR;
    *len = static_cast<std::size_t>(n);
    return ZDW_T_OK;
}

std::size_t encode_sample(unsigned char* out, std::size_t cap, unsigned long id) {
    zerodds::Writer w(out, cap, ZDW_LE);
    w.u32(id); w.u16(0); w.u8(0); w.f32(0.0f); w.u64(zdw_u64_from_ul(0));
    w.str("udp"); w.seq_u8(nullptr, 0);
    return w.size();
}

}  // namespace

int main() {
    Udp u{};
    u.fd = socket(AF_INET, SOCK_DGRAM, 0);
    if (u.fd < 0) { std::perror("socket"); return 1; }

    struct sockaddr_in addr{};
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    addr.sin_port = 0;
    if (bind(u.fd, reinterpret_cast<struct sockaddr*>(&addr), sizeof addr) < 0) {
        std::perror("bind"); return 1;
    }
    socklen_t alen = sizeof addr;
    getsockname(u.fd, reinterpret_cast<struct sockaddr*>(&addr), &alen);
    fcntl(u.fd, F_SETFL, O_NONBLOCK);  // reactor relies on EAGAIN -> ZDW_T_AGAIN
    u.peer = addr;

    zdw_transport t{&u, udp_deliver, udp_receive};

    unsigned char txbuf[256];
    zerodds::AsyncWriter writer(&t, txbuf, sizeof txbuf,
                                ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT);
    for (int i = 0; i < kN; i++) {
        unsigned char body[128];
        std::size_t n = encode_sample(body, sizeof body, 0x2000u + i);
        if (!writer.write(body, n)) { std::fprintf(stderr, "write %d\n", i); return 1; }
    }

    std::vector<unsigned long> got;
    unsigned char rxbuf[256];
    zerodds::AsyncReader reader(&t, rxbuf, sizeof rxbuf,
        [&got](const unsigned char* body, std::size_t len) {
            zerodds::Reader r(body, len, ZDW_LE);
            got.push_back(r.u32());
        });

    for (int tries = 0; tries < 1000 && static_cast<int>(got.size()) < kN; tries++) {
        reader.run();
        if (static_cast<int>(got.size()) < kN) usleep(1000);
    }
    close(u.fd);

    if (static_cast<int>(got.size()) != kN) {
        std::fprintf(stderr, "received %zu/%d over UDP\n", got.size(), kN);
        return 1;
    }
    std::printf("async UDP (C++17): %d/%d samples received + decoded via reactor\n", kN, kN);
    std::printf("ALL OK\n");
    return 0;
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The C++ subscriber of tests/integration/tests/bindings_ffi.rs: the RAII
// wrappers of crates/cpp/include/zerodds/dds.hpp over the C ABI
// (zerodds-ffi-loader-1.0 §3.4).
//
//   sub_cpp <domain> <topic> <type name> <samples>
//
// The protocol of sub.c: `EVENT matched`, `EVENT sample <hex>` per body,
// `EVENT received n=<count>`, one more take after standard input closed,
// `EVENT done`.

#include "zerodds/dds.hpp"

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <iostream>
#include <string>
#include <thread>
#include <vector>

namespace {

bool take_one(zerodds::Reader &reader) {
    std::vector<uint8_t> body = reader.take();
    if (body.empty()) {
        return false;
    }
    std::printf("EVENT sample ");
    for (uint8_t octet : body) {
        std::printf("%02x", octet);
    }
    std::printf("\n");
    std::fflush(stdout);
    return true;
}

}  // namespace

int main(int argc, char **argv) {
    if (argc != 5) {
        std::cerr << "usage: " << argv[0] << " <domain> <topic> <type name> <samples>\n";
        return 2;
    }
    const auto domain = static_cast<uint32_t>(std::strtoul(argv[1], nullptr, 10));
    const int wanted = std::atoi(argv[4]);
    try {
        zerodds::Runtime runtime(domain);
        zerodds::Reader reader(runtime, argv[2], argv[3], true);
        if (!reader.wait_for_matched(1, 10000)) {
            std::printf("EVENT unmatched\n");
            return 1;
        }
        std::printf("EVENT matched\n");
        std::fflush(stdout);
        int taken = 0;
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        while (taken < wanted && std::chrono::steady_clock::now() < deadline) {
            if (take_one(reader)) {
                ++taken;
            } else {
                // The reader has no blocking take.
                std::this_thread::sleep_for(std::chrono::milliseconds(5));
            }
        }
        std::printf("EVENT received n=%d\n", taken);
        std::fflush(stdout);
        std::string line;
        while (std::getline(std::cin, line)) {
        }
        while (take_one(reader)) {
        }
        std::printf("EVENT done\n");
        std::fflush(stdout);
        return 0;
    } catch (const zerodds::StatusError &error) {
        std::cerr << "zerodds: " << error.what() << "\n";
        return 1;
    }
}

// C++-Smoke-Test fuer zerodds/dds.hpp.
//
// Build:
//   cargo build --release -p dds-c-api
//   g++ -std=c++17 -O2 -Wall \
//       -I crates/dds-c-api/include \
//       -I crates/cpp/include \
//       -L target/release \
//       -o /tmp/zerodds_cpp_smoke \
//       crates/cpp/examples/cpp_smoke.cpp \
//       -lzerodds -lpthread -ldl -lm
//   ./tmp/zerodds_cpp_smoke

#include "zerodds/dds.hpp"

#include <chrono>
#include <iostream>
#include <thread>
#include <vector>

int main() {
    using namespace std::chrono_literals;

    std::cout << "zerodds version: " << zerodds::version() << "\n";

    try {
        zerodds::Runtime rt(123);
        zerodds::Writer w(rt, "CppSmokeTopic", "RawBytes", true);
        zerodds::Reader r(rt, "CppSmokeTopic", "RawBytes", true);

        // Discovery-wait via Writer.
        if (!w.wait_for_matched(1, 5000)) {
            std::cerr << "wait_for_matched timeout\n";
            return 1;
        }

        const std::vector<uint8_t> payload = {0xDE, 0xAD, 0xBE, 0xEF};
        for (int i = 0; i < 5; ++i) {
            w.write(payload);
            std::this_thread::sleep_for(10ms);
        }

        // Bis zu 2s einsammeln.
        int received = 0;
        for (int i = 0; i < 100; ++i) {
            auto sample = r.take();
            if (!sample.empty()) {
                received++;
            } else {
                std::this_thread::sleep_for(20ms);
            }
        }
        std::cout << "OK: " << received << " samples received\n";
        return received > 0 ? 0 : 4;
    } catch (const zerodds::StatusError &e) {
        std::cerr << "FAIL: " << e.what() << "\n";
        return 5;
    }
}

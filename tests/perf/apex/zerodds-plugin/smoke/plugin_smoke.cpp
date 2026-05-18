// plugin_smoke.cpp — Pub-Sub-Smoke ueber das ZeroDDS-Plugin.
//
// Zwei separate Communicators (= zwei DDS-Participants auf gleicher
// Domain) reden ueber UDP-Loopback miteinander. Vor Endpoint-Erstellung
// MUSS jeder Communicator via wait_for_peers() abwarten bis SPDP den
// Peer entdeckt hat — sonst wiren die Endpoints mit leerer Locator-
// Liste und Daten gehen ins Leere (siehe README, Section "Race").

#include <atomic>
#include <chrono>
#include <cstdio>
#include <cstring>
#include <thread>

#include "zerodds_plugin/zerodds_communicator.hpp"

using performance_test::plugins::zerodds::ZeroDdsCommunicator;

namespace {
constexpr uint32_t kDomain = 100;
constexpr const char *kTopic = "PluginSmokeTopic";
constexpr const char *kType = "RawBytes";
constexpr uint8_t kPayload[] = {0xde, 0xad, 0xbe, 0xef};

std::atomic<bool> g_stop{false};
std::atomic<int> g_received{0};

void sub_thread() {
  ZeroDdsCommunicator comm(kDomain);
  if (!comm.wait_for_peers(1, 5000)) {
    std::printf("sub: FAIL no peer discovered within 5s\n");
    g_stop.store(true);
    return;
  }
  auto sub = comm.create_subscriber(kTopic, kType, /*reliable=*/false);
  if (!sub->wait_for_matched(1, 5000)) {
    std::printf("sub: FAIL no match within 5s\n");
    g_stop.store(true);
    return;
  }
  std::printf("sub: matched\n");
  for (int i = 0; i < 400 && !g_stop.load(); ++i) {
    auto sample = sub->take();
    if (!sample.empty()) {
      if (sample.size() == sizeof(kPayload) &&
          std::memcmp(sample.data(), kPayload, sizeof(kPayload)) == 0) {
        g_received.fetch_add(1);
      } else {
        std::printf("sub: payload mismatch (size=%zu)\n", sample.size());
      }
    } else {
      std::this_thread::sleep_for(std::chrono::milliseconds(20));
    }
    if (g_received.load() >= 3) break;
  }
}
}  // namespace

int main() {
  std::printf("Plugin version: %s\n", ZeroDdsCommunicator::plugin_version());

  std::thread sub(sub_thread);

  ZeroDdsCommunicator comm(kDomain);
  if (!comm.wait_for_peers(1, 5000)) {
    std::printf("pub: FAIL no peer discovered within 5s\n");
    g_stop.store(true);
    sub.join();
    return 1;
  }
  auto pub = comm.create_publisher(kTopic, kType, /*reliable=*/false);
  if (!pub->wait_for_matched(1, 5000)) {
    std::printf("pub: FAIL no match within 5s\n");
    g_stop.store(true);
    sub.join();
    return 2;
  }
  std::printf("pub: matched\n");

  for (int i = 0; i < 30 && g_received.load() < 3 && !g_stop.load(); ++i) {
    pub->publish(kPayload, sizeof(kPayload));
    std::this_thread::sleep_for(std::chrono::milliseconds(30));
  }
  g_stop.store(true);
  sub.join();

  const int got = g_received.load();
  std::printf("plugin smoke: %d samples received\n", got);
  return got >= 1 ? 0 : 3;
}

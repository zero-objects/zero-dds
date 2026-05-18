// zerodds_communicator.cpp — Implementierung der Apex.AI-Bridge.

#include "zerodds_plugin/zerodds_communicator.hpp"

#include <cstring>
#include <stdexcept>

namespace performance_test::plugins::zerodds {

namespace {
constexpr int kStatusOk = 0;
constexpr int kStatusTimeout = -4;
}  // namespace

// ===== Publisher =====

ZeroDdsPublisher::ZeroDdsPublisher(zerodds_ZeroDdsRuntime *runtime,
                                   const std::string &topic,
                                   const std::string &type_name,
                                   bool reliable)
    : writer_(zerodds_writer_create(runtime, topic.c_str(), type_name.c_str(),
                                    reliable ? 1 : 0)) {
  if (!writer_) {
    throw std::runtime_error("zerodds_writer_create returned NULL");
  }
}

ZeroDdsPublisher::~ZeroDdsPublisher() {
  if (writer_) zerodds_writer_destroy(writer_);
}

void ZeroDdsPublisher::publish(const uint8_t *payload, std::size_t len) {
  const int rc = zerodds_writer_write(writer_, payload, len);
  if (rc != kStatusOk) {
    throw std::runtime_error("zerodds_writer_write failed (status=" +
                             std::to_string(rc) + ")");
  }
}

bool ZeroDdsPublisher::wait_for_matched(int min_count,
                                        std::uint64_t timeout_ms) {
  const int rc =
      zerodds_writer_wait_for_matched(writer_, min_count, timeout_ms);
  return rc == kStatusOk;
}

// ===== Subscriber =====

ZeroDdsSubscriber::ZeroDdsSubscriber(zerodds_ZeroDdsRuntime *runtime,
                                     const std::string &topic,
                                     const std::string &type_name,
                                     bool reliable)
    : reader_(zerodds_reader_create(runtime, topic.c_str(), type_name.c_str(),
                                    reliable ? 1 : 0)) {
  if (!reader_) {
    throw std::runtime_error("zerodds_reader_create returned NULL");
  }
}

ZeroDdsSubscriber::~ZeroDdsSubscriber() {
  if (reader_) zerodds_reader_destroy(reader_);
}

std::vector<uint8_t> ZeroDdsSubscriber::take() {
  uint8_t *buf = nullptr;
  std::size_t len = 0;
  const int rc = zerodds_reader_take(reader_, &buf, &len);
  if (rc != kStatusOk || !buf || len == 0) {
    return {};
  }
  std::vector<uint8_t> out(buf, buf + len);
  zerodds_buffer_free(buf, len);
  return out;
}

bool ZeroDdsSubscriber::wait_for_matched(int min_count,
                                         std::uint64_t timeout_ms) {
  const int rc =
      zerodds_reader_wait_for_matched(reader_, min_count, timeout_ms);
  return rc == kStatusOk;
}

// ===== Communicator =====

ZeroDdsCommunicator::ZeroDdsCommunicator(std::uint32_t domain_id)
    : runtime_(zerodds_runtime_create(domain_id)) {
  if (!runtime_) {
    throw std::runtime_error("zerodds_runtime_create returned NULL");
  }
}

ZeroDdsCommunicator::~ZeroDdsCommunicator() {
  if (runtime_) zerodds_runtime_destroy(runtime_);
}

std::unique_ptr<ZeroDdsPublisher> ZeroDdsCommunicator::create_publisher(
    const std::string &topic, const std::string &type_name, bool reliable) {
  return std::make_unique<ZeroDdsPublisher>(runtime_, topic, type_name,
                                            reliable);
}

std::unique_ptr<ZeroDdsSubscriber> ZeroDdsCommunicator::create_subscriber(
    const std::string &topic, const std::string &type_name, bool reliable) {
  return std::make_unique<ZeroDdsSubscriber>(runtime_, topic, type_name,
                                             reliable);
}

const char *ZeroDdsCommunicator::plugin_version() {
  return zerodds_version();
}

bool ZeroDdsCommunicator::wait_for_peers(int min_count,
                                         std::uint64_t timeout_ms) {
  const int rc =
      zerodds_runtime_wait_for_peers(runtime_, min_count, timeout_ms);
  return rc == kStatusOk;
}

}  // namespace performance_test::plugins::zerodds

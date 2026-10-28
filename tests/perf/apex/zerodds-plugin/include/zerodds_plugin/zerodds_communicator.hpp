// zerodds_communicator.hpp — ZeroDDS plugin for Apex.AI performance_test.
//
// Implementiert die Apex-Plugin-Interfaces `Publisher` + `Subscriber`
// against the ZeroDDS C-API (zerodds.h via libzerodds.so). This allows
// performance_test --communication ZeroDDS Latenz/Throughput-Bench
// running against the reference vendors (CycloneDDS, FastRTPS).
//
// Build expectation: the Apex 'performance_test' repo turns each
// plugin into a separate ament_cmake package and links it as
// shared-lib. Plugin-Auswahl per `-DPERFORMANCE_TEST_PLUGIN=ZERODDS`.

#pragma once

#include <cstddef>
#include <cstdint>
#include <memory>
#include <string>
#include <vector>

#include <zerodds.h>  // C-API from crates/dds-c-api

namespace performance_test::plugins::zerodds {

/// RAII wrapper around a ZeroDDS DataWriter, hooked into the Apex plugin.
/// Apex' Publisher-Interface ist minimal: `publish(span<const uint8_t>)`.
class ZeroDdsPublisher {
public:
  ZeroDdsPublisher(zerodds_ZeroDdsRuntime *runtime,
                   const std::string &topic,
                   const std::string &type_name,
                   bool reliable);
  ~ZeroDdsPublisher();
  ZeroDdsPublisher(const ZeroDdsPublisher &) = delete;
  ZeroDdsPublisher &operator=(const ZeroDdsPublisher &) = delete;

  /// Publishes a sample frame (raw bytes, pre-serialized by Apex).
  void publish(const uint8_t *payload, std::size_t len);

  /// Waits until `min_count` subscribers are matched (Apex sync step).
  bool wait_for_matched(int min_count, std::uint64_t timeout_ms);

private:
  zerodds_ZeroDdsWriter *writer_;
};

/// RAII wrapper around a ZeroDDS DataReader.
/// Apex' subscriber interface: `take()` returns vector<uint8_t> or empty.
class ZeroDdsSubscriber {
public:
  ZeroDdsSubscriber(zerodds_ZeroDdsRuntime *runtime,
                    const std::string &topic,
                    const std::string &type_name,
                    bool reliable);
  ~ZeroDdsSubscriber();
  ZeroDdsSubscriber(const ZeroDdsSubscriber &) = delete;
  ZeroDdsSubscriber &operator=(const ZeroDdsSubscriber &) = delete;

  /// Take a sample. Returns an empty vector if nothing was there.
  std::vector<uint8_t> take();

  bool wait_for_matched(int min_count, std::uint64_t timeout_ms);

private:
  zerodds_ZeroDdsReader *reader_;
};

/// Communicator-Factory — Apex' performance_test instantiiert pro
/// A bench run holds a communicator that then hands out pub/sub pairs.
class ZeroDdsCommunicator {
public:
  explicit ZeroDdsCommunicator(std::uint32_t domain_id);
  ~ZeroDdsCommunicator();
  ZeroDdsCommunicator(const ZeroDdsCommunicator &) = delete;
  ZeroDdsCommunicator &operator=(const ZeroDdsCommunicator &) = delete;

  std::unique_ptr<ZeroDdsPublisher> create_publisher(const std::string &topic,
                                                    const std::string &type_name,
                                                    bool reliable);
  std::unique_ptr<ZeroDdsSubscriber> create_subscriber(const std::string &topic,
                                                      const std::string &type_name,
                                                      bool reliable);

  /// Vor Endpoint-Erstellung aufrufen: blockiert bis SPDP mindestens
  /// has discovered `min_count` remote participants. Otherwise it wires
  /// endpoints with an empty locator list and data does not arrive.
  bool wait_for_peers(int min_count, std::uint64_t timeout_ms);

  static const char *plugin_name() { return "ZeroDDS"; }
  static const char *plugin_version();

private:
  zerodds_ZeroDdsRuntime *runtime_;
};

}  // namespace performance_test::plugins::zerodds

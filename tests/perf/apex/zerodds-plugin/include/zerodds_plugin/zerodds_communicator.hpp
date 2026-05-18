// zerodds_communicator.hpp — ZeroDDS-Plugin fuer Apex.AI performance_test.
//
// Implementiert die Apex-Plugin-Interfaces `Publisher` + `Subscriber`
// gegen die ZeroDDS C-API (zerodds.h via libzerodds.so). Damit kann
// performance_test --communication ZeroDDS Latenz/Throughput-Bench
// gegen die Reference-Vendoren (CycloneDDS, FastRTPS) laufen lassen.
//
// Build-Erwartung: Apex 'performance_test' Repo macht aus jedem
// Plugin eine separate ament_cmake-package und linkt sie als
// shared-lib. Plugin-Auswahl per `-DPERFORMANCE_TEST_PLUGIN=ZERODDS`.

#pragma once

#include <cstddef>
#include <cstdint>
#include <memory>
#include <string>
#include <vector>

#include <zerodds.h>  // C-API aus crates/dds-c-api

namespace performance_test::plugins::zerodds {

/// RAII-Wrapper um einen ZeroDDS-DataWriter, hooked ans Apex-Plugin.
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

  /// Publisht einen Sample-Frame (raw bytes, von Apex pre-serialized).
  void publish(const uint8_t *payload, std::size_t len);

  /// Wartet bis `min_count` Subscriber matched sind (Apex-Sync-Step).
  bool wait_for_matched(int min_count, std::uint64_t timeout_ms);

private:
  zerodds_ZeroDdsWriter *writer_;
};

/// RAII-Wrapper um einen ZeroDDS-DataReader.
/// Apex' Subscriber-Interface: `take()` returnt vector<uint8_t> oder leer.
class ZeroDdsSubscriber {
public:
  ZeroDdsSubscriber(zerodds_ZeroDdsRuntime *runtime,
                    const std::string &topic,
                    const std::string &type_name,
                    bool reliable);
  ~ZeroDdsSubscriber();
  ZeroDdsSubscriber(const ZeroDdsSubscriber &) = delete;
  ZeroDdsSubscriber &operator=(const ZeroDdsSubscriber &) = delete;

  /// Take ein Sample. Returnt leeren vector wenn nichts da war.
  std::vector<uint8_t> take();

  bool wait_for_matched(int min_count, std::uint64_t timeout_ms);

private:
  zerodds_ZeroDdsReader *reader_;
};

/// Communicator-Factory — Apex' performance_test instantiiert pro
/// Bench-Run einen Communicator, der dann Pub/Sub-Pairs herausgibt.
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
  /// `min_count` Remote-Participants entdeckt hat. Sonst wiren
  /// Endpoints mit leerer Locator-Liste und Daten kommen nicht an.
  bool wait_for_peers(int min_count, std::uint64_t timeout_ms);

  static const char *plugin_name() { return "ZeroDDS"; }
  static const char *plugin_version();

private:
  zerodds_ZeroDdsRuntime *runtime_;
};

}  // namespace performance_test::plugins::zerodds

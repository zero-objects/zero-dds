// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// An in-process binding of the entity classes `dds/rpc.hpp` leaves
// abstract, for the tests of the C++ binding of DDS-RPC 1.0 §7.11.1: a bus
// of topics in memory, writers that store a sample and call the readers'
// callbacks as a receive thread would, and a participant that records what
// was created through it. What is measured is the runtime and the
// generated code; no network is involved. Thread-safe, so a test can serve
// from one thread and call from another.

#ifndef ZERODDS_RPC_TESTS_LOOPBACK_HPP
#define ZERODDS_RPC_TESTS_LOOPBACK_HPP

#include <dds/rpc.hpp>

#include <algorithm>
#include <condition_variable>
#include <cstdint>
#include <functional>
#include <map>
#include <memory>
#include <mutex>
#include <string>
#include <utility>
#include <vector>

namespace {

using dds::rpc::Octets;

// -- the in-process binding of the three abstract entities -------------

class LoopbackReader;
class LoopbackWriter;

/// One topic: the samples written on it, in order.
struct Topic {
  std::vector<std::pair<Octets, dds::SampleInfo>> samples;
  std::size_t writers = 0;
  std::size_t readers = 0;
  /// The data callbacks of the readers of the topic, by reader. A write
  /// calls them after it has stored the sample, on the writer's thread,
  /// which is what a binding with a receive thread does on that thread.
  std::vector<std::pair<const void*, std::function<void()>>> callbacks;
  /// The readers of the topic, for a bus that gives each its own queue.
  std::vector<LoopbackReader*> reader_objects;
  /// The writers of the topic, for the discovery extension.
  std::vector<LoopbackWriter*> writer_objects;
  /// Every sample ever written on the topic, taken or not: what a test
  /// looks at to see the octets that went out.
  std::vector<Octets> history;
  /// The same samples with the info they were written with. A requester
  /// takes its replies when they arrive, so the queue is already empty when
  /// a test looks at the reply topic; this is what went out.
  std::vector<std::pair<Octets, dds::SampleInfo>> written;
};

class Bus {
 public:
  /// One lock for the whole bus: a test that serves from one thread and
  /// calls from another shares it. Recursive, because a callback that runs
  /// inside a write writes again.
  mutable std::recursive_mutex lock;

  Topic& topic(const std::string& name) {
    std::lock_guard<std::recursive_mutex> guard(lock);
    return topics_[name];
  }

  /// Tell the readers of `name` that samples are waiting. The callbacks run
  /// with the lock released: they take and write. A reader that removes its
  /// callback waits for one that runs (`wait_until_idle`).
  void notify(const std::string& name) {
    std::vector<std::pair<const void*, std::function<void()>>> callbacks;
    {
      std::lock_guard<std::recursive_mutex> guard(lock);
      callbacks = topic(name).callbacks;
      for (const auto& entry : callbacks) {
        ++running_[entry.first];
      }
    }
    for (const auto& entry : callbacks) {
      const void* previous = inside;
      inside = entry.first;
      entry.second();
      inside = previous;
      std::lock_guard<std::recursive_mutex> guard(lock);
      --running_[entry.first];
      idle_.notify_all();
    }
  }

  /// Store a sample for the readers of `name`: in the topic's queue, or in
  /// each reader's. With `readers`, a bus that gives each reader its own queue
  /// stores it in the queues of the readers named there only; a shared queue
  /// cannot address anybody and holds it for whoever takes it first.
  void deliver(const std::string& name, const Octets& sample, const dds::SampleInfo& info,
               const std::vector<dds::rpc::EndpointGuid>* readers = nullptr);

  /// Wait until no callback of `reader` runs, unless it is the caller's own.
  void wait_until_idle(const void* reader) {
    std::unique_lock<std::recursive_mutex> guard(lock);
    if (inside == reader) {
      return;
    }
    idle_.wait(guard, [this, reader]() { return running_[reader] == 0; });
  }

  /// Whether readers report the identity of a sample and its related
  /// identity, as the ZeroDDS binding does through
  /// `zerodds_reader_take_wire`. Off, the binding is the one of the Basic
  /// tests, where correlation must come from the headers (§7.8.1).
  bool expose_identity = false;

  /// Whether each reader has a queue of its own, as a DDS reader has. Off,
  /// the readers of a topic share one, and the first to take a sample has
  /// it: enough for tests that run one reader per topic and a convenience
  /// for those that want a second reader to consume what the first ignored.
  bool per_reader_queues = false;

  /// Whether the endpoints announce and read the DDS-RPC discovery
  /// extension (§7.6.2.1), as the ZeroDDS binding does. Off, they match by
  /// their count, as a binding without it must.
  bool rpc_discovery = false;

  /// The next GUID tag of an endpoint.
  std::uint8_t next_tag() {
    std::lock_guard<std::recursive_mutex> guard(lock);
    return static_cast<std::uint8_t>(tag_++);
  }

 private:
  std::uint8_t tag_ = 0xA0;
  static thread_local const void* inside;
  std::map<std::string, Topic> topics_;
  std::map<const void*, int> running_;
  std::condition_variable_any idle_;
};

inline thread_local const void* Bus::inside = nullptr;

/// What the participant records about the endpoints created through it,
/// so the test can hold the runtime against the clauses that name them.
struct Created {
  std::map<std::string, std::string> types;
  std::map<std::string, dds::rpc::EndpointQos> qos;
  /// The QoS profiles the participant resolves, by URL (§7.10.1).
  std::map<std::string, dds::rpc::EndpointQos> profiles;
};

/// The immutable policies of §7.10.2 and DDS 1.4 §2.2.3: a change is
/// refused. DEADLINE and LIFESPAN may change.
void check_mutable(const dds::rpc::EndpointQos& before, const dds::rpc::EndpointQos& after) {
  if (before.reliability != after.reliability || before.durability != after.durability ||
      before.history != after.history) {
    throw dds::rpc::ImmutablePolicyError("RELIABILITY, DURABILITY and HISTORY are immutable");
  }
}

class LoopbackWriter final : public dds::rpc::DataWriter {
 public:
  LoopbackWriter(Bus& bus, std::string topic, std::uint8_t tag, Created& created)
      : bus_(bus), topic_(std::move(topic)), created_(created) {
    for (std::size_t at = 0; at < 16; ++at) {
      guid_[at] = static_cast<std::uint8_t>(tag + at);
    }
    std::lock_guard<std::recursive_mutex> guard(bus_.lock);
    ++bus_.topic(topic_).writers;
    bus_.topic(topic_).writer_objects.push_back(this);
  }

  ~LoopbackWriter() override {
    std::lock_guard<std::recursive_mutex> guard(bus_.lock);
    --bus_.topic(topic_).writers;
    auto& objects = bus_.topic(topic_).writer_objects;
    objects.erase(std::remove(objects.begin(), objects.end(), this), objects.end());
  }

  const std::uint8_t* guid() const { return guid_; }
  const dds::rpc::RpcDiscovery& announced() const { return announced_; }

  bool supports_rpc_discovery() const override { return bus_.rpc_discovery; }

  void announce_rpc_discovery(const dds::rpc::RpcDiscovery& extension) override {
    if (!bus_.rpc_discovery) {
      throw dds::rpc::UnsupportedError("no discovery extension on this bus");
    }
    std::lock_guard<std::recursive_mutex> guard(bus_.lock);
    announced_ = extension;
  }

  std::vector<dds::rpc::RpcPeer> matched_rpc_readers() const override;

  bool service_matching_complete(const dds::rpc::DataReader& reply_reader,
                                 const std::string& instance_name,
                                 bool accept_unannounced) const override;

  dds::SampleIdentity write(const Octets& sample) override {
    dds::SampleIdentity identity;
    {
      std::lock_guard<std::recursive_mutex> guard(bus_.lock);
      identity = next_identity();
      ++sequence_;
      dds::SampleInfo info;
      info.identity = identity;
      bus_.deliver(topic_, sample, info);
    }
    bus_.notify(topic_);
    return identity;
  }

  dds::SampleIdentity write_to(const Octets& sample,
                               const std::vector<dds::rpc::EndpointGuid>& readers) override {
    dds::SampleIdentity identity;
    {
      std::lock_guard<std::recursive_mutex> guard(bus_.lock);
      identity = next_identity();
      ++sequence_;
      dds::SampleInfo info;
      info.identity = identity;
      bus_.deliver(topic_, sample, info, &readers);
    }
    bus_.notify(topic_);
    return identity;
  }

  dds::SampleIdentity write_related(const Octets& sample,
                                    const dds::SampleIdentity& related) override {
    dds::SampleIdentity identity;
    {
      std::lock_guard<std::recursive_mutex> guard(bus_.lock);
      identity = next_identity();
      ++sequence_;
      dds::SampleInfo info;
      info.identity = identity;
      info.related_identity = related;
      info.has_related_identity = true;
      bus_.deliver(topic_, sample, info);
    }
    bus_.notify(topic_);
    return identity;
  }

  dds::SampleIdentity next_identity() const override {
    dds::SampleIdentity identity;
    for (std::size_t at = 0; at < 16; ++at) {
      identity.writer_guid[at] = guid_[at];
    }
    identity.sequence_number.low = static_cast<std::uint32_t>(sequence_ + 1);
    return identity;
  }

  std::size_t matched_count() const override {
    std::lock_guard<std::recursive_mutex> guard(bus_.lock);
    return bus_.topic(topic_).readers;
  }

  void set_qos(const dds::rpc::EndpointQos& qos) override {
    check_mutable(created_.qos.at(topic_), qos);
    created_.qos[topic_] = qos;
  }

 private:
  Bus& bus_;
  std::string topic_;
  Created& created_;
  std::uint8_t guid_[16] = {};
  std::uint64_t sequence_ = 0;
  dds::rpc::RpcDiscovery announced_;
};

class LoopbackReader final : public dds::rpc::DataReader {
 public:
  LoopbackReader(Bus& bus, std::string topic, Created& created)
      : bus_(bus), topic_(std::move(topic)), created_(created) {
    const std::uint8_t tag = bus_.next_tag();
    for (std::size_t at = 0; at < 16; ++at) {
      guid_[at] = static_cast<std::uint8_t>(tag + at);
    }
    std::lock_guard<std::recursive_mutex> guard(bus_.lock);
    ++bus_.topic(topic_).readers;
    bus_.topic(topic_).reader_objects.push_back(this);
  }

  const std::uint8_t* key() const { return guid_; }
  const dds::rpc::RpcDiscovery& announced() const { return announced_; }

  bool supports_rpc_discovery() const override { return bus_.rpc_discovery; }

  void guid(std::uint8_t out[16]) const override { std::copy(guid_, guid_ + 16, out); }

  void announce_rpc_discovery(const dds::rpc::RpcDiscovery& extension) override {
    if (!bus_.rpc_discovery) {
      throw dds::rpc::UnsupportedError("no discovery extension on this bus");
    }
    std::lock_guard<std::recursive_mutex> guard(bus_.lock);
    announced_ = extension;
  }

  std::vector<dds::rpc::RpcPeer> matched_rpc_writers() const override;

  bool client_matching_complete(const dds::rpc::DataWriter& reply_writer,
                                const std::uint8_t client_writer_guid[16],
                                bool accept_unannounced) const override;

  /// A sample for this reader's own queue.
  void push(const Octets& sample, const dds::SampleInfo& info) {
    mine_.emplace_back(sample, info);
  }

  std::vector<std::pair<Octets, dds::SampleInfo>> take() override {
    std::lock_guard<std::recursive_mutex> guard(bus_.lock);
    auto& held = bus_.per_reader_queues ? mine_ : bus_.topic(topic_).samples;
    std::vector<std::pair<Octets, dds::SampleInfo>> out;
    out.swap(held);
    // The binding surfaces no sample identity — the ZeroDDS path, where
    // the reader has no accessor for PID_RELATED_SAMPLE_IDENTITY and the
    // writer's SampleIdentity is not on `SampleInfo`. Correlation must
    // then come from the explicit request-id of §7.8.1's Basic profile;
    // a runtime that reads identity off `SampleInfo` fails here.
    if (!bus_.expose_identity) {
      for (auto& sample : out) {
        sample.second.identity = dds::SampleIdentity{};
        sample.second.related_identity = dds::SampleIdentity{};
        sample.second.has_related_identity = false;
      }
    }
    return out;
  }

  std::size_t matched_count() const override {
    std::lock_guard<std::recursive_mutex> guard(bus_.lock);
    return bus_.topic(topic_).writers;
  }

  void set_qos(const dds::rpc::EndpointQos& qos) override {
    check_mutable(created_.qos.at(topic_), qos);
    created_.qos[topic_] = qos;
  }

  void set_data_callback(std::function<void()> callback) override {
    bool waiting = false;
    {
      std::lock_guard<std::recursive_mutex> guard(bus_.lock);
      auto& callbacks = bus_.topic(topic_).callbacks;
      callbacks.erase(std::remove_if(callbacks.begin(), callbacks.end(),
                                     [this](const auto& entry) { return entry.first == this; }),
                      callbacks.end());
      if (callback) {
        callbacks.emplace_back(this, callback);
        waiting = !(bus_.per_reader_queues ? mine_ : bus_.topic(topic_).samples).empty();
      }
    }
    if (!callback) {
      bus_.wait_until_idle(this);
      return;
    }
    // Samples that are already waiting are called for.
    if (waiting) {
      callback();
    }
  }

  ~LoopbackReader() override {
    set_data_callback(nullptr);
    std::lock_guard<std::recursive_mutex> guard(bus_.lock);
    --bus_.topic(topic_).readers;
    auto& objects = bus_.topic(topic_).reader_objects;
    objects.erase(std::remove(objects.begin(), objects.end(), this), objects.end());
  }

 private:
  Bus& bus_;
  std::string topic_;
  Created& created_;
  std::vector<std::pair<Octets, dds::SampleInfo>> mine_;
  std::uint8_t guid_[16] = {};
  dds::rpc::RpcDiscovery announced_;
};

inline std::vector<dds::rpc::RpcPeer> LoopbackWriter::matched_rpc_readers() const {
  std::lock_guard<std::recursive_mutex> guard(bus_.lock);
  std::vector<dds::rpc::RpcPeer> out;
  for (const LoopbackReader* reader : bus_.topic(topic_).reader_objects) {
    dds::rpc::RpcPeer peer;
    std::copy(reader->key(), reader->key() + 16, peer.key);
    peer.announced = reader->announced();
    out.push_back(peer);
  }
  return out;
}

inline std::vector<dds::rpc::RpcPeer> LoopbackReader::matched_rpc_writers() const {
  std::lock_guard<std::recursive_mutex> guard(bus_.lock);
  std::vector<dds::rpc::RpcPeer> out;
  for (const LoopbackWriter* writer : bus_.topic(topic_).writer_objects) {
    dds::rpc::RpcPeer peer;
    std::copy(writer->guid(), writer->guid() + 16, peer.key);
    peer.announced = writer->announced();
    out.push_back(peer);
  }
  return out;
}

inline bool LoopbackWriter::service_matching_complete(const dds::rpc::DataReader& reply_reader,
                                                      const std::string& instance_name,
                                                      bool accept_unannounced) const {
  return dds::rpc::service_matching_complete(matched_rpc_readers(), reply_reader.matched_rpc_writers(),
                                             instance_name, accept_unannounced);
}

inline bool LoopbackReader::client_matching_complete(const dds::rpc::DataWriter& reply_writer,
                                                     const std::uint8_t client_writer_guid[16],
                                                     bool accept_unannounced) const {
  for (const dds::rpc::RpcPeer& client : matched_rpc_writers()) {
    if (std::equal(client.key, client.key + 16, client_writer_guid)) {
      return dds::rpc::client_matching_complete(client, reply_writer.matched_rpc_readers(),
                                                accept_unannounced);
    }
  }
  return accept_unannounced;
}

inline void Bus::deliver(const std::string& name, const Octets& sample,
                         const dds::SampleInfo& info,
                         const std::vector<dds::rpc::EndpointGuid>* readers) {
  std::lock_guard<std::recursive_mutex> guard(lock);
  Topic& at = topic(name);
  at.history.push_back(sample);
  at.written.emplace_back(sample, info);
  if (per_reader_queues) {
    for (LoopbackReader* reader : at.reader_objects) {
      if (readers != nullptr &&
          std::none_of(readers->begin(), readers->end(), [reader](const dds::rpc::EndpointGuid& guid) {
            return std::equal(guid.begin(), guid.end(), reader->key());
          })) {
        continue;
      }
      reader->push(sample, info);
    }
  } else {
    at.samples.emplace_back(sample, info);
  }
}

class LoopbackPublisher final : public dds::rpc::Publisher {
 public:
  LoopbackPublisher(Bus& bus, Created& created, int& tag)
      : bus_(bus), created_(created), tag_(tag) {}

  std::shared_ptr<dds::rpc::DataWriter> create_writer(
      const std::string& topic, const std::string& type_name,
      const dds::rpc::EndpointQos& qos) override {
    // The type name is recorded so the test can hold the generator's
    // `${interfaceName}_Request`/`_Reply` against it (§7.5.1.1.6/7).
    created_.types[topic] = type_name;
    created_.qos[topic] = qos;
    return std::make_shared<LoopbackWriter>(bus_, topic, static_cast<std::uint8_t>(++tag_),
                                            created_);
  }

 private:
  Bus& bus_;
  Created& created_;
  int& tag_;
};

class LoopbackSubscriber final : public dds::rpc::Subscriber {
 public:
  LoopbackSubscriber(Bus& bus, Created& created) : bus_(bus), created_(created) {}

  std::shared_ptr<dds::rpc::DataReader> create_reader(
      const std::string& topic, const std::string& type_name,
      const dds::rpc::EndpointQos& qos) override {
    created_.types[topic] = type_name;
    created_.qos[topic] = qos;
    return std::make_shared<LoopbackReader>(bus_, topic, created_);
  }

 private:
  Bus& bus_;
  Created& created_;
};

class LoopbackParticipant final : public dds::rpc::DomainParticipant {
 public:
  explicit LoopbackParticipant(Bus& bus)
      : publisher_(std::make_shared<LoopbackPublisher>(bus, created_, tag_)),
        subscriber_(std::make_shared<LoopbackSubscriber>(bus, created_)) {}

  std::shared_ptr<dds::rpc::Publisher> publisher() override { return publisher_; }
  std::shared_ptr<dds::rpc::Subscriber> subscriber() override { return subscriber_; }

  const std::string& type_name_of(const std::string& topic) const {
    return created_.types.at(topic);
  }
  const dds::rpc::EndpointQos& qos_of(const std::string& topic) const {
    return created_.qos.at(topic);
  }

  dds::rpc::EndpointQos load_qos_profile(const std::string& url) override {
    const auto found = created_.profiles.find(url);
    if (found == created_.profiles.end()) {
      throw dds::rpc::InvalidArgumentError("no such QoS profile: " + url);
    }
    return found->second;
  }
  void define_profile(const std::string& url, const dds::rpc::EndpointQos& qos) {
    created_.profiles[url] = qos;
  }

 private:
  Created created_;
  int tag_ = 0x10;
  std::shared_ptr<dds::rpc::Publisher> publisher_;
  std::shared_ptr<dds::rpc::Subscriber> subscriber_;
};


}  // namespace

#endif  // ZERODDS_RPC_TESTS_LOOPBACK_HPP

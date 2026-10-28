// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/rpc/zerodds.hpp — the ZeroDDS binding of the three entity classes
// `dds/rpc.hpp` leaves abstract.
//
// §7.11.1.4.23 says `dds_entity_traits` "abstracts over the DDS entity
// types ... irrespective of the DDS C++ language bindings", so the
// request-reply binding is written once and the entities are supplied.
// This is the supply for the ZeroDDS C API: one participant over
// `zerodds_ZeroDdsRuntime`, one writer over `zerodds_writer_*` and one
// reader over `zerodds_reader_*`.
//
// A separate header because `dds/rpc.hpp` has to compile with no DDS API
// present at all — a test binding is a dozen lines, and that is what
// makes the runtime measurable without a network.

#ifndef ZERODDS_DDS_RPC_ZERODDS_HPP
#define ZERODDS_DDS_RPC_ZERODDS_HPP

#include <dds/rpc.hpp>

#include <zerodds.h>

#include <algorithm>
#include <cstdint>
#include <cstring>
#include <condition_variable>
#include <functional>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <string>
#include <thread>
#include <utility>
#include <vector>

namespace dds {
namespace rpc {
namespace zerodds {

/// The extensibility of every request and reply sample this binding writes,
/// in the C ABI's own numbering (`0` FINAL, `1` APPENDABLE, `2` MUTABLE —
/// see `zerodds_writer_set_wire_extensibility`).
///
/// FINAL, and it is a reading of `dds/rpc.hpp` and not a preference: the two
/// headers of §7.5.1.1.1 are plain aggregates, and `encode_request_header` /
/// `encode_reply_header` concatenate one with `dds_type_traits<T>::serialize`
/// with no outer delimiter and no per-member header. That is a FINAL body,
/// the same reading `crates/rpc/src/common_types.rs` states for the Rust side
/// ("Encoding: XCDR2 with Final extensibility") and the two agree octet for
/// octet in `crates/rpc/tests/cpp_rpc_runtime.rs`.
///
/// Stated here rather than left to the C ABI's default because the
/// encapsulation identifier has to name the framing this file produces, not
/// the one the ABI falls back to (XTypes 1.3 §7.6.3.1.2 Table 59 / DDSI-RTPS
/// 2.5 §10.5): an APPENDABLE body opens with a DHEADER and a MUTABLE one with
/// an EMHEADER per member, and a reader that believes the header would read
/// that delimiter as the first member. Measured against OpenDDS 3.35.0 on
/// this binding's own writers, 24 samples per topic: a FINAL body under
/// `0x0007` arrives 24 of 24 with its members intact, a delimited one under
/// the same identifier arrives 0 of 24 and OpenDDS says why —
/// "to_encoding_i: expected appendable extensibility, but got XCDR2 Little
/// Endian Plain". If the codec above ever grows an outer delimiter, this is
/// the line that has to move with it.
inline constexpr int kWireExtensibility = 0;

/// The extensibility of the samples of an Enhanced topic type: MUTABLE (`2`), the reading of
/// §7.5.1.2.1.1 for a `@Choice`, framed as PL_CDR2.
inline constexpr int kEnhancedWireExtensibility = 2;

/// Raise the local exception (§7.11.1.3, Figure 7.6) a C API status
/// stands for. A failure of the middleware underneath is never a remote
/// exception: no service refused anything.
[[noreturn]] inline void raise_local(int status, const std::string& what) {
  const std::string message = what + " returned " + std::to_string(status);
  switch (status) {
    case -2:  // BadHandle
      throw NullReferenceError(message);
    case -4:  // Timeout
      throw TimeoutError(message);
    case -5:  // PreconditionNotMet
    case -9:  // NotEnabled
      throw PreconditionNotMetError(message);
    case -6:  // BadParameter
    case -3:  // InvalidUtf8
      throw InvalidArgumentError(message);
    case -8:  // OutOfResources
      throw OutOfResourcesError(message);
    case -10:  // ImmutablePolicy
      throw ImmutablePolicyError(message);
    case -11:  // InconsistentPolicy
      throw InconsistentPolicyError(message);
    case -12:  // AlreadyDeleted
      throw AlreadyClosedError(message);
    case -13:  // Unsupported
      throw UnsupportedError(message);
    case -14:  // IllegalOperation
      throw IllegalOperationError(message);
    default:
      throw UnspecifiedError(message);
  }
}

/// The C API's QoS for `qos`. §7.10.2 gives the defaults; the history
/// depth is at least 1 under KEEP_LAST, as the C API requires.
inline zerodds_ZeroDdsEndpointQos to_c_qos(const EndpointQos& qos) {
  zerodds_ZeroDdsEndpointQos out = zerodds_endpoint_qos_default();
  out.reliable = qos.reliability == ReliabilityKind::RELIABLE ? 1 : 0;
  out.transient_local = qos.durability == DurabilityKind::TRANSIENT_LOCAL ? 1 : 0;
  out.history_keep_all = qos.history == HistoryKind::KEEP_ALL ? 1 : 0;
  out.history_depth = qos.history_depth == 0 ? 1u : qos.history_depth;
  out.deadline_ns = qos.deadline_ns;
  out.lifespan_ns = qos.lifespan_ns;
  return out;
}

/// The peers the C API reported, as the runtime's `RpcPeer`.
inline std::vector<RpcPeer> to_peers(const zerodds_ZeroDdsRpcPeer* raw, std::size_t count) {
  std::vector<RpcPeer> out;
  out.reserve(count);
  for (std::size_t at = 0; at < count; ++at) {
    const zerodds_ZeroDdsRpcPeer& peer = raw[at];
    RpcPeer item;
    std::memcpy(item.key, peer.key, sizeof(item.key));
    item.announced.has_related = peer.has_related != 0;
    std::memcpy(item.announced.related, peer.related, sizeof(item.announced.related));
    item.announced.has_instance_name = peer.has_instance_name != 0;
    if (item.announced.has_instance_name) {
      item.announced.instance_name = peer.instance_name;
      item.announced.name_truncated = peer.name_truncated != 0;
    }
    item.announced.aliases_truncated = peer.aliases_truncated != 0;
    if (peer.alias_count > 0) {
      // The aliases come joined by '\n'.
      const std::string joined(peer.aliases);
      std::size_t from = 0;
      while (from <= joined.size()) {
        const std::size_t to = joined.find('\n', from);
        item.announced.aliases.push_back(joined.substr(from, to - from));
        if (to == std::string::npos) {
          break;
        }
        from = to + 1;
      }
    }
    out.push_back(std::move(item));
  }
  return out;
}

/// What an endpoint announced to the C API, as its arguments.
template <typename Entity, typename Function>
void announce_extension(Entity* entity, Function function, const RpcDiscovery& extension,
                        const char* what) {
  std::vector<const char*> aliases;
  for (const std::string& alias : extension.aliases) {
    aliases.push_back(alias.c_str());
  }
  const int status = function(entity,
                              extension.has_instance_name ? extension.instance_name.c_str() : nullptr,
                              extension.has_related ? extension.related : nullptr,
                              aliases.empty() ? nullptr : aliases.data(), aliases.size());
  if (status != 0) {
    raise_local(status, what);
  }
}

/// The peers `function` reports for `entity`, however many there are.
template <typename Entity, typename Function>
std::vector<RpcPeer> read_peers(Entity* entity, Function function, const char* what) {
  std::vector<zerodds_ZeroDdsRpcPeer> raw(16);
  std::uintptr_t count = 0;
  int status = function(entity, raw.data(), raw.size(), &count);
  // -8: OutOfResources, the total is in `count` (§2.8 of the C API).
  if ((status == 0 || status == -8) && count > raw.size()) {
    raw.resize(count);
    status = function(entity, raw.data(), raw.size(), &count);
  }
  if (status != 0) {
    raise_local(status, what);
  }
  return to_peers(raw.data(), std::min<std::size_t>(count, raw.size()));
}

/// A `dds::rpc::DataWriter` over `zerodds_writer_*`.
class Writer final : public DataWriter {
 public:
  Writer(zerodds_ZeroDdsWriter* writer, bool owned, const EndpointQos& qos = EndpointQos{})
      : writer_(writer), owned_(owned), qos_(qos) {
    if (writer_ == nullptr) {
      throw OutOfResourcesError("zerodds_writer_create returned null");
    }
    // The identity §7.2.3 correlates by is the writer's own GUID and the
    // sequence number the writer reports.
    if (zerodds_writer_guid(writer_, guid_) != 0) {
      throw OutOfResourcesError("zerodds_writer_guid failed");
    }
  }

  ~Writer() override {
    if (owned_ && writer_ != nullptr) {
      zerodds_writer_destroy(writer_);
    }
  }

  Writer(const Writer&) = delete;
  Writer& operator=(const Writer&) = delete;

  dds::SampleIdentity write(const Octets& sample) override {
    // The identity is asked for and the sample written under one lock, so
    // that no other write of this writer takes the number in between.
    std::lock_guard<std::mutex> guard(write_lock_);
    // The identity this sample is about to carry, from the writer itself
    // (`zerodds_writer_next_sequence_number`): a request header filled from
    // it before the call names the number the write stamps (§7.5.1.1.1,
    // §7.8.1).
    const dds::SampleIdentity identity = next_identity();
    const int status = zerodds_writer_write(writer_, sample.data(), sample.size());
    if (status != 0) {
      raise_local(status, "zerodds_writer_write");
    }
    return identity;
  }

  /// §7.8.2: the sample goes out with the identity of the request it
  /// answers in `PID_RELATED_SAMPLE_IDENTITY`, through
  /// `zerodds_writer_write_related`. The sequence number the middleware
  /// stamped comes back and is the identity of this sample.
  dds::SampleIdentity write_related(const Octets& sample,
                                    const dds::SampleIdentity& related) override {
    std::lock_guard<std::mutex> guard(write_lock_);
    std::int64_t stamped = 0;
    const std::uint64_t related_sn = (static_cast<std::uint64_t>(
                                          static_cast<std::uint32_t>(related.sequence_number.high))
                                      << 32) |
                                     related.sequence_number.low;
    const int status = zerodds_writer_write_related(writer_, sample.data(), sample.size(),
                                                    related.writer_guid, related_sn, &stamped);
    if (status != 0) {
      raise_local(status, "zerodds_writer_write_related");
    }
    return identity_of(static_cast<std::uint64_t>(stamped));
  }

  /// §7.11.1.4.3: the sample goes to the readers named and no other, through
  /// `zerodds_writer_write_to_readers`. The sequence number the middleware
  /// stamped comes back and is the identity of the sample.
  dds::SampleIdentity write_to(const Octets& sample,
                               const std::vector<EndpointGuid>& readers) override {
    std::lock_guard<std::mutex> guard(write_lock_);
    std::vector<std::uint8_t> flat;
    flat.reserve(readers.size() * 16);
    for (const EndpointGuid& reader : readers) {
      flat.insert(flat.end(), reader.begin(), reader.end());
    }
    std::int64_t stamped = 0;
    const int status = zerodds_writer_write_to_readers(
        writer_, sample.data(), sample.size(), flat.empty() ? nullptr : flat.data(),
        readers.size(), &stamped);
    if (status != 0) {
      raise_local(status, "zerodds_writer_write_to_readers");
    }
    return identity_of(static_cast<std::uint64_t>(stamped));
  }

  /// The identity the next `write` will stamp, as the writer reports it.
  dds::SampleIdentity next_identity() const override {
    std::int64_t next = 0;
    const int status = zerodds_writer_next_sequence_number(writer_, &next);
    if (status != 0) {
      raise_local(status, "zerodds_writer_next_sequence_number");
    }
    return identity_of(static_cast<std::uint64_t>(next));
  }

  std::size_t matched_count() const override { return zerodds_writer_matched_count(writer_); }

  /// The C API's handle, for the algorithms that need two endpoints.
  zerodds_ZeroDdsWriter* handle() const { return writer_; }

  bool supports_rpc_discovery() const override { return true; }

  /// §7.6.2.1.1, through `zerodds_writer_set_rpc_discovery`.
  void announce_rpc_discovery(const RpcDiscovery& extension) override {
    announce_extension(writer_, zerodds_writer_set_rpc_discovery, extension,
                       "zerodds_writer_set_rpc_discovery");
  }

  std::vector<RpcPeer> matched_rpc_readers() const override {
    return read_peers(writer_, zerodds_writer_matched_rpc_readers,
                      "zerodds_writer_matched_rpc_readers");
  }

  /// §7.6.2.2.2, Figure 7.4, through `zerodds_rpc_service_matching_complete`.
  bool service_matching_complete(const DataReader& reply_reader, const std::string& instance_name,
                                 bool accept_unannounced) const override;

  /// §7.10.2. DDS 1.4 §2.2.3 makes only DEADLINE and LIFESPAN of the RPC QoS
  /// changeable once the writer is enabled; they are applied. RELIABILITY
  /// (with `max_blocking_time`), DURABILITY, HISTORY and RESOURCE_LIMITS the
  /// C API refuses to change (`ImmutablePolicyError`); a change of HISTORY
  /// is refused here first, without asking it.
  void set_qos(const EndpointQos& qos) override {
    if (qos.history != qos_.history ||
        (qos.history == HistoryKind::KEEP_LAST && qos.history_depth != qos_.history_depth)) {
      throw ImmutablePolicyError("HISTORY cannot change once the writer exists");
    }
    const zerodds_ZeroDdsEndpointQos c_qos = to_c_qos(qos);
    const int status = zerodds_writer_set_qos(writer_, &c_qos);
    if (status != 0) {
      raise_local(status, "zerodds_writer_set_qos");
    }
    qos_ = qos;
  }

 private:
  zerodds_ZeroDdsWriter* writer_ = nullptr;
  bool owned_ = false;
  EndpointQos qos_;
  dds::SampleIdentity identity_of(std::uint64_t sequence_number) const {
    dds::SampleIdentity identity;
    for (std::size_t at = 0; at < 16; ++at) {
      identity.writer_guid[at] = guid_[at];
    }
    identity.sequence_number.high = static_cast<std::int32_t>(sequence_number >> 32);
    identity.sequence_number.low = static_cast<std::uint32_t>(sequence_number & 0xFFFFFFFFu);
    return identity;
  }

  std::uint8_t guid_[16] = {};
  std::mutex write_lock_;
};

/// A `dds::rpc::DataReader` over `zerodds_reader_*`.
class Reader final : public DataReader {
 public:
  Reader(zerodds_ZeroDdsReader* reader, bool owned, const EndpointQos& qos = EndpointQos{})
      : reader_(reader), owned_(owned), qos_(qos) {
    if (reader_ == nullptr) {
      throw OutOfResourcesError("zerodds_reader_create returned null");
    }
  }

  ~Reader() override {
    stop_worker();
    if (owned_ && reader_ != nullptr) {
      zerodds_reader_destroy(reader_);
    }
  }

  Reader(const Reader&) = delete;
  Reader& operator=(const Reader&) = delete;

  std::vector<std::pair<Octets, dds::SampleInfo>> take() override {
    std::vector<std::pair<Octets, dds::SampleInfo>> out;
    for (;;) {
      std::uint8_t* buffer = nullptr;
      std::uintptr_t length = 0;
      std::uint8_t writer_guid[16] = {};
      std::int64_t sequence_number = -1;
      std::uint8_t representation = 0;
      std::uint8_t has_related = 0;
      std::uint8_t related_guid[16] = {};
      std::uint64_t related_sn = 0;
      // §7.8.2: the identity of the sample and the related identity of
      // its inline QoS come with the payload.
      const int status = zerodds_reader_take_wire(reader_, &buffer, &length, &representation, nullptr,
                                                  writer_guid, &sequence_number, &has_related,
                                                  related_guid, &related_sn);
      if (status != 0) {
        raise_local(status, "zerodds_reader_take_wire");
      }
      if (buffer == nullptr || length == 0) {
        if (buffer != nullptr) {
          zerodds_buffer_free(buffer, length);
        }
        return out;
      }
      Octets octets(buffer, buffer + length);
      zerodds_buffer_free(buffer, length);
      dds::SampleInfo info;
      info.valid_data = true;
      info.has_encoding = true;
      info.encoding = representation == 0 ? 0 : 1;
      std::memcpy(info.identity.writer_guid, writer_guid, 16);
      const std::uint64_t sn = static_cast<std::uint64_t>(sequence_number);
      info.identity.sequence_number.high = static_cast<std::int32_t>(sn >> 32);
      info.identity.sequence_number.low = static_cast<std::uint32_t>(sn & 0xFFFFFFFFu);
      if (has_related != 0) {
        info.has_related_identity = true;
        std::memcpy(info.related_identity.writer_guid, related_guid, 16);
        info.related_identity.sequence_number.high = static_cast<std::int32_t>(related_sn >> 32);
        info.related_identity.sequence_number.low =
            static_cast<std::uint32_t>(related_sn & 0xFFFFFFFFu);
      }
      out.emplace_back(std::move(octets), info);
    }
  }

  std::size_t matched_count() const override { return zerodds_reader_matched_count(reader_); }

  /// The C API's handle, for the algorithms that need two endpoints.
  zerodds_ZeroDdsReader* handle() const { return reader_; }

  bool supports_rpc_discovery() const override { return true; }

  void guid(std::uint8_t out[16]) const override {
    const int status = zerodds_reader_guid(reader_, out);
    if (status != 0) {
      raise_local(status, "zerodds_reader_guid");
    }
  }

  /// §7.6.2.1.2, through `zerodds_reader_set_rpc_discovery`.
  void announce_rpc_discovery(const RpcDiscovery& extension) override {
    announce_extension(reader_, zerodds_reader_set_rpc_discovery, extension,
                       "zerodds_reader_set_rpc_discovery");
  }

  std::vector<RpcPeer> matched_rpc_writers() const override {
    return read_peers(reader_, zerodds_reader_matched_rpc_writers,
                      "zerodds_reader_matched_rpc_writers");
  }

  /// §7.6.2.2.1, Figure 7.3, through `zerodds_rpc_client_matching_complete`.
  bool client_matching_complete(const DataWriter& reply_writer,
                                const std::uint8_t client_writer_guid[16],
                                bool accept_unannounced) const override;

  /// §7.9.2.2, §7.11.1.4.7-.10: the callback runs when samples arrive.
  ///
  /// The C API's arrival callback runs on the runtime's receive thread and
  /// must not call ZeroDDS, while a listener takes a request and writes a
  /// reply. So the C callback only signals a thread of this reader, and that
  /// thread calls `callback`. The samples stay in the reader for `take()`.
  void set_data_callback(std::function<void()> callback) override {
    std::lock_guard<std::mutex> guard(callback_lock_);
    stop_worker();
    if (!callback) {
      return;
    }
    auto arrival = std::make_shared<Arrival>();
    arrival->action = std::move(callback);
    // Samples that arrived before the callback was set are called for.
    arrival->pending = true;
    auto* holder = new std::shared_ptr<Arrival>(arrival);
    const int status = zerodds_reader_set_arrival_callback(reader_, &Reader::signalled, holder,
                                                           &Reader::release);
    if (status != 0) {
      delete holder;
      raise_local(status, "zerodds_reader_set_arrival_callback");
    }
    arrival->thread = std::thread([arrival]() { arrival->run(); });
    arrival_ = std::move(arrival);
  }

  /// See [`Writer::set_qos`]; a reader applies DEADLINE (LIFESPAN is a
  /// writer policy) and the C API refuses the others.
  void set_qos(const EndpointQos& qos) override {
    if (qos.history != qos_.history ||
        (qos.history == HistoryKind::KEEP_LAST && qos.history_depth != qos_.history_depth)) {
      throw ImmutablePolicyError("HISTORY cannot change once the reader exists");
    }
    const zerodds_ZeroDdsEndpointQos c_qos = to_c_qos(qos);
    const int status = zerodds_reader_set_qos(reader_, &c_qos);
    if (status != 0) {
      raise_local(status, "zerodds_reader_set_qos");
    }
    qos_ = qos;
  }

 private:
  /// What the C callback and the thread of the reader share.
  struct Arrival {
    std::mutex lock;
    std::condition_variable wake;
    bool pending = false;
    bool stop = false;
    std::function<void()> action;
    std::thread thread;

    void signal() {
      std::lock_guard<std::mutex> guard(lock);
      pending = true;
      wake.notify_one();
    }

    void run() {
      for (;;) {
        {
          std::unique_lock<std::mutex> guard(lock);
          wake.wait(guard, [this]() { return pending || stop; });
          if (stop) {
            return;
          }
          pending = false;
        }
        action();
      }
    }
  };

  static void signalled(void* user_data) {
    (*static_cast<std::shared_ptr<Arrival>*>(user_data))->signal();
  }
  static void release(void* user_data) {
    delete static_cast<std::shared_ptr<Arrival>*>(user_data);
  }

  /// Remove the C callback, then stop the thread and wait for a call that
  /// runs. Called from the callback itself it cannot wait for itself: the
  /// thread is let go and ends when the call returns.
  void stop_worker() {
    if (!arrival_) {
      return;
    }
    (void)zerodds_reader_set_arrival_callback(reader_, nullptr, nullptr, nullptr);
    {
      std::lock_guard<std::mutex> guard(arrival_->lock);
      arrival_->stop = true;
      arrival_->wake.notify_all();
    }
    if (arrival_->thread.joinable()) {
      if (arrival_->thread.get_id() == std::this_thread::get_id()) {
        arrival_->thread.detach();
      } else {
        arrival_->thread.join();
      }
    }
    arrival_.reset();
  }

  zerodds_ZeroDdsReader* reader_ = nullptr;
  bool owned_ = false;
  EndpointQos qos_;
  std::mutex callback_lock_;
  std::shared_ptr<Arrival> arrival_;
};

inline bool Writer::service_matching_complete(const DataReader& reply_reader,
                                               const std::string& instance_name,
                                               bool accept_unannounced) const {
  const auto* reader = dynamic_cast<const Reader*>(&reply_reader);
  if (reader == nullptr) {
    throw UnsupportedError("the reply reader is not a ZeroDDS reader");
  }
  std::uint8_t complete = 0;
  const int status = zerodds_rpc_service_matching_complete(
      writer_, reader->handle(), instance_name.empty() ? nullptr : instance_name.c_str(),
      accept_unannounced ? 1 : 0, &complete);
  if (status != 0) {
    raise_local(status, "zerodds_rpc_service_matching_complete");
  }
  return complete != 0;
}

inline bool Reader::client_matching_complete(const DataWriter& reply_writer,
                                             const std::uint8_t client_writer_guid[16],
                                             bool accept_unannounced) const {
  const auto* writer = dynamic_cast<const Writer*>(&reply_writer);
  if (writer == nullptr) {
    throw UnsupportedError("the reply writer is not a ZeroDDS writer");
  }
  std::uint8_t complete = 0;
  const int status = zerodds_rpc_client_matching_complete(
      reader_, writer->handle(), client_writer_guid, accept_unannounced ? 1 : 0, &complete);
  if (status != 0) {
    raise_local(status, "zerodds_rpc_client_matching_complete");
  }
  return complete != 0;
}

/// The type objects of a `TypeInformation` as the C API's typed create calls
/// take them: the root and the dependencies as pointers and lengths.
struct TypedObjects {
  const std::uint8_t* root = nullptr;
  std::size_t root_length = 0;
  std::vector<const std::uint8_t*> objects;
  std::vector<std::size_t> lengths;
};

/// Calls `create` with the objects of `information`.
template <typename Create>
auto create_typed(const TypeInformation& information, Create create) -> decltype(create(TypedObjects())) {
  TypedObjects typed;
  typed.root = information.object.data();
  typed.root_length = information.object.size();
  for (const Octets& dependency : information.dependencies) {
    typed.objects.push_back(dependency.data());
    typed.lengths.push_back(dependency.size());
  }
  return create(typed);
}

/// A `dds::rpc::Publisher` over `zerodds_ZeroDdsRuntime`.
///
/// Reliable endpoints and no key by default: a request and a reply topic
/// carry a stream of calls, not instances, and §7.4.2 gives them no key
/// member.
class RuntimePublisher final : public Publisher {
 public:
  explicit RuntimePublisher(zerodds_ZeroDdsRuntime* runtime) : runtime_(runtime) {}

  std::shared_ptr<DataWriter> create_writer(const std::string& topic, const std::string& type_name,
                                            const EndpointQos& qos) override {
    const zerodds_ZeroDdsEndpointQos c_qos = to_c_qos(qos);
    zerodds_ZeroDdsWriter* writer =
        qos.type_information
            ? create_typed(*qos.type_information, [&](const TypedObjects& typed) {
                return zerodds_writer_create_qos_typed(
                    runtime_, topic.c_str(), type_name.c_str(), 0, &c_qos, typed.root,
                    typed.root_length, typed.objects.data(), typed.lengths.data(),
                    typed.objects.size());
              })
            : zerodds_writer_create_qos(runtime_, topic.c_str(), type_name.c_str(), 0, &c_qos);
    if (writer != nullptr) {
      // The identifier has to name the framing (see `kWireExtensibility`).
      // Placed here, on the one call in this file that creates a writer, and
      // not in `Writer`'s constructor: that one also adopts a handle it did
      // not create (`owned == false`), and re-declaring an extensibility its
      // owner already chose would overwrite it.
      // A topic type with a description is an Enhanced @Choice, read as @mutable (§7.5.1.2.1.1):
      // its samples are PL_CDR2.
      int status = zerodds_writer_set_wire_extensibility(
          writer, qos.type_information ? kEnhancedWireExtensibility : kWireExtensibility);
      if (status == 0) {
        // DATA_REPRESENTATION: the encapsulation identifier of the samples
        // names the CDR form the codec produced (§7.5.1.1: classic CDR).
        status = zerodds_writer_set_data_representation(
            writer, qos.representation == WireEncoding::XCDR1 ? 0 : 2);
      }
      if (status != 0) {
        // Destroyed rather than handed on: a writer that could not declare
        // its own framing publishes samples no strict peer accepts, and doing
        // that quietly is the failure this call exists to end.
        zerodds_writer_destroy(writer);
        raise_local(status, "zerodds_writer_set_wire_extensibility/set_data_representation");
      }
    }
    return std::make_shared<Writer>(writer, true, qos);
  }

 private:
  zerodds_ZeroDdsRuntime* runtime_ = nullptr;
};

/// A `dds::rpc::Subscriber` over `zerodds_ZeroDdsRuntime`.
class RuntimeSubscriber final : public Subscriber {
 public:
  explicit RuntimeSubscriber(zerodds_ZeroDdsRuntime* runtime) : runtime_(runtime) {}

  std::shared_ptr<DataReader> create_reader(const std::string& topic, const std::string& type_name,
                                            const EndpointQos& qos) override {
    const zerodds_ZeroDdsEndpointQos c_qos = to_c_qos(qos);
    zerodds_ZeroDdsReader* reader =
        qos.type_information
            ? create_typed(*qos.type_information, [&](const TypedObjects& typed) {
                return zerodds_reader_create_qos_typed(
                    runtime_, topic.c_str(), type_name.c_str(), 0, &c_qos, typed.root,
                    typed.root_length, typed.objects.data(), typed.lengths.data(),
                    typed.objects.size());
              })
            : zerodds_reader_create_qos(runtime_, topic.c_str(), type_name.c_str(), 0, &c_qos);
    return std::make_shared<Reader>(reader, true, qos);
  }

 private:
  zerodds_ZeroDdsRuntime* runtime_ = nullptr;
};

/// A `dds::rpc::DomainParticipant` over `zerodds_ZeroDdsRuntime`.
class Participant final : public DomainParticipant {
 public:
  explicit Participant(zerodds_ZeroDdsRuntime* runtime, bool owned = false)
      : runtime_(runtime), owned_(owned) {
    if (runtime_ == nullptr) {
      throw OutOfResourcesError("zerodds_runtime_create returned null");
    }
    publisher_ = std::make_shared<RuntimePublisher>(runtime_);
    subscriber_ = std::make_shared<RuntimeSubscriber>(runtime_);
  }

  ~Participant() override {
    // The entities the publisher and subscriber created are destroyed with
    // their own handles; the runtime goes last.
    publisher_.reset();
    subscriber_.reset();
    if (owned_ && runtime_ != nullptr) {
      zerodds_runtime_destroy(runtime_);
    }
  }

  Participant(const Participant&) = delete;
  Participant& operator=(const Participant&) = delete;

  std::shared_ptr<Publisher> publisher() override { return publisher_; }
  std::shared_ptr<Subscriber> subscriber() override { return subscriber_; }

  /// §7.10.1: the XML QoS profile at `url`, through the C API.
  EndpointQos load_qos_profile(const std::string& url) override {
    zerodds_ZeroDdsEndpointQos c_qos = zerodds_endpoint_qos_default();
    const int status = zerodds_endpoint_qos_from_profile_url(url.c_str(), &c_qos);
    if (status != 0) {
      raise_local(status, "zerodds_endpoint_qos_from_profile_url(" + url + ")");
    }
    EndpointQos qos;
    qos.reliability = c_qos.reliable != 0 ? ReliabilityKind::RELIABLE : ReliabilityKind::BEST_EFFORT;
    qos.durability =
        c_qos.transient_local != 0 ? DurabilityKind::TRANSIENT_LOCAL : DurabilityKind::VOLATILE;
    qos.history = c_qos.history_keep_all != 0 ? HistoryKind::KEEP_ALL : HistoryKind::KEEP_LAST;
    qos.history_depth = c_qos.history_depth;
    qos.deadline_ns = c_qos.deadline_ns;
    qos.lifespan_ns = c_qos.lifespan_ns;
    return qos;
  }

 private:
  zerodds_ZeroDdsRuntime* runtime_ = nullptr;
  bool owned_ = false;
  std::shared_ptr<Publisher> publisher_;
  std::shared_ptr<Subscriber> subscriber_;
};

/// A participant over a runtime this object owns, on `domain_id`.
inline std::shared_ptr<Participant> participant_on(std::uint32_t domain_id) {
  return std::make_shared<Participant>(zerodds_runtime_create(domain_id), true);
}

}  // namespace zerodds

/// §7.11.1.4.23's traits for the ZeroDDS binding.
struct ZeroDdsBinding {
  using DomainParticipant = ::dds::rpc::zerodds::Participant;
  using Publisher = ::dds::rpc::zerodds::RuntimePublisher;
  using Subscriber = ::dds::rpc::zerodds::RuntimeSubscriber;
  using DataWriter = ::dds::rpc::zerodds::Writer;
  using DataReader = ::dds::rpc::zerodds::Reader;
};

template <>
struct entity_traits<ZeroDdsBinding> {
  using DomainParticipant = ::dds::rpc::zerodds::Participant;
  using Publisher = ::dds::rpc::zerodds::RuntimePublisher;
  using Subscriber = ::dds::rpc::zerodds::RuntimeSubscriber;
  using DataWriter = ::dds::rpc::zerodds::Writer;
  using DataReader = ::dds::rpc::zerodds::Reader;
  using DataWriterQos = ::dds::rpc::EndpointQos;
  using DataReaderQos = ::dds::rpc::EndpointQos;
};

}  // namespace rpc
}  // namespace dds

#endif  // ZERODDS_DDS_RPC_ZERODDS_HPP

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/rpc.hpp — the C++ request-reply and function-call language
// bindings of DDS-RPC 1.0 §7.11.1.
//
// §7.11.1.4.1 fixes the surface: "The dds, and dds::rpc namespaces
// define the classes and functions for the request-reply style language
// bindings. Specifically, the language binding includes RPCEntity,
// Requester, Replier, ServiceProxy, ListenerBase, SimpleReplierListener,
// ReplierListener, SimpleRequesterListener, RequesterListener,
// RequesterParams, ReplierParams, future, shared_future, Sample,
// SampleRef, WriteSample, WriteSampleRef, LoanedSamples, SharedSamples,
// SampleIterator, dds_type_traits, and dds_entity_traits." §7.11.1.5
// adds Service, ServiceEndpoint, Server, Client and ClientEndpoint for
// the function-call style. Every one of them is here.
//
// ## What it is built on
//
// Not on one DDS C++ API. §7.11.1.4.22 says dds_type_traits exists "to
// provide a consistent syntax to refer to the dependent types
// irrespective of the DDS C++ language binding", and §7.11.1.4.23 says
// dds_entity_traits "abstracts over the DDS entity types ... irrespective
// of the DDS C++ language bindings". The specification therefore names
// its own seam, and this header takes it: DomainParticipant, DataWriter
// and DataReader below are abstract, and a binding supplies them.
// `dds/rpc/zerodds.hpp` is that binding for the ZeroDDS C API; a test
// binding is a dozen lines, which is what makes the runtime measurable
// without a network.
//
// ## Framing
//
// A request sample is the XCDR2 RequestHeader of §7.5.1.1.1 followed by
// the request payload; a reply is the ReplyHeader followed by the reply
// payload. That is the layout crates/rpc/src/wire_codec.rs writes, and
// crates/rpc/tests/cpp_rpc_runtime.rs holds the two against each other
// octet for octet — a second reading of the clause here would be a
// second wire.

#ifndef ZERODDS_DDS_RPC_BASE_HPP
#define ZERODDS_DDS_RPC_BASE_HPP

#include <algorithm>
#include <array>
#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstring>
#include <deque>
#include <exception>
#include <functional>
#include <future>
#include <iterator>
#include <limits>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <string>
#include <thread>
#include <type_traits>
#include <utility>
#include <vector>

// DDS-PSM-Cxx 1.0 §7.5.5: `dds::core::Exception` is the base of every DDS
// exception. When the PSM headers are on the include path the exceptions of
// this header derive from it, so a handler for `dds::core::Exception` catches
// them (DDS-RPC 1.0 §7.11.1.3: "all DDS exceptions defined in sub clause 7.5.5
// in [DDS-Cxx-PSM] are valid exceptions"). Without the PSM headers the base is
// an empty class of the same shape.
#if defined(__has_include)
#if __has_include(<dds/core/Exception.hpp>)
#include <dds/core/Exception.hpp>
#define ZERODDS_RPC_HAS_DDS_CORE_EXCEPTION 1
#endif
#if __has_include(<dds/core/Duration.hpp>)
#include <dds/core/Duration.hpp>
#define ZERODDS_RPC_HAS_DDS_CORE_DURATION 1
#endif
#endif

namespace dds {

#ifdef ZERODDS_RPC_HAS_DDS_CORE_DURATION
/// The duration of the waits of DDS-RPC 1.0 §7.11.1.4: the PSM's
/// `dds::core::Duration` where its headers are on the include path.
using Duration = ::dds::core::Duration;
#else
/// A time interval in seconds and nanoseconds, the subset of the PSM's
/// `dds::core::Duration` (DDS-PSM-Cxx 1.0 §7.5.6) the waits of
/// DDS-RPC 1.0 §7.11.1.4 take, for a build without the PSM headers.
class Duration {
 public:
  static Duration zero() { return Duration(0, 0); }
  /// DURATION_INFINITE: both halves are 0x7fffffff.
  static Duration infinite() {
    return Duration(std::numeric_limits<std::int32_t>::max(), 0x7FFFFFFFu);
  }
  static Duration from_millisecs(std::int64_t milliseconds) {
    return Duration(static_cast<std::int32_t>(milliseconds / 1000),
                    static_cast<std::uint32_t>((milliseconds % 1000) * 1000000));
  }

  Duration() = default;
  explicit Duration(std::int32_t sec, std::uint32_t nanosec = 0) : sec_(sec), nanosec_(nanosec) {}

  std::int32_t sec() const { return sec_; }
  std::uint32_t nanosec() const { return nanosec_; }

  bool operator==(const Duration& other) const {
    return sec_ == other.sec_ && nanosec_ == other.nanosec_;
  }
  bool operator!=(const Duration& other) const { return !(*this == other); }

 private:
  std::int32_t sec_ = 0;
  std::uint32_t nanosec_ = 0;
};
#endif

/// `struct SequenceNumber_t {long high; unsigned long low;}`, the RTPS
/// sequence number §7.5.1.1.1 builds `SampleIdentity` from.
struct SequenceNumber_t {
  std::int32_t high = 0;
  std::uint32_t low = 0;

  bool operator==(const SequenceNumber_t& other) const {
    return high == other.high && low == other.low;
  }
  bool operator!=(const SequenceNumber_t& other) const { return !(*this == other); }

  /// Exchanges two values (DDS-PSM-Cxx 1.0 §7.12), which the free `swap`
  /// the C++ column writes for every structure calls.
  void swap(SequenceNumber_t& other) {
    std::swap(high, other.high);
    std::swap(low, other.low);
  }
};

/// A GUID and a sequence number, as §7.5.1.1.1 defines the pair.
///
/// "SampleIdentity = struct {GUID_t writer_guid; SequenceNumber_t
/// sequence_number;}". The GUID is the sixteen octets RTPS gives an
/// entity; the sequence number is the writer's, split into the high and
/// the low half the wire carries.
struct SampleIdentity {
  std::uint8_t writer_guid[16] = {};
  SequenceNumber_t sequence_number;

  bool operator==(const SampleIdentity& other) const {
    return std::memcmp(writer_guid, other.writer_guid, sizeof(writer_guid)) == 0 &&
           sequence_number == other.sequence_number;
  }
  bool operator!=(const SampleIdentity& other) const { return !(*this == other); }

  /// Exchanges two values (DDS-PSM-Cxx 1.0 §7.12).
  void swap(SampleIdentity& other) {
    std::swap_ranges(writer_guid, writer_guid + 16, other.writer_guid);
    sequence_number.swap(other.sequence_number);
  }

  /// Whether this identity is unset — an all-zero GUID and a zero
  /// sequence number, what a `WriteSample` carries before its writer
  /// stamps it. The spec's invalid value is
  /// `related_sample_identity_invalid()`.
  bool is_unknown() const { return *this == SampleIdentity{}; }
};

/// §7.8.2 — `RELATED_SAMPLE_IDENTITY_INVALID`:
/// `{ GUID_UNKNOWN, SEQUENCENUMBER_UNKNOWN }`, both defined in [RTPS].
inline SampleIdentity related_sample_identity_invalid() {
  SampleIdentity id;
  id.sequence_number.high = -1;
  id.sequence_number.low = 0;
  return id;
}

/// What the middleware knows about one sample beside its value.
struct SampleInfo {
  /// The identity the writer gave the sample.
  SampleIdentity identity;
  /// For a reply, the request it answers. Unknown on a request.
  SampleIdentity related_identity;
  /// Whether the sample carried a related sample identity: the
  /// `PID_RELATED_SAMPLE_IDENTITY` of §7.8.2, or the request id of a
  /// Basic reply header.
  bool has_related_identity = false;
  /// Whether the sample carries data rather than only a state change.
  bool valid_data = true;
  /// Whether the binding reports the CDR form the sample arrived in.
  bool has_encoding = false;
  /// That form: 0 for XCDR1, 1 for XCDR2. Meaningful when `has_encoding`.
  std::uint8_t encoding = 0;
};

/// The sequence of the infos of a batch of samples,
/// `dds::SampleInfoSeq` of the normative header (§7.11.1.4.19).
using SampleInfoSeq = std::vector<SampleInfo>;

namespace rpc {

// -- §7.5.1.1.1 Common types -------------------------------------------

/// The error codes of §7.5.1.1.1, in the order the IDL declares them.
enum RemoteExceptionCode_t : std::uint32_t {
  REMOTE_EX_OK = 0,
  REMOTE_EX_UNSUPPORTED = 1,
  REMOTE_EX_INVALID_ARGUMENT = 2,
  REMOTE_EX_OUT_OF_RESOURCES = 3,
  REMOTE_EX_UNKNOWN_OPERATION = 4,
  REMOTE_EX_UNKNOWN_EXCEPTION = 5,
};

/// `typedef octet UnusedMember` — §7.5.1.1.1.
///
/// The type of the `dummy` member an `In` or an `Out` structure carries
/// when the operation gives it nothing else (§7.5.1.1.4 rule 3c,
/// §7.5.1.1.5 rule 3c). One octet and not an empty structure: XCDR
/// gives an empty structure no octets, and a reader cannot tell a
/// sample that arrived from one that did not.
using UnusedMember = std::uint8_t;

/// `typedef octet UnknownOperation` — §7.5.1.1.1.
///
/// The type of the `unknownOp` member on the `default` case of a `Call`
/// or `Return` union. §7.5.1.1.6 rule 3 gives the reason: a client built
/// against a newer interface selects a case the service does not have,
/// and the default is where that lands instead of on whichever case
/// happens to be first.
using UnknownOperation = std::uint8_t;

/// `typedef octet UnknownException` — §7.5.1.1.1. The counterpart of
/// [`UnknownOperation`] on the reply side.
using UnknownException = std::uint8_t;

/// The label §7.5.1.1.5 rule 3 gives the `Result` union's success case.
/// DDS-DCPS gives `RETCODE_OK` the value 0.
constexpr std::int32_t RETCODE_OK = 0;

/// The header every request sample carries. §7.5.1.1.1.
struct RequestHeader {
  /// The identity the client correlates the reply by. §7.8.1.
  dds::SampleIdentity requestId;
  /// The service instance this request is bound to, empty for any.
  std::string instanceName;

  /// Exchanges two values (DDS-PSM-Cxx 1.0 §7.12).
  void swap(RequestHeader& other) {
    requestId.swap(other.requestId);
    instanceName.swap(other.instanceName);
  }
};

/// The header every reply sample carries. §7.5.1.1.1.
struct ReplyHeader {
  /// The request this reply answers. §7.8.1.
  dds::SampleIdentity relatedRequestId;
  /// The outcome. §7.5.2.
  RemoteExceptionCode_t remoteEx = REMOTE_EX_OK;

  /// Exchanges two values (DDS-PSM-Cxx 1.0 §7.12).
  void swap(ReplyHeader& other) {
    relatedRequestId.swap(other.relatedRequestId);
    std::swap(remoteEx, other.remoteEx);
  }
};

// -- §7.11.1.3 Mapping of exceptions -----------------------------------

namespace detail {

#ifdef ZERODDS_RPC_HAS_DDS_CORE_EXCEPTION
/// `dds::core::Exception` of the PSM.
using ExceptionRoot = ::dds::core::Exception;
#else
/// The shape of `dds::core::Exception`, for a build without the PSM.
class ExceptionRoot {
 public:
  virtual ~ExceptionRoot() noexcept = default;
  virtual const char* what() const noexcept = 0;

 protected:
  ExceptionRoot() = default;
};
#endif

}  // namespace detail

// Figure 7.6: every remote exception derives from std::runtime_error or
// from std::logic_error, both of which are thrown without being
// declared. That is why §7.11.1.1.2 rule 6.c gives a generated function
// no exception specification.

/// `REMOTE_EX_OUT_OF_RESOURCES`.
class RemoteOutOfResourcesError : public std::runtime_error, public detail::ExceptionRoot {
 public:
  explicit RemoteOutOfResourcesError(const std::string& what) : std::runtime_error(what) {}
  const char* what() const noexcept override { return std::runtime_error::what(); }
};

/// `REMOTE_EX_UNSUPPORTED`.
class RemoteUnsupportedError : public std::logic_error, public detail::ExceptionRoot {
 public:
  explicit RemoteUnsupportedError(const std::string& what) : std::logic_error(what) {}
  const char* what() const noexcept override { return std::logic_error::what(); }
};

/// `REMOTE_EX_INVALID_ARGUMENT`.
class RemoteInvalidArgumentError : public std::invalid_argument, public detail::ExceptionRoot {
 public:
  explicit RemoteInvalidArgumentError(const std::string& what) : std::invalid_argument(what) {}
  const char* what() const noexcept override { return std::invalid_argument::what(); }
};

/// `REMOTE_EX_UNKNOWN_OPERATION`.
class RemoteUnknownOperationError : public std::logic_error, public detail::ExceptionRoot {
 public:
  explicit RemoteUnknownOperationError(const std::string& what) : std::logic_error(what) {}
  const char* what() const noexcept override { return std::logic_error::what(); }
};

/// `REMOTE_EX_UNKNOWN_EXCEPTION`.
class RemoteUnknownExceptionError : public std::logic_error, public detail::ExceptionRoot {
 public:
  explicit RemoteUnknownExceptionError(const std::string& what) : std::logic_error(what) {}
  const char* what() const noexcept override { return std::logic_error::what(); }
};

/// Raise the exception §7.11.1.3's table gives a code, or return for
/// `REMOTE_EX_OK`.
inline void raise_remote_exception(RemoteExceptionCode_t code, const std::string& what) {
  switch (code) {
    case REMOTE_EX_OK:
      return;
    case REMOTE_EX_UNSUPPORTED:
      throw RemoteUnsupportedError(what);
    case REMOTE_EX_INVALID_ARGUMENT:
      throw RemoteInvalidArgumentError(what);
    case REMOTE_EX_OUT_OF_RESOURCES:
      throw RemoteOutOfResourcesError(what);
    case REMOTE_EX_UNKNOWN_OPERATION:
      throw RemoteUnknownOperationError(what);
    case REMOTE_EX_UNKNOWN_EXCEPTION:
      throw RemoteUnknownExceptionError(what);
  }
  // A code outside the six the enum declares. Reported rather than
  // ignored: a reply carrying one is a reply this binding did not
  // understand, and returning normally would present it as success.
  throw RemoteUnknownExceptionError(what);
}

// -- Local exceptions, Figure 7.6 --------------------------------------

// §7.11.1.3: "The C++ language binding may throw locally generated and
// remotely generated exceptions. As such, all DDS exceptions defined in
// sub clause 7.5.5 in [DDS-Cxx-PSM] are valid exceptions." Figure 7.6
// shows the two halves of the hierarchy. The remote half is above; this
// is the local half, for what fails in this process: a reference that
// holds nothing, an entity already closed, a binding refusing a call.
//
// A failure of the middleware underneath is reported as a local error,
// never as a remote one: `RemoteOutOfResourcesError` means the *service*
// answered `REMOTE_EX_OUT_OF_RESOURCES` (§7.5.2), and raising it for a
// writer that could not allocate would tell the caller a peer refused
// when no peer was involved.

#ifdef ZERODDS_RPC_HAS_DDS_CORE_EXCEPTION
// With the PSM's exception classes at hand, every local error is the class of
// that name in `dds::core` as well (§7.11.1.3: "all DDS exceptions defined in
// sub clause 7.5.5 in [DDS-Cxx-PSM] are valid exceptions"), so
// `catch (const dds::core::TimeoutError&)` reaches `dds::rpc::TimeoutError`.
// `LocalError` is then a tag with a message: the std base each error has comes
// from its `dds::core` class (std::runtime_error or std::logic_error), and a
// second one through `LocalError` would be ambiguous.

/// The base of every locally generated exception.
class LocalError : public virtual detail::ExceptionRoot {
 public:
  explicit LocalError(const std::string& what) : what_(what) {}
  const char* what() const noexcept override { return what_.c_str(); }

 private:
  std::string what_;
};

#define ZERODDS_RPC_LOCAL_ERROR(Name)                                          \
  class Name : public LocalError, public ::dds::core::Name {                   \
   public:                                                                     \
    explicit Name(const std::string& what) : LocalError(what), ::dds::core::Name(what) {} \
    const char* what() const noexcept override { return ::dds::core::Name::what(); } \
  }
#else
/// The base of every locally generated exception.
class LocalError : public std::runtime_error, public detail::ExceptionRoot {
 public:
  explicit LocalError(const std::string& what) : std::runtime_error(what) {}
  const char* what() const noexcept override { return std::runtime_error::what(); }
};

#define ZERODDS_RPC_LOCAL_ERROR(Name)                                          \
  class Name : public LocalError {                                             \
   public:                                                                     \
    explicit Name(const std::string& what) : LocalError(what) {}               \
  }
#endif

/// An operation on a reference that holds no entity (DDS-Cxx-PSM
/// `dds::core::NullReferenceError`).
ZERODDS_RPC_LOCAL_ERROR(NullReferenceError);

/// An operation on an entity that was closed (`dds::core::AlreadyClosedError`).
ZERODDS_RPC_LOCAL_ERROR(AlreadyClosedError);

/// An operation the entity's state does not allow
/// (`dds::core::IllegalOperationError`).
ZERODDS_RPC_LOCAL_ERROR(IllegalOperationError);

/// A precondition of the operation does not hold
/// (`dds::core::PreconditionNotMetError`).
ZERODDS_RPC_LOCAL_ERROR(PreconditionNotMetError);

/// The middleware ran out of a resource (`dds::core::OutOfResourcesError`).
ZERODDS_RPC_LOCAL_ERROR(OutOfResourcesError);

/// An operation did not complete in time (`dds::core::TimeoutError`).
ZERODDS_RPC_LOCAL_ERROR(TimeoutError);

/// A QoS policy that cannot be changed once the entity is enabled was
/// changed (`dds::core::ImmutablePolicyError`, DDS 1.4 §2.2.3).
ZERODDS_RPC_LOCAL_ERROR(ImmutablePolicyError);

/// A set of QoS policies is inconsistent
/// (`dds::core::InconsistentPolicyError`).
ZERODDS_RPC_LOCAL_ERROR(InconsistentPolicyError);

/// The binding does not support the operation (`dds::core::UnsupportedError`).
ZERODDS_RPC_LOCAL_ERROR(UnsupportedError);

/// An argument is outside what the operation accepts
/// (`dds::core::InvalidArgumentError`).
ZERODDS_RPC_LOCAL_ERROR(InvalidArgumentError);

#undef ZERODDS_RPC_LOCAL_ERROR

/// A failure of the binding that no more specific class describes
/// (`dds::core::Error`, RETCODE_ERROR).
#ifdef ZERODDS_RPC_HAS_DDS_CORE_EXCEPTION
class UnspecifiedError : public LocalError, public ::dds::core::Error {
 public:
  explicit UnspecifiedError(const std::string& what)
      : LocalError(what), ::dds::core::Error(what) {}
  const char* what() const noexcept override { return ::dds::core::Error::what(); }
};
#else
class UnspecifiedError : public LocalError {
 public:
  explicit UnspecifiedError(const std::string& what) : LocalError(what) {}
};
#endif

// -- The DDS entities, as §7.11.1.4.23 abstracts over them -------------

/// One sample as it lies on the wire.
using Octets = std::vector<std::uint8_t>;

namespace detail {

/// Whether `duration` is `DURATION_INFINITE`.
inline bool is_infinite(const dds::Duration& duration) {
  return duration.sec() == std::numeric_limits<std::int32_t>::max() &&
         duration.nanosec() == 0x7FFFFFFFu;
}

/// The point in time a wait of `duration` from now ends, or none for the
/// infinite duration.
class Deadline {
 public:
  explicit Deadline(const dds::Duration& duration)
      : infinite_(is_infinite(duration)),
        at_(std::chrono::steady_clock::now() +
            std::chrono::seconds(infinite_ ? 0 : duration.sec()) +
            std::chrono::nanoseconds(infinite_ ? 0 : duration.nanosec())) {}
  explicit Deadline(std::chrono::milliseconds duration)
      : infinite_(false), at_(std::chrono::steady_clock::now() + duration) {}

  bool expired() const { return !infinite_ && std::chrono::steady_clock::now() >= at_; }

 private:
  bool infinite_;
  std::chrono::steady_clock::time_point at_;
};

/// The pause between two looks when a wait has nothing to be woken by.
inline void pause_briefly() { std::this_thread::sleep_for(std::chrono::milliseconds(1)); }

}  // namespace detail

/// The CDR form of the samples. The Basic Service Mapping "maps interfaces
/// to data-types that can be used by DDS version 1.3 compliant
/// implementations, which may lack support for the [DDS-XTypes]
/// specification" (§7.5.1.1), so the default is classic CDR (XCDR1): an
/// eight-octet member aligns to eight. XCDR2 caps every alignment at four
/// (XTypes 1.3 §7.4.1); it is a named setting for peers that speak it. The
/// two headers of §7.5.1.1.1 are the same octets in both.
enum class WireEncoding { XCDR1, XCDR2 };

/// The alignment cap of an encoding: eight for XCDR1, four for XCDR2.
constexpr std::size_t max_alignment(WireEncoding encoding) {
  return encoding == WireEncoding::XCDR1 ? 8 : 4;
}

/// RELIABILITY kind. §7.10.2 gives RELIABLE as the default.
enum class ReliabilityKind { BEST_EFFORT, RELIABLE };

/// DURABILITY kind. §7.10.2 gives VOLATILE as the default.
enum class DurabilityKind { VOLATILE, TRANSIENT_LOCAL };

/// HISTORY kind. §7.10.2 gives KEEP_ALL as the default.
enum class HistoryKind { KEEP_LAST, KEEP_ALL };

/// The QoS of one DataWriter or DataReader, as far as the DDS-RPC
/// default table of §7.10.2 and the XML QoS profiles of §7.10.1 reach.
/// The defaults are §7.10.2's.
/// The XTypes description of a topic type (DDS-RPC 1.0 §7.5.1.2, XTypes 1.3
/// §7.6.3): the COMPLETE `TypeObject` of the type and the objects it names by
/// hash, in the `TypeObject::to_bytes_le` form. A binding that announces
/// them lets a peer match by assignability and not by the type name alone.
struct TypeInformation {
  Octets object;
  std::vector<Octets> dependencies;
};

/// What a generated `dds_type_traits` returns from `type_information()`.
inline std::shared_ptr<const TypeInformation> make_type_information(
    Octets object, std::vector<Octets> dependencies) {
  auto out = std::make_shared<TypeInformation>();
  out->object = std::move(object);
  out->dependencies = std::move(dependencies);
  return out;
}

struct EndpointQos {
  /// The description of the endpoint's type, or none. A binding that does
  /// not announce type information creates the endpoint without it, and
  /// its peers match it by the type name.
  std::shared_ptr<const TypeInformation> type_information;
  ReliabilityKind reliability = ReliabilityKind::RELIABLE;
  DurabilityKind durability = DurabilityKind::VOLATILE;
  HistoryKind history = HistoryKind::KEEP_ALL;
  /// The KEEP_LAST depth, at least 1; unused under KEEP_ALL.
  std::uint32_t history_depth = 1;
  /// DEADLINE period in nanoseconds, 0 for infinite.
  std::uint64_t deadline_ns = 0;
  /// LIFESPAN duration in nanoseconds, 0 for infinite (writers only).
  std::uint64_t lifespan_ns = 0;
  /// DATA_REPRESENTATION: the CDR form a writer's samples have.
  WireEncoding representation = WireEncoding::XCDR1;

  bool operator==(const EndpointQos& other) const {
    return reliability == other.reliability && durability == other.durability &&
           history == other.history && history_depth == other.history_depth &&
           deadline_ns == other.deadline_ns && lifespan_ns == other.lifespan_ns &&
           representation == other.representation;
  }
  bool operator!=(const EndpointQos& other) const { return !(*this == other); }
};

/// The QoS of a Requester, a Replier, a Client or a Service: one set for
/// the endpoints on the request topic and one for those on the reply
/// topic, which is how §7.10.1's `@Qos(RequestProfile, ReplyProfile)`
/// splits it. With nothing specified both are the defaults of §7.10.2.
struct RpcQos {
  EndpointQos request;
  EndpointQos reply;
  /// How long a call of the function-call style waits for its reply
  /// (§7.9.1); `0` waits without end. The Rust runtime's
  /// `RpcQos::request_timeout` default of five seconds.
  std::chrono::milliseconds request_timeout{5000};
};

/// How a reply is tied to the request it answers. §7.8.
enum class Correlation {
  /// §7.8.1: the Basic Service Profile. `RequestHeader` and `ReplyHeader`
  /// are members of the sample and carry the identities.
  HEADER,
  /// §7.3.1.4, the Basic Service Mapping: the request and reply types are
  /// structures of the user that carry the header of §7.5.1.1.1 as their
  /// own `header` member. The sample is the encoding of that structure; the
  /// runtime puts the identity and the instance name into the request's
  /// header, reads the identity from the leading octets (the header is the
  /// first member and its layout is fixed), and puts the identity of the
  /// request and the remote exception code into the reply's header. The
  /// traits of the two types supply `set_request_header`, `set_reply_header`.
  HEADER_IN_TYPE,
  /// §7.8.2: the Enhanced Service Profile. The sample is the bare topic
  /// type; the request is identified by the sample identity of its DATA
  /// and a reply carries it in `PID_RELATED_SAMPLE_IDENTITY`.
  SAMPLE_IDENTITY,
};

/// What a requester does with a request while the service has not been
/// matched. §7.6.2.2: "The client-side DataWriter (request DataWriter)
/// shall not send out any request to a service DataReader unless the
/// client-side DataReader (reply DataReader) has discovered and matched
/// the corresponding service DataWriter (reply DataWriter)." §7.6.2.2.2:
/// "Attempts to send requests would be queued."
enum class ServiceMatching {
  /// QUEUE for the Enhanced profile, NONE for the Basic one, whose
  /// discovery §7.6.1 describes as relying on plain DDS discovery.
  DEFAULT,
  /// §7.6.2.2.2: the request waits and goes out, in order, once the
  /// service has matched.
  QUEUE,
  /// The request is refused with `PreconditionNotMetError`.
  REFUSE,
  /// The request is written at once, whether the service has matched or
  /// not. A reply to it may be lost (§7.6.1's note).
  NONE,
};

/// What a replier does with a request while the client has not been
/// matched. §7.6.2.2: "The service-side DataReader (request DataReader)
/// shall reject requests from a client DataWriter (request DataWriter) as
/// long as the service-side DataWriter (reply DataWriter) has not
/// discovered and matched the corresponding client-side DataReader (reply
/// DataReader)."
enum class ClientMatching {
  /// HOLD for the Enhanced profile, NONE for the Basic one.
  DEFAULT,
  /// §7.6.2.2.1: the request is not processed until the reply writer has
  /// matched a client's reply reader. It is kept, not discarded, and
  /// handed out once the client has matched.
  HOLD,
  /// The request is processed at once.
  NONE,
};

/// The DDS-RPC discovery extension of an endpoint (§7.6.2.1): the members
/// `PID_SERVICE_INSTANCE_NAME` (0x0080), `PID_RELATED_ENTITY_GUID` (0x0081)
/// and `PID_TOPIC_ALIASES` (0x0082) of the extended publication and
/// subscription data. A member that is not announced is absent; an absent
/// related endpoint reads as `GUID_UNKNOWN`.
struct RpcDiscovery {
  bool has_instance_name = false;
  /// `string<256>`.
  std::string instance_name;
  bool has_related = false;
  std::uint8_t related[16] = {};
  /// `sequence<string<256>>`; empty is absent.
  std::vector<std::string> aliases;
  /// Set by a binding that read the announcement into a buffer too small
  /// for it: `instance_name` (or `aliases`) is then a cut of what the peer
  /// announced and is never compared (`service_matching_complete` does not
  /// match on a truncated name).
  bool name_truncated = false;
  bool aliases_truncated = false;
};

/// A matched endpoint as the Enhanced discovery algorithm sees it: its
/// GUID and what it announced.
struct RpcPeer {
  std::uint8_t key[16] = {};
  RpcDiscovery announced;
};

/// Whether `peer` names `counterpart` as its related endpoint, or — with
/// `accept_unannounced` — names none.
inline bool rpc_peer_names(const RpcPeer& peer, const std::uint8_t counterpart[16],
                           bool accept_unannounced) {
  if (peer.announced.has_related) {
    return std::memcmp(peer.announced.related, counterpart, 16) == 0;
  }
  return accept_unannounced;
}

/// §7.6.2.2.2, Figure 7.4: has the client discovered both endpoints of a
/// service instance it may address? `readers` are the readers its request
/// writer is matched with, `writers` the writers its reply reader is matched
/// with. An instance is complete when one reader and one writer name each
/// other and the reader announced `instance_name` (the empty name: none).
/// A peer that announced no related endpoint never completes unless
/// `accept_unannounced` reads it as complete by the DDS match alone. A
/// binding that reads the extension can implement
/// `DataWriter::service_matching_complete` with this.
inline bool service_matching_complete(const std::vector<RpcPeer>& readers,
                                      const std::vector<RpcPeer>& writers,
                                      const std::string& instance_name, bool accept_unannounced) {
  for (const RpcPeer& reader : readers) {
    if (reader.announced.name_truncated) {
      // Only a cut of the name is known; equal to the wanted name by chance
      // is not a match.
      continue;
    }
    const std::string announced =
        reader.announced.has_instance_name ? reader.announced.instance_name : std::string();
    if (announced != instance_name) {
      continue;
    }
    for (const RpcPeer& writer : writers) {
      if (rpc_peer_names(reader, writer.key, accept_unannounced) &&
          rpc_peer_names(writer, reader.key, accept_unannounced)) {
        return true;
      }
    }
  }
  return false;
}

/// §7.6.2.2.1, Figure 7.3: has the service discovered both endpoints of the
/// client whose request writer is `client_writer`? `readers` are the readers
/// the service's reply writer is matched with. The client's request writer
/// names its reply reader; the service has discovered the client when that
/// reader is among `readers`. A client that announced no related endpoint is
/// complete only with `accept_unannounced`.
inline bool client_matching_complete(const RpcPeer& client_writer,
                                    const std::vector<RpcPeer>& readers,
                                    bool accept_unannounced) {
  if (!client_writer.announced.has_related) {
    return accept_unannounced;
  }
  for (const RpcPeer& reader : readers) {
    if (std::memcmp(reader.key, client_writer.announced.related, 16) == 0 &&
        rpc_peer_names(reader, client_writer.key, accept_unannounced)) {
      return true;
    }
  }
  return false;
}

/// The GUID of an endpoint, 16 octets.
using EndpointGuid = std::array<std::uint8_t, 16>;

/// A writer of serialised samples on one topic.
class DataWriter {
 public:
  virtual ~DataWriter() = default;
  /// Whether this binding announces and reads the DDS-RPC discovery
  /// extension (§7.6.2.1). One that does not matches the Enhanced
  /// mapping's endpoints by their count, and cannot address a service
  /// instance by name.
  virtual bool supports_rpc_discovery() const { return false; }
  /// Announce the extension of this writer (§7.6.2.1.1).
  virtual void announce_rpc_discovery(const RpcDiscovery&) {
    throw UnsupportedError("this binding does not announce the DDS-RPC discovery extension");
  }
  /// The readers this writer is matched with and what each announced
  /// (§7.6.2.1.2).
  virtual std::vector<RpcPeer> matched_rpc_readers() const {
    throw UnsupportedError("this binding does not read the DDS-RPC discovery extension");
  }
  /// §7.6.2.2.2, Figure 7.4: whether the client with this request writer
  /// and that reply reader has discovered both endpoints of the service
  /// instance named `instance_name` (the empty name: one that announced
  /// none). With `accept_unannounced` a service that announced no related
  /// endpoint is complete by the DDS match alone.
  virtual bool service_matching_complete(const class DataReader& reply_reader,
                                         const std::string& instance_name,
                                         bool accept_unannounced) const {
    (void)reply_reader;
    (void)instance_name;
    (void)accept_unannounced;
    throw UnsupportedError("this binding does not read the DDS-RPC discovery extension");
  }
  /// Write one sample and return the identity the middleware gave it.
  virtual dds::SampleIdentity write(const Octets& sample) = 0;
  /// Write one sample with the sample it answers in the DATA's inline QoS
  /// (`PID_RELATED_SAMPLE_IDENTITY`, §7.8.2) and return the identity the
  /// middleware gave it. A binding that cannot send the parameter must
  /// raise `UnsupportedError`: a reply without it cannot be correlated by
  /// an Enhanced client, and sending it anyway would lose the reply
  /// unnoticed.
  virtual dds::SampleIdentity write_related(const Octets& sample,
                                            const dds::SampleIdentity& related) = 0;
  /// Write one sample for the matched readers `readers` only, each named by
  /// its GUID, and return the identity the middleware gave it. DDS-RPC 1.0
  /// §7.11.1.4.3: "Requests sent through a bound requester reference shall be
  /// sent to the bound service instance only", and the instance is the
  /// readers that announced its name (§7.6.2.1). A reader that is not named
  /// does not get the sample. A binding that cannot address readers must
  /// raise `UnsupportedError`: sending the request to every reader instead
  /// would break the clause unnoticed.
  virtual dds::SampleIdentity write_to(const Octets& sample,
                                       const std::vector<EndpointGuid>& readers) {
    (void)sample;
    (void)readers;
    throw UnsupportedError("this binding cannot write to a subset of the matched readers");
  }
  /// The identity `write()` will stamp on the next sample, without
  /// consuming it.
  ///
  /// A request header carries the identity of the sample it is part of
  /// (§7.5.1.1.1 `requestId`, §7.8.1), and the header has to be filled
  /// before the sample is written, so the requester asks the writer for
  /// the identity it is about to stamp and then writes. A binding that
  /// cannot predict its writer's identity has no explicit request-id and
  /// cannot satisfy the Basic mapping, which is why this is abstract
  /// rather than a default that would silently correlate everything to
  /// the invalid identity.
  ///
  /// The same call twice without an intervening `write()` returns the
  /// same identity; the two agree with what `write()` then returns.
  virtual dds::SampleIdentity next_identity() const = 0;
  /// How many readers this writer is matched with.
  virtual std::size_t matched_count() const = 0;
  /// Change the mutable policies at run time. §7.10.2: "The users may
  /// modify the mutable QoS policies at run-time." A policy that cannot
  /// change once the writer exists — RELIABILITY, DURABILITY, HISTORY — is
  /// an `ImmutablePolicyError`, and nothing is changed.
  virtual void set_qos(const EndpointQos& qos) = 0;
};

/// A reader of serialised samples on one topic.
class DataReader {
 public:
  virtual ~DataReader() = default;
  /// See [`DataWriter::supports_rpc_discovery`].
  virtual bool supports_rpc_discovery() const { return false; }
  /// This reader's own GUID, 16 octets.
  virtual void guid(std::uint8_t out[16]) const {
    (void)out;
    throw UnsupportedError("this binding reports no reader GUID");
  }
  /// Announce the extension of this reader (§7.6.2.1.2).
  virtual void announce_rpc_discovery(const RpcDiscovery&) {
    throw UnsupportedError("this binding does not announce the DDS-RPC discovery extension");
  }
  /// The writers this reader is matched with and what each announced
  /// (§7.6.2.1.1).
  virtual std::vector<RpcPeer> matched_rpc_writers() const {
    throw UnsupportedError("this binding does not read the DDS-RPC discovery extension");
  }
  /// §7.6.2.2.1, Figure 7.3: the mask of one client. Whether the service
  /// with this request reader and that reply writer has discovered both
  /// endpoints of the client whose request writer is `client_writer_guid`.
  /// The service processes that client's requests only once it has.
  virtual bool client_matching_complete(const class DataWriter& reply_writer,
                                        const std::uint8_t client_writer_guid[16],
                                        bool accept_unannounced) const {
    (void)reply_writer;
    (void)client_writer_guid;
    (void)accept_unannounced;
    throw UnsupportedError("this binding does not read the DDS-RPC discovery extension");
  }
  /// Take every sample that has arrived, removing it from the reader.
  ///
  /// The `SampleInfo` carries `identity`, the writer GUID and sequence
  /// number of the sample's DATA, and the related sample identity of its
  /// `PID_RELATED_SAMPLE_IDENTITY` (§7.8.2) with `has_related_identity`
  /// set, as far as the binding can report them. The Basic profile does
  /// not need either (§7.8.1); the Enhanced one needs both.
  virtual std::vector<std::pair<Octets, dds::SampleInfo>> take() = 0;
  /// How many writers this reader is matched with.
  virtual std::size_t matched_count() const = 0;
  /// See [`DataWriter::set_qos`].
  virtual void set_qos(const EndpointQos& qos) = 0;
  /// Call `callback` when samples have arrived, so that listeners (§7.9.2.2,
  /// §7.11.1.4.7-.10) and futures are served when the sample comes and
  /// not when the application next looks. The callback only says that
  /// there is something to take: the samples stay in the reader for
  /// `take()`. It runs on a thread of the binding, one call at a time, and
  /// may call the reader and the writers. A callback set while samples
  /// are already waiting is called for them. An empty function removes the
  /// callback, and does not return before a call that is running has
  /// returned (unless it is called from the callback itself).
  virtual void set_data_callback(std::function<void()> callback) = 0;
};

/// The entity that creates the DataWriters of a participant.
class Publisher {
 public:
  virtual ~Publisher() = default;
  /// A writer on `topic`, publishing `type_name`, under `qos`.
  ///
  /// The type name is not the topic name: §7.5.1.1.6/7 name the request
  /// and reply types `${interfaceName}_Request`/`${interfaceName}_Reply`
  /// while §7.4.1 makes the topic `${interface}_<service_name>_Request`.
  /// The two cannot be derived from each other (the service segment may
  /// itself hold underscores), so the binding is told both.
  virtual std::shared_ptr<DataWriter> create_writer(const std::string& topic,
                                                    const std::string& type_name,
                                                    const EndpointQos& qos) = 0;
};

/// The entity that creates the DataReaders of a participant.
class Subscriber {
 public:
  virtual ~Subscriber() = default;
  /// A reader on `topic` of `type_name` under `qos`. See [`Publisher`].
  virtual std::shared_ptr<DataReader> create_reader(const std::string& topic,
                                                    const std::string& type_name,
                                                    const EndpointQos& qos) = 0;
};

/// The entity a `Requester`, a `Replier`, a `Client` and a `Service`
/// are created from.
class DomainParticipant {
 public:
  virtual ~DomainParticipant() = default;
  /// The publisher the participant creates request and reply writers
  /// from unless the params name another.
  virtual std::shared_ptr<Publisher> publisher() = 0;
  /// The subscriber the participant creates readers from unless the
  /// params name another.
  virtual std::shared_ptr<Subscriber> subscriber() = 0;
  /// Resolve a QoS profile URL (§7.10.1): `<protocol>://path/resource`
  /// with the protocol, `:` and `//` optional and `file` as the default,
  /// optionally ending in `#library_name`. The profile is an XML QoS
  /// profile as DDS4CCM defines it. Support for the `file` protocol is
  /// mandatory; a binding that resolves no profiles says so rather than
  /// returning defaults the profile did not ask for.
  virtual EndpointQos load_qos_profile(const std::string& url) {
    (void)url;
    throw UnsupportedError("this binding resolves no QoS profile URLs");
  }
};

/// §7.11.1.4.23: "dds_entity_traits abstracts over the DDS entity types
/// and provides consistent syntax to get DDS entity types irrespective
/// of the DDS C++ language bindings (e.g., DomainParticipant,
/// Publisher, Subscriber, DataReaderQos, and DataWriterQos)."
///
/// A binding specialises it; the primary template names the abstract
/// entities above, which is what makes the runtime compile with no DDS
/// C++ API present.
template <typename Binding = void>
struct entity_traits {
  using DomainParticipant = ::dds::rpc::DomainParticipant;
  using Publisher = ::dds::rpc::Publisher;
  using Subscriber = ::dds::rpc::Subscriber;
  using DataWriter = ::dds::rpc::DataWriter;
  using DataReader = ::dds::rpc::DataReader;
  using DataWriterQos = ::dds::rpc::EndpointQos;
  using DataReaderQos = ::dds::rpc::EndpointQos;
};

template <typename T>
class Sample;
template <typename T>
class SampleRef;
template <typename T>
class WriteSample;
template <typename T>
class WriteSampleRef;
template <typename T, bool IsConst = false>
class SampleIterator;
template <typename T>
class LoanedSamples;
template <typename T>
class SharedSamples;

/// §7.11.1.4.22: "dds_type_traits<T> is a collection of meta-functions
/// that given a type T, provides commonly needed dependent types, such
/// as a DataReader for T, DataWriter for T, Sample<T>,
/// LoanedSamples<T>, etc."
///
/// Specialised per topic type by the generated code or by hand. The
/// primary template is only declared, so a type with no specialisation
/// is a compile error naming the type rather than a sample that never
/// serialises or is announced under the wrong type. The specialisation
/// supplies `static const char* type_name()` — §7.5.1.1.6 rule 1's
/// `${interfaceName}_Request` / rule 1's `${interfaceName}_Reply` —
/// alongside `serialize` and `deserialize`, and derives from
/// [`dds_type_traits_base`], which has the dependent types.
template <typename T>
struct dds_type_traits;

/// The dependent types of §7.11.1.4.22 for a topic type `T`, as the
/// normative `dds_type_traits` of the specification's header names them:
/// the sequence, the reader and writer, the sample carriers and their
/// iterators. In this runtime the reader and writer of a topic type are
/// the binding's byte-level ones, shared.
template <typename T>
struct dds_type_traits_base {
  /// The XTypes description of the topic type, none by default. The
  /// generated traits of the Enhanced mapping return the objects of
  /// `${I}_Request` and `${I}_Reply`.
  static std::shared_ptr<const TypeInformation> type_information() { return nullptr; }

  typedef std::vector<T> Seq;
  typedef const std::vector<T> ConstSeq;
  typedef std::shared_ptr<::dds::rpc::DataReader> DataReader;
  typedef std::shared_ptr<::dds::rpc::DataWriter> DataWriter;

  typedef ::dds::rpc::SampleRef<T> SampleRefType;
  typedef ::dds::rpc::SampleRef<T> SampleIteratorValueType;
  typedef ::dds::rpc::SampleRef<const T> ConstSampleIteratorValueType;
  typedef ::dds::rpc::LoanedSamples<T> LoanedSamplesType;
  typedef ::dds::rpc::SharedSamples<T> SharedSamplesType;
  typedef ::dds::rpc::SampleIterator<T, false> iterator;
  typedef ::dds::rpc::SampleIterator<T, true> const_iterator;
};

template <typename T>
struct dds_type_traits : dds_type_traits_base<T> {};

/// The sequences of the infos, which iterators run alongside the data.
template <>
struct dds_type_traits<dds::SampleInfo> {
  typedef dds::SampleInfoSeq Seq;
  typedef const dds::SampleInfoSeq ConstSeq;
};

namespace detail {

/// `dds_type_traits<T>::type_information()`, none for a traits class that
/// has no such function (a hand-written one).
template <typename T>
auto type_information_of(int) -> decltype(dds_type_traits<T>::type_information()) {
  return dds_type_traits<T>::type_information();
}
template <typename T>
std::shared_ptr<const TypeInformation> type_information_of(long) {
  return nullptr;
}

/// `identity` moved on by `count` samples of its writer.
inline dds::SampleIdentity advance_identity(dds::SampleIdentity identity, std::size_t count) {
  const std::uint64_t sn =
      ((static_cast<std::uint64_t>(static_cast<std::uint32_t>(identity.sequence_number.high)) << 32) |
       identity.sequence_number.low) +
      count;
  identity.sequence_number.high = static_cast<std::int32_t>(sn >> 32);
  identity.sequence_number.low = static_cast<std::uint32_t>(sn & 0xFFFFFFFFu);
  return identity;
}

/// `offset` rounded up to a multiple of four: the union that follows a
/// header starts there (§7.5.1.1.1 puts the headers on four octets, and
/// the discriminator of the union is a `long`).
inline std::size_t round_up_4(std::size_t offset) { return (offset + 3u) & ~std::size_t{3u}; }

/// The CDR form a sample is read in: the one the binding reports it
/// arrived in, else `fallback`.
inline WireEncoding encoding_of(const dds::SampleInfo& info, WireEncoding fallback) {
  if (!info.has_encoding) {
    return fallback;
  }
  return info.encoding == 0 ? WireEncoding::XCDR1 : WireEncoding::XCDR2;
}

/// Whether `dds_type_traits<T>` serialises at an offset of the sample and in
/// an encoding: `serialize_at(const T&, std::size_t offset, WireEncoding)`.
/// A payload codec has to count the alignment of its members from the
/// start of the sample, which is not where the payload starts.
template <typename T, typename = void>
struct has_serialize_at : std::false_type {};
template <typename T>
struct has_serialize_at<
    T, std::void_t<decltype(dds_type_traits<T>::serialize_at(
           std::declval<const T&>(), std::size_t{}, WireEncoding::XCDR1))>> : std::true_type {};

/// Whether `dds_type_traits<T>::deserialize` takes the encoding the sample
/// arrived in.
template <typename T, typename = void>
struct has_deserialize_in : std::false_type {};
template <typename T>
struct has_deserialize_in<
    T, std::void_t<decltype(dds_type_traits<T>::deserialize(
           std::declval<const std::vector<std::uint8_t>&>(), std::declval<std::size_t&>(),
           std::declval<T&>(), WireEncoding::XCDR1))>> : std::true_type {};

template <typename T>
std::vector<std::uint8_t> serialize_payload(const T& value, std::size_t offset,
                                            WireEncoding encoding, std::true_type) {
  return dds_type_traits<T>::serialize_at(value, offset, encoding);
}
template <typename T>
std::vector<std::uint8_t> serialize_payload(const T& value, std::size_t, WireEncoding,
                                            std::false_type) {
  return dds_type_traits<T>::serialize(value);
}

template <typename T>
bool deserialize_payload(const std::vector<std::uint8_t>& in, std::size_t& at, T& out,
                         WireEncoding encoding, std::true_type) {
  return dds_type_traits<T>::deserialize(in, at, out, encoding);
}
template <typename T>
bool deserialize_payload(const std::vector<std::uint8_t>& in, std::size_t& at, T& out,
                         WireEncoding, std::false_type) {
  return dds_type_traits<T>::deserialize(in, at, out);
}

/// Whether `dds_type_traits<T>` can say which remote exception code a
/// reply value carries: `static RemoteExceptionCode_t remote_exception(const T&)`.
///
/// §7.5.1.2.5 rule 5: an Enhanced `Reply` structure has a `remoteEx`
/// member of its own, so the code is part of the reply type and not of a
/// header. The traits of an Enhanced reply type supply the accessor.
template <typename T, typename = void>
struct has_remote_exception : std::false_type {};
template <typename T>
struct has_remote_exception<
    T, std::void_t<decltype(dds_type_traits<T>::remote_exception(std::declval<const T&>()))>>
    : std::true_type {};

/// Whether `dds_type_traits<T>` can build a reply value that carries a
/// code: `static T from_remote_exception(RemoteExceptionCode_t)`.
template <typename T, typename = void>
struct can_build_remote_exception : std::false_type {};
template <typename T>
struct can_build_remote_exception<
    T, std::void_t<decltype(dds_type_traits<T>::from_remote_exception(REMOTE_EX_OK))>>
    : std::true_type {};

/// Whether `dds_type_traits<T>` can put a header into a request:
/// `static void set_request_header(T&, const RequestHeader&)`. A request
/// type that carries its own header (§7.3.1.4) supplies it.
template <typename T, typename = void>
struct has_set_request_header : std::false_type {};
template <typename T>
struct has_set_request_header<
    T, std::void_t<decltype(dds_type_traits<T>::set_request_header(
           std::declval<T&>(), std::declval<const RequestHeader&>()))>> : std::true_type {};

/// Whether `dds_type_traits<T>` can put a header into a reply:
/// `static void set_reply_header(T&, const ReplyHeader&)`.
template <typename T, typename = void>
struct has_set_reply_header : std::false_type {};
template <typename T>
struct has_set_reply_header<
    T, std::void_t<decltype(dds_type_traits<T>::set_reply_header(
           std::declval<T&>(), std::declval<const ReplyHeader&>()))>> : std::true_type {};

template <typename T>
void stamp_request_header(T& value, const RequestHeader& header, std::true_type) {
  dds_type_traits<T>::set_request_header(value, header);
}
template <typename T>
void stamp_request_header(T&, const RequestHeader&, std::false_type) {
  throw UnsupportedError(
      "Correlation::HEADER_IN_TYPE needs dds_type_traits::set_request_header in the request type");
}
template <typename T>
void stamp_reply_header(T& value, const ReplyHeader& header, std::true_type) {
  dds_type_traits<T>::set_reply_header(value, header);
}
template <typename T>
void stamp_reply_header(T&, const ReplyHeader&, std::false_type) {
  throw UnsupportedError(
      "Correlation::HEADER_IN_TYPE needs dds_type_traits::set_reply_header in the reply type");
}

template <typename T>
RemoteExceptionCode_t remote_exception_of(const T& value, std::true_type) {
  return dds_type_traits<T>::remote_exception(value);
}
template <typename T>
RemoteExceptionCode_t remote_exception_of(const T&, std::false_type) {
  return REMOTE_EX_OK;
}

}  // namespace detail

// -- §7.11.1.4.13-21 The sample carriers -------------------------------

/// §7.11.1.4.13: "future<T> provides a mechanism to access the result of
/// an asynchronous operation. […] In C++11 and C++14 environments,
/// dds::rpc::future<T> is a typedef for std::future<T>."
template <typename T>
using future = std::future<T>;

/// §7.11.1.4.14: "shared_future<T> is closely related to future<T> and
/// the only difference is that shared_future<T> is copyable and
/// movable."
template <typename T>
using shared_future = std::shared_future<T>;

/// §7.11.1.4.15: "Sample<T> is a valuetype that combines a value of type
/// T and a value of type SampleInfo. Sample is conceptually immutable."
///
/// The members are those of the normative header, including the mutable
/// accessors and the setters it lists.
template <typename T>
class Sample {
 public:
  typedef T Data;
  typedef T& DataReference;
  typedef const T& ConstDataReference;
  typedef typename dds_type_traits<T>::Seq Seq;
  typedef typename dds_type_traits<T>::DataReader DataReader;
  typedef typename dds_type_traits<T>::DataWriter DataWriter;
  using value_type = T;

  Sample() = default;
  Sample(T data, dds::SampleInfo info) : data_(std::move(data)), info_(std::move(info)) {}
  /// A copy of the sample `reference` refers to.
  Sample(SampleRef<T> reference) : data_(reference.data()), info_(reference.info()) {}

  Sample& operator=(SampleRef<T> reference) {
    data_ = reference.data();
    info_ = reference.info();
    return *this;
  }

  /// The identity the writer gave this sample. §7.2.1.5.
  const dds::SampleIdentity& identity() const { return info_.identity; }
  /// The request a reply answers. §7.8.1.
  const dds::SampleIdentity& related_identity() const { return info_.related_identity; }

  void swap(Sample& other) {
    using std::swap;
    swap(data_, other.data_);
    swap(info_, other.info_);
  }

  T& data() { return data_; }
  const T& data() const { return data_; }
  dds::SampleInfo& info() { return info_; }
  const dds::SampleInfo& info() const { return info_; }

  operator DataReference() { return data_; }
  operator ConstDataReference() const { return data_; }

  void set_data(const T& data) { data_ = data; }
  /// Copy the value `data` points at; a null pointer is a null reference.
  void set_data(const T* data) {
    if (data == nullptr) {
      throw NullReferenceError("the data pointer is null");
    }
    data_ = *data;
  }
  void set_info(const dds::SampleInfo& info) { info_ = info; }
  void set_info(const dds::SampleInfo* info) {
    if (info == nullptr) {
      throw NullReferenceError("the info pointer is null");
    }
    info_ = *info;
  }

 private:
  T data_{};
  dds::SampleInfo info_{};
};

/// §7.11.1.4.16: "SampleRef<T> is a reference type that combines a value
/// of type T and a value of type SampleInfo. Copying SampleRef<T> makes
/// a shall[ow] copy."
///
/// It refers to a datum and an info that something else owns — the
/// sequences of a `LoanedSamples<T>` or a `SharedSamples<T>`, or a
/// `Sample<T>` — and is valid as long as that is. `SampleRef<const T>` is
/// the value of a `const_iterator`.
template <typename T>
class SampleRef {
 public:
  typedef typename std::remove_const<T>::type Value;
  typedef typename std::conditional<std::is_const<T>::value, const dds::SampleInfo,
                                    dds::SampleInfo>::type Info;
  typedef T Data;
  typedef T& DataReference;
  typedef const T& ConstDataReference;
  typedef typename dds_type_traits<Value>::Seq Seq;
  typedef typename dds_type_traits<Value>::DataReader DataReader;
  typedef typename dds_type_traits<Value>::DataWriter DataWriter;
  using value_type = T;

  SampleRef() = default;
  SampleRef(T* data, Info* info) : data_(data), info_(info) {}
  SampleRef(T& data, Info& info) : data_(&data), info_(&info) {}
  SampleRef(Sample<Value>& sample) : data_(&sample.data()), info_(&sample.info()) {}
  /// A reference to constant data may refer to a constant sample.
  template <typename U = T, typename = typename std::enable_if<std::is_const<U>::value>::type>
  SampleRef(const Sample<Value>& sample) : data_(&sample.data()), info_(&sample.info()) {}

  SampleRef& operator=(Sample<Value>& sample) {
    data_ = &sample.data();
    info_ = &sample.info();
    return *this;
  }

  T& data() const {
    if (data_ == nullptr) {
      throw NullReferenceError("the SampleRef refers to no data");
    }
    return *data_;
  }
  Info& info() const {
    if (info_ == nullptr) {
      throw NullReferenceError("the SampleRef refers to no SampleInfo");
    }
    return *info_;
  }

  operator T&() const { return data(); }

  SampleRef<T>* operator->() { return this; }
  const SampleRef<T>* operator->() const { return this; }

  void set_data(T* data) { data_ = data; }
  void set_data(T& data) { data_ = &data; }
  void set_info(Info* info) { info_ = info; }
  void set_info(Info& info) { info_ = &info; }

  bool is_nil_data() const { return data_ == nullptr; }
  bool is_nil_info() const { return info_ == nullptr; }
  bool is_null() const { return data_ == nullptr || info_ == nullptr; }

  void swap(SampleRef& other) {
    std::swap(data_, other.data_);
    std::swap(info_, other.info_);
  }

  dds::SampleIdentity identity() const { return info().identity; }
  dds::SampleIdentity related_identity() const { return info().related_identity; }

  /// A copy of the sample this refers to, for a listener whose callback
  /// takes one.
  Sample<Value> sample() const { return Sample<Value>(data(), info()); }

 private:
  T* data_ = nullptr;
  Info* info_ = nullptr;
};

/// §7.11.1.4.17: "WriteSample<T> is a valuetype that combines a value of
/// type T and a value of type dds::SampleIdentity. The user populates
/// the value of T and the middleware populates the value of
/// dds::SampleIdentity. When a request is sent as a WriteSample, upon
/// function return, the WriteSample.identity() uniquely identifies the
/// request sent."
template <typename T>
class WriteSample {
 public:
  using value_type = T;

  WriteSample() = default;
  WriteSample(T data) : data_(std::move(data)) {}
  /// A copy of the value and the identity `reference` refers to.
  WriteSample(const WriteSampleRef<T>& reference)
      : data_(reference.data()), identity_(reference.identity()) {}

  WriteSample& operator=(const WriteSampleRef<T>& reference) {
    data_ = reference.data();
    identity_ = reference.identity();
    return *this;
  }

  T& data() { return data_; }
  const T& data() const { return data_; }
  void set_data(const T& data) { data_ = data; }
  const dds::SampleIdentity& identity() const { return identity_; }
  /// Written by the middleware, not by the user.
  void set_identity(const dds::SampleIdentity& identity) { identity_ = identity; }

  void swap(WriteSample& other) {
    using std::swap;
    swap(data_, other.data_);
    swap(identity_, other.identity_);
  }

 private:
  T data_{};
  dds::SampleIdentity identity_{};
};

/// §7.11.1.4.18: "WriteSampleRef<T> is a reference type, which groups a
/// reference to T and a value of type dds::SampleIdentity."
///
/// Made over a `WriteSample<T>` it refers to that sample and the identity
/// the middleware gives goes into it; made over a bare value it keeps the
/// identity itself.
template <typename T>
class WriteSampleRef {
 public:
  using value_type = T;

  WriteSampleRef() = default;
  WriteSampleRef(const T& data) : data_(const_cast<T*>(&data)) {}
  WriteSampleRef(WriteSample<T>& sample) : target_(&sample) {}
  /// A reference that keeps its sample alive.
  explicit WriteSampleRef(std::shared_ptr<WriteSample<T>> sample)
      : target_(sample.get()), owner_(std::move(sample)) {}

  WriteSampleRef& operator=(WriteSample<T>& sample) {
    *this = WriteSampleRef(sample);
    return *this;
  }

  const T& data() const { return const_cast<WriteSampleRef*>(this)->data(); }
  T& data() {
    if (target_ != nullptr) {
      return target_->data();
    }
    if (data_ == nullptr) {
      throw NullReferenceError("the WriteSampleRef refers to no data");
    }
    return *data_;
  }
  void set_data(const T& data) {
    if (target_ != nullptr) {
      target_->set_data(data);
    } else {
      data_ = const_cast<T*>(&data);
    }
  }
  const dds::SampleIdentity& identity() const {
    return target_ != nullptr ? target_->identity() : identity_;
  }
  void set_identity(const dds::SampleIdentity& identity) {
    if (target_ != nullptr) {
      target_->set_identity(identity);
    } else {
      identity_ = identity;
    }
  }
  void swap(WriteSampleRef& other) {
    std::swap(data_, other.data_);
    std::swap(target_, other.target_);
    std::swap(owner_, other.owner_);
    std::swap(identity_, other.identity_);
  }
  bool is_null() const { return data_ == nullptr && target_ == nullptr; }

 private:
  T* data_ = nullptr;
  WriteSample<T>* target_ = nullptr;
  std::shared_ptr<WriteSample<T>> owner_;
  dds::SampleIdentity identity_{};
};

/// §7.11.1.4.21: "SampleIterator<T> is a random-access iterator over
/// LoanedSamples<T> and SharedSamples<T>. SampleIterator<T>::value_type
/// is a SampleRef<T>."
///
/// It runs over the data sequence and the info sequence together; its
/// value is made from the pair at the position.
template <typename T, bool IsConst>
class SampleIterator {
 public:
  typedef std::random_access_iterator_tag iterator_category;

  typedef typename std::conditional<
      IsConst, typename dds_type_traits<T>::ConstSampleIteratorValueType,
      typename dds_type_traits<T>::SampleIteratorValueType>::type value_type;

  typedef value_type reference;
  typedef value_type pointer;
  typedef std::ptrdiff_t difference_type;

  typedef typename std::conditional<IsConst, typename dds_type_traits<T>::ConstSeq,
                                    typename dds_type_traits<T>::Seq>::type Seq;
  typedef typename std::conditional<IsConst, typename dds_type_traits<dds::SampleInfo>::ConstSeq,
                                    typename dds_type_traits<dds::SampleInfo>::Seq>::type InfoSeq;

  SampleIterator() = default;

  explicit SampleIterator(Seq& seq, InfoSeq& info_seq, int position = 0)
      : seq_(&seq), info_seq_(&info_seq), at_(position) {}

  /// A `const_iterator` is made from an `iterator`.
  SampleIterator(const SampleIterator<T, false>& other)
      : seq_(other.seq_), info_seq_(other.info_seq_), at_(other.at_) {}

  SampleIterator& operator=(const SampleIterator<T, false>& other) {
    seq_ = other.seq_;
    info_seq_ = other.info_seq_;
    at_ = other.at_;
    return *this;
  }

  value_type operator*() const { return element(at_); }
  value_type operator->() const { return element(at_); }
  value_type operator[](difference_type offset) const { return element(at_ + offset); }

  SampleIterator& operator++() {
    ++at_;
    return *this;
  }
  SampleIterator operator++(int) {
    SampleIterator copy = *this;
    ++at_;
    return copy;
  }
  SampleIterator& operator--() {
    --at_;
    return *this;
  }
  SampleIterator operator--(int) {
    SampleIterator copy = *this;
    --at_;
    return copy;
  }
  SampleIterator& operator-=(difference_type by) {
    at_ -= by;
    return *this;
  }
  SampleIterator& operator+=(difference_type by) {
    at_ += by;
    return *this;
  }
  SampleIterator operator-(difference_type by) const {
    SampleIterator copy = *this;
    copy.at_ -= by;
    return copy;
  }
  SampleIterator operator+(difference_type by) const {
    SampleIterator copy = *this;
    copy.at_ += by;
    return copy;
  }
  difference_type operator-(const SampleIterator& other) const { return at_ - other.at_; }

  bool operator<(const SampleIterator& other) const { return at_ < other.at_; }
  bool operator>(const SampleIterator& other) const { return at_ > other.at_; }
  bool operator<=(const SampleIterator& other) const { return at_ <= other.at_; }
  bool operator>=(const SampleIterator& other) const { return at_ >= other.at_; }
  bool operator==(const SampleIterator& other) const { return at_ == other.at_; }
  bool operator!=(const SampleIterator& other) const { return at_ != other.at_; }

  /// Whether the iterator is at or past the end of its sequence.
  bool is_end() const {
    return seq_ == nullptr || at_ < 0 || static_cast<std::size_t>(at_) >= seq_->size();
  }

 private:
  template <typename, bool>
  friend class SampleIterator;

  value_type element(difference_type at) const {
    if (seq_ == nullptr || info_seq_ == nullptr || at < 0 ||
        static_cast<std::size_t>(at) >= seq_->size() ||
        static_cast<std::size_t>(at) >= info_seq_->size()) {
      throw std::out_of_range("the SampleIterator is outside its sequences");
    }
    return value_type(&(*seq_)[static_cast<std::size_t>(at)],
                      &(*info_seq_)[static_cast<std::size_t>(at)]);
  }

  Seq* seq_ = nullptr;
  InfoSeq* info_seq_ = nullptr;
  difference_type at_ = 0;
};

/// §7.11.1.4.20: "SharedSamples<T> is container that contains loaned
/// samples from the middleware but the SharedSamples<T> object may be
/// copied in the application space. All copies of a SharedSamples<T>
/// refer to the same set of loaned samples. […] Upon destruction of the
/// last SharedSamples<T>, the underlying loaned samples are returned to
/// the middleware."
template <typename T>
class SharedSamples {
 public:
  typedef typename dds_type_traits<T>::Seq Seq;
  typedef typename dds_type_traits<T>::DataReader DataReader;

  typedef SampleIterator<T, false> iterator;
  typedef SampleIterator<T, true> const_iterator;

  typedef typename dds_type_traits<T>::SampleIteratorValueType value_type;
  typedef typename dds_type_traits<T>::ConstSampleIteratorValueType const_value_type;
  typedef std::ptrdiff_t difference_type;

  SharedSamples() : store_(std::make_shared<Store>()) {}

  Seq& data_seq() { return store_->data; }
  dds::SampleInfoSeq& info_seq() { return store_->info; }
  const Seq& data_seq() const { return store_->data; }
  const dds::SampleInfoSeq& info_seq() const { return store_->info; }

  value_type operator[](std::size_t index) { return at<value_type>(*store_, index); }
  const_value_type operator[](std::size_t index) const {
    return at<const_value_type>(*store_, index);
  }

  int length() const { return static_cast<int>(store_->data.size()); }
  bool empty() const { return store_->data.empty(); }

  void swap(SharedSamples& other) { store_.swap(other.store_); }

  iterator begin() { return iterator(store_->data, store_->info, 0); }
  iterator end() { return iterator(store_->data, store_->info, length()); }
  const_iterator begin() const {
    return const_iterator(static_cast<const Seq&>(store_->data),
                          static_cast<const dds::SampleInfoSeq&>(store_->info), 0);
  }
  const_iterator end() const {
    return const_iterator(static_cast<const Seq&>(store_->data),
                          static_cast<const dds::SampleInfoSeq&>(store_->info), length());
  }

 private:
  template <typename U>
  friend class LoanedSamples;

  struct Store {
    Seq data;
    dds::SampleInfoSeq info;
    DataReader reader;
  };

  template <typename Ref, typename Held>
  static Ref at(Held& store, std::size_t index) {
    if (index >= store.data.size() || index >= store.info.size()) {
      throw std::out_of_range("the sample index is outside the SharedSamples");
    }
    return Ref(&store.data[index], &store.info[index]);
  }

  explicit SharedSamples(std::shared_ptr<Store> store) : store_(std::move(store)) {}

  std::shared_ptr<Store> store_;
};

/// §7.11.1.4.19: "LoanedSamples<T> is conceptually a container of loaned
/// Sample<T> from the middleware. LoanedSamples<T> is not copyable but
/// it is movable. LoanedSamples<T>::value_type is SampleRef<T>. Upon
/// destruction, LoanedSamples<T> returns the loaned samples to the
/// middleware."
///
/// It holds the data sequence and the info sequence the samples were
/// taken into. A reader whose samples are copies out of the middleware
/// has nothing to give back; `return_loan()` empties the container.
template <typename T>
class LoanedSamples {
 public:
  typedef typename dds_type_traits<T>::Seq Seq;
  typedef typename dds_type_traits<T>::DataReader DataReader;

  typedef SampleIterator<T, false> iterator;
  typedef SampleIterator<T, true> const_iterator;

  typedef typename dds_type_traits<T>::SampleIteratorValueType value_type;
  typedef typename dds_type_traits<T>::ConstSampleIteratorValueType const_value_type;
  typedef std::ptrdiff_t difference_type;

  LoanedSamples() = default;

  /// The samples a reader or a replier has taken, one by one.
  explicit LoanedSamples(std::vector<Sample<T>> samples) {
    data_.reserve(samples.size());
    info_.reserve(samples.size());
    for (auto& sample : samples) {
      data_.push_back(std::move(sample.data()));
      info_.push_back(std::move(sample.info()));
    }
  }

  /// Take over the loan of `reader`: `data_seq` and `info_seq` are
  /// emptied into the container.
  static LoanedSamples<T> move_construct_from_loans(DataReader reader, Seq& data_seq,
                                                    dds::SampleInfoSeq& info_seq) {
    LoanedSamples<T> loaned;
    loaned.reader_ = std::move(reader);
    loaned.data_.swap(data_seq);
    loaned.info_.swap(info_seq);
    return loaned;
  }

  /// Hand the loan back: the sequences go to the caller and the container
  /// is empty.
  void release(DataReader& reader, Seq& data_seq, dds::SampleInfoSeq& info_seq) {
    reader = std::move(reader_);
    reader_.reset();
    data_seq.clear();
    info_seq.clear();
    data_seq.swap(data_);
    info_seq.swap(info_);
  }

  LoanedSamples(const LoanedSamples&) = delete;
  LoanedSamples& operator=(const LoanedSamples&) = delete;
  LoanedSamples(LoanedSamples&&) noexcept = default;
  LoanedSamples& operator=(LoanedSamples&&) noexcept = default;
  ~LoanedSamples() = default;

  Seq& data_seq() { return data_; }
  dds::SampleInfoSeq& info_seq() { return info_; }
  const Seq& data_seq() const { return data_; }
  const dds::SampleInfoSeq& info_seq() const { return info_; }

  value_type operator[](std::size_t index) {
    check(index);
    return value_type(&data_[index], &info_[index]);
  }
  const_value_type operator[](std::size_t index) const {
    check(index);
    return const_value_type(&data_[index], &info_[index]);
  }

  int length() const { return static_cast<int>(data_.size()); }
  bool empty() const { return data_.empty(); }

  /// Return the loan: the container is empty afterwards.
  void return_loan() {
    data_.clear();
    info_.clear();
    reader_.reset();
  }

  void swap(LoanedSamples& other) {
    data_.swap(other.data_);
    info_.swap(other.info_);
    reader_.swap(other.reader_);
  }

  /// §7.11.1.4.20: "SharedSamples<T> can be obtained from
  /// LoanedSample<T> but not vice versa."
  SharedSamples<T> to_shared() {
    auto store = std::make_shared<typename SharedSamples<T>::Store>();
    store->data.swap(data_);
    store->info.swap(info_);
    store->reader = std::move(reader_);
    reader_.reset();
    return SharedSamples<T>(std::move(store));
  }

  iterator begin() { return iterator(data_, info_, 0); }
  iterator end() { return iterator(data_, info_, length()); }
  const_iterator begin() const {
    return const_iterator(static_cast<const Seq&>(data_),
                          static_cast<const dds::SampleInfoSeq&>(info_), 0);
  }
  const_iterator end() const {
    return const_iterator(static_cast<const Seq&>(data_),
                          static_cast<const dds::SampleInfoSeq&>(info_), length());
  }

 private:
  void check(std::size_t index) const {
    if (index >= data_.size() || index >= info_.size()) {
      throw std::out_of_range("the sample index is outside the LoanedSamples");
    }
  }

  Seq data_;
  dds::SampleInfoSeq info_;
  DataReader reader_;
};

template <typename T>
typename LoanedSamples<T>::iterator begin(LoanedSamples<T>& samples) {
  return samples.begin();
}
template <typename T>
typename LoanedSamples<T>::const_iterator begin(const LoanedSamples<T>& samples) {
  return samples.begin();
}
template <typename T>
typename LoanedSamples<T>::iterator end(LoanedSamples<T>& samples) {
  return samples.end();
}
template <typename T>
typename LoanedSamples<T>::const_iterator end(const LoanedSamples<T>& samples) {
  return samples.end();
}
template <typename T>
void swap(LoanedSamples<T>& first, LoanedSamples<T>& second) noexcept {
  first.swap(second);
}
template <typename T>
typename SharedSamples<T>::iterator begin(SharedSamples<T>& samples) {
  return samples.begin();
}
template <typename T>
typename SharedSamples<T>::const_iterator begin(const SharedSamples<T>& samples) {
  return samples.begin();
}
template <typename T>
typename SharedSamples<T>::iterator end(SharedSamples<T>& samples) {
  return samples.end();
}
template <typename T>
typename SharedSamples<T>::const_iterator end(const SharedSamples<T>& samples) {
  return samples.end();
}
template <typename T>
void swap(SharedSamples<T>& first, SharedSamples<T>& second) noexcept {
  first.swap(second);
}

}  // namespace rpc

// The normative header declares the sample carriers in `dds` itself
// (§7.11.1.4.1: "The dds, and dds::rpc namespaces define the classes").
using rpc::LoanedSamples;
using rpc::Sample;
using rpc::SampleIterator;
using rpc::SampleRef;
using rpc::SharedSamples;
using rpc::WriteSample;
using rpc::WriteSampleRef;

}  // namespace dds

#endif  // ZERODDS_DDS_RPC_BASE_HPP

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// rmw_zerodds_ros2_ext.h — the ZeroDDS rmw extension surface for REP-2007
// (Type Adaptation) and REP-2008 (Hardware Acceleration).
//
// Both REPs place their user-facing API above rmw (rclcpp templates, ament
// CMake macros), and both name a concrete responsibility that only the
// middleware can discharge. This header is that responsibility, exported from
// librmw_zerodds_cpp.so next to the standard rmw_* surface.
//
// REP-2007, "Adding this feature in rclcpp":
//   "Perhaps we can support a form of this feature in other languages in rcl or
//    rmw in the future. One challenge in doing this is that it may require
//    custom type support, which may be middleware specific."
// REP-2007, "Comparison to ROS 1's Type Adaptation":
//   "This feature will support both convert and (de)serialize functions [...]
//    convert is superior for intra-process communication and the
//    (de)serialization functions are better for inter-process communication."
//   "An advantage of following the ROS 1 approach is that an extra copy can be
//    avoided; although it is likely much more challenging to implement this
//    feature the ROS 1 way because of the middleware."
// `rmw_zerodds_type_adapter_t` is that middleware-specific custom type support:
// the adapter's serialize/deserialize functions write and read the CDR body
// in place, so a custom type reaches the wire without an intermediate
// ros_message_type instance — the avoided extra copy, implemented in the
// middleware.
//
// REP-2008, "Kernel levels in ROS 2":
//   "Level 3 - ROS 2 underlayers: Groups together all accelerators below the
//    ROS 2 core layers belonging to OSI L2-L7, including the communication
//    middleware (e.g. DDS)."
// An accelerator runtime (CUDA, ROCm, XRT) must pin or register host memory
// before it can DMA into it. `rmw_zerodds_loan_memory_t` reports the exact
// page-aligned extent of a loaned message buffer, plus whether that memory is
// shareable across processes, so the accelerator registers the buffer that DDS
// actually sends — with no staging copy between the accelerator and the wire.
//
// Wire invariant for both: an endpoint using this surface is byte-identical on
// the wire to one that does not. The DDS topic name, the ROS type name and the
// RIHS type hash come from the endpoint's rosidl type support and are untouched
// by adaptation, so an adapted publisher matches a plain subscription of the
// ros_message_type and vice versa.

#ifndef RMW_ZERODDS_ROS2_EXT_H_
#define RMW_ZERODDS_ROS2_EXT_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include <rmw/rmw.h>
#include <rmw/types.h>

#ifdef __cplusplus
extern "C" {
#endif

// ===========================================================================
// REP-2007 — Type Adaptation: middleware-specific custom type support
// ===========================================================================

/// The (de)serialize half of a REP-2007 type adapter, expressed at the rmw
/// layer. `custom` always points at the REP's `custom_type`; the middleware
/// never materializes the `ros_message_type`.
///
/// `serialize` writes the **CDR body** — the bytes that follow the 4-byte
/// encapsulation header — exactly as the rosidl type support for the
/// `ros_message_type` would. `deserialize` reads that same body. The
/// encapsulation header is written and consumed by the middleware, so an
/// adapter never has to know the representation identifier.
typedef struct rmw_zerodds_type_adapter_s
{
  /// The ROS type this adapter stands in for, e.g. "std_msgs/msg/String".
  /// Informational: the wire type name and RIHS hash come from the endpoint's
  /// rosidl type support, never from this field.
  const char * ros_type_name;

  /// Exact number of CDR body bytes `serialize` will write for `custom`.
  size_t (* serialized_size)(const void * custom);

  /// Writes the CDR body for `custom` into `buffer` (`capacity` bytes) and
  /// reports the written length. Returns 0 on success, non-zero on failure.
  int (* serialize)(const void * custom, uint8_t * buffer, size_t capacity, size_t * written);

  /// Reads a CDR body of `length` bytes into `custom`. `big_endian` is the
  /// wire byte order taken from the encapsulation header. Returns 0 on
  /// success, non-zero on failure.
  int (* deserialize)(const uint8_t * buffer, size_t length, bool big_endian, void * custom);
} rmw_zerodds_type_adapter_t;

/// Binds an adapter to a publisher created with the `ros_message_type`'s type
/// support. Pass NULL to unbind. The publisher keeps working unchanged for
/// `rmw_publish`.
rmw_ret_t rmw_zerodds_publisher_set_type_adapter(
  rmw_publisher_t * publisher, const rmw_zerodds_type_adapter_t * adapter);

/// Binds an adapter to a subscription. Pass NULL to unbind.
rmw_ret_t rmw_zerodds_subscription_set_type_adapter(
  rmw_subscription_t * subscription, const rmw_zerodds_type_adapter_t * adapter);

/// REP-2007 "Type Adaptation in Topics", publish side. Serializes `custom`
/// straight into the wire buffer — one serialization, no intermediate
/// ros_message_type, no second copy.
rmw_ret_t rmw_zerodds_publish_adapted(
  const rmw_publisher_t * publisher, const void * custom);

/// REP-2007 "Type Adaptation in Topics", take side. Deserializes the CDR body
/// straight into `custom`.
rmw_ret_t rmw_zerodds_take_adapted(
  const rmw_subscription_t * subscription, void * custom, bool * taken,
  rmw_message_info_t * message_info);

/// REP-2007 "Type Adaptation in Services": adapters for the request and the
/// response half, either of which may be NULL to leave that half unadapted.
rmw_ret_t rmw_zerodds_client_set_type_adapters(
  rmw_client_t * client, const rmw_zerodds_type_adapter_t * request,
  const rmw_zerodds_type_adapter_t * response);

rmw_ret_t rmw_zerodds_service_set_type_adapters(
  rmw_service_t * service, const rmw_zerodds_type_adapter_t * request,
  const rmw_zerodds_type_adapter_t * response);

rmw_ret_t rmw_zerodds_send_request_adapted(
  const rmw_client_t * client, const void * custom_request, int64_t * sequence_id);

rmw_ret_t rmw_zerodds_take_response_adapted(
  const rmw_client_t * client, rmw_service_info_t * header, void * custom_response,
  bool * taken);

rmw_ret_t rmw_zerodds_take_request_adapted(
  const rmw_service_t * service, rmw_service_info_t * header, void * custom_request,
  bool * taken);

rmw_ret_t rmw_zerodds_send_response_adapted(
  const rmw_service_t * service, rmw_request_id_t * header, const void * custom_response);

// ===========================================================================
// REP-2008 — Hardware Acceleration: accelerator-registerable loan memory
// ===========================================================================

/// Where a loaned message buffer lives.
typedef enum rmw_zerodds_loan_memory_kind_e
{
  /// Portable delivery: process-local host memory. Registerable by an
  /// accelerator runtime in this process; not mappable by another process.
  RMW_ZERODDS_LOAN_MEMORY_PROCESS_HEAP = 0,
  /// RawSameHost delivery: a slot in a file-linked POSIX shared-memory
  /// segment. Registerable and mappable by any process on the host.
  RMW_ZERODDS_LOAN_MEMORY_SHARED_SEGMENT = 1,
  /// Iceoryx delivery: a slot in an iceoryx2-managed shared segment.
  RMW_ZERODDS_LOAN_MEMORY_ICEORYX = 2
} rmw_zerodds_loan_memory_kind_t;

/// The memory provenance of one loaned message buffer.
///
/// `registration_base` / `registration_size` are the page-aligned extent that
/// covers the buffer — the arguments an accelerator runtime pins:
/// `cudaHostRegister(base, size, cudaHostRegisterDefault)`,
/// `hipHostRegister(...)`, `xrt::bo(device, base, size, ...)`.
typedef struct rmw_zerodds_loan_memory_s
{
  rmw_zerodds_loan_memory_kind_t kind;
  /// The loaned buffer as handed out by rmw_borrow_loaned_message.
  void * host_ptr;
  /// Its exact byte extent (the in-memory message struct size).
  size_t size;
  /// Page-aligned base at or below `host_ptr`.
  void * registration_base;
  /// Page-multiple length covering [host_ptr, host_ptr + size).
  size_t registration_size;
  /// The host page size the two fields above are aligned to.
  size_t page_size;
  /// True when another process on the host can map the same bytes.
  bool cross_process;
  /// Filesystem link of the shared segment (SHARED_SEGMENT) or the iceoryx2
  /// service name (ICEORYX); empty string for PROCESS_HEAP.
  char segment_link[256];
} rmw_zerodds_loan_memory_t;

/// Reports the provenance of a buffer obtained from rmw_borrow_loaned_message.
rmw_ret_t rmw_zerodds_publisher_get_loan_memory(
  const rmw_publisher_t * publisher, const void * loaned_message,
  rmw_zerodds_loan_memory_t * out);

/// Reports the provenance of a buffer obtained from rmw_take_loaned_message.
rmw_ret_t rmw_zerodds_subscription_get_loan_memory(
  const rmw_subscription_t * subscription, const void * loaned_message,
  rmw_zerodds_loan_memory_t * out);

/// Requests that subsequent Portable-mode loans from this publisher start on an
/// `alignment`-byte boundary. `alignment` must be a power of two and a multiple
/// of sizeof(void *); pass the host page size to make every loan pinnable
/// without the accelerator runtime having to pin a neighbour's page. Shared
/// segments are page-aligned by construction and ignore this setting.
///
/// Default: the value of the environment variable `ZERODDS_LOAN_ALIGNMENT`, or
/// the platform's natural malloc alignment when it is unset.
rmw_ret_t rmw_zerodds_publisher_set_loan_alignment(
  rmw_publisher_t * publisher, size_t alignment);

/// REP-2008 kernel level support declared by this rmw implementation, as a
/// bitmask of (1 << (level - 1)). Consumers and the "Template for Vendors"
/// README read the same value.
#define RMW_ZERODDS_ACCEL_LEVEL_1 0x1u  // ROS 2 applications and libraries
#define RMW_ZERODDS_ACCEL_LEVEL_2 0x2u  // ROS 2 core packages (rcl/rmw/rmw_adapters)
#define RMW_ZERODDS_ACCEL_LEVEL_3 0x4u  // ROS 2 underlayers (the middleware itself)

/// The kernel levels this build serves. Level 3 is unconditional (ZeroDDS *is*
/// the underlayer); level 2 is reported when the loan path can hand an
/// accelerator a registerable buffer.
uint32_t rmw_zerodds_acceleration_kernel_levels(void);

#ifdef __cplusplus
}  // extern "C"
#endif

#endif  // RMW_ZERODDS_ROS2_EXT_H_

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// rmw_zerodds.c — the ABI-correct ROS 2 `rmw` implementation layer for ZeroDDS.
//
// rmw_implementation (rcl/rclpy) `dlopen`s lib<RMW_IMPLEMENTATION>.so and
// resolves the plain `rmw_*` symbols. The Rust crate `rmw-zerodds-shim` provides
// a *simplified* DDS bridge (`rmw_zerodds_*`: raw-CDR publish/take over the
// ZeroDDS C-API) but NOT the real rmw ABI. This C translation unit is compiled
// against the actual Humble rmw headers (so every struct layout is ABI-correct
// by construction) and exports the real `rmw_*` surface, bridging to the Rust
// `rmw_zerodds_*` functions for the DDS work.
//
// Build (on a host with ROS 2 Humble dev headers):
//   gcc -shared -fPIC -o librmw_zerodds_cpp.so rmw_zerodds.c \
//       -I$ROS/include/rmw/... <other -I> -lrmw_zerodds -lzerodds
//
// The four rmw_*_allocation functions return RMW_RET_UNSUPPORTED (nothing is
// pre-allocated); other entry points return it only for a case they do not
// serve, and set an error message.

#include <stdlib.h>
#include <string.h>

#include <rcutils/allocator.h>
#include <rcutils/logging_macros.h>
#include <rcutils/strdup.h>
#include <rcutils/types/uint8_array.h>
#include <rmw/rmw.h>
#include <rmw/error_handling.h>
#include <rmw/event.h>
#include <rmw/events_statuses/events_statuses.h>
#include <rmw/init.h>
#include <rmw/init_options.h>
// Iron and later carry `discovery_options` in the init options, with an
// allocated static-peer array that a copy has to duplicate and a fini has
// to release. Humble has neither the header nor the field.
#if defined(__has_include)
#if __has_include(<rmw/discovery_options.h>)
#include <rmw/discovery_options.h>
#define ZERODDS_RMW_HAS_DISCOVERY_OPTIONS 1
#endif
#endif
#ifndef ZERODDS_RMW_HAS_DISCOVERY_OPTIONS
#define ZERODDS_RMW_HAS_DISCOVERY_OPTIONS 0
#endif
#include <rmw/allocators.h>
#include <rmw/features.h>
#include <rmw/names_and_types.h>
#include <rmw/network_flow_endpoint_array.h>
#include <rmw/get_network_flow_endpoints.h>
// Rolling carries endpoint information for services (`ros2 service info
// --verbose`); earlier distributions have neither the headers nor the two
// entry points, and their rmw_implementation does not look them up.
#if defined(__has_include)
#if __has_include(<rmw/get_service_endpoint_info.h>)
#include <rmw/get_service_endpoint_info.h>
#include <rmw/service_endpoint_info_array.h>
#define ZERODDS_RMW_HAS_SERVICE_ENDPOINT_INFO 1
#endif
#endif
#ifndef ZERODDS_RMW_HAS_SERVICE_ENDPOINT_INFO
#define ZERODDS_RMW_HAS_SERVICE_ENDPOINT_INFO 0
#endif
// Iron and later give every type support a type hash (REP 2011), which every
// RMW announces in USER_DATA as `typehash=RIHS01_<64 hex>;` and reads back
// into the endpoint info. Humble has neither the header nor the hash.
#if defined(__has_include)
#if __has_include(<rosidl_runtime_c/type_hash.h>)
#include <rosidl_runtime_c/type_hash.h>
#define ZERODDS_RMW_HAS_TYPE_HASH 1
#endif
#endif
#ifndef ZERODDS_RMW_HAS_TYPE_HASH
#define ZERODDS_RMW_HAS_TYPE_HASH 0
#endif

// REP-2007 type adaptation + REP-2008 accelerator loan memory (the two REP
// responsibilities that live at the rmw layer). Declares the extension surface
// this translation unit exports next to the standard rmw_* symbols.
#include "rmw_zerodds_ros2_ext.h"

#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <time.h>
#include <unistd.h>

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

static const char * const ZERODDS_IDENTIFIER = "rmw_zerodds_cpp";
static const char * const ZERODDS_SERIALIZATION_FORMAT = "cdr";

const char *
rmw_get_implementation_identifier(void)
{
  return ZERODDS_IDENTIFIER;
}

const char *
rmw_get_serialization_format(void)
{
  return ZERODDS_SERIALIZATION_FORMAT;
}

// ---------------------------------------------------------------------------
// init options (REP-2007 §4) — real, allocator-aware.
// ---------------------------------------------------------------------------

rmw_ret_t
rmw_init_options_init(rmw_init_options_t * init_options, rcutils_allocator_t allocator)
{
  if (!init_options) {
    RMW_SET_ERROR_MSG("init_options is null");
    return RMW_RET_INVALID_ARGUMENT;
  }
  if (init_options->implementation_identifier != NULL) {
    RMW_SET_ERROR_MSG("expected zero-initialized init_options");
    return RMW_RET_INVALID_ARGUMENT;
  }
  init_options->instance_id = 0;
  init_options->implementation_identifier = ZERODDS_IDENTIFIER;
  init_options->allocator = allocator;
  init_options->impl = NULL;
  init_options->enclave = NULL;
  init_options->domain_id = RMW_DEFAULT_DOMAIN_ID;
  init_options->security_options = rmw_get_zero_initialized_security_options();
  // `localhost_only` is not set: Kilted removed the field (its discovery
  // range is `discovery_options`), and on Humble and Jazzy the
  // zero-initialized options this function insists on already hold
  // RMW_LOCALHOST_ONLY_DEFAULT, which rmw_get_zero_initialized_init_options
  // puts there.
  return RMW_RET_OK;
}

rmw_ret_t
rmw_init_options_copy(const rmw_init_options_t * src, rmw_init_options_t * dst)
{
  if (!src || !dst) {
    RMW_SET_ERROR_MSG("src/dst is null");
    return RMW_RET_INVALID_ARGUMENT;
  }
  if (!src->implementation_identifier) {
    RMW_SET_ERROR_MSG("src not initialized");
    return RMW_RET_INVALID_ARGUMENT;
  }
  if (src->implementation_identifier != ZERODDS_IDENTIFIER) {
    RMW_SET_ERROR_MSG("src from another rmw implementation");
    return RMW_RET_INCORRECT_RMW_IMPLEMENTATION;
  }
  if (dst->implementation_identifier != NULL) {
    RMW_SET_ERROR_MSG("expected zero-initialized dst");
    return RMW_RET_INVALID_ARGUMENT;
  }
  const rcutils_allocator_t * alloc = &src->allocator;
  *dst = *src;
  dst->enclave = NULL;
  if (src->enclave) {
    dst->enclave = rcutils_strdup(src->enclave, *alloc);
    if (!dst->enclave) {
      return RMW_RET_BAD_ALLOC;
    }
  }
  // A deep copy of the security options, as rmw_init_options_copy requires;
  // the shallow `*dst = *src` shared security_root_path with the source.
  dst->security_options = rmw_get_zero_initialized_security_options();
  rmw_ret_t sret = rmw_security_options_copy(&src->security_options, alloc,
    &dst->security_options);
  if (sret != RMW_RET_OK) {
    if (dst->enclave) {
      alloc->deallocate(dst->enclave, alloc->state);
      dst->enclave = NULL;
    }
    return sret;
  }
#if ZERODDS_RMW_HAS_DISCOVERY_OPTIONS
  // Since Iron the options carry `discovery_options`, whose static peers
  // are an allocated array; the shallow `*dst = *src` shared it with the
  // source, and rmw_init_options_fini never released it.
  dst->discovery_options = rmw_get_zero_initialized_discovery_options();
  rcutils_allocator_t copy_alloc = *alloc;
  rmw_ret_t dret = rmw_discovery_options_copy(&src->discovery_options, &copy_alloc,
    &dst->discovery_options);
  if (dret != RMW_RET_OK) {
    { rmw_ret_t __r = rmw_security_options_fini(&dst->security_options, &copy_alloc); (void)__r; }
    if (dst->enclave) {
      alloc->deallocate(dst->enclave, alloc->state);
      dst->enclave = NULL;
    }
    return dret;
  }
#endif
  return RMW_RET_OK;
}

rmw_ret_t
rmw_init_options_fini(rmw_init_options_t * init_options)
{
  if (!init_options) {
    RMW_SET_ERROR_MSG("init_options is null");
    return RMW_RET_INVALID_ARGUMENT;
  }
  if (!init_options->implementation_identifier) {
    RMW_SET_ERROR_MSG("init_options not initialized");
    return RMW_RET_INVALID_ARGUMENT;
  }
  rcutils_allocator_t * alloc = &init_options->allocator;
  if (init_options->enclave) {
    alloc->deallocate(init_options->enclave, alloc->state);
  }
  { rmw_ret_t __r = rmw_security_options_fini(&init_options->security_options, alloc); (void)__r; }
#if ZERODDS_RMW_HAS_DISCOVERY_OPTIONS
  { rmw_ret_t __r = rmw_discovery_options_fini(&init_options->discovery_options); (void)__r; }
#endif
  *init_options = rmw_get_zero_initialized_init_options();
  return RMW_RET_OK;
}

// ---------------------------------------------------------------------------
// init / shutdown / context — wires the ZeroDDS runtime via the Rust bridge.
// ---------------------------------------------------------------------------

// Rust bridge (librmw_zerodds.so): create/destroy a ZeroDDS runtime context.
extern void * rmw_zerodds_init(unsigned int domain_id);
extern void * rmw_zerodds_init_with_security(unsigned int domain_id,
  const char * security_root_path, int enforce);
extern int rmw_zerodds_shutdown(void * ctx);
extern int rmw_zerodds_context_fini(void * ctx);

struct rmw_context_impl_s
{
  void * bridge_ctx;  // RmwZerodsContext* from rmw_zerodds_init
};

rmw_ret_t
rmw_init(const rmw_init_options_t * options, rmw_context_t * context)
{
  if (!options || !context) {
    RMW_SET_ERROR_MSG("options/context is null");
    return RMW_RET_INVALID_ARGUMENT;
  }
  if (!options->implementation_identifier) {
    RMW_SET_ERROR_MSG("options not initialized");
    return RMW_RET_INVALID_ARGUMENT;
  }
  if (options->implementation_identifier != ZERODDS_IDENTIFIER) {
    RMW_SET_ERROR_MSG("options from another rmw implementation");
    return RMW_RET_INCORRECT_RMW_IMPLEMENTATION;
  }

  size_t domain = options->domain_id;
  if (domain == RMW_DEFAULT_DOMAIN_ID) {
    domain = 0;
  }

  *context = rmw_get_zero_initialized_context();
  context->instance_id = options->instance_id;
  context->implementation_identifier = ZERODDS_IDENTIFIER;
  context->actual_domain_id = domain;

  rmw_ret_t ret = rmw_init_options_copy(options, &context->options);
  if (ret != RMW_RET_OK) {
    return ret;
  }

  context->impl = calloc(1, sizeof(struct rmw_context_impl_s));
  if (!context->impl) {
    { rmw_ret_t __r = rmw_init_options_fini(&context->options); (void)__r; }
    return RMW_RET_BAD_ALLOC;
  }
  // The security options rcl resolved from ROS_SECURITY_ENABLE /
  // ROS_SECURITY_STRATEGY / ROS_SECURITY_KEYSTORE and the node's enclave.
  context->impl->bridge_ctx = rmw_zerodds_init_with_security((unsigned int) domain,
    options->security_options.security_root_path,
    options->security_options.enforce_security == RMW_SECURITY_ENFORCEMENT_ENFORCE);
  if (!context->impl->bridge_ctx) {
    free(context->impl);
    context->impl = NULL;
    { rmw_ret_t __r = rmw_init_options_fini(&context->options); (void)__r; }
    RMW_SET_ERROR_MSG("zerodds runtime init failed");
    return RMW_RET_ERROR;
  }
  return RMW_RET_OK;
}

rmw_ret_t
rmw_shutdown(rmw_context_t * context)
{
  if (!context || !context->impl) {
    RMW_SET_ERROR_MSG("context not initialized");
    return RMW_RET_INVALID_ARGUMENT;
  }
  // Logical shutdown only — keep bridge_ctx alive so entities created from this
  // context (nodes/publishers/…) can still be destroyed afterwards. The actual
  // free happens in rmw_context_fini. rclcpp::shutdown() is commonly called
  // while nodes are still in scope; freeing here would dangle their ctx pointer.
  if (context->impl->bridge_ctx) {
    rmw_zerodds_shutdown(context->impl->bridge_ctx);
  }
  return RMW_RET_OK;
}

rmw_ret_t
rmw_context_fini(rmw_context_t * context)
{
  if (!context || !context->impl) {
    RMW_SET_ERROR_MSG("context not initialized");
    return RMW_RET_INVALID_ARGUMENT;
  }
  if (context->impl->bridge_ctx) {
    rmw_zerodds_context_fini(context->impl->bridge_ctx);
    context->impl->bridge_ctx = NULL;
  }
  free(context->impl);
  context->impl = NULL;
  { rmw_ret_t __r = rmw_init_options_fini(&context->options); (void)__r; }
  *context = rmw_get_zero_initialized_context();
  return RMW_RET_OK;
}

// ===========================================================================
// Bridge externs (librmw_zerodds.so) — the simplified DDS API.
// ===========================================================================
extern void * rmw_zerodds_create_node(void * ctx, const char * name, const char * ns);
extern int rmw_zerodds_destroy_node(void * node);
// rmw_qos_profile_t flattened for the Rust side (RmwZerodsQos, repr(C)):
// the rmw enum values unchanged, rmw_time_t as nanoseconds with 0 for both
// RMW_DURATION_UNSPECIFIED and RMW_DURATION_INFINITE.
typedef struct zerodds_rmw_qos_s
{
  uint32_t history;
  uint64_t depth;
  uint32_t reliability;
  uint32_t durability;
  uint64_t deadline_ns;
  uint64_t lifespan_ns;
  uint32_t liveliness;
  uint64_t liveliness_lease_ns;
  uint32_t avoid_ros_namespace_conventions;
} zerodds_rmw_qos_t;

extern void * rmw_zerodds_create_publisher_hashed(void * node, const char * type_name,
                                                  const char * topic_name,
                                                  const zerodds_rmw_qos_t * qos,
                                                  zerodds_rmw_qos_t * actual,
                                                  const char * type_hash);
extern int rmw_zerodds_publisher_assert_liveliness(void * pub);
extern int rmw_zerodds_publisher_wait_for_all_acked(void * pub, uint64_t timeout_ns);
extern int rmw_zerodds_destroy_publisher(void * pub);
extern size_t rmw_zerodds_publisher_matched_count(void * pub);
extern size_t rmw_zerodds_subscription_matched_count(void * sub);
extern int rmw_zerodds_publish(void * pub, const unsigned char * data, size_t len);
extern void * rmw_zerodds_create_subscription_hashed(void * node, const char * type_name,
                                                     const char * topic_name,
                                                     const zerodds_rmw_qos_t * qos,
                                                     zerodds_rmw_qos_t * actual,
                                                     const char * type_hash);
extern int rmw_zerodds_destroy_subscription(void * sub);
extern int rmw_zerodds_take(void * sub, unsigned char ** out_buf, size_t * out_len, unsigned char * out_big_endian);
extern void rmw_zerodds_buffer_free(unsigned char * buf, size_t len);
extern void * rmw_zerodds_create_wait_set(void);
extern int rmw_zerodds_destroy_wait_set(void * ws);
extern int rmw_zerodds_wait_set_add_subscription(void * ws, void * sub);
extern int rmw_zerodds_wait(void * ws, unsigned long timeout_ms);
// Event-driven readiness bridge (P1): non-consuming has-data peek + condvar
// block parked on the context's shared wakeup edge (no spin, no fixed tick).
extern int rmw_zerodds_subscription_has_data(void * sub);
extern unsigned long long rmw_zerodds_context_wait_generation(void * ctx);
extern int rmw_zerodds_context_wait_block(void * ctx, unsigned long long since_gen,
                                          unsigned long long timeout_ms);
extern int rmw_zerodds_context_notify(void * ctx);

#include <rosidl_runtime_c/message_type_support_struct.h>
#include <rosidl_typesupport_introspection_c/message_introspection.h>
#include <rosidl_typesupport_introspection_c/identifier.h>
#include <rmw/get_topic_endpoint_info.h>
#include <rmw/topic_endpoint_info_array.h>
#include <rmw/topic_endpoint_info.h>

// The C++ introspection identifier. The cpp typesupport variable lives in a C++
// namespace (mangled symbol), so from this C TU we match it by string — the
// rosidl typesupport resolver (`ts->func`) compares identifiers with strcmp, so
// a plain literal resolves the handle. The introspection_c and introspection_cpp
// `MessageMembers`/`MessageMember` structs are deliberately layout-identical for
// every field we read (member_count_, size_of_, members_, type_id_, offset_,
// is_array_, array_size_, is_upper_bound_), so the cpp data can be read through
// the C struct. (Strings differ in memory — std::string vs rosidl C string — but
// loaning requires fixed-POD, and the rclpy path uses C typesupport.)
static const char ZERODDS_INTROSPECTION_CPP_ID[] = "rosidl_typesupport_introspection_cpp";

// Resolve the introspection MessageMembers from any rosidl message typesupport
// (handles both the C and C++ typesupport wrappers) and build a stable
// "ns::name" type string.
// `is_cpp` (optional out) reports which introspection flavour was resolved.
// The MessageMembers layout is identical for both, but the runtime type of a
// string/wstring member is not: the C flavour uses rosidl_runtime_c__String /
// __U16String, the C++ flavour std::string / std::u16string. Decoding a
// std::string with the C assign corrupts memory, so every codec entry point
// carries the flavour (see rmw_zerodds_cxx.cpp).
static const rosidl_typesupport_introspection_c__MessageMembers *
zerodds_introspect_ex(const rosidl_message_type_support_t * ts, int * is_cpp)
{
  if (is_cpp) { *is_cpp = 0; }
  if (!ts) { return NULL; }
  // Already an introspection handle?
  if (ts->typesupport_identifier == rosidl_typesupport_introspection_c__identifier && ts->data) {
    return (const rosidl_typesupport_introspection_c__MessageMembers *) ts->data;
  }
  if (ts->func != NULL) {
    // Prefer the C introspection (rclpy / C messages) …
    const rosidl_message_type_support_t * h =
      ts->func(ts, rosidl_typesupport_introspection_c__identifier);
    if (h && h->data) {
      return (const rosidl_typesupport_introspection_c__MessageMembers *) h->data;
    }
    // … then fall back to the C++ introspection (rclcpp messages). The miss
    // above left its error message set; the fallback is the answer, so the
    // message is cleared rather than left for the next rcl call to
    // overwrite (the rmw_fastrtps lookup does the same).
    rcutils_reset_error();
    h = ts->func(ts, ZERODDS_INTROSPECTION_CPP_ID);
    if (h && h->data) {
      if (is_cpp) { *is_cpp = 1; }
      return (const rosidl_typesupport_introspection_c__MessageMembers *) h->data;
    }
  }
  // The handle itself may be the cpp introspection (identifier matched by string).
  if (ts->typesupport_identifier && ts->data &&
      strcmp(ts->typesupport_identifier, ZERODDS_INTROSPECTION_CPP_ID) == 0)
  {
    if (is_cpp) { *is_cpp = 1; }
    return (const rosidl_typesupport_introspection_c__MessageMembers *) ts->data;
  }
  return NULL;
}

static const rosidl_typesupport_introspection_c__MessageMembers *
zerodds_introspect(const rosidl_message_type_support_t * ts)
{
  return zerodds_introspect_ex(ts, NULL);
}

// The DDS type name of a ROS message, as every ROS 2 middleware announces it:
// the introspection namespace with `__` as `::`, then `::dds_::`, the message
// name and a trailing `_` — `std_msgs__msg` + `Int32` is
// `std_msgs::msg::dds_::Int32_`. The RMW announced `std_msgs__msg::Int32`,
// which no other middleware's reader matches, so it exchanged data with
// ZeroDDS nodes only. Service request/response members carry their own
// names (`AddTwoInts_Request`) and take the same form.
static char * zerodds_dds_type_name(const char * ns, const char * nm)
{
  size_t len = 2 * strlen(ns) + strlen("::dds_::") + strlen(nm) + 2;
  char * out = (char *) malloc(len);
  if (!out) { return NULL; }
  size_t o = 0;
  for (const char * p = ns; *p; ++p) {
    if (p[0] == '_' && p[1] == '_') { out[o++] = ':'; out[o++] = ':'; ++p; }
    else { out[o++] = *p; }
  }
  snprintf(out + o, len - o, "::dds_::%s_", nm);
  return out;
}

static char *
zerodds_type_name(const rosidl_message_type_support_t * ts)
{
  const rosidl_typesupport_introspection_c__MessageMembers * m = zerodds_introspect(ts);
  const char * ns = (m && m->message_namespace_) ? m->message_namespace_ : "rosidl";
  const char * nm = (m && m->message_name_) ? m->message_name_ : "Msg";
  return zerodds_dds_type_name(ns, nm);
}

#include <stdint.h>
#include <rosidl_typesupport_introspection_c/service_introspection.h>

// Bridge externs (librmw_zerodds.so, `src/services.rs`) for services: the
// topics rq/<service>Request and rr/<service>Reply, correlated in the form the
// peer's RMW uses (rmw_cyclonedds_cpp: a guid/seq header in the payload;
// rmw_fastrtps_cpp: the related sample identity in the inline QoS). Streams
// cross this boundary as [CDR_LE encapsulation][body]; the request id rcl
// holds is what take_request gives and send_response takes back.
extern void * rmw_zerodds_create_client_hashed(void * node, const char * service,
                                               const char * request_type, const char * response_type,
                                               const zerodds_rmw_qos_t * qos,
                                               zerodds_rmw_qos_t * actual, const char * type_hash);
extern int rmw_zerodds_destroy_client(void * client);
extern int rmw_zerodds_send_request(void * client, const unsigned char * data, size_t len, int64_t * out_seq);
extern int rmw_zerodds_take_response(void * client, unsigned char ** out_buf, size_t * out_len,
                                     unsigned char * out_big_endian, int64_t * out_seq);
extern int rmw_zerodds_client_has_data(void * client);
extern int rmw_zerodds_client_server_available(void * client);
extern void * rmw_zerodds_create_service_hashed(void * node, const char * service,
                                                const char * request_type, const char * response_type,
                                                const zerodds_rmw_qos_t * qos,
                                                zerodds_rmw_qos_t * actual, const char * type_hash);

// The type hash of a type support as rosidl_stringify_type_hash writes it
// ("RIHS01_<64 hex>"), allocated with the default allocator and released with
// zerodds_free_type_hash; NULL where the distribution or the type support has
// none. The shim announces it in USER_DATA.
#if ZERODDS_RMW_HAS_TYPE_HASH
static char * zerodds_stringify_type_hash(const rosidl_type_hash_t * h)
{
  if (!h || h->version == ROSIDL_TYPE_HASH_VERSION_UNSET) { return NULL; }
  char * out = NULL;
  if (rosidl_stringify_type_hash(h, rcutils_get_default_allocator(), &out) != RCUTILS_RET_OK) {
    return NULL;
  }
  return out;
}
#endif
static char * zerodds_message_type_hash(const rosidl_message_type_support_t * ts)
{
#if ZERODDS_RMW_HAS_TYPE_HASH
  if (!ts || !ts->get_type_hash_func) { return NULL; }
  return zerodds_stringify_type_hash(ts->get_type_hash_func(ts));
#else
  (void) ts;
  return NULL;
#endif
}
static char * zerodds_service_type_hash(const rosidl_service_type_support_t * ts)
{
#if ZERODDS_RMW_HAS_TYPE_HASH
  if (!ts || !ts->get_type_hash_func) { return NULL; }
  return zerodds_stringify_type_hash(ts->get_type_hash_func(ts));
#else
  (void) ts;
  return NULL;
#endif
}
static void zerodds_free_type_hash(char * h)
{
  if (!h) { return; }
  rcutils_allocator_t a = rcutils_get_default_allocator();
  a.deallocate(h, a.state);
}
extern int rmw_zerodds_destroy_service(void * service);
extern int rmw_zerodds_take_request(void * service, unsigned char ** out_buf, size_t * out_len,
                                    unsigned char * out_big_endian, unsigned char * out_writer_guid,
                                    int64_t * out_seq);
extern int rmw_zerodds_send_response(void * service, const unsigned char * writer_guid, int64_t seq,
                                     const unsigned char * data, size_t len);
extern int rmw_zerodds_service_has_data(void * service);
// on_new_* event callbacks (EventsExecutor) — fired on each arrival.
extern int rmw_zerodds_subscription_set_event_callback(void * sub, rmw_event_callback_t cb, const void * ud);
extern int rmw_zerodds_service_set_event_callback(void * service, rmw_event_callback_t cb, const void * ud);
extern int rmw_zerodds_client_set_event_callback(void * client, rmw_event_callback_t cb, const void * ud);

// REP-2007 service/client adapter-table release (defined with the adaptation
// section at the end); called from rmw_destroy_client/_service so a destroyed
// handle never keeps a table slot (a later handle could reuse the address).
static void zerodds_svc_adapters_release(const void * handle);

// Loaning helpers (defined after the introspection CDR section): a message is
// loanable iff it is a fixed POD (no strings/sequences); `members` is the
// introspection MessageMembers (as void* so the prototype needs no typedef).
static int zerodds_msg_can_loan(const void * members);
static size_t zerodds_msg_struct_size(const void * members);

// Same-host zero-copy loaning bridge (delivery mode `RawSameHost`,
// `zerodds-delivery-modes-1.0`). The shim is built with `flatdata-loan` (default
// on), so these resolve in librmw_zerodds.so.
extern int rmw_zerodds_publisher_enable_raw_loan(void * pub, const char * name, size_t slots, size_t cap);
extern int rmw_zerodds_publisher_loan(void * pub, size_t len, unsigned char ** out_ptr);
extern int rmw_zerodds_publisher_commit(void * pub, unsigned char * ptr, size_t len);
extern int rmw_zerodds_publisher_discard(void * pub, unsigned char * ptr, size_t len);
extern int rmw_zerodds_subscription_enable_shm(void * sub, const char * name, unsigned char reader_index);
extern int rmw_zerodds_subscription_take_shm(void * sub, const unsigned char ** out_ptr, size_t * out_len, unsigned int * out_slot);
extern int rmw_zerodds_subscription_has_shm_data(void * sub);
extern int rmw_zerodds_subscription_release_shm(void * sub, unsigned int slot_index);
// Iceoryx delivery (mode 2). Always linkable; return RMW_RET_UNSUPPORTED (3) when
// the shim is built without `delivery-iceoryx`.
extern int rmw_zerodds_publisher_enable_iceoryx(void * pub, const char * name, size_t max_len);
extern int rmw_zerodds_publisher_gid(void * pub, uint8_t * out16);
extern int rmw_zerodds_client_gid(void * client, unsigned char * out16);
// QoS events (defined with rmw_take_event below): whether an event's
// counter moved since the last take; rmw_wait asks it.
static int zerodds_event_pending(const rmw_event_t * e);
extern int rmw_zerodds_subscription_enable_iceoryx(void * sub, const char * name);
// Starts the raw-mode doorbell (event-driven rmw_wait wakeups). Idempotent.
extern int rmw_zerodds_subscription_start_doorbell(void * sub);

// Delivery mode (zerodds-delivery-modes-1.0 §4): the participant default is
// the runtime setting `delivery.mode` (env `ZERODDS_DELIVERY_MODE`, or a
// configuration file), parsed and validated once by the Rust runtime — a
// misspelling fails rmw_init instead of silently selecting Portable.
// 0 = Portable (default, interop-safe), 1 = RawSameHost (same-host zero-copy
// SHM, no wire), 2 = Iceoryx (same-host cross-stack via iceoryx2). Modes 1/2
// share the loan/commit/take_shm surface; they differ only in the
// writer/reader enable call.
extern int rmw_zerodds_delivery_mode(const void * ctx);
static int zerodds_delivery_mode(const rmw_node_t * node)
{
  return rmw_zerodds_delivery_mode(node->context->impl->bridge_ctx);
}

// REP-2008: host page size, cached. The unit an accelerator runtime pins.
static size_t zerodds_page_size(void)
{
  static size_t cached = 0;
  if (cached == 0) {
    long p = sysconf(_SC_PAGESIZE);
    cached = (p > 0) ? (size_t) p : 4096u;
  }
  return cached;
}

// REP-2008: participant-default alignment of Portable-mode loans, from the env
// `ZERODDS_LOAN_ALIGNMENT`. `page` resolves to the host page size; a decimal
// value is taken verbatim. 0 (unset / unusable) means natural malloc alignment.
// Non-power-of-two and sub-pointer values are rejected — aligned_alloc would
// otherwise be undefined.
static size_t zerodds_default_loan_alignment(void)
{
  static long cached = -1;
  if (cached < 0) {
    cached = 0;
    const char * e = getenv("ZERODDS_LOAN_ALIGNMENT");
    if (e && *e) {
      size_t v = (strcmp(e, "page") == 0) ? zerodds_page_size() : (size_t) strtoul(e, NULL, 10);
      if (v >= sizeof(void *) && (v & (v - 1)) == 0) { cached = (long) v; }
    }
  }
  return (size_t) cached;
}

// iceoryx2 service name for a topic — same on the matching writer + reader.
static void zerodds_iceoryx_service(const char * topic, char * out, size_t outlen)
{
  const char prefix[] = "zerodds_rmw_";
  size_t n = 0;
  for (const char * c = prefix; *c && n + 2 < outlen; ++c) { out[n++] = *c; }
  for (const char * c = topic; *c && n + 2 < outlen; ++c) {
    char ch = *c;
    int ok = (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9');
    out[n++] = ok ? ch : '_';
  }
  out[n] = '\0';
}

// Deterministic SHM flink path shared by the matching writer + reader. Both
// sides derive it from the same ROS topic name, so no discovery is needed for
// the same-host segment. `/tmp/zerodds/rmw_<sanitized-topic>.flink`.
#define ZERODDS_MAX_SHM_LOANS 32
static void zerodds_shm_path(const char * topic, char * out, size_t outlen)
{
  const char prefix[] = "/tmp/zerodds/rmw_";
  size_t n = 0;
  for (const char * c = prefix; *c && n + 8 < outlen; ++c) { out[n++] = *c; }
  for (const char * c = topic; *c && n + 8 < outlen; ++c) {
    char ch = *c;
    int ok = (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9');
    out[n++] = ok ? ch : '_';
  }
  const char suffix[] = ".flink";
  for (const char * c = suffix; *c && n + 1 < outlen; ++c) { out[n++] = *c; }
  out[n] = '\0';
}

// ---------------------------------------------------------------------------
// Node
// ---------------------------------------------------------------------------
typedef struct
{
  void * bridge_node;          // RmwZerodsNode*
  rmw_guard_condition_t graph_gc;
  unsigned int graph_flag;
  // Fingerprint of the endpoints this node has discovered, recomputed in
  // rmw_wait whenever somebody is actually waiting on the graph guard
  // condition. A change raises `graph_flag` — see zerodds_refresh_graph_guards.
  unsigned long long graph_fp;
  int graph_fp_valid;
} zerodds_node_data_t;

// Live nodes, for the graph-guard evaluation in rmw_wait. A guard condition
// arrives there as a bare `rmw_guard_condition_t *`; only this table can tell a
// node's graph guard from a user guard condition.
#define ZERODDS_MAX_NODES 128
static pthread_mutex_t zerodds_node_table_mx = PTHREAD_MUTEX_INITIALIZER;
static zerodds_node_data_t * zerodds_node_table[ZERODDS_MAX_NODES];

static void zerodds_node_register(zerodds_node_data_t * nd)
{
  pthread_mutex_lock(&zerodds_node_table_mx);
  for (int i = 0; i < ZERODDS_MAX_NODES; ++i) {
    if (zerodds_node_table[i] == NULL) { zerodds_node_table[i] = nd; break; }
  }
  pthread_mutex_unlock(&zerodds_node_table_mx);
}

static void zerodds_node_unregister(zerodds_node_data_t * nd)
{
  pthread_mutex_lock(&zerodds_node_table_mx);
  for (int i = 0; i < ZERODDS_MAX_NODES; ++i) {
    if (zerodds_node_table[i] == nd) { zerodds_node_table[i] = NULL; break; }
  }
  pthread_mutex_unlock(&zerodds_node_table_mx);
}

rmw_node_t *
rmw_create_node(rmw_context_t * context, const char * name, const char * namespace_)
{
  if (!context || !context->impl || !name || !namespace_) { return NULL; }
  void * bn = rmw_zerodds_create_node(context->impl->bridge_ctx, name, namespace_);
  if (!bn) { return NULL; }
  rmw_node_t * node = rmw_node_allocate();
  zerodds_node_data_t * nd = (zerodds_node_data_t *) calloc(1, sizeof(zerodds_node_data_t));
  if (!node || !nd) { free(nd); if (node) { rmw_node_free(node); } rmw_zerodds_destroy_node(bn); return NULL; }
  nd->bridge_node = bn;
  zerodds_node_register(nd);
  nd->graph_gc.implementation_identifier = ZERODDS_IDENTIFIER;
  nd->graph_gc.data = &nd->graph_flag;
  nd->graph_gc.context = context;
  node->implementation_identifier = ZERODDS_IDENTIFIER;
  node->data = nd;
  node->name = rcutils_strdup(name, context->options.allocator);
  node->namespace_ = rcutils_strdup(namespace_, context->options.allocator);
  node->context = context;
  return node;
}

rmw_ret_t
rmw_destroy_node(rmw_node_t * node)
{
  if (!node || !node->data) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) node->data;
  zerodds_node_unregister(nd);
  rmw_zerodds_destroy_node(nd->bridge_node);
  rcutils_allocator_t alloc = node->context->options.allocator;
  if (node->name) { alloc.deallocate((char *) node->name, alloc.state); }
  if (node->namespace_) { alloc.deallocate((char *) node->namespace_, alloc.state); }
  free(nd);
  rmw_node_free(node);
  return RMW_RET_OK;
}

const rmw_guard_condition_t *
rmw_node_get_graph_guard_condition(const rmw_node_t * node)
{
  if (!node || !node->data) { return NULL; }
  return &((zerodds_node_data_t *) node->data)->graph_gc;
}

rmw_ret_t
rmw_node_assert_liveliness(const rmw_node_t * node) { (void) node; return RMW_RET_OK; }

// ---------------------------------------------------------------------------
// Guard conditions (a triggerable flag; the wait loop polls it).
// ---------------------------------------------------------------------------
rmw_guard_condition_t *
rmw_create_guard_condition(rmw_context_t * context)
{
  if (!context) { return NULL; }
  rmw_guard_condition_t * gc = (rmw_guard_condition_t *) calloc(1, sizeof(rmw_guard_condition_t));
  unsigned int * flag = (unsigned int *) calloc(1, sizeof(unsigned int));
  if (!gc || !flag) { free(gc); free(flag); return NULL; }
  gc->implementation_identifier = ZERODDS_IDENTIFIER;
  gc->data = flag;
  gc->context = context;
  return gc;
}

rmw_ret_t
rmw_destroy_guard_condition(rmw_guard_condition_t * gc)
{
  if (!gc) { return RMW_RET_INVALID_ARGUMENT; }
  free(gc->data);
  free(gc);
  return RMW_RET_OK;
}

rmw_ret_t
rmw_trigger_guard_condition(const rmw_guard_condition_t * gc)
{
  if (!gc || !gc->data) { return RMW_RET_INVALID_ARGUMENT; }
  *(unsigned int *) gc->data = 1;
  // Wake any rmw_wait blocked on this context so the executor re-evaluates the
  // guard immediately (event-driven, not on the next poll tick).
  if (gc->context && gc->context->impl && gc->context->impl->bridge_ctx) {
    rmw_zerodds_context_notify(gc->context->impl->bridge_ctx);
  }
  return RMW_RET_OK;
}

// ---------------------------------------------------------------------------
// Publisher / Subscription
// ---------------------------------------------------------------------------
#include <rmw/validate_full_topic_name.h>

typedef struct {
  void * bridge_pub; rmw_gid_t gid; rmw_qos_profile_t qos; const void * members;
  int members_cpp;     // introspection flavour of `members` (0 = C, 1 = C++)
  int mode;            // 0 = Portable, 1 = RawSameHost (SHM loan active)
  size_t struct_size;  // in-memory message struct size (the SHM slot size)
  // REP-2007: bound custom type support, NULL when the publisher is unadapted.
  const rmw_zerodds_type_adapter_t * adapter;
  // REP-2008: requested alignment of Portable-mode loans (0 = natural malloc
  // alignment). A page-sized value makes every loan pinnable on its own pages.
  size_t loan_align;
  // REP-2008: flink path of the shared segment backing raw-mode loans (mode 1)
  // or the iceoryx2 service name (mode 2); empty in Portable mode.
  char shm_path[256];
  // QoS events: the counter of each publisher event kind (0..4, the shim's
  // numbering) as the last rmw_take_event saw it, and the matched total.
  uint64_t ev_seen[5];
  uint64_t ev_matched_total;
} zerodds_pub_data_t;
typedef struct {
  void * bridge_sub; rmw_qos_profile_t qos; const void * members;
  int members_cpp;     // introspection flavour of `members` (0 = C, 1 = C++)
  int mode;            // 0 = Portable, 1 = RawSameHost, 2 = Iceoryx
  size_t struct_size;
  // REP-2007: bound custom type support, NULL when the subscription is unadapted.
  const rmw_zerodds_type_adapter_t * adapter;
  // Monotonic reception counter for rmw_message_info_t.reception_sequence_number.
  uint64_t recv_seq;
  int shm_attached;    // raw source mapped yet (SHM: lazy; iceoryx: eager)
  int doorbell_started; // event-driven wakeup thread started yet
  char shm_path[256];  // flink path (mode 1) or iceoryx2 service name (mode 2)
  // Prefetched sample: readiness must not consume (iceoryx `take` is a
  // destructive FIFO receive), so the readiness check takes one sample and holds
  // it here until the matching rmw_take/take_loaned consumes it.
  int has_pending;
  const unsigned char * pending_ptr;
  size_t pending_len;
  unsigned int pending_slot;
  // Loan tracking: returned-message pointer → slot index, for release on return.
  struct { const unsigned char * ptr; unsigned int slot; } loans[ZERODDS_MAX_SHM_LOANS];
  // QoS events: the counter of each subscription event kind (10..15, index
  // kind - 10) as the last rmw_take_event saw it; LIVELINESS_CHANGED keeps
  // its alive count in ev_seen[1] and its not-alive count here.
  uint64_t ev_seen[6];
  uint64_t ev_seen_not_alive;
  uint64_t ev_matched_total;
  // Content filter (rmw_subscription_set_content_filter): the options as set,
  // for rmw_subscription_get_content_filter; the shim evaluates them.
  rmw_subscription_content_filter_options_t cft;
  int cft_on;
} zerodds_sub_data_t;
// Content filter (defined with the take path below).
static rmw_ret_t zerodds_sub_set_filter(zerodds_sub_data_t * sd,
  const rmw_subscription_content_filter_options_t * o, rcutils_allocator_t al);

// Lazily map the reader's raw source. Mode 1 (SHM) maps the writer's segment
// (the writer must have created it first; pub/sub order is arbitrary in ROS, so
// retry on first use). Mode 2 (Iceoryx) subscribes to the iceoryx2 service
// (order-independent). `sd->shm_path` holds the flink path (mode 1) or the
// iceoryx service name (mode 2).
static int zerodds_sub_ensure_shm(zerodds_sub_data_t * sd)
{
  if (!sd || sd->mode == 0) { return 0; }
  if (!sd->shm_attached) {
    int rc = (sd->mode == 2)
      ? rmw_zerodds_subscription_enable_iceoryx(sd->bridge_sub, sd->shm_path)
      : rmw_zerodds_subscription_enable_shm(sd->bridge_sub, sd->shm_path, 0);
    if (rc != RMW_RET_OK) { return 0; }
    sd->shm_attached = 1;
  }
  // Start the event-driven wakeup thread now the raw source exists (once).
  if (!sd->doorbell_started &&
      rmw_zerodds_subscription_start_doorbell(sd->bridge_sub) == RMW_RET_OK) {
    sd->doorbell_started = 1;
  }
  return 1;
}

// Prefetch one raw sample into the pending slot if none is held. Non-destructive
// readiness: a take from the raw source (esp. iceoryx's FIFO receive) consumes,
// so we take once and hold it until rmw_take/take_loaned delivers it. Idempotent
// while a sample is pending. Returns 1 if a sample is now held.
static int zerodds_sub_prefetch(zerodds_sub_data_t * sd)
{
  if (!sd || sd->mode == 0) { return 0; }
  if (sd->has_pending) { return 1; }
  if (!zerodds_sub_ensure_shm(sd)) { return 0; }
  const unsigned char * ptr = NULL;
  size_t len = 0;
  unsigned int slot = 0;
  if (rmw_zerodds_subscription_take_shm(sd->bridge_sub, &ptr, &len, &slot) != RMW_RET_OK || !ptr) {
    return 0;
  }
  sd->has_pending = 1;
  sd->pending_ptr = ptr;
  sd->pending_len = len;
  sd->pending_slot = slot;
  return 1;
}

// Combined readiness: RTPS inbox (Portable) OR a prefetched raw sample
// (RawSameHost/Iceoryx).
static int zerodds_sub_ready(zerodds_sub_data_t * sd)
{
  if (!sd) { return 0; }
  if (rmw_zerodds_subscription_has_data(sd->bridge_sub) > 0) { return 1; }
  return zerodds_sub_prefetch(sd);
}

// rmw_time_t -> nanoseconds; 0 for unspecified ({0,0}) and for infinite
// (RMW_DURATION_INFINITE, or anything past what u64 nanoseconds hold).
static uint64_t zerodds_time_ns(rmw_time_t t)
{
  if (t.sec == 0 && t.nsec == 0) { return 0; }
  if (t.sec >= 18446744073ULL) { return 0; }
  uint64_t ns = t.sec * 1000000000ULL + t.nsec;
  if (ns >= 9223372036854775807ULL) { return 0; }
  return ns;
}

static rmw_time_t zerodds_ns_time(uint64_t ns, rmw_time_t unset)
{
  if (ns == 0) { return unset; }
  rmw_time_t t = { ns / 1000000000ULL, ns % 1000000000ULL };
  return t;
}

// Every field of the profile reaches the endpoint; the RMW used to pass
// reliability alone, and durability, history, deadline, lifespan and
// liveliness were dropped.
static zerodds_rmw_qos_t zerodds_qos_from(const rmw_qos_profile_t * q)
{
  zerodds_rmw_qos_t z;
  memset(&z, 0, sizeof(z));
  z.history = (uint32_t) q->history;
  z.depth = (uint64_t) q->depth;
  z.reliability = (uint32_t) q->reliability;
  z.durability = (uint32_t) q->durability;
  z.deadline_ns = zerodds_time_ns(q->deadline);
  z.lifespan_ns = zerodds_time_ns(q->lifespan);
  z.liveliness = (uint32_t) q->liveliness;
  z.liveliness_lease_ns = zerodds_time_ns(q->liveliness_lease_duration);
  z.avoid_ros_namespace_conventions = q->avoid_ros_namespace_conventions ? 1u : 0u;
  return z;
}

// The QoS the endpoint was created with, for rmw_*_get_actual_qos.
static rmw_qos_profile_t zerodds_qos_to(const zerodds_rmw_qos_t * z, const rmw_qos_profile_t * requested)
{
  rmw_qos_profile_t q = *requested;
  q.history = (enum rmw_qos_history_policy_e) z->history;
  q.depth = (size_t) z->depth;
  q.reliability = (enum rmw_qos_reliability_policy_e) z->reliability;
  q.durability = (enum rmw_qos_durability_policy_e) z->durability;
  q.deadline = zerodds_ns_time(z->deadline_ns, (rmw_time_t) RMW_DURATION_INFINITE);
  q.lifespan = zerodds_ns_time(z->lifespan_ns, (rmw_time_t) RMW_DURATION_INFINITE);
  q.liveliness = (enum rmw_qos_liveliness_policy_e) z->liveliness;
  q.liveliness_lease_duration =
    zerodds_ns_time(z->liveliness_lease_ns, (rmw_time_t) RMW_DURATION_INFINITE);
  q.avoid_ros_namespace_conventions = z->avoid_ros_namespace_conventions != 0;
  return q;
}

rmw_publisher_t *
rmw_create_publisher(
  const rmw_node_t * node, const rosidl_message_type_support_t * type_support,
  const char * topic_name, const rmw_qos_profile_t * qos_policies,
  const rmw_publisher_options_t * publisher_options)
{
  if (!node || !node->data || !type_support || !topic_name || !qos_policies) { return NULL; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) node->data;
  char * tn = zerodds_type_name(type_support);
  if (!tn) { return NULL; }
  zerodds_rmw_qos_t zq = zerodds_qos_from(qos_policies);
  zerodds_rmw_qos_t za;
  memset(&za, 0, sizeof(za));
  char * th = zerodds_message_type_hash(type_support);
  void * bp = rmw_zerodds_create_publisher_hashed(nd->bridge_node, tn, topic_name, &zq, &za, th);
  zerodds_free_type_hash(th);
  free(tn);
  if (!bp) { return NULL; }
  rmw_publisher_t * pub = rmw_publisher_allocate();
  zerodds_pub_data_t * pd = (zerodds_pub_data_t *) calloc(1, sizeof(zerodds_pub_data_t));
  if (!pub || !pd) { free(pd); if (pub) { rmw_publisher_free(pub); } rmw_zerodds_destroy_publisher(bp); return NULL; }
  pd->bridge_pub = bp;
  pd->qos = zerodds_qos_to(&za, qos_policies);
  pd->members = zerodds_introspect_ex(type_support, &pd->members_cpp);
  // The DDS writer GUID: what the graph reports for this publisher, and what
  // a subscription on another host sees as the sender.
  pd->gid.implementation_identifier = ZERODDS_IDENTIFIER;
  if (rmw_zerodds_publisher_gid(bp, pd->gid.data) != RMW_RET_OK) {
    rmw_publisher_free(pub); free(pd); rmw_zerodds_destroy_publisher(bp);
    RMW_SET_ERROR_MSG("publisher has no writer GUID");
    return NULL;
  }
  pub->implementation_identifier = ZERODDS_IDENTIFIER;
  pub->data = pd;
  pub->topic_name = rcutils_strdup(topic_name, node->context->options.allocator);
  if (publisher_options) { pub->options = *publisher_options; }
  pub->can_loan_messages = zerodds_msg_can_loan(pd->members) ? true : false;
  // Raw delivery modes (zerodds-delivery-modes-1.0 §3.2/§3.3): if selected and
  // the type is loanable, switch the writer to the same-host SHM loan (mode 1)
  // or iceoryx2 (mode 2). On any failure fall back to Portable so the publisher
  // still works (interop-safe).
  pd->struct_size = zerodds_msg_struct_size(pd->members);
  pd->loan_align = zerodds_default_loan_alignment();
  int dmode = zerodds_delivery_mode(node);
  if (dmode != 0 && pub->can_loan_messages && pd->struct_size > 0) {
    char name[256];
    if (dmode == 2) {
      zerodds_iceoryx_service(topic_name, name, sizeof(name));
      if (rmw_zerodds_publisher_enable_iceoryx(pd->bridge_pub, name, pd->struct_size) == RMW_RET_OK) {
        pd->mode = 2;
        snprintf(pd->shm_path, sizeof(pd->shm_path), "%s", name);
      }
    } else {
      zerodds_shm_path(topic_name, name, sizeof(name));
      if (rmw_zerodds_publisher_enable_raw_loan(pd->bridge_pub, name, ZERODDS_MAX_SHM_LOANS,
                                                pd->struct_size) == RMW_RET_OK) {
        pd->mode = 1;
        snprintf(pd->shm_path, sizeof(pd->shm_path), "%s", name);
      }
    }
  }
  // The operator asked for a raw mode; say when this publisher does not get it.
  if (dmode != 0 && pd->mode == 0) {
    RCUTILS_LOG_WARN_NAMED(
      "rmw_zerodds_cpp", "delivery.mode %s is not used for publisher '%s' (%s); it sends Portable",
      dmode == 2 ? "iceoryx" : "raw-same-host", topic_name,
      !(pub->can_loan_messages && pd->struct_size > 0)
        ? "the message type is not a fixed-size plain struct"
        : (dmode == 2 ? "iceoryx2 is not available in this build or refused the service"
                      : "the shared-memory segment could not be set up"));
  }
  return pub;
}

rmw_ret_t
rmw_destroy_publisher(rmw_node_t * node, rmw_publisher_t * publisher)
{
  if (!publisher || !publisher->data) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_pub_data_t * pd = (zerodds_pub_data_t *) publisher->data;
  rmw_zerodds_destroy_publisher(pd->bridge_pub);
  if (node && publisher->topic_name) {
    rcutils_allocator_t a = node->context->options.allocator;
    a.deallocate((char *) publisher->topic_name, a.state);
  }
  free(pd);
  rmw_publisher_free(publisher);
  return RMW_RET_OK;
}

rmw_ret_t
rmw_publisher_count_matched_subscriptions(const rmw_publisher_t * publisher, size_t * count)
{
  if (!publisher || !publisher->data || !count) { return RMW_RET_INVALID_ARGUMENT; }
  *count = rmw_zerodds_publisher_matched_count(((zerodds_pub_data_t *) publisher->data)->bridge_pub);
  return RMW_RET_OK;
}

rmw_ret_t
rmw_publisher_get_actual_qos(const rmw_publisher_t * publisher, rmw_qos_profile_t * qos)
{
  if (!publisher || !publisher->data || !qos) { return RMW_RET_INVALID_ARGUMENT; }
  *qos = ((zerodds_pub_data_t *) publisher->data)->qos;
  return RMW_RET_OK;
}

rmw_ret_t
rmw_get_gid_for_publisher(const rmw_publisher_t * publisher, rmw_gid_t * gid)
{
  if (!publisher || !publisher->data || !gid) { return RMW_RET_INVALID_ARGUMENT; }
  *gid = ((zerodds_pub_data_t *) publisher->data)->gid;
  return RMW_RET_OK;
}

rmw_ret_t
rmw_publisher_assert_liveliness(const rmw_publisher_t * publisher)
{
  if (!publisher || !publisher->data) { return RMW_RET_INVALID_ARGUMENT; }
  return rmw_zerodds_publisher_assert_liveliness(((zerodds_pub_data_t *) publisher->data)->bridge_pub);
}

rmw_ret_t
rmw_publisher_wait_for_all_acked(const rmw_publisher_t * publisher, rmw_time_t wait_timeout)
{
  if (!publisher || !publisher->data) { return RMW_RET_INVALID_ARGUMENT; }
  // RMW_DURATION_INFINITE waits without bound; zerodds_time_ns maps it to 0,
  // which here would mean "do not wait", so it becomes the largest wait.
  uint64_t ns = zerodds_time_ns(wait_timeout);
  if (ns == 0 && !(wait_timeout.sec == 0 && wait_timeout.nsec == 0)) { ns = UINT64_MAX; }
  return rmw_zerodds_publisher_wait_for_all_acked(((zerodds_pub_data_t *) publisher->data)->bridge_pub, ns);
}

rmw_subscription_t *
rmw_create_subscription(
  const rmw_node_t * node, const rosidl_message_type_support_t * type_support,
  const char * topic_name, const rmw_qos_profile_t * qos_policies,
  const rmw_subscription_options_t * subscription_options)
{
  if (!node || !node->data || !type_support || !topic_name || !qos_policies) { return NULL; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) node->data;
  char * tn = zerodds_type_name(type_support);
  if (!tn) { return NULL; }
  zerodds_rmw_qos_t zq = zerodds_qos_from(qos_policies);
  zerodds_rmw_qos_t za;
  memset(&za, 0, sizeof(za));
  char * th = zerodds_message_type_hash(type_support);
  void * bs = rmw_zerodds_create_subscription_hashed(nd->bridge_node, tn, topic_name, &zq, &za, th);
  zerodds_free_type_hash(th);
  free(tn);
  if (!bs) { return NULL; }
  rmw_subscription_t * sub = rmw_subscription_allocate();
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) calloc(1, sizeof(zerodds_sub_data_t));
  if (!sub || !sd) { free(sd); if (sub) { rmw_subscription_free(sub); } rmw_zerodds_destroy_subscription(bs); return NULL; }
  sd->bridge_sub = bs;
  sd->qos = zerodds_qos_to(&za, qos_policies);
  sd->members = zerodds_introspect_ex(type_support, &sd->members_cpp);
  sub->implementation_identifier = ZERODDS_IDENTIFIER;
  sub->data = sd;
  sub->topic_name = rcutils_strdup(topic_name, node->context->options.allocator);
  if (subscription_options) { sub->options = *subscription_options; }
  sub->can_loan_messages = zerodds_msg_can_loan(sd->members) ? true : false;
  sub->is_cft_enabled = false;
  if (subscription_options && subscription_options->content_filter_options &&
      subscription_options->content_filter_options->filter_expression &&
      subscription_options->content_filter_options->filter_expression[0] != '\0')
  {
    if (zerodds_sub_set_filter(sd, subscription_options->content_filter_options,
          rcutils_get_default_allocator()) != RMW_RET_OK)
    {
      // The filter error is the one to report; a failure to tear the
      // half-built subscription down again cannot replace it. Assigned,
      // not cast to void: GCC keeps -Wunused-result for a (void) cast.
      rmw_ret_t teardown = rmw_destroy_subscription((rmw_node_t *) node, sub);
      (void) teardown;
      return NULL;
    }
    sub->is_cft_enabled = true;
  }
  // Raw delivery modes. Mode 1 (SHM): remember the mode + flink path; the
  // mapping is lazy (the writer must create the segment first). Mode 2
  // (Iceoryx): the iceoryx2 subscribe is order-independent, so attach eagerly —
  // and if iceoryx is unavailable (shim built without `delivery-iceoryx`),
  // degrade to Portable now so the normal RTPS take still receives the writer's
  // Portable fallback (both sides degrade identically for the same build).
  sd->struct_size = zerodds_msg_struct_size(sd->members);
  int dmode = zerodds_delivery_mode(node);
  if (dmode != 0 && sub->can_loan_messages && sd->struct_size > 0) {
    if (dmode == 2) {
      zerodds_iceoryx_service(topic_name, sd->shm_path, sizeof(sd->shm_path));
      if (rmw_zerodds_subscription_enable_iceoryx(sd->bridge_sub, sd->shm_path) == RMW_RET_OK) {
        sd->mode = 2;
        sd->shm_attached = 1;
      }  // else: leave mode 0 → Portable
    } else {
      sd->mode = 1;
      zerodds_shm_path(topic_name, sd->shm_path, sizeof(sd->shm_path));
    }
  }
  if (dmode != 0 && sd->mode == 0) {
    RCUTILS_LOG_WARN_NAMED(
      "rmw_zerodds_cpp", "delivery.mode %s is not used for subscription '%s' (%s); it takes Portable",
      dmode == 2 ? "iceoryx" : "raw-same-host", topic_name,
      !(sub->can_loan_messages && sd->struct_size > 0)
        ? "the message type is not a fixed-size plain struct"
        : "iceoryx2 is not available in this build or refused the service");
  }
  return sub;
}

rmw_ret_t
rmw_destroy_subscription(rmw_node_t * node, rmw_subscription_t * subscription)
{
  if (!subscription || !subscription->data) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) subscription->data;
  rmw_zerodds_destroy_subscription(sd->bridge_sub);
  if (sd->cft_on) {
    // Copied with the default allocator in zerodds_sub_set_filter.
    rcutils_allocator_t al = rcutils_get_default_allocator();
    (void) rmw_subscription_content_filter_options_fini(&sd->cft, &al);
  }
  if (node && subscription->topic_name) {
    rcutils_allocator_t a = node->context->options.allocator;
    a.deallocate((char *) subscription->topic_name, a.state);
  }
  free(sd);
  rmw_subscription_free(subscription);
  return RMW_RET_OK;
}

rmw_ret_t
rmw_subscription_count_matched_publishers(const rmw_subscription_t * subscription, size_t * count)
{
  if (!subscription || !subscription->data || !count) { return RMW_RET_INVALID_ARGUMENT; }
  *count = rmw_zerodds_subscription_matched_count(((zerodds_sub_data_t *) subscription->data)->bridge_sub);
  return RMW_RET_OK;
}

rmw_ret_t
rmw_subscription_get_actual_qos(const rmw_subscription_t * subscription, rmw_qos_profile_t * qos)
{
  if (!subscription || !subscription->data || !qos) { return RMW_RET_INVALID_ARGUMENT; }
  *qos = ((zerodds_sub_data_t *) subscription->data)->qos;
  return RMW_RET_OK;
}

// ---------------------------------------------------------------------------
// Wait set + wait (poll bridge subscriptions + guard-condition flags)
// ---------------------------------------------------------------------------
rmw_wait_set_t *
rmw_create_wait_set(rmw_context_t * context, size_t max_conditions)
{
  (void) max_conditions;
  if (!context) { return NULL; }
  rmw_wait_set_t * ws = (rmw_wait_set_t *) calloc(1, sizeof(rmw_wait_set_t));
  if (!ws) { return NULL; }
  ws->implementation_identifier = ZERODDS_IDENTIFIER;
  // Stash the context so rmw_wait can reach the bridge context for the
  // event-driven block. The context is owned elsewhere — not freed on destroy.
  ws->data = context;
  return ws;
}

rmw_ret_t
rmw_destroy_wait_set(rmw_wait_set_t * wait_set)
{
  if (!wait_set) { return RMW_RET_INVALID_ARGUMENT; }
  free(wait_set);
  return RMW_RET_OK;
}

// ===========================================================================
// Introspection-driven CDR (XCDR1) serialize/deserialize — REP-2007 wire.
// Encapsulation: 4-byte header { 0x00, 0x01, 0x00, 0x00 } (CDR_LE); alignment
// is computed from the first data byte (the 4-byte header excluded).
// ===========================================================================
typedef rosidl_typesupport_introspection_c__MessageMembers ms_t;
typedef rosidl_typesupport_introspection_c__MessageMember mm_t;

// Client/service impl data — defined here (before rmw_wait, which inspects them
// for readiness); the full service request-reply logic is further below.
typedef struct {
  void * bridge_client;
  const ms_t * req_members;
  const ms_t * resp_members;
  int members_cpp;     // introspection flavour of both member sets
  unsigned char gid[16];
  rmw_qos_profile_t qos;  // the QoS the endpoints were created with
} zerodds_client_data_t;

typedef struct {
  void * bridge_service;
  const ms_t * req_members;
  const ms_t * resp_members;
  int members_cpp;     // introspection flavour of both member sets
  rmw_qos_profile_t qos;  // the QoS the endpoints were created with
} zerodds_service_data_t;

// `cpp` = the introspection flavour of the members being walked (see
// zerodds_introspect_ex): 1 = std::string/std::u16string, 0 = the rosidl C
// runtime strings.
typedef struct { unsigned char * buf; size_t len; size_t cap; int cpp; } cdr_t;
static int cdr_reserve(cdr_t * c, size_t n)
{
  if (c->len + n <= c->cap) { return 0; }
  size_t cap = c->cap ? c->cap * 2 : 64;
  while (cap < c->len + n) { cap *= 2; }
  unsigned char * nb = (unsigned char *) realloc(c->buf, cap);
  if (!nb) { return -1; }
  c->buf = nb; c->cap = cap; return 0;
}
static int cdr_align(cdr_t * c, size_t a)
{
  size_t off = (c->len - 4) % a;
  if (off) {
    size_t pad = a - off;
    if (cdr_reserve(c, pad)) { return -1; }
    memset(c->buf + c->len, 0, pad); c->len += pad;
  }
  return 0;
}
static int cdr_put(cdr_t * c, const void * p, size_t n, size_t align)
{ if (cdr_align(c, align) || cdr_reserve(c, n)) { return -1; } memcpy(c->buf + c->len, p, n); c->len += n; return 0; }

// `big_endian` = the wire byte order from the encapsulation header (RTPS 2.5
// §10.5): a remote big-endian publisher (or a *_BE serialized message) needs
// every multi-byte scalar byte-swapped on a little-endian host. Octet/byte runs
// (string/wstring char data) are never swapped.
// `xcdr2`: the stream's encapsulation is XCDR2 (PLAIN_CDR2 0x0006/0x0007),
// whose alignment stops at 4 bytes (XTypes 1.3 §7.4.3.4.2); XCDR1 aligns
// 8-byte scalars to 8. Initializers that do not name it read XCDR1.
typedef struct { const unsigned char * buf; size_t len; size_t pos; int big_endian; int cpp; int xcdr2; } rdr_t;
static void rdr_align(rdr_t * r, size_t a)
{
  if (r->xcdr2 && a > 4) { a = 4; }
  size_t off = (r->pos - 4) % a;
  if (off) { r->pos += (a - off); }
}
// The representation of a CDR stream from its encapsulation header.
static int zerodds_stream_is_xcdr2(const unsigned char * buf, size_t len)
{
  return len >= 4 && buf[0] == 0x00 && (buf[1] == 0x06 || buf[1] == 0x07);
}
// Reverse an n-byte scalar in place when the wire is big-endian (host assumed
// little-endian, consistent with the LE-only serialize side).
static void rdr_bswap(void * p, size_t n, int big_endian)
{
  if (!big_endian || n < 2) { return; }
  unsigned char * b = (unsigned char *) p;
  for (size_t i = 0; i < n / 2; ++i) { unsigned char t = b[i]; b[i] = b[n - 1 - i]; b[n - 1 - i] = t; }
}
static int rdr_get(rdr_t * r, void * out, size_t n, size_t align)
{ rdr_align(r, align); if (r->pos + n > r->len) { return -1; } memcpy(out, r->buf + r->pos, n); r->pos += n;
  if (n == 2 || n == 4 || n == 8) { rdr_bswap(out, n, r->big_endian); } return 0; }
// Reads a 4-byte length/count word honoring the wire byte order.
static int rdr_u32(rdr_t * r, uint32_t * out)
{ rdr_align(r, 4); if (r->pos + 4 > r->len) { return -1; } memcpy(out, r->buf + r->pos, 4); r->pos += 4;
  rdr_bswap(out, 4, r->big_endian); return 0; }

// CDR *wire* size of a primitive field (used both as the wire put/get size and,
// via prim_inmem_size below, as the in-memory array stride). type_id 5 (wchar)
// is special: it occupies 2 bytes in memory (char16_t) but is serialized as a
// 4-byte UTF-32LE code unit on the wire — so it is NOT listed here (it would put
// the wrong wire width and read past the 2-byte slot) and is handled explicitly
// in cdr_ser_one/cdr_de_one. type_id 17 (wstring) is variable-length, likewise
// explicit. The wire width of a wchar is 4 (align 4); that lives in the explicit
// path, not in prim_size.
static size_t prim_size(uint8_t t)
{
  switch (t) {
    case 4: case 6: case 7: case 8: case 9: return 1;
    case 10: case 11: return 2;
    case 1: case 12: case 13: return 4;
    case 2: case 14: case 15: return 8;
    default: return 0;
  }
}

// In-memory element stride for array/sequence iteration. Identical to the wire
// size for every fixed primitive EXCEPT wchar (type_id 5): in memory a wchar is
// a 2-byte char16_t (the rosidl C type), even though its wire form is 4 bytes.
// Returns 0 for non-fixed-stride members (string/wstring/nested), which routes
// the array loop through the introspection get_(const_)function accessor.
static size_t prim_inmem_size(uint8_t t)
{
  if (t == 5) { return sizeof(uint16_t); }  // wchar: char16_t in memory
  return prim_size(t);
}

// std::string / std::u16string accessors, compiled from rmw_zerodds_cxx.cpp.
extern const char * zerodds_cxx_string_data(const void * s, size_t * out_len);
extern void zerodds_cxx_string_assign(void * s, const char * data, size_t n);
extern const uint16_t * zerodds_cxx_u16string_data(const void * s, size_t * out_len);
extern void zerodds_cxx_u16string_assign(void * s, const uint16_t * data, size_t n);

// Serialize a wstring (rosidl_runtime_c__U16String { uint16_t* data; size_t size;
// … }) as ROS 2 XCDR1. The native wire form (verified byte-identical against
// rmw_cyclonedds AND rmw_fastrtps on a Humble host) is: align(4); uint32 length =
// the **UTF-16 unit count** (== U16String.size; NO null terminator); then one
// uint32 per UTF-16 unit, each `data[i]` zero-extended to 32 bits, LE. Crucially
// the rosidl typesupport does NOT combine surrogate pairs: an astral code point
// (e.g. 🎉 U+1F389, stored in memory as the surrogate pair 0xD83C 0xDF89) is
// written as TWO uint32 slots 0x0000D83C, 0x0000DF89 — length counts both. So
// "code-point count" / surrogate-combining is NOT the wire reality; the unit is
// the UTF-16 code unit. Cyclone is the canonical zero-trailing form; FastDDS
// appends a 4-byte pad after the data (deserialize-irrelevant — see cdr_de_*).
static int cdr_ser_wstring(cdr_t * c, const unsigned char * p)
{
  const uint16_t * data;
  size_t units;
  if (c->cpp) {
    data = zerodds_cxx_u16string_data(p, &units);  // std::u16string
  } else {
    const uint16_t * const * dp = (const uint16_t * const *) p;
    const size_t * szp = (const size_t *) (p + sizeof(uint16_t *));
    data = *dp;
    units = data ? *szp : 0;
  }
  uint32_t u16len = (uint32_t) units;
  if (cdr_put(c, &u16len, 4, 4)) { return -1; }
  for (uint32_t i = 0; i < u16len; ++i) {
    uint32_t unit = data[i];  // zero-extend UTF-16 unit to UTF-32 slot
    if (cdr_put(c, &unit, 4, 4)) { return -1; }
  }
  return 0;
}

static int cdr_ser_msg(cdr_t * c, const ms_t * m, const unsigned char * base);
static int cdr_ser_one(cdr_t * c, const mm_t * mem, const unsigned char * p)
{
  uint8_t t = mem->type_id_;
  size_t ps = prim_size(t);
  if (ps) { return cdr_put(c, p, ps, ps); }
  if (t == 5) {
    // wchar: char16_t (2 bytes) in memory → one 4-byte UTF-32LE unit, align(4).
    uint16_t u; memcpy(&u, p, sizeof(u));
    uint32_t cp = u;  // a single wchar is one code unit (no surrogate combining)
    return cdr_put(c, &cp, 4, 4);
  }
  if (t == 16) {
    const char * data;
    size_t slen;
    if (c->cpp) {
      data = zerodds_cxx_string_data(p, &slen);  // std::string
    } else {
      const char * const * sp = (const char * const *) p;
      const size_t * szp = (const size_t *) (p + sizeof(char *));
      data = *sp ? *sp : "";
      slen = *sp ? *szp : 0;
    }
    uint32_t n = (uint32_t) slen + 1;
    if (cdr_align(c, 4) || cdr_reserve(c, 4 + n)) { return -1; }
    memcpy(c->buf + c->len, &n, 4); c->len += 4;
    memcpy(c->buf + c->len, data, n - 1); c->len += (n - 1);
    c->buf[c->len++] = '\0';
    return 0;
  }
  if (t == 17) { return cdr_ser_wstring(c, p); }
  if (t == 18) { return cdr_ser_msg(c, (const ms_t *) mem->members_->data, p); }
  return -1;
}
static int cdr_ser_msg(cdr_t * c, const ms_t * m, const unsigned char * base)
{
  for (uint32_t i = 0; i < m->member_count_; ++i) {
    const mm_t * mem = &m->members_[i];
    const unsigned char * fp = base + mem->offset_;
    if (!mem->is_array_) {
      if (cdr_ser_one(c, mem, fp)) { return -1; }
    } else if (mem->array_size_ > 0 && !mem->is_upper_bound_) {
      size_t es = prim_inmem_size(mem->type_id_);
      for (size_t k = 0; k < mem->array_size_; ++k) {
        const unsigned char * ep = es ? fp + k * es : (const unsigned char *) mem->get_const_function(fp, k);
        if (cdr_ser_one(c, mem, ep)) { return -1; }
      }
    } else {
      size_t cnt = mem->size_function(fp);
      uint32_t c32 = (uint32_t) cnt;
      if (cdr_put(c, &c32, 4, 4)) { return -1; }
      for (size_t k = 0; k < cnt; ++k) {
        const unsigned char * ep = (const unsigned char *) mem->get_const_function(fp, k);
        if (cdr_ser_one(c, mem, ep)) { return -1; }
      }
    }
  }
  return 0;
}

extern bool rosidl_runtime_c__String__assignn(void * str, const char * value, size_t n);
// rosidl wstring assign: copies `n` UTF-16 units into the U16String (allocating
// + null-terminating). Declared here to keep this C TU header-light.
extern bool rosidl_runtime_c__U16String__assignn(void * str, const uint16_t * value, size_t n);

// Deserialize a wstring body — the inverse of cdr_ser_wstring. Read the uint32
// UTF-16-unit count, then that many UTF-32LE slots, truncating each back to its
// low 16 bits (the UTF-16 unit; the high 16 bits are always zero in the ROS wire
// form) and store via rosidl_runtime_c__U16String__assignn. No surrogate
// re-pairing — units are stored exactly as carried, so an astral character's
// surrogate pair round-trips bit-exactly. FastDDS's trailing 4-byte pad is never
// read: we consume exactly `count` slots after the length, so the pad is inert.
static int cdr_de_wstring(rdr_t * r, unsigned char * p)
{
  uint32_t count;
  if (rdr_u32(r, &count)) { return -1; }
  if ((uint64_t) r->pos + (uint64_t) count * 4u > r->len) { return -1; }
  uint16_t stackbuf[256];
  uint16_t * u16 = stackbuf;
  if ((size_t) count > sizeof(stackbuf) / sizeof(stackbuf[0])) {
    u16 = (uint16_t *) malloc((size_t) count * sizeof(uint16_t));
    if (!u16) { return -1; }
  }
  for (uint32_t i = 0; i < count; ++i) {
    uint32_t unit;
    memcpy(&unit, r->buf + r->pos, 4); r->pos += 4;
    rdr_bswap(&unit, 4, r->big_endian);
    u16[i] = (uint16_t) unit;
  }
  bool ok;
  if (r->cpp) {
    zerodds_cxx_u16string_assign(p, u16, count);  // std::u16string
    ok = true;
  } else {
    ok = rosidl_runtime_c__U16String__assignn(p, u16, count);
  }
  if (u16 != stackbuf) { free(u16); }
  return ok ? 0 : -1;
}

static int cdr_de_msg(rdr_t * r, const ms_t * m, unsigned char * base);
static int cdr_de_one(rdr_t * r, const mm_t * mem, unsigned char * p)
{
  uint8_t t = mem->type_id_;
  size_t ps = prim_size(t);
  if (ps) { return rdr_get(r, p, ps, ps); }
  if (t == 5) {
    // wchar: read 4-byte UTF-32 code unit (wire byte order), store low 16 bits.
    uint32_t cp;
    if (rdr_u32(r, &cp)) { return -1; }
    uint16_t u = (uint16_t) cp;
    memcpy(p, &u, sizeof(u));
    return 0;
  }
  if (t == 16) {
    uint32_t n;
    if (rdr_u32(r, &n)) { return -1; }
    if (n == 0 || r->pos + n > r->len) { return -1; }
    bool ok;
    if (r->cpp) {
      zerodds_cxx_string_assign(p, (const char *) (r->buf + r->pos), n - 1);  // std::string
      ok = true;
    } else {
      ok = rosidl_runtime_c__String__assignn(p, (const char *) (r->buf + r->pos), n - 1);
    }
    r->pos += n;
    return ok ? 0 : -1;
  }
  if (t == 17) { return cdr_de_wstring(r, p); }
  if (t == 18) { return cdr_de_msg(r, (const ms_t *) mem->members_->data, p); }
  return -1;
}
static int cdr_de_msg(rdr_t * r, const ms_t * m, unsigned char * base)
{
  for (uint32_t i = 0; i < m->member_count_; ++i) {
    const mm_t * mem = &m->members_[i];
    unsigned char * fp = base + mem->offset_;
    if (!mem->is_array_) {
      if (cdr_de_one(r, mem, fp)) { return -1; }
    } else if (mem->array_size_ > 0 && !mem->is_upper_bound_) {
      size_t es = prim_inmem_size(mem->type_id_);
      for (size_t k = 0; k < mem->array_size_; ++k) {
        unsigned char * ep = es ? fp + k * es : (unsigned char *) mem->get_function(fp, k);
        if (cdr_de_one(r, mem, ep)) { return -1; }
      }
    } else {
      uint32_t cnt;
      if (rdr_u32(r, &cnt)) { return -1; }
      // The one place where the C and C++ introspection structs genuinely
      // differ: the C `resize_function` returns bool, the C++ one returns void.
      // Calling the C++ function through the bool prototype yields whatever
      // happens to be in the return register — a garbage 0 aborted the decode of
      // *every* sequence member coming from rclcpp, and the sample was dropped
      // without a trace. Call each through its own prototype.
      if (r->cpp) {
        ((void (*)(void *, size_t)) mem->resize_function)(fp, cnt);
      } else if (!mem->resize_function(fp, cnt)) {
        return -1;
      }
      for (size_t k = 0; k < cnt; ++k) {
        unsigned char * ep = (unsigned char *) mem->get_function(fp, k);
        if (cdr_de_one(r, mem, ep)) { return -1; }
      }
    }
  }
  return 0;
}

// Fills the locally knowable half of rmw_message_info_t. rclcpp hands this
// struct to a REP-2007 adapted subscription callback together with the message,
// so it must not be left as an all-zero struct: `reception_sequence_number` 0
// would claim "first message ever" on every take. The bridge take API carries
// neither the publisher gid nor the source timestamp, so those two stay at
// their documented unknown values.
// Where a sample came from, as the shim reports it with a take.
typedef struct { uint8_t writer_guid[16]; int64_t source_timestamp_ns; int64_t sequence_number; } zerodds_origin_t;
extern int rmw_zerodds_take_with_origin(void * sub, unsigned char ** out_buf, size_t * out_len,
  unsigned char * out_big_endian, zerodds_origin_t * out_origin);

// `origin` NULL: the raw delivery modes, which carry no writer identity.
static void zerodds_fill_message_info_from(zerodds_sub_data_t * sd, rmw_message_info_t * mi,
  const zerodds_origin_t * origin)
{
  if (!mi) { return; }
  memset(mi, 0, sizeof(*mi));
  struct timespec ts;
  if (clock_gettime(CLOCK_REALTIME, &ts) == 0) {
    mi->received_timestamp = (rmw_time_point_value_t) ts.tv_sec * 1000000000LL + (int64_t) ts.tv_nsec;
  }
  mi->source_timestamp = 0;
  mi->publication_sequence_number = RMW_MESSAGE_INFO_SEQUENCE_NUMBER_UNSUPPORTED;
  mi->reception_sequence_number = sd ? ++sd->recv_seq : RMW_MESSAGE_INFO_SEQUENCE_NUMBER_UNSUPPORTED;
  mi->publisher_gid.implementation_identifier = ZERODDS_IDENTIFIER;
  if (origin) {
    // The writer GUID is the publisher's GID (rmw_get_gid_for_publisher).
    memcpy(mi->publisher_gid.data, origin->writer_guid, 16);
    if (origin->source_timestamp_ns >= 0) { mi->source_timestamp = origin->source_timestamp_ns; }
    if (origin->sequence_number >= 0) { mi->publication_sequence_number = (uint64_t) origin->sequence_number; }
  }
  mi->from_intra_process = false;
}
static void zerodds_fill_message_info(zerodds_sub_data_t * sd, rmw_message_info_t * mi)
{
  zerodds_fill_message_info_from(sd, mi, NULL);
}

rmw_ret_t
rmw_publish(const rmw_publisher_t * publisher, const void * ros_message,
            rmw_publisher_allocation_t * allocation)
{
  (void) allocation;
  if (!publisher || !publisher->data || !ros_message) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_pub_data_t * pd = (zerodds_pub_data_t *) publisher->data;
  if (!pd->members) { return RMW_RET_OK; }
  cdr_t c = {0};
  c.cpp = pd->members_cpp;
  if (cdr_reserve(&c, 4)) { return RMW_RET_ERROR; }
  c.buf[0] = 0x00; c.buf[1] = 0x01; c.buf[2] = 0x00; c.buf[3] = 0x00;
  c.len = 4;
  if (cdr_ser_msg(&c, (const ms_t *) pd->members, (const unsigned char *) ros_message)) { free(c.buf); return RMW_RET_ERROR; }
  int rc = rmw_zerodds_publish(pd->bridge_pub, c.buf, c.len);
  free(c.buf);
  return rc == 0 ? RMW_RET_OK : RMW_RET_ERROR;
}

// ---------------------------------------------------------------------------
// Content filter: the shim evaluates the expression (zerodds-sql-filter, DDS
// 1.4 Annex B); this getter reads a field of a deserialized message through
// its introspection members, `a.b` for a nested member. A field a filter
// cannot compare (array, wide string, message) reads as absent, and the
// message then does not pass.
// ---------------------------------------------------------------------------
typedef struct {
  uint32_t kind; uint8_t b; int64_t i; double f; const char * s; size_t slen;
} zerodds_filter_value_t;
typedef struct { const ms_t * members; int cpp; const unsigned char * msg; } zerodds_filter_row_t;
typedef int (* zerodds_field_getter_t)(const void * ctx, const char * path, zerodds_filter_value_t * out);
extern int rmw_zerodds_subscription_set_filter(void * sub, const char * expression,
  const char * const * params, size_t count);
extern int rmw_zerodds_subscription_filter_accepts(void * sub, const void * ctx, zerodds_field_getter_t getter);

static int zerodds_filter_field(const void * ctx, const char * path, zerodds_filter_value_t * out)
{
  const zerodds_filter_row_t * row = (const zerodds_filter_row_t *) ctx;
  const ms_t * m = row->members;
  const unsigned char * base = row->msg;
  const char * p = path;
  for (;;) {
    const char * dot = strchr(p, '.');
    size_t n = dot ? (size_t) (dot - p) : strlen(p);
    const mm_t * mem = NULL;
    for (uint32_t i = 0; i < m->member_count_; ++i) {
      const char * name = m->members_[i].name_;
      if (strlen(name) == n && strncmp(name, p, n) == 0) { mem = &m->members_[i]; break; }
    }
    if (!mem || mem->is_array_) { return 0; }
    const unsigned char * fp = base + mem->offset_;
    if (dot) {
      if (mem->type_id_ != 18) { return 0; }
      m = (const ms_t *) mem->members_->data;
      base = fp;
      p = dot + 1;
      continue;
    }
    switch (mem->type_id_) {
      case 1: { float v; memcpy(&v, fp, sizeof(v)); out->kind = 3; out->f = v; return 1; }
      case 2: { double v; memcpy(&v, fp, sizeof(v)); out->kind = 3; out->f = v; return 1; }
      case 3: { long double v; memcpy(&v, fp, sizeof(v)); out->kind = 3; out->f = (double) v; return 1; }
      case 6: { out->kind = 1; out->b = *fp ? 1 : 0; return 1; }
      case 4: case 7: case 8: { out->kind = 2; out->i = *fp; return 1; }
      case 9: { int8_t v; memcpy(&v, fp, 1); out->kind = 2; out->i = v; return 1; }
      case 10: { uint16_t v; memcpy(&v, fp, 2); out->kind = 2; out->i = v; return 1; }
      case 11: { int16_t v; memcpy(&v, fp, 2); out->kind = 2; out->i = v; return 1; }
      case 12: { uint32_t v; memcpy(&v, fp, 4); out->kind = 2; out->i = v; return 1; }
      case 13: { int32_t v; memcpy(&v, fp, 4); out->kind = 2; out->i = v; return 1; }
      case 14: {
        uint64_t v; memcpy(&v, fp, 8);
        if (v > (uint64_t) INT64_MAX) { out->kind = 3; out->f = (double) v; } else { out->kind = 2; out->i = (int64_t) v; }
        return 1;
      }
      case 15: { int64_t v; memcpy(&v, fp, 8); out->kind = 2; out->i = v; return 1; }
      case 16: {
        if (row->cpp) {
          out->s = zerodds_cxx_string_data(fp, &out->slen);
        } else {
          const char * const * sp = (const char * const *) fp;
          const size_t * szp = (const size_t *) (fp + sizeof(char *));
          out->s = *sp ? *sp : "";
          out->slen = *sp ? *szp : 0;
        }
        out->kind = 4;
        return 1;
      }
      default: return 0;
    }
  }
}

// Whether the deserialized `msg` passes the subscription's filter (1 without one).
static int zerodds_sub_accepts(zerodds_sub_data_t * sd, const void * msg)
{
  if (!sd->cft_on) { return 1; }
  if (!sd->members) { return 0; }
  zerodds_filter_row_t row = { (const ms_t *) sd->members, sd->members_cpp, (const unsigned char *) msg };
  return rmw_zerodds_subscription_filter_accepts(sd->bridge_sub, &row, zerodds_filter_field);
}

// The filter's verdict on a CDR stream ([encap 4][body]) the caller does not
// deserialize into the ROS type itself (serialized and adapted takes): it is
// deserialized into a scratch message the type's own init/fini set up and
// tear down. 1 passes, 0 does not, -1 out of memory.
static int zerodds_sub_accepts_cdr(zerodds_sub_data_t * sd, const unsigned char * buf, size_t len,
  unsigned char be)
{
  if (!sd->cft_on) { return 1; }
  const ms_t * mem = (const ms_t *) sd->members;
  if (!mem || len < 4) { return 0; }
  void * tmp = calloc(1, mem->size_of_);
  if (!tmp) { return -1; }
  if (mem->init_function) { mem->init_function(tmp, ROSIDL_RUNTIME_C_MSG_INIT_ALL); }
  rdr_t r = { buf, len, 4, be, sd->members_cpp, zerodds_stream_is_xcdr2(buf, len) };
  int pass = cdr_de_msg(&r, mem, (unsigned char *) tmp) == 0 && zerodds_sub_accepts(sd, tmp);
  if (mem->fini_function) { mem->fini_function(tmp); }
  free(tmp);
  return pass;
}

static rmw_ret_t zerodds_sub_set_filter(zerodds_sub_data_t * sd,
  const rmw_subscription_content_filter_options_t * o, rcutils_allocator_t al)
{
  const char * expr = o->filter_expression ? o->filter_expression : "";
  size_t n = o->expression_parameters.size;
  rmw_ret_t ret = rmw_zerodds_subscription_set_filter(sd->bridge_sub, expr,
    (const char * const *) o->expression_parameters.data, n);
  if (ret != RMW_RET_OK) {
    RMW_SET_ERROR_MSG("content filter expression does not parse or names a missing parameter");
    return ret;
  }
  if (sd->cft_on) { (void) rmw_subscription_content_filter_options_fini(&sd->cft, &al); }
  sd->cft_on = 0;
  if (expr[0] != '\0') {
    sd->cft = rmw_get_zero_initialized_content_filter_options();
    ret = rmw_subscription_content_filter_options_copy(o, &al, &sd->cft);
    if (ret != RMW_RET_OK) { return ret; }
    sd->cft_on = 1;
  }
  return RMW_RET_OK;
}

rmw_ret_t
rmw_take_with_info(const rmw_subscription_t * subscription, void * ros_message,
                   bool * taken, rmw_message_info_t * message_info,
                   rmw_subscription_allocation_t * allocation)
{
  (void) allocation;
  if (taken) { *taken = false; }
  if (!subscription || !subscription->data || !ros_message) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) subscription->data;
  // Raw modes (RawSameHost / Iceoryx): a raw-mode writer delivers the struct
  // only same-host (no RTPS), so a normal (non-loaning) subscriber reads it from
  // the raw source and copies it into `ros_message` — one struct memcpy, no
  // deserialization, no network. The explicit loaned take
  // (rmw_take_loaned_message) returns the slot pointer with zero copies; this is
  // the standard-callback path.
  if (sd->mode != 0) {
    // Consume the prefetched sample (readiness already took it; a fresh take
    // here would double-consume an iceoryx FIFO). Prefetch on demand in case
    // rmw_take is called without a preceding rmw_wait. A sample the content
    // filter refuses is consumed and the next one taken.
    for (;;) {
      if (!zerodds_sub_prefetch(sd)) { return RMW_RET_OK; }  // no sample
      size_t slen = sd->pending_len;
      size_t n = (sd->struct_size && slen >= sd->struct_size) ? sd->struct_size : slen;
      memcpy(ros_message, sd->pending_ptr, n);
      rmw_zerodds_subscription_release_shm(sd->bridge_sub, sd->pending_slot);
      sd->has_pending = 0;
      if (zerodds_sub_accepts(sd, ros_message)) { break; }
    }
    zerodds_fill_message_info(sd, message_info);
    if (taken) { *taken = true; }
    return RMW_RET_OK;
  }
  // A sample the content filter refuses is taken and dropped, and the next
  // one tried; the reader's queue holds only what arrived.
  zerodds_origin_t origin;
  for (;;) {
  unsigned char * buf = NULL; size_t len = 0; unsigned char be = 0;
  int rc = rmw_zerodds_take_with_origin(sd->bridge_sub, &buf, &len, &be, &origin);
  if (rc != 0 || !buf) { if (buf) { rmw_zerodds_buffer_free(buf, len); } return RMW_RET_OK; }
  if (len < 4) {
    rmw_zerodds_buffer_free(buf, len);
    RCUTILS_LOG_WARN_NAMED("rmw_zerodds_cpp",
      "subscription '%s': dropped a %zu-byte sample without an encapsulation header",
      subscription->topic_name, len);
    return RMW_RET_OK;
  }
  rdr_t r = { buf, len, 4, be, sd->members_cpp, zerodds_stream_is_xcdr2(buf, len) };
  int dr = sd->members ? cdr_de_msg(&r, (const ms_t *) sd->members, (unsigned char *) ros_message) : -1;
  rmw_zerodds_buffer_free(buf, len);
  if (dr != 0) {
    // Not delivered, and not silently: a writer whose type does not match
    // the subscription's, or a malformed sample.
    RCUTILS_LOG_WARN_NAMED("rmw_zerodds_cpp",
      "subscription '%s': dropped a %zu-byte sample that does not deserialize as its type",
      subscription->topic_name, len);
    return RMW_RET_OK;
  }
  if (zerodds_sub_accepts(sd, ros_message)) { break; }
  }
  zerodds_fill_message_info_from(sd, message_info, &origin);
  if (taken) { *taken = true; }
  return RMW_RET_OK;
}


rmw_ret_t
rmw_take(const rmw_subscription_t * subscription, void * ros_message, bool * taken,
         rmw_subscription_allocation_t * allocation)
{
  return rmw_take_with_info(subscription, ros_message, taken, NULL, allocation);
}

// ---------------------------------------------------------------------------
// wait — event-driven. Readiness is the non-consuming has-data peek + guard
// flags; when nothing is ready we park on the context's shared condvar via the
// bridge (woken by a reader data callback or a guard trigger) until an event or
// the deadline. Non-ready entries are nulled per the rmw contract; ready guard
// flags are consumed. No spin loop, no fixed-tick poll.
// ---------------------------------------------------------------------------
#include <time.h>

static long long zerodds_now_ns(void)
{
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (long long) ts.tv_sec * 1000000000LL + (long long) ts.tv_nsec;
}

// Graph enumeration over the node's runtime SEDP view (also used by the
// topic-graph and count_* entry points further below).
typedef void (*zerodds_topic_cb_t)(void * ud, const char * topic, const char * type);
extern int rmw_zerodds_node_for_each_publication(void * node, zerodds_topic_cb_t cb, void * ud);
extern int rmw_zerodds_node_for_each_subscription(void * node, zerodds_topic_cb_t cb, void * ud);

// REP-2009 §"Negotiation Algorithm" step 1 ("Use discovery to find all the
// connected subscriptions that are using the user specified negotiation topic")
// and §"Renegotiating Selected Message Types" ("In the case that the ROS 2
// network is changing …") both need the graph to *notify*, not just to be
// pollable: a negotiating publisher has to learn that a subscription appeared
// or left. rcl surfaces that as the node's graph guard condition.
//
// The fingerprint folds every discovered publication and subscription (topic +
// type) of the node into one value, so both a count change and an identity
// change raise the guard. It is computed only for graph guard conditions that
// are actually in a wait set — a node nobody watches costs nothing.
typedef struct { unsigned long long h; } zerodds_fp_ctx_t;
static void zerodds_fp_cb(void * ud, const char * topic, const char * type)
{
  zerodds_fp_ctx_t * c = (zerodds_fp_ctx_t *) ud;
  unsigned long long h = c->h;
  for (const char * p = topic; p && *p; ++p) { h = (h ^ (unsigned char) *p) * 1099511628211ULL; }
  h = (h ^ 0x2fu) * 1099511628211ULL;  // separator, so "ab"+"c" != "a"+"bc"
  for (const char * p = type; p && *p; ++p) { h = (h ^ (unsigned char) *p) * 1099511628211ULL; }
  h = (h ^ 0x0au) * 1099511628211ULL;
  c->h = h;
}

static void zerodds_refresh_graph_guards(rmw_guard_conditions_t * guard_conditions)
{
  if (!guard_conditions) { return; }
  for (size_t i = 0; i < guard_conditions->guard_condition_count; ++i) {
    // rcl stores a guard condition's rmw `data` pointer here, not the
    // rmw_guard_condition_t (rcl_wait_set_add_guard_condition:
    // `RMWStorage[n] = rmw_handle->data`). For us that pointer is the
    // `unsigned int` trigger flag — and for a node's graph guard condition it
    // is `&node_data->graph_flag`, which is how the node is recognized here.
    void * flag = guard_conditions->guard_conditions[i];
    if (!flag) { continue; }
    zerodds_node_data_t * nd = NULL;
    pthread_mutex_lock(&zerodds_node_table_mx);
    for (int k = 0; k < ZERODDS_MAX_NODES; ++k) {
      if (zerodds_node_table[k] && flag == (void *) &zerodds_node_table[k]->graph_flag) {
        nd = zerodds_node_table[k];
        break;
      }
    }
    pthread_mutex_unlock(&zerodds_node_table_mx);
    if (!nd) { continue; }  // an ordinary user guard condition
    zerodds_fp_ctx_t cx = { 1469598103934665603ULL };
    rmw_zerodds_node_for_each_publication(nd->bridge_node, zerodds_fp_cb, &cx);
    rmw_zerodds_node_for_each_subscription(nd->bridge_node, zerodds_fp_cb, &cx);
    if (!nd->graph_fp_valid) {
      nd->graph_fp = cx.h;
      nd->graph_fp_valid = 1;
    } else if (nd->graph_fp != cx.h) {
      nd->graph_fp = cx.h;
      nd->graph_flag = 1;  // the graph changed → wake whoever waits on it
    }
  }
}

rmw_ret_t
rmw_wait(rmw_subscriptions_t * subscriptions, rmw_guard_conditions_t * guard_conditions,
         rmw_services_t * services, rmw_clients_t * clients, rmw_events_t * events,
         rmw_wait_set_t * wait_set, const rmw_time_t * wait_timeout)
{
  void * bridge_ctx = NULL;
  if (wait_set && wait_set->data) {
    rmw_context_t * c = (rmw_context_t *) wait_set->data;
    if (c->impl) { bridge_ctx = c->impl->bridge_ctx; }
  }

  long long timeout_ns = -1;  // < 0 → block until an event
  if (wait_timeout) {
    timeout_ns = (long long) wait_timeout->sec * 1000000000LL + (long long) wait_timeout->nsec;
    if (timeout_ns < 0) { timeout_ns = 0; }
  }
  long long deadline_ns = (timeout_ns < 0) ? -1 : zerodds_now_ns() + timeout_ns;

  // A notify (data / guard trigger / executor cancel) wakes the block. On such a
  // wake we must return even if nothing is ready, so the caller's executor can
  // re-evaluate its own state (e.g. cancel/spinning) instead of us re-blocking
  // indefinitely — the cancel's interrupt guard is not always in our array.
  int woke = 0;

  for (;;) {
    // Snapshot the wakeup generation BEFORE checking readiness so an event in
    // between is not lost (it bumps the generation → the block returns at once).
    unsigned long long gen = bridge_ctx ? rmw_zerodds_context_wait_generation(bridge_ctx) : 0;

    // Raise the graph guard of every watched node whose discovered endpoint set
    // changed since the last pass (REP-2009 renegotiation trigger).
    zerodds_refresh_graph_guards(guard_conditions);

    size_t ready = 0;
    if (subscriptions) {
      for (size_t i = 0; i < subscriptions->subscriber_count; ++i) {
        zerodds_sub_data_t * sd = (zerodds_sub_data_t *) subscriptions->subscribers[i];
        if (zerodds_sub_ready(sd)) { ready++; }
      }
    }
    if (guard_conditions) {
      for (size_t i = 0; i < guard_conditions->guard_condition_count; ++i) {
        // The entry is the guard condition's rmw `data` pointer (see
        // zerodds_refresh_graph_guards), i.e. our `unsigned int` trigger flag.
        // Every guard condition rcl can put here came from
        // rmw_create_guard_condition or rmw_node_get_graph_guard_condition, so
        // the pointer is always ours.
        unsigned int * f = (unsigned int *) guard_conditions->guard_conditions[i];
        if (f && *f) { ready++; }
      }
    }
    if (services) {
      for (size_t i = 0; i < services->service_count; ++i) {
        zerodds_service_data_t * sd = (zerodds_service_data_t *) services->services[i];
        if (sd && rmw_zerodds_service_has_data(sd->bridge_service) > 0) { ready++; }
      }
    }
    if (clients) {
      for (size_t i = 0; i < clients->client_count; ++i) {
        zerodds_client_data_t * cd = (zerodds_client_data_t *) clients->clients[i];
        if (cd && rmw_zerodds_client_has_data(cd->bridge_client) > 0) { ready++; }
      }
    }
    if (events) {
      for (size_t i = 0; i < events->event_count; ++i) {
        if (zerodds_event_pending((const rmw_event_t *) events->events[i])) { ready++; }
      }
    }

    int timed_out = 0;
    if (deadline_ns >= 0 && zerodds_now_ns() >= deadline_ns) { timed_out = 1; }

    // `woke` (set by the previous block) forces a return so the executor can
    // re-evaluate even when nothing is ready (e.g. a cancel guard not in our
    // array). `ready`/`timed_out` are the normal exits.
    if (ready > 0 || timed_out || woke) {
      // Finalize: null non-ready subscriptions; consume/keep ready guard flags.
      if (subscriptions) {
        for (size_t i = 0; i < subscriptions->subscriber_count; ++i) {
          zerodds_sub_data_t * sd = (zerodds_sub_data_t *) subscriptions->subscribers[i];
          if (!zerodds_sub_ready(sd)) {
            subscriptions->subscribers[i] = NULL;
          }
        }
      }
      if (guard_conditions) {
        for (size_t i = 0; i < guard_conditions->guard_condition_count; ++i) {
          // Consume a triggered flag and keep the entry; null the rest. rcl
          // reads back exactly this array to decide which guard conditions
          // fired.
          unsigned int * f = (unsigned int *) guard_conditions->guard_conditions[i];
          if (f && *f) { *f = 0; } else { guard_conditions->guard_conditions[i] = NULL; }
        }
      }
      if (services) {
        for (size_t i = 0; i < services->service_count; ++i) {
          zerodds_service_data_t * sd = (zerodds_service_data_t *) services->services[i];
          if (!sd || rmw_zerodds_service_has_data(sd->bridge_service) <= 0) {
            services->services[i] = NULL;
          }
        }
      }
      if (clients) {
        for (size_t i = 0; i < clients->client_count; ++i) {
          zerodds_client_data_t * cd = (zerodds_client_data_t *) clients->clients[i];
          if (!cd || rmw_zerodds_client_has_data(cd->bridge_client) <= 0) {
            clients->clients[i] = NULL;
          }
        }
      }
      if (events) {
        for (size_t i = 0; i < events->event_count; ++i) {
          if (!zerodds_event_pending((const rmw_event_t *) events->events[i])) { events->events[i] = NULL; }
        }
      }
      // An infinite wait cannot time out. We still return here on a spurious
      // wake so the caller can re-evaluate its own state, but reporting
      // RMW_RET_TIMEOUT for a wait that was never given a deadline is a lie rcl
      // treats as fatal — rcl's graph listener aborts the process with
      // "rcl_wait unexpectedly timed out".
      if (ready > 0 || deadline_ns < 0) { return RMW_RET_OK; }
      return RMW_RET_TIMEOUT;
    }

    // Nothing ready yet — block until a notify or the deadline.
    unsigned long long block_ms;
    if (deadline_ns < 0) {
      block_ms = 1000;  // indefinite: 1s liveness backstop; the condvar wakes early
    } else {
      long long rem_ns = deadline_ns - zerodds_now_ns();
      block_ms = (rem_ns <= 0) ? 1 : (unsigned long long) (rem_ns / 1000000LL) + 1;
    }
    if (bridge_ctx) {
      // 1 → woken by a notify (re-evaluate + return next iteration); 0 → timeout.
      woke = rmw_zerodds_context_wait_block(bridge_ctx, gen, block_ms);
    } else {
      // No bridge context (degenerate): bounded sleep, then re-evaluate + return.
      struct timespec ts = { 0, 2 * 1000 * 1000 };
      nanosleep(&ts, NULL);
      woke = 1;
    }
  }
}

// ---------------------------------------------------------------------------
// serialize / deserialize — introspection CDR (XCDR1), [encap 4][body]. Used by
// rosbag2 record/play and any serialized publish/take path.
// ---------------------------------------------------------------------------
rmw_ret_t
rmw_serialize(const void * ros_message, const rosidl_message_type_support_t * type_support,
              rmw_serialized_message_t * serialized_message)
{
  if (!ros_message || !type_support || !serialized_message) { return RMW_RET_INVALID_ARGUMENT; }
  int members_cpp = 0;
  const ms_t * members = zerodds_introspect_ex(type_support, &members_cpp);
  if (!members) { return RMW_RET_ERROR; }
  cdr_t c = {0};
  c.cpp = members_cpp;
  if (cdr_reserve(&c, 4)) { return RMW_RET_BAD_ALLOC; }
  c.buf[0] = 0x00; c.buf[1] = 0x01; c.buf[2] = 0x00; c.buf[3] = 0x00;
  c.len = 4;
  if (cdr_ser_msg(&c, members, (const unsigned char *) ros_message)) { free(c.buf); return RMW_RET_ERROR; }
  if (rcutils_uint8_array_resize(serialized_message, c.len) != RCUTILS_RET_OK) {
    free(c.buf);
    return RMW_RET_BAD_ALLOC;
  }
  memcpy(serialized_message->buffer, c.buf, c.len);
  serialized_message->buffer_length = c.len;
  free(c.buf);
  return RMW_RET_OK;
}
rmw_ret_t
rmw_deserialize(const rmw_serialized_message_t * serialized_message,
                const rosidl_message_type_support_t * type_support, void * ros_message)
{
  if (!serialized_message || !serialized_message->buffer || !type_support || !ros_message) {
    return RMW_RET_INVALID_ARGUMENT;
  }
  int members_cpp = 0;
  const ms_t * members = zerodds_introspect_ex(type_support, &members_cpp);
  if (!members || serialized_message->buffer_length < 4) { return RMW_RET_ERROR; }
  // RTPS 2.5 §10.5: an even second octet is big-endian (CDR_BE 0x00,
  // PL_CDR_BE 0x02, CDR2_BE 0x06), an odd one little-endian.
  rdr_t r = { serialized_message->buffer, serialized_message->buffer_length, 4,
              (unsigned char) ((serialized_message->buffer[1] & 1) == 0), members_cpp,
              zerodds_stream_is_xcdr2(serialized_message->buffer, serialized_message->buffer_length) };
  if (cdr_de_msg(&r, members, (unsigned char *) ros_message)) { return RMW_RET_ERROR; }
  return RMW_RET_OK;
}
// Conservative CDR upper bound per member (incl. worst-case alignment pad of 7).
// Unbounded strings/sequences use caps so the result is a usable pre-alloc hint;
// rmw_serialize still resizes dynamically for the exact length.
#define ZERODDS_STR_CAP 256u
#define ZERODDS_SEQ_CAP 256u
static size_t zerodds_cdr_max_msg(const ms_t * m);
static size_t zerodds_cdr_max_elem(const mm_t * mem)
{
  size_t ps = prim_size(mem->type_id_);
  if (ps) { return 7 + ps; }
  if (mem->type_id_ == 16) { return 7 + 4 + ZERODDS_STR_CAP + 1; }   // string
  if (mem->type_id_ == 18) { return zerodds_cdr_max_msg((const ms_t *) mem->members_->data); }
  return 0;
}
static size_t zerodds_cdr_max_msg(const ms_t * m)
{
  size_t total = 0;
  for (uint32_t i = 0; i < m->member_count_; ++i) {
    const mm_t * mem = &m->members_[i];
    size_t elem = zerodds_cdr_max_elem(mem);
    if (!mem->is_array_) { total += elem; }
    else if (mem->array_size_ > 0 && !mem->is_upper_bound_) { total += mem->array_size_ * elem; }
    else { total += 4 + (size_t) ZERODDS_SEQ_CAP * elem; }  // sequence: 4 len + cap*elem
  }
  return total;
}
rmw_ret_t
rmw_get_serialized_message_size(const rosidl_message_type_support_t * type_support,
                                const rosidl_runtime_c__Sequence__bound * message_bounds,
                                size_t * size)
{
  (void) message_bounds;
  if (!type_support || !size) { return RMW_RET_INVALID_ARGUMENT; }
  const ms_t * members = (const ms_t *) zerodds_introspect(type_support);
  if (!members) { return RMW_RET_ERROR; }
  *size = 4 + zerodds_cdr_max_msg(members);  // 4-byte encapsulation header
  return RMW_RET_OK;
}

// ---------------------------------------------------------------------------
// allocations (we don't pre-allocate) + events + graph + misc stubs.
// ---------------------------------------------------------------------------
rmw_ret_t rmw_init_publisher_allocation(const rosidl_message_type_support_t * ts,
  const rosidl_runtime_c__Sequence__bound * b, rmw_publisher_allocation_t * a)
{ (void) ts; (void) b; (void) a; return RMW_RET_UNSUPPORTED; }
rmw_ret_t rmw_fini_publisher_allocation(rmw_publisher_allocation_t * a) { (void) a; return RMW_RET_UNSUPPORTED; }
rmw_ret_t rmw_init_subscription_allocation(const rosidl_message_type_support_t * ts,
  const rosidl_runtime_c__Sequence__bound * b, rmw_subscription_allocation_t * a)
{ (void) ts; (void) b; (void) a; return RMW_RET_UNSUPPORTED; }
rmw_ret_t rmw_fini_subscription_allocation(rmw_subscription_allocation_t * a) { (void) a; return RMW_RET_UNSUPPORTED; }

// ---------------------------------------------------------------------------
// QoS events (rmw/event.h). The status lives in the runtime, cumulative; an
// event's `data` is its publisher's or subscription's data, which keeps what
// the last rmw_take_event saw, so a take reports the change since then and a
// wait knows whether there is one. rmw_event_fini (in rmw itself) frees
// nothing, and nothing here is allocated per event.
// ---------------------------------------------------------------------------
#if __has_include(<rmw/events_statuses/matched.h>)
#define ZERODDS_HAS_MATCHED_EVENTS 1
#endif
#if __has_include(<rmw/events_statuses/incompatible_type.h>)
#define ZERODDS_HAS_INCOMPATIBLE_TYPE_EVENTS 1
#endif

typedef struct {
  uint64_t offered_deadline_missed_total;
  uint64_t liveliness_lost_total;
  uint64_t offered_incompatible_qos_total;
  uint32_t offered_incompatible_qos_last_policy_id;
  uint64_t matched_current;
  uint64_t incompatible_type_total;
} zerodds_pub_status_t;
typedef struct {
  uint64_t requested_deadline_missed_total;
  uint64_t liveliness_alive;
  uint64_t liveliness_not_alive;
  uint64_t requested_incompatible_qos_total;
  uint32_t requested_incompatible_qos_last_policy_id;
  uint64_t sample_lost_total;
  uint64_t matched_current;
  uint64_t incompatible_type_total;
} zerodds_sub_status_t;
extern int rmw_zerodds_publisher_get_status(void * pub, zerodds_pub_status_t * out);
extern int rmw_zerodds_subscription_get_status(void * sub, zerodds_sub_status_t * out);
extern int rmw_zerodds_publisher_set_status_callback(void * pub, uint32_t kind,
  rmw_event_callback_t cb, const void * user_data);
extern int rmw_zerodds_subscription_set_status_callback(void * sub, uint32_t kind,
  rmw_event_callback_t cb, const void * user_data);

// The shim's number for an event type: 0..4 publisher, 10..15 subscription,
// -1 for a type this build does not serve.
static int zerodds_event_kind(rmw_event_type_t t)
{
  switch (t) {
    case RMW_EVENT_OFFERED_DEADLINE_MISSED: return 0;
    case RMW_EVENT_LIVELINESS_LOST: return 1;
    case RMW_EVENT_OFFERED_QOS_INCOMPATIBLE: return 2;
#ifdef ZERODDS_HAS_MATCHED_EVENTS
    case RMW_EVENT_PUBLICATION_MATCHED: return 3;
    case RMW_EVENT_SUBSCRIPTION_MATCHED: return 14;
#endif
#ifdef ZERODDS_HAS_INCOMPATIBLE_TYPE_EVENTS
    case RMW_EVENT_PUBLISHER_INCOMPATIBLE_TYPE: return 4;
    case RMW_EVENT_SUBSCRIPTION_INCOMPATIBLE_TYPE: return 15;
#endif
    case RMW_EVENT_REQUESTED_DEADLINE_MISSED: return 10;
    case RMW_EVENT_LIVELINESS_CHANGED: return 11;
    case RMW_EVENT_REQUESTED_QOS_INCOMPATIBLE: return 12;
    case RMW_EVENT_MESSAGE_LOST: return 13;
    default: return -1;
  }
}

// DDS 1.4 Tab. 2.13 QosPolicyId → rmw_qos_policy_kind_t.
static rmw_qos_policy_kind_t zerodds_policy_kind(uint32_t id)
{
  switch (id) {
    case 2: return RMW_QOS_POLICY_DURABILITY;
    case 4: return RMW_QOS_POLICY_DEADLINE;
    case 8: return RMW_QOS_POLICY_LIVELINESS;
    case 11: return RMW_QOS_POLICY_RELIABILITY;
    case 13: return RMW_QOS_POLICY_HISTORY;
    case 21: return RMW_QOS_POLICY_LIFESPAN;
    default: return RMW_QOS_POLICY_INVALID;
  }
}

static int32_t zerodds_i32(uint64_t v) { return v > INT32_MAX ? INT32_MAX : (int32_t) v; }
static int32_t zerodds_delta(uint64_t now, uint64_t before)
{
  int64_t d = (int64_t) now - (int64_t) before;
  return d > INT32_MAX ? INT32_MAX : (d < INT32_MIN ? INT32_MIN : (int32_t) d);
}

static rmw_ret_t zerodds_event_init(rmw_event_t * e, void * data, rmw_event_type_t t, int want_pub)
{
  if (!e || !data) { return RMW_RET_INVALID_ARGUMENT; }
  int kind = zerodds_event_kind(t);
  if (kind < 0 || (kind < 10) != (want_pub != 0)) {
    RMW_SET_ERROR_MSG("event type not supported by rmw_zerodds_cpp for this entity");
    return RMW_RET_UNSUPPORTED;
  }
  e->implementation_identifier = ZERODDS_IDENTIFIER;
  e->data = data;
  e->event_type = t;
  return RMW_RET_OK;
}

rmw_ret_t rmw_publisher_event_init(rmw_event_t * e, const rmw_publisher_t * p, rmw_event_type_t t)
{
  if (!p) { return RMW_RET_INVALID_ARGUMENT; }
  return zerodds_event_init(e, p->data, t, 1);
}
rmw_ret_t rmw_subscription_event_init(rmw_event_t * e, const rmw_subscription_t * s, rmw_event_type_t t)
{
  if (!s) { return RMW_RET_INVALID_ARGUMENT; }
  return zerodds_event_init(e, s->data, t, 0);
}

// Jazzy's rmw_implementation resolves this with every other symbol and
// rcl asks it before creating an event: an event rmw_publisher_event_init
// or rmw_subscription_event_init accepts for its entity is supported.
bool rmw_event_type_is_supported(rmw_event_type_t rmw_event_type)
{
  return zerodds_event_kind(rmw_event_type) >= 0;
}

// Whether the event's counter moved since the last rmw_take_event.
static int zerodds_event_pending(const rmw_event_t * e)
{
  if (!e || !e->data || e->implementation_identifier != ZERODDS_IDENTIFIER) { return 0; }
  int kind = zerodds_event_kind(e->event_type);
  if (kind < 0) { return 0; }
  if (kind < 10) {
    zerodds_pub_data_t * pd = (zerodds_pub_data_t *) e->data;
    zerodds_pub_status_t st;
    if (rmw_zerodds_publisher_get_status(pd->bridge_pub, &st) != RMW_RET_OK) { return 0; }
    const uint64_t now[5] = { st.offered_deadline_missed_total, st.liveliness_lost_total,
      st.offered_incompatible_qos_total, st.matched_current, st.incompatible_type_total };
    return now[kind] != pd->ev_seen[kind];
  }
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) e->data;
  zerodds_sub_status_t st;
  if (rmw_zerodds_subscription_get_status(sd->bridge_sub, &st) != RMW_RET_OK) { return 0; }
  const uint64_t now[6] = { st.requested_deadline_missed_total, st.liveliness_alive,
    st.requested_incompatible_qos_total, st.sample_lost_total, st.matched_current,
    st.incompatible_type_total };
  if (kind == 11 && st.liveliness_not_alive != sd->ev_seen_not_alive) { return 1; }
  return now[kind - 10] != sd->ev_seen[kind - 10];
}

rmw_ret_t rmw_take_event(const rmw_event_t * e, void * d, bool * taken)
{
  if (taken) { *taken = false; }
  if (!e || !e->data || !d || !taken) { return RMW_RET_INVALID_ARGUMENT; }
  if (e->implementation_identifier != ZERODDS_IDENTIFIER) { return RMW_RET_INCORRECT_RMW_IMPLEMENTATION; }
  int kind = zerodds_event_kind(e->event_type);
  if (kind < 0) { return RMW_RET_UNSUPPORTED; }
  if (kind < 10) {
    zerodds_pub_data_t * pd = (zerodds_pub_data_t *) e->data;
    zerodds_pub_status_t st;
    if (rmw_zerodds_publisher_get_status(pd->bridge_pub, &st) != RMW_RET_OK) { return RMW_RET_ERROR; }
    uint64_t * seen = &pd->ev_seen[kind];
    switch (kind) {
      case 0: {
        rmw_offered_deadline_missed_status_t * o = (rmw_offered_deadline_missed_status_t *) d;
        o->total_count = zerodds_i32(st.offered_deadline_missed_total);
        o->total_count_change = zerodds_delta(st.offered_deadline_missed_total, *seen);
        *seen = st.offered_deadline_missed_total;
        break;
      }
      case 1: {
        rmw_liveliness_lost_status_t * o = (rmw_liveliness_lost_status_t *) d;
        o->total_count = zerodds_i32(st.liveliness_lost_total);
        o->total_count_change = zerodds_delta(st.liveliness_lost_total, *seen);
        *seen = st.liveliness_lost_total;
        break;
      }
      case 2: {
        rmw_offered_qos_incompatible_event_status_t * o = (rmw_offered_qos_incompatible_event_status_t *) d;
        o->total_count = zerodds_i32(st.offered_incompatible_qos_total);
        o->total_count_change = zerodds_delta(st.offered_incompatible_qos_total, *seen);
        o->last_policy_kind = zerodds_policy_kind(st.offered_incompatible_qos_last_policy_id);
        *seen = st.offered_incompatible_qos_total;
        break;
      }
#ifdef ZERODDS_HAS_MATCHED_EVENTS
      case 3: {
        rmw_matched_status_t * o = (rmw_matched_status_t *) d;
        uint64_t added = st.matched_current > *seen ? st.matched_current - *seen : 0;
        pd->ev_matched_total += added;
        o->total_count = (size_t) pd->ev_matched_total;
        o->total_count_change = (size_t) added;
        o->current_count = (size_t) st.matched_current;
        o->current_count_change = zerodds_delta(st.matched_current, *seen);
        *seen = st.matched_current;
        break;
      }
#endif
#ifdef ZERODDS_HAS_INCOMPATIBLE_TYPE_EVENTS
      case 4: {
        rmw_incompatible_type_status_t * o = (rmw_incompatible_type_status_t *) d;
        o->total_count = zerodds_i32(st.incompatible_type_total);
        o->total_count_change = zerodds_delta(st.incompatible_type_total, *seen);
        *seen = st.incompatible_type_total;
        break;
      }
#endif
      default: return RMW_RET_UNSUPPORTED;
    }
    *taken = true;
    return RMW_RET_OK;
  }
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) e->data;
  zerodds_sub_status_t st;
  if (rmw_zerodds_subscription_get_status(sd->bridge_sub, &st) != RMW_RET_OK) { return RMW_RET_ERROR; }
  uint64_t * seen = &sd->ev_seen[kind - 10];
  switch (kind) {
    case 10: {
      rmw_requested_deadline_missed_status_t * o = (rmw_requested_deadline_missed_status_t *) d;
      o->total_count = zerodds_i32(st.requested_deadline_missed_total);
      o->total_count_change = zerodds_delta(st.requested_deadline_missed_total, *seen);
      *seen = st.requested_deadline_missed_total;
      break;
    }
    case 11: {
      // Writers seen becoming alive / not alive, as the DCPS DataReader
      // reports LIVELINESS_CHANGED.
      rmw_liveliness_changed_status_t * o = (rmw_liveliness_changed_status_t *) d;
      o->alive_count = zerodds_i32(st.liveliness_alive);
      o->not_alive_count = zerodds_i32(st.liveliness_not_alive);
      o->alive_count_change = zerodds_delta(st.liveliness_alive, *seen);
      o->not_alive_count_change = zerodds_delta(st.liveliness_not_alive, sd->ev_seen_not_alive);
      *seen = st.liveliness_alive;
      sd->ev_seen_not_alive = st.liveliness_not_alive;
      break;
    }
    case 12: {
      rmw_requested_qos_incompatible_event_status_t * o = (rmw_requested_qos_incompatible_event_status_t *) d;
      o->total_count = zerodds_i32(st.requested_incompatible_qos_total);
      o->total_count_change = zerodds_delta(st.requested_incompatible_qos_total, *seen);
      o->last_policy_kind = zerodds_policy_kind(st.requested_incompatible_qos_last_policy_id);
      *seen = st.requested_incompatible_qos_total;
      break;
    }
    case 13: {
      rmw_message_lost_status_t * o = (rmw_message_lost_status_t *) d;
      o->total_count = (size_t) st.sample_lost_total;
      o->total_count_change = (size_t) (st.sample_lost_total - *seen);
      *seen = st.sample_lost_total;
      break;
    }
#ifdef ZERODDS_HAS_MATCHED_EVENTS
    case 14: {
      rmw_matched_status_t * o = (rmw_matched_status_t *) d;
      uint64_t added = st.matched_current > *seen ? st.matched_current - *seen : 0;
      sd->ev_matched_total += added;
      o->total_count = (size_t) sd->ev_matched_total;
      o->total_count_change = (size_t) added;
      o->current_count = (size_t) st.matched_current;
      o->current_count_change = zerodds_delta(st.matched_current, *seen);
      *seen = st.matched_current;
      break;
    }
#endif
#ifdef ZERODDS_HAS_INCOMPATIBLE_TYPE_EVENTS
    case 15: {
      rmw_incompatible_type_status_t * o = (rmw_incompatible_type_status_t *) d;
      o->total_count = zerodds_i32(st.incompatible_type_total);
      o->total_count_change = zerodds_delta(st.incompatible_type_total, *seen);
      *seen = st.incompatible_type_total;
      break;
    }
#endif
    default: return RMW_RET_UNSUPPORTED;
  }
  *taken = true;
  return RMW_RET_OK;
}

rmw_ret_t rmw_event_set_callback(rmw_event_t * e, rmw_event_callback_t cb, const void * u)
{
  if (!e || !e->data) { return RMW_RET_INVALID_ARGUMENT; }
  if (e->implementation_identifier != ZERODDS_IDENTIFIER) { return RMW_RET_INCORRECT_RMW_IMPLEMENTATION; }
  int kind = zerodds_event_kind(e->event_type);
  if (kind < 0) { return RMW_RET_UNSUPPORTED; }
  if (kind < 10) {
    return rmw_zerodds_publisher_set_status_callback(
      ((zerodds_pub_data_t *) e->data)->bridge_pub, (uint32_t) kind, cb, u);
  }
  return rmw_zerodds_subscription_set_status_callback(
    ((zerodds_sub_data_t *) e->data)->bridge_sub, (uint32_t) kind, cb, u);
}

// Both sequence numbers are filled on the serialized (RTPS) path; the raw
// delivery modes report RMW_MESSAGE_INFO_SEQUENCE_NUMBER_UNSUPPORTED for the
// publication one, as rmw allows per message.
bool rmw_feature_supported(rmw_feature_t feature)
{
  return feature == RMW_FEATURE_MESSAGE_INFO_PUBLICATION_SEQUENCE_NUMBER ||
         feature == RMW_FEATURE_MESSAGE_INFO_RECEPTION_SEQUENCE_NUMBER;
}

rmw_ret_t rmw_compare_gids_equal(const rmw_gid_t * a, const rmw_gid_t * b, bool * result)
{ if (!a || !b || !result) { return RMW_RET_INVALID_ARGUMENT; } *result = (memcmp(a->data, b->data, RMW_GID_STORAGE_SIZE) == 0); return RMW_RET_OK; }

// ---------------------------------------------------------------------------
// Graph introspection (P4a) — discovered topics via the node's runtime SEDP.
// rmw_get_node_names needs the rmw_dds_common graph cache (ros_discovery_info)
// and stays empty (see ros2-rmw.md, P4b).
// ---------------------------------------------------------------------------
#include <rcutils/types/string_array.h>

// "rt/chatter" → "/chatter"; NULL for a non-"rt/" topic (service/internal),
// excluding it from topic listings.
static char * zerodds_demangle_topic(const char * t)
{
  if (strncmp(t, "rt/", 3) != 0) { return NULL; }
  size_t n = strlen(t) - 3;
  char * out = (char *) malloc(n + 2);
  if (!out) { return NULL; }
  out[0] = '/'; memcpy(out + 1, t + 3, n); out[n + 1] = '\0';
  return out;
}
// DDS type name -> ROS type name, as rmw_get_topic_names_and_types reports
// it: "std_msgs::msg::dds_::String_" (every ROS middleware, and this RMW
// since it announces that form) -> "std_msgs/msg/String". The `dds_`
// segment goes and the trailing `_` of the name after it. The earlier form
// this RMW announced, "std_msgs__msg::String", still maps: "__" and "::"
// are both separators.
static char * zerodds_demangle_type(const char * ty)
{
  size_t n = strlen(ty);
  char * out = (char *) malloc(n + 1);
  if (!out) { return NULL; }
  size_t j = 0;
  int dds_segment = 0;
  for (size_t i = 0; i < n; ) {
    if (strncmp(ty + i, "dds_::", 6) == 0 && (i == 0 || out[j - 1] == '/')) {
      dds_segment = 1; i += 6; continue;
    }
    if (ty[i] == ':' && i + 1 < n && ty[i + 1] == ':') { out[j++] = '/'; i += 2; }
    else if (ty[i] == '_' && i + 1 < n && ty[i + 1] == '_') { out[j++] = '/'; i += 2; }
    else { out[j++] = ty[i++]; }
  }
  if (dds_segment && j > 0 && out[j - 1] == '_') { --j; }
  out[j] = '\0';
  return out;
}

typedef struct { const char * target; size_t count; } zerodds_count_ctx_t;
static void zerodds_count_cb(void * ud, const char * topic, const char * type)
{
  (void) type;
  zerodds_count_ctx_t * c = (zerodds_count_ctx_t *) ud;
  char * d = zerodds_demangle_topic(topic);
  if (d) { if (strcmp(d, c->target) == 0) { c->count++; } free(d); }
}

rmw_ret_t rmw_count_publishers(const rmw_node_t * n, const char * t, size_t * c)
{
  if (!n || !n->data || !t || !c) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;
  zerodds_count_ctx_t cx = { t, 0 };
  rmw_zerodds_node_for_each_publication(nd->bridge_node, zerodds_count_cb, &cx);
  *c = cx.count;
  return RMW_RET_OK;
}
rmw_ret_t rmw_count_subscribers(const rmw_node_t * n, const char * t, size_t * c)
{
  if (!n || !n->data || !t || !c) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;
  zerodds_count_ctx_t cx = { t, 0 };
  rmw_zerodds_node_for_each_subscription(nd->bridge_node, zerodds_count_cb, &cx);
  *c = cx.count;
  return RMW_RET_OK;
}
// "rq/add_two_intsRequest" and "rr/add_two_intsReply" → "/add_two_ints"; NULL
// for any other topic. The service topic names of every ROS 2 RMW
// (rmw_cyclonedds_cpp `make_fqtopic`, `src/ros2_config.rs`).
static char * zerodds_demangle_service(const char * t, int * is_request)
{
  const char * suffix;
  if (strncmp(t, "rq/", 3) == 0) { suffix = "Request"; if (is_request) { *is_request = 1; } }
  else if (strncmp(t, "rr/", 3) == 0) { suffix = "Reply"; if (is_request) { *is_request = 0; } }
  else { return NULL; }
  size_t n = strlen(t), sl = strlen(suffix);
  if (n <= 3 + sl || strcmp(t + n - sl, suffix) != 0) { return NULL; }
  size_t body = n - 3 - sl;
  char * out = (char *) malloc(body + 2);
  if (!out) { return NULL; }
  out[0] = '/'; memcpy(out + 1, t + 3, body); out[body + 1] = '\0';
  return out;
}
// "example_interfaces::srv::dds_::AddTwoInts_Request_" (or `_Response_`) →
// "example_interfaces/srv/AddTwoInts"; NULL for a type of neither form.
static char * zerodds_demangle_service_type(const char * ty)
{
  char * d = zerodds_demangle_type(ty);
  if (!d) { return NULL; }
  size_t n = strlen(d);
  const char * suffixes[] = { "_Request", "_Response" };
  for (size_t i = 0; i < 2; ++i) {
    size_t sl = strlen(suffixes[i]);
    if (n > sl && strcmp(d + n - sl, suffixes[i]) == 0) { d[n - sl] = '\0'; return d; }
  }
  free(d);
  return NULL;
}
static void zerodds_count_request_cb(void * ud, const char * topic, const char * type)
{
  (void) type;
  zerodds_count_ctx_t * c = (zerodds_count_ctx_t *) ud;
  int is_request = 0;
  char * d = zerodds_demangle_service(topic, &is_request);
  if (d) { if (is_request && strcmp(d, c->target) == 0) { c->count++; } free(d); }
}
// A client has a writer on the request topic, a service a reader.
rmw_ret_t rmw_count_clients(const rmw_node_t * n, const char * t, size_t * c)
{
  if (!n || !n->data || !t || !c) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;
  zerodds_count_ctx_t cx = { t, 0 };
  rmw_zerodds_node_for_each_publication(nd->bridge_node, zerodds_count_request_cb, &cx);
  *c = cx.count;
  return RMW_RET_OK;
}
rmw_ret_t rmw_count_services(const rmw_node_t * n, const char * t, size_t * c)
{
  if (!n || !n->data || !t || !c) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;
  zerodds_count_ctx_t cx = { t, 0 };
  rmw_zerodds_node_for_each_subscription(nd->bridge_node, zerodds_count_request_cb, &cx);
  *c = cx.count;
  return RMW_RET_OK;
}

// ---------------------------------------------------------------------------
// Services / clients. The bridge (`src/services.rs`) owns the topics and the
// correlation; this layer serializes [encap][body] and maps rcl's request ids.
// ---------------------------------------------------------------------------
// (zerodds_client_data_t / zerodds_service_data_t are declared above rmw_wait.)

// Resolve the introspection ServiceMembers (request/response MessageMembers).
typedef rosidl_typesupport_introspection_c__ServiceMembers svc_ms_t;
// `is_cpp` (optional out) reports the introspection flavour, exactly as
// zerodds_introspect_ex does for messages. The C and C++ ServiceMembers have
// the same layout ({namespace, name, request_members_, response_members_}), but
// the request/response MessageMembers they point at describe C respectively C++
// runtime types, so the codec has to know which.
static const svc_ms_t * zerodds_service_introspect_ex(
  const rosidl_service_type_support_t * ts, int * is_cpp)
{
  if (is_cpp) { *is_cpp = 0; }
  if (!ts) { return NULL; }
  if (ts->typesupport_identifier == rosidl_typesupport_introspection_c__identifier && ts->data) {
    return (const svc_ms_t *) ts->data;
  }
  if (ts->func) {
    // Prefer the C introspection (rclpy / C services) …
    const rosidl_service_type_support_t * h =
      ts->func(ts, rosidl_typesupport_introspection_c__identifier);
    if (h && h->data) { return (const svc_ms_t *) h->data; }
    // … then fall back to the C++ introspection (rclcpp services). Without this
    // an rclcpp client/service resolves to nothing and rmw_create_client
    // returns NULL. The miss above left its error message set; cleared as
    // for messages.
    rcutils_reset_error();
    h = ts->func(ts, ZERODDS_INTROSPECTION_CPP_ID);
    if (h && h->data) {
      if (is_cpp) { *is_cpp = 1; }
      return (const svc_ms_t *) h->data;
    }
  }
  if (ts->typesupport_identifier && ts->data &&
      strcmp(ts->typesupport_identifier, ZERODDS_INTROSPECTION_CPP_ID) == 0)
  {
    if (is_cpp) { *is_cpp = 1; }
    return (const svc_ms_t *) ts->data;
  }
  return NULL;
}

static void zerodds_make_gid(unsigned char gid[16], const void * uniq)
{
  static unsigned long long counter = 1;
  unsigned long long a = (unsigned long long) (uintptr_t) uniq;  // instance address
  unsigned long long b = counter++;                              // process-local
  memcpy(gid, &a, 8);
  memcpy(gid + 8, &b, 8);
}

// Serialize `msg` (introspection members) to [encap 4][cdr body] in `out`.
static int zerodds_cdr_message(cdr_t * out, const ms_t * members, const void * msg, int cpp)
{
  out->cpp = cpp;
  if (cdr_reserve(out, 4)) { return -1; }
  out->buf[0] = 0x00; out->buf[1] = 0x01; out->buf[2] = 0x00; out->buf[3] = 0x00;
  out->len = 4;
  return cdr_ser_msg(out, members, (const unsigned char *) msg);
}

// The DDS type names of a service's request and response, as the other RMWs
// announce them: `example_interfaces::srv::dds_::AddTwoInts_Request_` and
// `..._Response_` (the introspection members are named `AddTwoInts_Request`).
// The RMW used to announce one name, `example_interfaces__srv::AddTwoInts`,
// for both, which no other RMW's endpoint matches.
static int zerodds_service_type_names(const svc_ms_t * sm, char ** req, char ** resp)
{
  *req = NULL; *resp = NULL;
  const ms_t * rq = sm->request_members_;
  const ms_t * rp = sm->response_members_;
  if (!rq || !rp || !rq->message_namespace_ || !rq->message_name_ ||
      !rp->message_namespace_ || !rp->message_name_) { return -1; }
  *req = zerodds_dds_type_name(rq->message_namespace_, rq->message_name_);
  *resp = zerodds_dds_type_name(rp->message_namespace_, rp->message_name_);
  if (!*req || !*resp) { free(*req); free(*resp); *req = NULL; *resp = NULL; return -1; }
  return 0;
}

rmw_client_t * rmw_create_client(const rmw_node_t * n, const rosidl_service_type_support_t * ts,
  const char * name, const rmw_qos_profile_t * q)
{
  if (!n || !n->data || !ts || !name || !q) { return NULL; }
  int members_cpp = 0;
  const svc_ms_t * sm = zerodds_service_introspect_ex(ts, &members_cpp);
  if (!sm) { return NULL; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;
  char * req_t = NULL; char * resp_t = NULL;
  if (zerodds_service_type_names(sm, &req_t, &resp_t) != 0) { return NULL; }
  zerodds_rmw_qos_t zq = zerodds_qos_from(q);
  zerodds_rmw_qos_t za;
  memset(&za, 0, sizeof(za));
  char * th = zerodds_service_type_hash(ts);
  void * bc = rmw_zerodds_create_client_hashed(nd->bridge_node, name, req_t, resp_t, &zq, &za, th);
  zerodds_free_type_hash(th);
  free(req_t); free(resp_t);
  if (!bc) { return NULL; }
  rmw_client_t * c = rmw_client_allocate();
  zerodds_client_data_t * cd = (zerodds_client_data_t *) calloc(1, sizeof(zerodds_client_data_t));
  if (!c || !cd) { free(cd); if (c) { rmw_client_free(c); } rmw_zerodds_destroy_client(bc); return NULL; }
  cd->bridge_client = bc;
  cd->req_members = sm->request_members_;
  cd->resp_members = sm->response_members_;
  cd->members_cpp = members_cpp;
  cd->qos = zerodds_qos_to(&za, q);
  // The request writer's GUID, the writer_guid of every rmw_request_id_t of
  // this client.
  if (rmw_zerodds_client_gid(bc, cd->gid) != RMW_RET_OK) { zerodds_make_gid(cd->gid, cd); }
  c->implementation_identifier = ZERODDS_IDENTIFIER;
  c->data = cd;
  c->service_name = rcutils_strdup(name, n->context->options.allocator);
  return c;
}

rmw_ret_t rmw_destroy_client(rmw_node_t * n, rmw_client_t * c)
{
  if (!c || !c->data) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_client_data_t * cd = (zerodds_client_data_t *) c->data;
  zerodds_svc_adapters_release(c->data);  // REP-2007: drop any bound adapters
  rmw_zerodds_destroy_client(cd->bridge_client);
  if (n && c->service_name) {
    rcutils_allocator_t a = n->context->options.allocator;
    a.deallocate((char *) c->service_name, a.state);
  }
  free(cd);
  rmw_client_free(c);
  return RMW_RET_OK;
}

rmw_ret_t rmw_send_request(const rmw_client_t * c, const void * req, int64_t * seq)
{
  if (!c || !c->data || !req || !seq) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_client_data_t * cd = (zerodds_client_data_t *) c->data;
  if (!cd->req_members) { return RMW_RET_ERROR; }
  cdr_t cc = {0};
  if (zerodds_cdr_message(&cc, cd->req_members, req, cd->members_cpp)) { free(cc.buf); return RMW_RET_ERROR; }
  int rc = rmw_zerodds_send_request(cd->bridge_client, cc.buf, cc.len, seq);
  free(cc.buf);
  return rc == 0 ? RMW_RET_OK : RMW_RET_ERROR;
}

rmw_ret_t rmw_take_response(const rmw_client_t * c, rmw_service_info_t * h, void * r, bool * taken)
{
  if (taken) { *taken = false; }
  if (!c || !c->data || !r) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_client_data_t * cd = (zerodds_client_data_t *) c->data;
  unsigned char * buf = NULL; size_t len = 0; unsigned char be = 0; int64_t s = 0;
  if (rmw_zerodds_take_response(cd->bridge_client, &buf, &len, &be, &s) != 0 || !buf) { return RMW_RET_OK; }
  if (len < 4) { rmw_zerodds_buffer_free(buf, len); return RMW_RET_ERROR; }
  rdr_t rr = { buf, len, 4, be, cd->members_cpp, zerodds_stream_is_xcdr2(buf, len) };
  int dr = cd->resp_members ? cdr_de_msg(&rr, cd->resp_members, (unsigned char *) r) : -1;
  rmw_zerodds_buffer_free(buf, len);
  if (dr != 0) { return RMW_RET_ERROR; }
  if (h) {
    memcpy(h->request_id.writer_guid, cd->gid, 16);
    h->request_id.sequence_number = s;
    h->source_timestamp = 0;
    h->received_timestamp = 0;
  }
  if (taken) { *taken = true; }
  return RMW_RET_OK;
}

rmw_ret_t rmw_client_request_publisher_get_actual_qos(const rmw_client_t * c, rmw_qos_profile_t * q)
{
  if (!c || !c->data || !q) { return RMW_RET_INVALID_ARGUMENT; }
  *q = ((zerodds_client_data_t *) c->data)->qos;
  return RMW_RET_OK;
}
rmw_ret_t rmw_client_response_subscription_get_actual_qos(const rmw_client_t * c, rmw_qos_profile_t * q)
{
  if (!c || !c->data || !q) { return RMW_RET_INVALID_ARGUMENT; }
  *q = ((zerodds_client_data_t *) c->data)->qos;
  return RMW_RET_OK;
}
rmw_ret_t rmw_client_set_on_new_response_callback(rmw_client_t * c, rmw_event_callback_t cb, const void * u)
{
  if (!c || !c->data) { return RMW_RET_INVALID_ARGUMENT; }
  rmw_zerodds_client_set_event_callback(((zerodds_client_data_t *) c->data)->bridge_client, cb, u);
  return RMW_RET_OK;
}

rmw_service_t * rmw_create_service(const rmw_node_t * n, const rosidl_service_type_support_t * ts,
  const char * name, const rmw_qos_profile_t * q)
{
  if (!n || !n->data || !ts || !name || !q) { return NULL; }
  int members_cpp = 0;
  const svc_ms_t * sm = zerodds_service_introspect_ex(ts, &members_cpp);
  if (!sm) { return NULL; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;
  char * req_t = NULL; char * resp_t = NULL;
  if (zerodds_service_type_names(sm, &req_t, &resp_t) != 0) { return NULL; }
  zerodds_rmw_qos_t zq = zerodds_qos_from(q);
  zerodds_rmw_qos_t za;
  memset(&za, 0, sizeof(za));
  char * th = zerodds_service_type_hash(ts);
  void * bs = rmw_zerodds_create_service_hashed(nd->bridge_node, name, req_t, resp_t, &zq, &za, th);
  zerodds_free_type_hash(th);
  free(req_t); free(resp_t);
  if (!bs) { return NULL; }
  rmw_service_t * sv = rmw_service_allocate();
  zerodds_service_data_t * sd = (zerodds_service_data_t *) calloc(1, sizeof(zerodds_service_data_t));
  if (!sv || !sd) { free(sd); if (sv) { rmw_service_free(sv); } rmw_zerodds_destroy_service(bs); return NULL; }
  sd->bridge_service = bs;
  sd->req_members = sm->request_members_;
  sd->resp_members = sm->response_members_;
  sd->members_cpp = members_cpp;
  sd->qos = zerodds_qos_to(&za, q);
  sv->implementation_identifier = ZERODDS_IDENTIFIER;
  sv->data = sd;
  sv->service_name = rcutils_strdup(name, n->context->options.allocator);
  return sv;
}

rmw_ret_t rmw_set_log_severity(rmw_log_severity_t severity) { (void) severity; return RMW_RET_OK; }

rmw_ret_t rmw_destroy_service(rmw_node_t * n, rmw_service_t * s)
{
  if (!s || !s->data) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_service_data_t * sd = (zerodds_service_data_t *) s->data;
  zerodds_svc_adapters_release(s->data);  // REP-2007: drop any bound adapters
  rmw_zerodds_destroy_service(sd->bridge_service);
  if (n && s->service_name) {
    rcutils_allocator_t a = n->context->options.allocator;
    a.deallocate((char *) s->service_name, a.state);
  }
  free(sd);
  rmw_service_free(s);
  return RMW_RET_OK;
}

rmw_ret_t rmw_take_request(const rmw_service_t * s, rmw_service_info_t * h, void * r, bool * taken)
{
  if (taken) { *taken = false; }
  if (!s || !s->data || !r) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_service_data_t * sd = (zerodds_service_data_t *) s->data;
  unsigned char * buf = NULL; size_t len = 0; unsigned char be = 0;
  unsigned char guid[16]; int64_t s_seq = 0;
  if (rmw_zerodds_take_request(sd->bridge_service, &buf, &len, &be, guid, &s_seq) != 0 || !buf) {
    return RMW_RET_OK;
  }
  if (len < 4) { rmw_zerodds_buffer_free(buf, len); return RMW_RET_ERROR; }
  rdr_t rr = { buf, len, 4, be, sd->members_cpp, zerodds_stream_is_xcdr2(buf, len) };
  int dr = sd->req_members ? cdr_de_msg(&rr, sd->req_members, (unsigned char *) r) : -1;
  rmw_zerodds_buffer_free(buf, len);
  if (dr != 0) { return RMW_RET_ERROR; }
  if (h) {
    memcpy(h->request_id.writer_guid, guid, 16);  // the bridge's id of the request
    h->request_id.sequence_number = s_seq;
    h->source_timestamp = 0;
    h->received_timestamp = 0;
  }
  if (taken) { *taken = true; }
  return RMW_RET_OK;
}

rmw_ret_t rmw_send_response(const rmw_service_t * s, rmw_request_id_t * id, void * r)
{
  if (!s || !s->data || !id || !r) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_service_data_t * sd = (zerodds_service_data_t *) s->data;
  if (!sd->resp_members) { return RMW_RET_ERROR; }
  cdr_t cc = {0};
  if (zerodds_cdr_message(&cc, sd->resp_members, r, sd->members_cpp)) { free(cc.buf); return RMW_RET_ERROR; }
  int rc = rmw_zerodds_send_response(sd->bridge_service, (const unsigned char *) id->writer_guid,
                                     id->sequence_number, cc.buf, cc.len);
  free(cc.buf);
  return (rmw_ret_t) rc;  // RMW_RET_OK, _TIMEOUT (client reader not matched) or _ERROR
}

rmw_ret_t rmw_service_request_subscription_get_actual_qos(const rmw_service_t * s, rmw_qos_profile_t * q)
{
  if (!s || !s->data || !q) { return RMW_RET_INVALID_ARGUMENT; }
  *q = ((zerodds_service_data_t *) s->data)->qos;
  return RMW_RET_OK;
}
rmw_ret_t rmw_service_response_publisher_get_actual_qos(const rmw_service_t * s, rmw_qos_profile_t * q)
{
  if (!s || !s->data || !q) { return RMW_RET_INVALID_ARGUMENT; }
  *q = ((zerodds_service_data_t *) s->data)->qos;
  return RMW_RET_OK;
}
rmw_ret_t rmw_service_set_on_new_request_callback(rmw_service_t * s, rmw_event_callback_t cb, const void * u)
{
  if (!s || !s->data) { return RMW_RET_INVALID_ARGUMENT; }
  rmw_zerodds_service_set_event_callback(((zerodds_service_data_t *) s->data)->bridge_service, cb, u);
  return RMW_RET_OK;
}
rmw_ret_t rmw_service_server_is_available(const rmw_node_t * n, const rmw_client_t * c, bool * avail)
{
  (void) n;
  if (avail) { *avail = false; }
  if (!c || !c->data) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_client_data_t * cd = (zerodds_client_data_t *) c->data;
  if (avail) { *avail = rmw_zerodds_client_server_available(cd->bridge_client) > 0; }
  return RMW_RET_OK;
}

rmw_ret_t rmw_subscription_set_on_new_message_callback(rmw_subscription_t * s, rmw_event_callback_t cb, const void * u)
{
  if (!s || !s->data) { return RMW_RET_INVALID_ARGUMENT; }
  rmw_zerodds_subscription_set_event_callback(((zerodds_sub_data_t *) s->data)->bridge_sub, cb, u);
  return RMW_RET_OK;
}
rmw_ret_t rmw_subscription_set_content_filter(rmw_subscription_t * s, const rmw_subscription_content_filter_options_t * o)
{
  if (!s || !s->data || !o) { return RMW_RET_INVALID_ARGUMENT; }
  if (s->implementation_identifier != ZERODDS_IDENTIFIER) { return RMW_RET_INCORRECT_RMW_IMPLEMENTATION; }
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) s->data;
  rmw_ret_t ret = zerodds_sub_set_filter(sd, o, rcutils_get_default_allocator());
  if (ret == RMW_RET_OK) { s->is_cft_enabled = sd->cft_on ? true : false; }
  return ret;
}
rmw_ret_t rmw_subscription_get_content_filter(const rmw_subscription_t * s, rcutils_allocator_t * a,
  rmw_subscription_content_filter_options_t * o)
{
  if (!s || !s->data || !a || !o) { return RMW_RET_INVALID_ARGUMENT; }
  if (s->implementation_identifier != ZERODDS_IDENTIFIER) { return RMW_RET_INCORRECT_RMW_IMPLEMENTATION; }
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) s->data;
  if (!sd->cft_on) { RMW_SET_ERROR_MSG("the subscription has no content filter"); return RMW_RET_ERROR; }
  return rmw_subscription_content_filter_options_copy(&sd->cft, a, o);
}

// ---------------------------------------------------------------------------
// Graph: nodes from ros_discovery_info, topics and services from discovery.
// ---------------------------------------------------------------------------
// Node-graph bridge (ros_discovery_info): enumerate (name, namespace) pairs.
typedef void (*zerodds_node_cb_t)(void * ud, const char * name, const char * ns);
extern int rmw_zerodds_for_each_node(void * ctx, zerodds_node_cb_t cb, void * ud);

typedef struct { char ** names; char ** nss; size_t count; size_t cap; } zerodds_nodes_t;
static void zerodds_node_accum(void * ud, const char * name, const char * ns)
{
  zerodds_nodes_t * a = (zerodds_nodes_t *) ud;
  for (size_t i = 0; i < a->count; ++i) {
    if (strcmp(a->names[i], name) == 0 && strcmp(a->nss[i], ns) == 0) { return; }
  }
  if (a->count == a->cap) {
    size_t nc = a->cap ? a->cap * 2 : 8;
    a->names = (char **) realloc(a->names, nc * sizeof(char *));
    a->nss = (char **) realloc(a->nss, nc * sizeof(char *));
    a->cap = nc;
  }
  a->names[a->count] = strdup(name);
  a->nss[a->count] = strdup(ns);
  a->count++;
}
static void * zerodds_node_bridge_ctx(const rmw_node_t * n)
{
  if (!n || !n->context || !n->context->impl) { return NULL; }
  return n->context->impl->bridge_ctx;
}
rmw_ret_t rmw_get_node_names(const rmw_node_t * n, rcutils_string_array_t * names, rcutils_string_array_t * ns)
{
  if (!n || !names || !ns) { return RMW_RET_INVALID_ARGUMENT; }
  void * ctx = zerodds_node_bridge_ctx(n);
  if (!ctx) { return RMW_RET_ERROR; }
  zerodds_nodes_t acc = {0};
  rmw_zerodds_for_each_node(ctx, zerodds_node_accum, &acc);
  rcutils_allocator_t a = n->context->options.allocator;
  rmw_ret_t ret = RMW_RET_OK;
  if (rcutils_string_array_init(names, acc.count, &a) != RCUTILS_RET_OK ||
      rcutils_string_array_init(ns, acc.count, &a) != RCUTILS_RET_OK)
  {
    ret = RMW_RET_BAD_ALLOC;
  } else {
    for (size_t i = 0; i < acc.count; ++i) {
      names->data[i] = rcutils_strdup(acc.names[i], a);
      ns->data[i] = rcutils_strdup(acc.nss[i], a);
    }
  }
  for (size_t i = 0; i < acc.count; ++i) { free(acc.names[i]); free(acc.nss[i]); }
  free(acc.names); free(acc.nss);
  return ret;
}
rmw_ret_t rmw_get_node_names_with_enclaves(const rmw_node_t * n, rcutils_string_array_t * names,
  rcutils_string_array_t * ns, rcutils_string_array_t * enc)
{
  rmw_ret_t ret = rmw_get_node_names(n, names, ns);
  if (ret != RMW_RET_OK) { return ret; }
  // Enclaves are not tracked per node — report the default "/" for each.
  rcutils_allocator_t a = n->context->options.allocator;
  if (enc && rcutils_string_array_init(enc, names->size, &a) == RCUTILS_RET_OK) {
    for (size_t i = 0; i < names->size; ++i) { enc->data[i] = rcutils_strdup("/", a); }
  }
  return RMW_RET_OK;
}
// Accumulate unique (topic → distinct types) from the discovery callbacks.
typedef struct { char ** topics; char *** types; size_t * ntypes; size_t count; size_t cap; } zerodds_tt_t;
static void zerodds_tt_add(zerodds_tt_t * a, char * topic, char * type)
{
  for (size_t i = 0; i < a->count; ++i) {
    if (strcmp(a->topics[i], topic) == 0) {
      free(topic);
      for (size_t j = 0; j < a->ntypes[i]; ++j) {
        if (strcmp(a->types[i][j], type) == 0) { free(type); return; }
      }
      char ** nt = (char **) realloc(a->types[i], (a->ntypes[i] + 1) * sizeof(char *));
      if (!nt) { free(type); return; }
      a->types[i] = nt;
      a->types[i][a->ntypes[i]++] = type;
      return;
    }
  }
  if (a->count == a->cap) {
    size_t nc = a->cap ? a->cap * 2 : 8;
    a->topics = (char **) realloc(a->topics, nc * sizeof(char *));
    a->types = (char ***) realloc(a->types, nc * sizeof(char **));
    a->ntypes = (size_t *) realloc(a->ntypes, nc * sizeof(size_t));
    a->cap = nc;
  }
  a->topics[a->count] = topic;
  a->types[a->count] = (char **) malloc(sizeof(char *));
  a->types[a->count][0] = type;
  a->ntypes[a->count] = 1;
  a->count++;
}
static void zerodds_tt_cb(void * ud, const char * topic, const char * type)
{
  zerodds_tt_t * a = (zerodds_tt_t *) ud;
  char * dt = zerodds_demangle_topic(topic);
  if (!dt) { return; }
  char * dy = zerodds_demangle_type(type);
  if (!dy) { free(dt); return; }
  zerodds_tt_add(a, dt, dy);
}
// no_demangle: every topic under its DDS name and type, services and
// internal topics included (what rmw_cyclonedds_cpp reports then).
static void zerodds_tt_raw_cb(void * ud, const char * topic, const char * type)
{
  zerodds_tt_t * a = (zerodds_tt_t *) ud;
  char * t = strdup(topic);
  char * y = strdup(type);
  if (!t || !y) { free(t); free(y); return; }
  zerodds_tt_add(a, t, y);
}
static void zerodds_service_tt_cb(void * ud, const char * topic, const char * type)
{
  zerodds_tt_t * a = (zerodds_tt_t *) ud;
  char * dt = zerodds_demangle_service(topic, NULL);
  if (!dt) { return; }
  char * dy = zerodds_demangle_service_type(type);
  if (!dy) { free(dt); return; }
  zerodds_tt_add(a, dt, dy);
}
// Moves the accumulated names and types into `nt` and frees the accumulator.
static rmw_ret_t zerodds_tt_emit(zerodds_tt_t * acc, rcutils_allocator_t * a, rmw_names_and_types_t * nt)
{
  rmw_ret_t ret = rmw_names_and_types_init(nt, acc->count, a);
  if (ret == RMW_RET_OK) {
    for (size_t i = 0; i < acc->count; ++i) {
      nt->names.data[i] = rcutils_strdup(acc->topics[i], *a);
      if (rcutils_string_array_init(&nt->types[i], acc->ntypes[i], a) == RCUTILS_RET_OK) {
        for (size_t j = 0; j < acc->ntypes[i]; ++j) {
          nt->types[i].data[j] = rcutils_strdup(acc->types[i][j], *a);
        }
      }
    }
  }
  for (size_t i = 0; i < acc->count; ++i) {
    for (size_t j = 0; j < acc->ntypes[i]; ++j) { free(acc->types[i][j]); }
    free(acc->types[i]);
    free(acc->topics[i]);
  }
  free(acc->topics); free(acc->types); free(acc->ntypes);
  return ret;
}
rmw_ret_t rmw_get_topic_names_and_types(const rmw_node_t * n, rcutils_allocator_t * a, bool d, rmw_names_and_types_t * nt)
{
  if (!n || !n->data || !a || !nt) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;
  zerodds_tt_t acc = {0};
  zerodds_topic_cb_t cb = d ? zerodds_tt_raw_cb : zerodds_tt_cb;
  rmw_zerodds_node_for_each_publication(nd->bridge_node, cb, &acc);
  rmw_zerodds_node_for_each_subscription(nd->bridge_node, cb, &acc);
  return zerodds_tt_emit(&acc, a, nt);
}
rmw_ret_t rmw_get_service_names_and_types(const rmw_node_t * n, rcutils_allocator_t * a, rmw_names_and_types_t * nt)
{
  if (!n || !n->data || !a || !nt) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;
  zerodds_tt_t acc = {0};
  rmw_zerodds_node_for_each_publication(nd->bridge_node, zerodds_service_tt_cb, &acc);
  rmw_zerodds_node_for_each_subscription(nd->bridge_node, zerodds_service_tt_cb, &acc);
  return zerodds_tt_emit(&acc, a, nt);
}
// ---------------------------------------------------------------------------
// Endpoint info by topic (REP-2009) — rmw_get_publishers/subscriptions_info_by_topic.
// Enumerates per-endpoint info (GUID + QoS) from the shim, filters by topic,
// resolves each endpoint's owning node from the ros_discovery_info graph, and
// fills rmw_topic_endpoint_info_array_t. Backs `ros2 topic info -v`.
// ---------------------------------------------------------------------------

// Mirror of the Rust #[repr(C)] zerodds::ZeroDdsEndpointInfo (zerodds.h).
typedef struct {
  const char * topic_name;
  const char * type_name;
  const uint8_t * endpoint_guid;  // 16 bytes
  uint8_t reliable;
  uint8_t transient_local;
  uint64_t deadline_ns;          // 0 = INFINITE
  uint64_t lifespan_ns;          // 0 = INFINITE
  uint64_t liveliness_lease_ns;  // 0 = INFINITE
  const uint8_t * user_data;     // USER_DATA as announced, NULL for none
  size_t user_data_len;
} zerodds_endpoint_info_t;
typedef void (*zerodds_endpoint_cb_t)(void * ud, const zerodds_endpoint_info_t * info);
extern int rmw_zerodds_node_for_each_publication_endpoint(void * node, zerodds_endpoint_cb_t cb, void * ud);
extern int rmw_zerodds_node_for_each_subscription_endpoint(void * node, zerodds_endpoint_cb_t cb, void * ud);
extern int rmw_zerodds_node_resolve_endpoint(
  void * node, const uint8_t * gid16,
  char * out_ns, size_t ns_cap, char * out_name, size_t name_cap);

// One collected endpoint: GUID + demangled type + flattened QoS.
typedef struct {
  uint8_t gid[16];
  char * type;             // demangled, owned (free on cleanup)
  uint8_t reliable;
  uint8_t transient_local;
  uint64_t deadline_ns;
  uint64_t lifespan_ns;
  uint64_t liveliness_ns;
  // The type hash the endpoint announced in USER_DATA, version 0 for none.
  uint8_t type_hash_version;
  uint8_t type_hash_value[32];
} zerodds_ep_row_t;

// The type hash an endpoint announced in USER_DATA as `typehash=RIHS01_…;`
// among its `;`-separated `key=value` pairs, the form rmw_dds_common's
// parse_type_hash_from_user_data reads. Left at version 0 for none, for a
// distribution without type hashes and for one that does not parse.
static void zerodds_row_type_hash(zerodds_ep_row_t * row, const zerodds_endpoint_info_t * e)
{
  row->type_hash_version = 0;
  memset(row->type_hash_value, 0, sizeof(row->type_hash_value));
#if ZERODDS_RMW_HAS_TYPE_HASH
  static const char key[] = "typehash=";
  const size_t kl = sizeof(key) - 1;
  const uint8_t * ud = e->user_data;
  size_t len = ud ? e->user_data_len : 0;
  size_t i = 0;
  while (i < len) {
    size_t end = i;
    while (end < len && ud[end] != ';') { end++; }
    if (end - i > kl && memcmp(ud + i, key, kl) == 0) {
      char text[96];
      size_t vl = end - i - kl;
      if (vl < sizeof(text)) {
        memcpy(text, ud + i + kl, vl);
        text[vl] = '\0';
        rosidl_type_hash_t h = rosidl_get_zero_initialized_type_hash();
        if (rosidl_parse_type_hash_string(text, &h) == RCUTILS_RET_OK &&
          sizeof(h.value) == sizeof(row->type_hash_value))
        {
          row->type_hash_version = h.version;
          memcpy(row->type_hash_value, h.value, sizeof(row->type_hash_value));
        }
      }
      return;
    }
    i = end + 1;
  }
#else
  (void) e;
#endif
}

#if ZERODDS_RMW_HAS_TYPE_HASH
static rosidl_type_hash_t zerodds_row_hash(const zerodds_ep_row_t * row)
{
  rosidl_type_hash_t h = rosidl_get_zero_initialized_type_hash();
  h.version = row->type_hash_version;
  memcpy(h.value, row->type_hash_value, sizeof(h.value));
  return h;
}
#endif

typedef struct {
  const char * target_topic;  // demangled ROS topic to match (e.g. "/chatter")
  zerodds_ep_row_t * rows;
  size_t count;
  size_t cap;
  int oom;                    // a row could not be stored: the call fails
} zerodds_ep_collect_t;

static void zerodds_ep_collect_cb(void * ud, const zerodds_endpoint_info_t * e)
{
  zerodds_ep_collect_t * c = (zerodds_ep_collect_t *) ud;
  char * dtopic = zerodds_demangle_topic(e->topic_name);
  if (!dtopic) { return; }                       // non-"rt/" (service/internal)
  if (strcmp(dtopic, c->target_topic) != 0) { free(dtopic); return; }
  free(dtopic);
  if (c->oom) { return; }
  if (c->count == c->cap) {
    size_t ncap = c->cap ? c->cap * 2 : 4;
    zerodds_ep_row_t * nr = (zerodds_ep_row_t *) realloc(c->rows, ncap * sizeof(*nr));
    if (!nr) { c->oom = 1; return; }
    c->rows = nr; c->cap = ncap;
  }
  zerodds_ep_row_t * row = &c->rows[c->count];
  memcpy(row->gid, e->endpoint_guid, 16);
  row->type = zerodds_demangle_type(e->type_name);
  row->reliable = e->reliable;
  row->transient_local = e->transient_local;
  row->deadline_ns = e->deadline_ns;
  row->lifespan_ns = e->lifespan_ns;
  row->liveliness_ns = e->liveliness_lease_ns;
  zerodds_row_type_hash(row, e);
  c->count++;
}

// The QoS an endpoint's discovery data carried, in rmw's terms.
static rmw_qos_profile_t zerodds_ep_qos(const zerodds_ep_row_t * row)
{
  rmw_qos_profile_t qos = rmw_qos_profile_default;
  qos.reliability = row->reliable ?
    RMW_QOS_POLICY_RELIABILITY_RELIABLE : RMW_QOS_POLICY_RELIABILITY_BEST_EFFORT;
  qos.durability = row->transient_local ?
    RMW_QOS_POLICY_DURABILITY_TRANSIENT_LOCAL : RMW_QOS_POLICY_DURABILITY_VOLATILE;
  // History/depth are not carried on the wire — leave them as "unknown".
  qos.history = RMW_QOS_POLICY_HISTORY_UNKNOWN;
  qos.depth = 0;
  // Nanoseconds as discovery carried them, INFINITE as rmw spells it.
  qos.deadline = zerodds_ns_time(row->deadline_ns, (rmw_time_t) RMW_DURATION_INFINITE);
  qos.lifespan = zerodds_ns_time(row->lifespan_ns, (rmw_time_t) RMW_DURATION_INFINITE);
  qos.liveliness_lease_duration =
    zerodds_ns_time(row->liveliness_ns, (rmw_time_t) RMW_DURATION_INFINITE);
  return qos;
}

// Shared body: `is_pub` selects the publication vs subscription enumerator +
// endpoint type.
static rmw_ret_t zerodds_get_endpoint_info_by_topic(
  const rmw_node_t * n, rcutils_allocator_t * a, const char * t, bool no_mangle,
  rmw_topic_endpoint_info_array_t * info, bool is_pub)
{
  if (!n || !n->data || !a || !t || !info) { return RMW_RET_INVALID_ARGUMENT; }
  (void) no_mangle;  // we always operate on the rt/-mangled wire form
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;

  zerodds_ep_collect_t cx = { t, NULL, 0, 0, 0 };
  if (is_pub) {
    rmw_zerodds_node_for_each_publication_endpoint(nd->bridge_node, zerodds_ep_collect_cb, &cx);
  } else {
    rmw_zerodds_node_for_each_subscription_endpoint(nd->bridge_node, zerodds_ep_collect_cb, &cx);
  }

  rmw_ret_t rc = RMW_RET_OK;
  if (cx.oom) {
    RMW_SET_ERROR_MSG("out of memory collecting the topic's endpoints");
    rc = RMW_RET_BAD_ALLOC;
  } else {
    rc = rmw_topic_endpoint_info_array_init_with_size(info, cx.count, a);
  }
  if (rc != RMW_RET_OK) {
    for (size_t i = 0; i < cx.count; ++i) { free(cx.rows[i].type); }
    free(cx.rows);
    return rc;
  }

  for (size_t i = 0; i < cx.count; ++i) {
    zerodds_ep_row_t * row = &cx.rows[i];
    rmw_topic_endpoint_info_t * ep = &info->info_array[i];

    // Resolve the owning node from the endpoint GUID (empty if unknown).
    char ns[256] = {0}, name[256] = {0};
    rmw_zerodds_node_resolve_endpoint(nd->bridge_node, row->gid, ns, sizeof(ns), name, sizeof(name));

    rmw_qos_profile_t qos = zerodds_ep_qos(row);

    // Chain the setters so each warn_unused_result is consumed; short-circuit on
    // the first failure (allocator OOM).
    rmw_ret_t sr = rmw_topic_endpoint_info_set_node_namespace(ep, ns, a);
    if (sr == RMW_RET_OK) { sr = rmw_topic_endpoint_info_set_node_name(ep, name, a); }
    if (sr == RMW_RET_OK) { sr = rmw_topic_endpoint_info_set_topic_type(ep, row->type ? row->type : "", a); }
    if (sr == RMW_RET_OK) {
      sr = rmw_topic_endpoint_info_set_endpoint_type(
        ep, is_pub ? RMW_ENDPOINT_PUBLISHER : RMW_ENDPOINT_SUBSCRIPTION);
    }
    if (sr == RMW_RET_OK) { sr = rmw_topic_endpoint_info_set_gid(ep, row->gid, 16); }
    if (sr == RMW_RET_OK) { sr = rmw_topic_endpoint_info_set_qos_profile(ep, &qos); }
#if ZERODDS_RMW_HAS_TYPE_HASH
    if (sr == RMW_RET_OK && row->type_hash_version != 0) {
      rosidl_type_hash_t h = zerodds_row_hash(row);
      sr = rmw_topic_endpoint_info_set_topic_type_hash(ep, &h);
    }
#endif

    if (sr != RMW_RET_OK) {
      // Free the remaining (incl. current) collected type strings + the array.
      for (size_t k = i; k < cx.count; ++k) { free(cx.rows[k].type); }
      rmw_ret_t fr = rmw_topic_endpoint_info_array_fini(info, a);
      (void) fr;
      free(cx.rows);
      return sr;
    }
    free(row->type);
  }
  free(cx.rows);
  return RMW_RET_OK;
}

rmw_ret_t rmw_get_publishers_info_by_topic(const rmw_node_t * n, rcutils_allocator_t * a,
  const char * t, bool d, rmw_topic_endpoint_info_array_t * info)
{ return zerodds_get_endpoint_info_by_topic(n, a, t, d, info, true); }
rmw_ret_t rmw_get_subscriptions_info_by_topic(const rmw_node_t * n, rcutils_allocator_t * a,
  const char * t, bool d, rmw_topic_endpoint_info_array_t * info)
{ return zerodds_get_endpoint_info_by_topic(n, a, t, d, info, false); }

#if ZERODDS_RMW_HAS_SERVICE_ENDPOINT_INFO
// ---------------------------------------------------------------------------
// Endpoint info by service (Rolling) — rmw_get_servers/clients_info_by_service,
// behind `ros2 service info --verbose`. A DDS service side is a reader and a
// writer on "rq/<service>Request" and "rr/<service>Reply": a server reads
// requests and writes replies, a client writes requests and reads replies.
// One entry per reader of the side, paired with the first writer of the same
// node, namespace and service type; endpoint_count 2, the reader's GID and
// QoS first. That is the pairing and the order of rmw_dds_common's
// `GraphCache::get_servers_info_by_service`, which rmw_fastrtps and
// rmw_cyclonedds call. The service type hash is the one the reader announced
// in USER_DATA (`typehash=…;`).
// ---------------------------------------------------------------------------
typedef struct {
  zerodds_ep_row_t ep;  // `type`: the demangled service type
  char ns[256];
  char name[256];
} zerodds_svc_row_t;

typedef struct {
  void * bridge_node;
  const char * service;  // "/add_two_ints"
  int want_request;      // 1: the request topic, 0: the reply topic
  zerodds_svc_row_t * rows;
  size_t count;
  size_t cap;
  int oom;               // a row could not be stored: the call fails
} zerodds_svc_collect_t;

static void zerodds_svc_collect_cb(void * ud, const zerodds_endpoint_info_t * e)
{
  zerodds_svc_collect_t * c = (zerodds_svc_collect_t *) ud;
  int is_request = 0;
  char * s = zerodds_demangle_service(e->topic_name, &is_request);
  if (!s) { return; }
  int match = is_request == c->want_request && strcmp(s, c->service) == 0;
  free(s);
  if (!match || c->oom) { return; }
  if (c->count == c->cap) {
    size_t ncap = c->cap ? c->cap * 2 : 4;
    zerodds_svc_row_t * nr = (zerodds_svc_row_t *) realloc(c->rows, ncap * sizeof(*nr));
    if (!nr) { c->oom = 1; return; }
    c->rows = nr; c->cap = ncap;
  }
  zerodds_svc_row_t * row = &c->rows[c->count];
  memset(row, 0, sizeof(*row));
  memcpy(row->ep.gid, e->endpoint_guid, 16);
  row->ep.type = zerodds_demangle_service_type(e->type_name);
  row->ep.reliable = e->reliable;
  row->ep.transient_local = e->transient_local;
  row->ep.deadline_ns = e->deadline_ns;
  row->ep.lifespan_ns = e->lifespan_ns;
  row->ep.liveliness_ns = e->liveliness_lease_ns;
  zerodds_row_type_hash(&row->ep, e);
  // Resolve the owning node from the endpoint GUID (empty if unknown).
  rmw_zerodds_node_resolve_endpoint(c->bridge_node, row->ep.gid, row->ns, sizeof(row->ns),
    row->name, sizeof(row->name));
  c->count++;
}

static void zerodds_svc_collect_free(zerodds_svc_collect_t * c)
{
  for (size_t i = 0; i < c->count; ++i) { free(c->rows[i].ep.type); }
  free(c->rows);
}

static rmw_ret_t zerodds_get_service_endpoint_info(
  const rmw_node_t * n, rcutils_allocator_t * a, const char * service, bool no_mangle,
  rmw_service_endpoint_info_array_t * info, bool is_server)
{
  if (!n || !service || !info) {
    RMW_SET_ERROR_MSG("node, service_name and the info array must not be null");
    return RMW_RET_INVALID_ARGUMENT;
  }
  if (n->implementation_identifier != ZERODDS_IDENTIFIER) {
    RMW_SET_ERROR_MSG("node implementation identifier does not match rmw_zerodds_cpp");
    return RMW_RET_INCORRECT_RMW_IMPLEMENTATION;
  }
  if (!a || !rcutils_allocator_is_valid(a)) {
    RMW_SET_ERROR_MSG("allocator argument is invalid");
    return RMW_RET_INVALID_ARGUMENT;
  }
  if (rmw_service_endpoint_info_array_check_zero(info) != RMW_RET_OK) {
    return RMW_RET_INVALID_ARGUMENT;  // the check set the message
  }
  if (no_mangle) {
    RMW_SET_ERROR_MSG(
      "'no_mangle' is not supported for services because they rely on internally mangled "
      "topic names");
    return RMW_RET_INVALID_ARGUMENT;
  }
  if (!n->data) {
    RMW_SET_ERROR_MSG("node has no implementation data");
    return RMW_RET_INVALID_ARGUMENT;
  }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;
  zerodds_svc_collect_t readers = { nd->bridge_node, service, is_server ? 1 : 0, NULL, 0, 0, 0 };
  zerodds_svc_collect_t writers = { nd->bridge_node, service, is_server ? 0 : 1, NULL, 0, 0, 0 };
  rmw_zerodds_node_for_each_subscription_endpoint(nd->bridge_node, zerodds_svc_collect_cb, &readers);
  rmw_zerodds_node_for_each_publication_endpoint(nd->bridge_node, zerodds_svc_collect_cb, &writers);

  rmw_ret_t rc = RMW_RET_OK;
  if (readers.oom || writers.oom) {
    RMW_SET_ERROR_MSG("out of memory collecting the service's endpoints");
    rc = RMW_RET_BAD_ALLOC;
  } else if (readers.count > 0) {
    rc = rmw_service_endpoint_info_array_init_with_size(info, readers.count, a);
    for (size_t i = 0; rc == RMW_RET_OK && i < readers.count; ++i) {
      const zerodds_svc_row_t * rd = &readers.rows[i];
      const zerodds_svc_row_t * wr = NULL;
      for (size_t k = 0; rd->ep.type && k < writers.count; ++k) {
        const zerodds_svc_row_t * c = &writers.rows[k];
        if (c->ep.type && strcmp(c->name, rd->name) == 0 && strcmp(c->ns, rd->ns) == 0 &&
          strcmp(c->ep.type, rd->ep.type) == 0)
        {
          wr = c;
          break;
        }
      }
      // Without a writer of the same node the second GID is zero and its QoS
      // zero-initialised, as rmw_dds_common leaves them.
      uint8_t gids[2][RMW_GID_STORAGE_SIZE];
      memset(gids, 0, sizeof(gids));
      memcpy(gids[0], rd->ep.gid, 16);
      rmw_qos_profile_t qos[2];
      memset(qos, 0, sizeof(qos));
      qos[0] = zerodds_ep_qos(&rd->ep);
      if (wr) {
        memcpy(gids[1], wr->ep.gid, 16);
        qos[1] = zerodds_ep_qos(&wr->ep);
      }
      rmw_service_endpoint_info_t * ep = &info->info_array[i];
      rc = rmw_service_endpoint_info_set_node_name(ep, rd->name, a);
      if (rc == RMW_RET_OK) { rc = rmw_service_endpoint_info_set_node_namespace(ep, rd->ns, a); }
      if (rc == RMW_RET_OK) {
        rc = rmw_service_endpoint_info_set_endpoint_type(
          ep, is_server ? RMW_ENDPOINT_SERVER : RMW_ENDPOINT_CLIENT);
      }
      if (rc == RMW_RET_OK) { rc = rmw_service_endpoint_info_set_endpoint_count(ep, 2); }
      if (rc == RMW_RET_OK) {
        rc = rmw_service_endpoint_info_set_service_type(ep, rd->ep.type ? rd->ep.type : "", a);
      }
      if (rc == RMW_RET_OK) {
        rc = rmw_service_endpoint_info_set_gids(ep, &gids[0][0], 2, RMW_GID_STORAGE_SIZE, a);
      }
      if (rc == RMW_RET_OK) { rc = rmw_service_endpoint_info_set_qos_profiles(ep, qos, 2, a); }
      // The service type hash both endpoints of a side announce; the
      // reader's, as rmw_dds_common takes it from the reader's entry.
      if (rc == RMW_RET_OK && rd->ep.type_hash_version != 0) {
        rosidl_type_hash_t h = zerodds_row_hash(&rd->ep);
        rc = rmw_service_endpoint_info_set_service_type_hash(ep, &h);
      }
    }
    if (rc != RMW_RET_OK && info->info_array) {
      rmw_ret_t fr = rmw_service_endpoint_info_array_fini(info, a);
      (void) fr;
    }
  }
  zerodds_svc_collect_free(&readers);
  zerodds_svc_collect_free(&writers);
  return rc;
}

rmw_ret_t rmw_get_servers_info_by_service(const rmw_node_t * n, rcutils_allocator_t * a,
  const char * service, bool no_mangle, rmw_service_endpoint_info_array_t * info)
{ return zerodds_get_service_endpoint_info(n, a, service, no_mangle, info, true); }
rmw_ret_t rmw_get_clients_info_by_service(const rmw_node_t * n, rcutils_allocator_t * a,
  const char * service, bool no_mangle, rmw_service_endpoint_info_array_t * info)
{ return zerodds_get_service_endpoint_info(n, a, service, no_mangle, info, false); }
#endif  // ZERODDS_RMW_HAS_SERVICE_ENDPOINT_INFO

// ---------------------------------------------------------------------------
// Names and types by node (`ros2 node info`): the endpoints whose owning node,
// resolved from the ros_discovery_info graph, is `node_name` in
// `node_namespace`. Topics are the rt/ topics; a node's services are its
// readers on request topics, its clients its writers on them — the endpoints
// every RMW announces for them.
// ---------------------------------------------------------------------------
typedef struct {
  void * bridge_node;
  const char * name;
  const char * ns;
  int services;       // 1: request topics as services, 0: rt/ topics
  bool no_demangle;
  zerodds_tt_t acc;
} zerodds_by_node_t;

static void zerodds_by_node_cb(void * ud, const zerodds_endpoint_info_t * e)
{
  zerodds_by_node_t * c = (zerodds_by_node_t *) ud;
  char ns[256] = {0}, name[256] = {0};
  if (rmw_zerodds_node_resolve_endpoint(c->bridge_node, e->endpoint_guid, ns, sizeof(ns), name,
      sizeof(name)) != 0) { return; }
  if (strcmp(ns, c->ns) != 0 || strcmp(name, c->name) != 0) { return; }
  char * t = NULL; char * y = NULL;
  if (c->services) {
    int is_request = 0;
    t = zerodds_demangle_service(e->topic_name, &is_request);
    if (!t || !is_request) { free(t); return; }
    y = zerodds_demangle_service_type(e->type_name);
  } else if (c->no_demangle) {
    t = strdup(e->topic_name);
    y = strdup(e->type_name);
  } else {
    t = zerodds_demangle_topic(e->topic_name);
    if (!t) { return; }
    y = zerodds_demangle_type(e->type_name);
  }
  if (!t || !y) { free(t); free(y); return; }
  zerodds_tt_add(&c->acc, t, y);
}

typedef struct { const char * name; const char * ns; int found; } zerodds_node_find_t;
static void zerodds_node_find_cb(void * ud, const char * name, const char * ns)
{
  zerodds_node_find_t * f = (zerodds_node_find_t *) ud;
  if (strcmp(name, f->name) == 0 && strcmp(ns, f->ns) == 0) { f->found = 1; }
}

// publications: 1 walks the node's writers, 0 its readers.
static rmw_ret_t zerodds_names_and_types_by_node(
  const rmw_node_t * n, rcutils_allocator_t * a, const char * node_name, const char * node_ns,
  bool no_demangle, int services, int publications, rmw_names_and_types_t * nt)
{
  if (!n || !n->data || !a || !node_name || !node_ns || !nt) { return RMW_RET_INVALID_ARGUMENT; }
  void * ctx = zerodds_node_bridge_ctx(n);
  if (!ctx) { return RMW_RET_ERROR; }
  zerodds_node_find_t f = { node_name, node_ns, 0 };
  rmw_zerodds_for_each_node(ctx, zerodds_node_find_cb, &f);
  if (!f.found) { return RMW_RET_NODE_NAME_NON_EXISTENT; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;
  zerodds_by_node_t c = { nd->bridge_node, node_name, node_ns, services, no_demangle, {0} };
  if (publications) {
    rmw_zerodds_node_for_each_publication_endpoint(nd->bridge_node, zerodds_by_node_cb, &c);
  } else {
    rmw_zerodds_node_for_each_subscription_endpoint(nd->bridge_node, zerodds_by_node_cb, &c);
  }
  return zerodds_tt_emit(&c.acc, a, nt);
}
rmw_ret_t rmw_get_publisher_names_and_types_by_node(const rmw_node_t * n, rcutils_allocator_t * a,
  const char * nn, const char * ns, bool d, rmw_names_and_types_t * nt)
{ return zerodds_names_and_types_by_node(n, a, nn, ns, d, 0, 1, nt); }
rmw_ret_t rmw_get_subscriber_names_and_types_by_node(const rmw_node_t * n, rcutils_allocator_t * a,
  const char * nn, const char * ns, bool d, rmw_names_and_types_t * nt)
{ return zerodds_names_and_types_by_node(n, a, nn, ns, d, 0, 0, nt); }
rmw_ret_t rmw_get_service_names_and_types_by_node(const rmw_node_t * n, rcutils_allocator_t * a,
  const char * nn, const char * ns, rmw_names_and_types_t * nt)
{ return zerodds_names_and_types_by_node(n, a, nn, ns, false, 1, 0, nt); }
rmw_ret_t rmw_get_client_names_and_types_by_node(const rmw_node_t * n, rcutils_allocator_t * a,
  const char * nn, const char * ns, rmw_names_and_types_t * nt)
{ return zerodds_names_and_types_by_node(n, a, nn, ns, false, 1, 1, nt); }

// rmw_qos_profile_check_compatible: rmw_zerodds_qos.cpp (rmw_dds_common's check).

// ---------------------------------------------------------------------------
// Typed-message loaning (`zerodds-delivery-modes-1.0`). A message is loanable
// iff it is a fixed POD (only primitives, fixed arrays of POD, nested fixed-POD
// structs — no strings, no sequences). The loaned buffer is the ROS message
// struct (the in-memory C layout). Two delivery modes, selected per the
// runtime setting `delivery.mode` (env `ZERODDS_DELIVERY_MODE`; participant
// default):
//
//  * `Portable` (default, mode 0): borrow → zeroed heap struct; the user fills
//    it; publish_loaned serializes it to CDR + publishes over RTPS, so
//    cross-host / cross-vendor / non-loaning subscribers all work. Interop-safe.
//
//  * `RawSameHost` (mode 1): borrow → a pointer into the writer's POSIX SHM
//    slot; the user writes the struct directly into shared memory; commit
//    finalizes it in place — no serialization, no wire (same-host only). The
//    reader maps the same segment (lazy attach) and takes a struct pointer
//    zero-copy. Because the raw mode never publishes over RTPS there is no
//    double delivery (the c-api `publishes_to_wire` gate). A type-layout or
//    host mismatch simply does not exchange data — never a garbage read.
//
//  * `Iceoryx` (mode 2, shim feature `delivery-iceoryx`): same loan/commit/take
//    surface, but the writer/reader are bound to an iceoryx2 service (derived
//    from the topic); commit sends over iceoryx2, the reader receives from it.
//    Same-host, cross-stack (iceoryx peers). When the shim is built without the
//    feature, `ZERODDS_DELIVERY_MODE=iceoryx` degrades to `Portable` on both
//    ends (the enable returns UNSUPPORTED for the same build).
// ---------------------------------------------------------------------------
static int zerodds_members_fixed_pod(const ms_t * m);
static int zerodds_member_fixed_pod(const mm_t * mem)
{
  if (mem->type_id_ == 16 || mem->type_id_ == 17) { return 0; }  // string / wstring
  // A variable-length array (sequence / bounded) disqualifies; only a fixed
  // array (array_size_ > 0 && !is_upper_bound_) keeps POD-ness.
  if (mem->is_array_ && (mem->array_size_ == 0 || mem->is_upper_bound_)) { return 0; }
  if (mem->type_id_ == 18) { return zerodds_members_fixed_pod((const ms_t *) mem->members_->data); }
  return 1;
}
static int zerodds_members_fixed_pod(const ms_t * m)
{
  for (uint32_t i = 0; i < m->member_count_; ++i) {
    if (!zerodds_member_fixed_pod(&m->members_[i])) { return 0; }
  }
  return 1;
}
static int zerodds_msg_can_loan(const void * members)
{
  const ms_t * m = (const ms_t *) members;
  return (m && m->member_count_ > 0) ? zerodds_members_fixed_pod(m) : 0;
}
static size_t zerodds_msg_struct_size(const void * members)
{
  // The introspection MessageMembers carries the exact in-memory C struct size.
  const ms_t * m = (const ms_t *) members;
  return m ? m->size_of_ : 0;
}

rmw_ret_t rmw_borrow_loaned_message(const rmw_publisher_t * p, const rosidl_message_type_support_t * ts, void ** msg)
{
  (void) ts;
  if (!p || !p->data || !msg) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_pub_data_t * pd = (zerodds_pub_data_t *) p->data;
  if (!pd->members || !zerodds_msg_can_loan(pd->members)) { return RMW_RET_UNSUPPORTED; }
  size_t sz = zerodds_msg_struct_size(pd->members);
  if (sz == 0) { return RMW_RET_ERROR; }
  if (pd->mode != 0) {
    // Raw modes (RawSameHost / Iceoryx): hand back a pointer into the writer's
    // loan slot — the caller writes the message struct directly into it. For
    // RawSameHost the slot lives in the writer's POSIX SHM segment (zero-copy);
    // for Iceoryx it is the iceoryx2-bound buffer. The commit publishes it
    // same-host with no serialization and no RTPS.
    unsigned char * slot = NULL;
    if (rmw_zerodds_publisher_loan(pd->bridge_pub, sz, &slot) != RMW_RET_OK || !slot) {
      return RMW_RET_ERROR;
    }
    memset(slot, 0, sz);  // clean fixed-POD struct (the slot ring may be reused)
    *msg = slot;
    return RMW_RET_OK;
  }
  // Portable: heap struct, serialized at publish. REP-2008: when a loan
  // alignment is requested, allocate on that boundary and round the size up to
  // a whole number of alignment units, so an accelerator runtime can pin
  // exactly this buffer's pages and nothing else.
  void * m = NULL;
  if (pd->loan_align > 0) {
    size_t rounded = ((sz + pd->loan_align - 1) / pd->loan_align) * pd->loan_align;
    m = aligned_alloc(pd->loan_align, rounded);
    if (m) { memset(m, 0, rounded); }
  } else {
    m = calloc(1, sz);
  }
  if (!m) { return RMW_RET_BAD_ALLOC; }
  *msg = m;
  return RMW_RET_OK;
}
rmw_ret_t rmw_return_loaned_message_from_publisher(const rmw_publisher_t * p, void * msg)
{
  if (!p || !p->data || !msg) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_pub_data_t * pd = (zerodds_pub_data_t *) p->data;
  if (pd->mode != 0) {  // cancel an un-published loan → discard the slot
    return rmw_zerodds_publisher_discard(pd->bridge_pub, (unsigned char *) msg, pd->struct_size);
  }
  free(msg);
  return RMW_RET_OK;
}
rmw_ret_t rmw_publish_loaned_message(const rmw_publisher_t * p, void * msg, rmw_publisher_allocation_t * a)
{
  if (!p || !p->data || !msg) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_pub_data_t * pd = (zerodds_pub_data_t *) p->data;
  if (pd->mode != 0) {
    // RawSameHost: the struct already lives in the slot — commit in place (no
    // serialization, no RTPS). The commit consumes the slot.
    return rmw_zerodds_publisher_commit(pd->bridge_pub, (unsigned char *) msg, pd->struct_size);
  }
  rmw_ret_t ret = rmw_publish(p, msg, a);  // serialize fixed-POD struct → CDR → publish
  free(msg);
  return ret;
}
rmw_ret_t rmw_take_loaned_message_with_info(const rmw_subscription_t * s, void ** msg, bool * taken,
  rmw_message_info_t * mi, rmw_subscription_allocation_t * a)
{
  if (taken) { *taken = false; }
  if (!s || !s->data || !msg) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) s->data;
  if (!sd->members || !zerodds_msg_can_loan(sd->members)) { return RMW_RET_UNSUPPORTED; }
  size_t sz = zerodds_msg_struct_size(sd->members);
  if (sz == 0) { return RMW_RET_ERROR; }
  if (sd->mode != 0) {
    // Raw modes: zero-copy take — a read-only pointer into the raw source, no
    // deserialize. Consume the prefetched sample (readiness already took it) and
    // track its slot so the matching return releases it.
    if (!zerodds_sub_prefetch(sd)) { return RMW_RET_OK; }  // no sample → taken stays false
    const unsigned char * ptr = sd->pending_ptr;
    unsigned int slot = sd->pending_slot;
    sd->has_pending = 0;
    for (int k = 0; k < ZERODDS_MAX_SHM_LOANS; ++k) {
      if (sd->loans[k].ptr == NULL) { sd->loans[k].ptr = ptr; sd->loans[k].slot = slot; break; }
    }
    zerodds_fill_message_info(sd, mi);
    *msg = (void *) ptr;
    if (taken) { *taken = true; }
    return RMW_RET_OK;
  }
  void * m = calloc(1, sz);
  if (!m) { return RMW_RET_BAD_ALLOC; }
  bool t = false;
  rmw_ret_t ret = rmw_take_with_info(s, m, &t, mi, a);  // CDR → struct
  if (ret != RMW_RET_OK || !t) { free(m); return ret; }
  *msg = m;
  if (taken) { *taken = true; }
  return RMW_RET_OK;
}
rmw_ret_t rmw_take_loaned_message(const rmw_subscription_t * s, void ** msg, bool * taken, rmw_subscription_allocation_t * a)
{ return rmw_take_loaned_message_with_info(s, msg, taken, NULL, a); }
rmw_ret_t rmw_return_loaned_message_from_subscription(const rmw_subscription_t * s, void * msg)
{
  if (!s || !s->data || !msg) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) s->data;
  if (sd->mode != 0) {  // release the SHM slot back to the writer's ring
    for (int k = 0; k < ZERODDS_MAX_SHM_LOANS; ++k) {
      if (sd->loans[k].ptr == (const unsigned char *) msg) {
        unsigned int slot = sd->loans[k].slot;
        sd->loans[k].ptr = NULL;
        return rmw_zerodds_subscription_release_shm(sd->bridge_sub, slot);
      }
    }
    return RMW_RET_OK;  // unknown pointer — nothing to release
  }
  free(msg);
  return RMW_RET_OK;
}

rmw_ret_t rmw_publish_serialized_message(const rmw_publisher_t * p, const rmw_serialized_message_t * m, rmw_publisher_allocation_t * a)
{
  (void) a;
  if (!p || !p->data || !m || !m->buffer) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_pub_data_t * pd = (zerodds_pub_data_t *) p->data;
  int rc = rmw_zerodds_publish(pd->bridge_pub, m->buffer, m->buffer_length);
  return rc == 0 ? RMW_RET_OK : RMW_RET_ERROR;
}
rmw_ret_t rmw_take_serialized_message_with_info(const rmw_subscription_t * s, rmw_serialized_message_t * m,
  bool * taken, rmw_message_info_t * mi, rmw_subscription_allocation_t * a)
{
  (void) a;
  if (taken) { *taken = false; }
  if (!s || !s->data || !m) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) s->data;
  unsigned char * buf = NULL; size_t len = 0;
  zerodds_origin_t origin;
  for (;;) {
    unsigned char be = 0;
    if (rmw_zerodds_take_with_origin(sd->bridge_sub, &buf, &len, &be, &origin) != 0 || !buf || len == 0) {
      if (buf) { rmw_zerodds_buffer_free(buf, len); }
      return RMW_RET_OK;
    }
    int pass = zerodds_sub_accepts_cdr(sd, buf, len, be);
    if (pass < 0) { rmw_zerodds_buffer_free(buf, len); return RMW_RET_BAD_ALLOC; }
    if (pass) { break; }
    rmw_zerodds_buffer_free(buf, len);
    buf = NULL;
  }
  if (rcutils_uint8_array_resize(m, len) != RCUTILS_RET_OK) {
    rmw_zerodds_buffer_free(buf, len);
    return RMW_RET_BAD_ALLOC;
  }
  memcpy(m->buffer, buf, len);
  m->buffer_length = len;
  rmw_zerodds_buffer_free(buf, len);
  zerodds_fill_message_info_from(sd, mi, &origin);
  if (taken) { *taken = true; }
  return RMW_RET_OK;
}
rmw_ret_t rmw_take_serialized_message(const rmw_subscription_t * s, rmw_serialized_message_t * m, bool * taken, rmw_subscription_allocation_t * a)
{ return rmw_take_serialized_message_with_info(s, m, taken, NULL, a); }
rmw_ret_t rmw_take_sequence(const rmw_subscription_t * s, size_t count, rmw_message_sequence_t * seq,
  rmw_message_info_sequence_t * iseq, size_t * taken, rmw_subscription_allocation_t * a)
{
  if (!s || !seq || !iseq || !taken) { return RMW_RET_INVALID_ARGUMENT; }
  if (s->implementation_identifier != ZERODDS_IDENTIFIER) { return RMW_RET_INCORRECT_RMW_IMPLEMENTATION; }
  if (count == 0) { RMW_SET_ERROR_MSG("count cannot be 0"); return RMW_RET_INVALID_ARGUMENT; }
  if (count > seq->capacity || count > iseq->capacity) {
    RMW_SET_ERROR_MSG("count exceeds the capacity of the sequences");
    return RMW_RET_INVALID_ARGUMENT;
  }
  // Up to `count` messages through the single take, which leaves nothing
  // behind: a message it reports as not taken was not there.
  *taken = 0;
  for (size_t i = 0; i < count; ++i) {
    bool one = false;
    rmw_ret_t ret = rmw_take_with_info(s, seq->data[i], &one, &iseq->data[i], a);
    if (ret != RMW_RET_OK) { seq->size = *taken; iseq->size = *taken; return ret; }
    if (!one) { break; }
    ++*taken;
  }
  seq->size = *taken;
  iseq->size = *taken;
  return RMW_RET_OK;
}

rmw_ret_t rmw_get_gid_for_client(const rmw_client_t * c, rmw_gid_t * gid)
{
  if (!c || !c->data || !gid) { return RMW_RET_INVALID_ARGUMENT; }
  if (c->implementation_identifier != ZERODDS_IDENTIFIER) { return RMW_RET_INCORRECT_RMW_IMPLEMENTATION; }
  memset(gid, 0, sizeof(*gid));
  gid->implementation_identifier = ZERODDS_IDENTIFIER;
  memcpy(gid->data, ((zerodds_client_data_t *) c->data)->gid, 16);
  return RMW_RET_OK;
}
// Network flow endpoints (REP-2005 style): the unicast locators user traffic
// of the endpoint's participant arrives on — the ones announced over SEDP.
typedef struct { int32_t kind; uint32_t port; uint8_t address[16]; } zerodds_locator_t;
extern size_t rmw_zerodds_publisher_unicast_locators(void * pub, zerodds_locator_t * out, size_t cap);
extern size_t rmw_zerodds_subscription_unicast_locators(void * sub, zerodds_locator_t * out, size_t cap);

static rmw_ret_t zerodds_flow_endpoints(const zerodds_locator_t * loc, size_t n,
  rcutils_allocator_t * a, rmw_network_flow_endpoint_array_t * arr)
{
  size_t ip = 0;
  for (size_t i = 0; i < n; ++i) {
    if (loc[i].kind == 1 || loc[i].kind == 2 || loc[i].kind == 4 || loc[i].kind == 8) { ip++; }
  }
  rmw_ret_t ret = rmw_network_flow_endpoint_array_init(arr, ip, a);
  if (ret != RMW_RET_OK) { return ret; }
  size_t k = 0;
  for (size_t i = 0; i < n; ++i) {
    int32_t kind = loc[i].kind;
    if (!(kind == 1 || kind == 2 || kind == 4 || kind == 8)) { continue; }
    rmw_network_flow_endpoint_t * ep = &arr->network_flow_endpoint[k++];
    ep->transport_protocol = (kind == 1 || kind == 2) ? RMW_TRANSPORT_PROTOCOL_UDP : RMW_TRANSPORT_PROTOCOL_TCP;
    ep->internet_protocol = (kind == 1 || kind == 4) ? RMW_INTERNET_PROTOCOL_IPV4 : RMW_INTERNET_PROTOCOL_IPV6;
    ep->transport_port = (uint16_t) loc[i].port;
    char text[RMW_INET_ADDRSTRLEN];
    const uint8_t * b = loc[i].address;
    if (kind == 1 || kind == 4) {
      snprintf(text, sizeof(text), "%u.%u.%u.%u", b[12], b[13], b[14], b[15]);
    } else {
      snprintf(text, sizeof(text), "%x:%x:%x:%x:%x:%x:%x:%x",
        (b[0] << 8) | b[1], (b[2] << 8) | b[3], (b[4] << 8) | b[5], (b[6] << 8) | b[7],
        (b[8] << 8) | b[9], (b[10] << 8) | b[11], (b[12] << 8) | b[13], (b[14] << 8) | b[15]);
    }
    ret = rmw_network_flow_endpoint_set_internet_address(ep, text, strlen(text));
    if (ret != RMW_RET_OK) { (void) rmw_network_flow_endpoint_array_fini(arr); return ret; }
  }
  return RMW_RET_OK;
}

rmw_ret_t rmw_publisher_get_network_flow_endpoints(const rmw_publisher_t * p, rcutils_allocator_t * a, rmw_network_flow_endpoint_array_t * arr)
{
  if (!p || !p->data || !a || !arr) { return RMW_RET_INVALID_ARGUMENT; }
  if (p->implementation_identifier != ZERODDS_IDENTIFIER) { return RMW_RET_INCORRECT_RMW_IMPLEMENTATION; }
  void * bp = ((zerodds_pub_data_t *) p->data)->bridge_pub;
  size_t n = rmw_zerodds_publisher_unicast_locators(bp, NULL, 0);
  zerodds_locator_t * loc = n ? (zerodds_locator_t *) calloc(n, sizeof(*loc)) : NULL;
  if (n && !loc) { return RMW_RET_BAD_ALLOC; }
  n = loc ? rmw_zerodds_publisher_unicast_locators(bp, loc, n) : 0;
  rmw_ret_t ret = zerodds_flow_endpoints(loc, n, a, arr);
  free(loc);
  return ret;
}
rmw_ret_t rmw_subscription_get_network_flow_endpoints(const rmw_subscription_t * s, rcutils_allocator_t * a, rmw_network_flow_endpoint_array_t * arr)
{
  if (!s || !s->data || !a || !arr) { return RMW_RET_INVALID_ARGUMENT; }
  if (s->implementation_identifier != ZERODDS_IDENTIFIER) { return RMW_RET_INCORRECT_RMW_IMPLEMENTATION; }
  void * bs = ((zerodds_sub_data_t *) s->data)->bridge_sub;
  size_t n = rmw_zerodds_subscription_unicast_locators(bs, NULL, 0);
  zerodds_locator_t * loc = n ? (zerodds_locator_t *) calloc(n, sizeof(*loc)) : NULL;
  if (n && !loc) { return RMW_RET_BAD_ALLOC; }
  n = loc ? rmw_zerodds_subscription_unicast_locators(bs, loc, n) : 0;
  rmw_ret_t ret = zerodds_flow_endpoints(loc, n, a, arr);
  free(loc);
  return ret;
}

// ===========================================================================
// REP-2007 — Type Adaptation, the rmw-layer half
// ===========================================================================
//
// REP-2007 puts the TypeAdapter template in rclcpp, and says why: "placing this
// feature in rclcpp allows us to avoid type erasure [...] and to use ownership
// mechanics". It then names what it left open for the middleware:
//
//   "Perhaps we can support a form of this feature in other languages in rcl or
//    rmw in the future. One challenge in doing this is that it may require
//    custom type support, which may be middleware specific."
//
// and what the rclcpp-only placement costs:
//
//   "This feature will support both convert and (de)serialize functions [...]
//    the (de)serialization functions are better for inter-process
//    communication." / "An advantage of following the ROS 1 approach is that an
//    extra copy can be avoided; although it is likely much more challenging to
//    implement this feature the ROS 1 way because of the middleware."
//
// The functions below are that middleware-specific custom type support. An
// adapted publish serializes the custom type straight into the wire buffer:
// the ros_message_type is never materialized, so the copy the REP calls
// avoidable is not made. An adapted take deserializes the wire body straight
// into the custom type, likewise without an intermediate ROS message.
//
// The endpoint itself is unchanged: it was created from the ros_message_type's
// rosidl type support, so the DDS topic name, the ROS type name and the RIHS
// type hash are exactly those of the unadapted endpoint. An adapted publisher
// and a plain subscription therefore match and exchange identical bytes.

rmw_ret_t rmw_zerodds_publisher_set_type_adapter(
  rmw_publisher_t * publisher, const rmw_zerodds_type_adapter_t * adapter)
{
  if (!publisher || !publisher->data) { return RMW_RET_INVALID_ARGUMENT; }
  if (adapter && (!adapter->serialized_size || !adapter->serialize)) {
    return RMW_RET_INVALID_ARGUMENT;  // publish side needs both halves
  }
  ((zerodds_pub_data_t *) publisher->data)->adapter = adapter;
  return RMW_RET_OK;
}

rmw_ret_t rmw_zerodds_subscription_set_type_adapter(
  rmw_subscription_t * subscription, const rmw_zerodds_type_adapter_t * adapter)
{
  if (!subscription || !subscription->data) { return RMW_RET_INVALID_ARGUMENT; }
  if (adapter && !adapter->deserialize) { return RMW_RET_INVALID_ARGUMENT; }
  ((zerodds_sub_data_t *) subscription->data)->adapter = adapter;
  return RMW_RET_OK;
}

// Builds the full wire buffer (4-byte encapsulation header + adapter body) for
// `custom`. On success `*out` is a malloc'd buffer of `*out_len` bytes.
static rmw_ret_t zerodds_adapted_wire(
  const rmw_zerodds_type_adapter_t * ad, const void * custom,
  unsigned char ** out, size_t * out_len)
{
  size_t body = ad->serialized_size(custom);
  unsigned char * wire = (unsigned char *) malloc(4 + body);
  if (!wire) { return RMW_RET_BAD_ALLOC; }
  // CDR_LE encapsulation, identical to the introspection publish path.
  wire[0] = 0x00; wire[1] = 0x01; wire[2] = 0x00; wire[3] = 0x00;
  size_t written = 0;
  if (ad->serialize(custom, wire + 4, body, &written) != 0 || written > body) {
    free(wire);
    return RMW_RET_ERROR;
  }
  *out = wire;
  *out_len = 4 + written;
  return RMW_RET_OK;
}

rmw_ret_t rmw_zerodds_publish_adapted(const rmw_publisher_t * publisher, const void * custom)
{
  if (!publisher || !publisher->data || !custom) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_pub_data_t * pd = (zerodds_pub_data_t *) publisher->data;
  if (!pd->adapter) { return RMW_RET_INVALID_ARGUMENT; }  // no adapter bound
  unsigned char * wire = NULL; size_t len = 0;
  rmw_ret_t r = zerodds_adapted_wire(pd->adapter, custom, &wire, &len);
  if (r != RMW_RET_OK) { return r; }
  int rc = rmw_zerodds_publish(pd->bridge_pub, wire, len);
  free(wire);
  return rc == 0 ? RMW_RET_OK : RMW_RET_ERROR;
}

rmw_ret_t rmw_zerodds_take_adapted(
  const rmw_subscription_t * subscription, void * custom, bool * taken,
  rmw_message_info_t * message_info)
{
  if (taken) { *taken = false; }
  if (!subscription || !subscription->data || !custom) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) subscription->data;
  if (!sd->adapter) { return RMW_RET_INVALID_ARGUMENT; }
  if (sd->mode != 0) {
    // Raw same-host delivery carries the message struct, not CDR — there is no
    // body for an adapter to read. Adaptation runs on the Portable (wire) path.
    return RMW_RET_UNSUPPORTED;
  }
  unsigned char * buf = NULL; size_t len = 0; unsigned char be = 0;
  zerodds_origin_t origin;
  for (;;) {
    if (rmw_zerodds_take_with_origin(sd->bridge_sub, &buf, &len, &be, &origin) != 0 || !buf || len < 4) {
      if (buf) { rmw_zerodds_buffer_free(buf, len); }
      return RMW_RET_OK;  // nothing available — taken stays false
    }
    // The content filter judges the ROS type's fields, not the adapted one's.
    int pass = zerodds_sub_accepts_cdr(sd, buf, len, be);
    if (pass < 0) { rmw_zerodds_buffer_free(buf, len); return RMW_RET_BAD_ALLOC; }
    if (pass) { break; }
    rmw_zerodds_buffer_free(buf, len);
    buf = NULL;
  }
  int dr = sd->adapter->deserialize(buf + 4, len - 4, be ? true : false, custom);
  rmw_zerodds_buffer_free(buf, len);
  if (dr != 0) { return RMW_RET_ERROR; }
  zerodds_fill_message_info_from(sd, message_info, &origin);
  if (taken) { *taken = true; }
  return RMW_RET_OK;
}

// --- REP-2007 "Type Adaptation in Services" --------------------------------
// The request and the response half are adapted independently, exactly as the
// REP's MyServiceTypeAdapter allows ("either the request or the response can be
// adapted"). Actions are composed by rcl from services (goal, cancel, result)
// plus topics (feedback, status), so the two surfaces below plus the topic
// surface above cover REP-2007 "Type Adaptation in Actions" as well.

typedef struct {
  const rmw_zerodds_type_adapter_t * req;
  const rmw_zerodds_type_adapter_t * resp;
} zerodds_svc_adapters_t;

// Adapter pairs are keyed by the client/service handle. A ROS process holds a
// handful of adapted services at most, so a small linear table is enough and
// keeps the client/service structs (which rmw_wait walks) untouched.
#define ZERODDS_MAX_SVC_ADAPTERS 64
static struct { const void * handle; zerodds_svc_adapters_t ad; }
  zerodds_svc_adapter_table[ZERODDS_MAX_SVC_ADAPTERS];

static zerodds_svc_adapters_t * zerodds_svc_adapters(const void * handle, int create)
{
  for (int i = 0; i < ZERODDS_MAX_SVC_ADAPTERS; ++i) {
    if (zerodds_svc_adapter_table[i].handle == handle) { return &zerodds_svc_adapter_table[i].ad; }
  }
  if (!create) { return NULL; }
  for (int i = 0; i < ZERODDS_MAX_SVC_ADAPTERS; ++i) {
    if (zerodds_svc_adapter_table[i].handle == NULL) {
      zerodds_svc_adapter_table[i].handle = handle;
      return &zerodds_svc_adapter_table[i].ad;
    }
  }
  return NULL;  // table full — the caller reports RMW_RET_ERROR, never silently drops
}

static void zerodds_svc_adapters_release(const void * handle)
{
  if (!handle) { return; }
  for (int i = 0; i < ZERODDS_MAX_SVC_ADAPTERS; ++i) {
    if (zerodds_svc_adapter_table[i].handle == handle) {
      zerodds_svc_adapter_table[i].handle = NULL;
      zerodds_svc_adapter_table[i].ad.req = NULL;
      zerodds_svc_adapter_table[i].ad.resp = NULL;
      return;
    }
  }
}

static rmw_ret_t zerodds_set_svc_adapters(
  const void * handle, const rmw_zerodds_type_adapter_t * req,
  const rmw_zerodds_type_adapter_t * resp)
{
  if (!handle) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_svc_adapters_t * a = zerodds_svc_adapters(handle, 1);
  if (!a) { return RMW_RET_ERROR; }
  a->req = req;
  a->resp = resp;
  return RMW_RET_OK;
}

rmw_ret_t rmw_zerodds_client_set_type_adapters(
  rmw_client_t * client, const rmw_zerodds_type_adapter_t * request,
  const rmw_zerodds_type_adapter_t * response)
{
  if (!client || !client->data) { return RMW_RET_INVALID_ARGUMENT; }
  return zerodds_set_svc_adapters(client->data, request, response);
}

rmw_ret_t rmw_zerodds_service_set_type_adapters(
  rmw_service_t * service, const rmw_zerodds_type_adapter_t * request,
  const rmw_zerodds_type_adapter_t * response)
{
  if (!service || !service->data) { return RMW_RET_INVALID_ARGUMENT; }
  return zerodds_set_svc_adapters(service->data, request, response);
}

rmw_ret_t rmw_zerodds_send_request_adapted(
  const rmw_client_t * client, const void * custom_request, int64_t * sequence_id)
{
  if (!client || !client->data || !custom_request || !sequence_id) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_client_data_t * cd = (zerodds_client_data_t *) client->data;
  zerodds_svc_adapters_t * a = zerodds_svc_adapters(client->data, 0);
  if (!a || !a->req) { return RMW_RET_INVALID_ARGUMENT; }
  unsigned char * wire = NULL; size_t total = 0;
  rmw_ret_t r = zerodds_adapted_wire(a->req, custom_request, &wire, &total);
  if (r != RMW_RET_OK) { return r; }
  int rc = rmw_zerodds_send_request(cd->bridge_client, wire, total, sequence_id);
  free(wire);
  return rc == 0 ? RMW_RET_OK : RMW_RET_ERROR;
}

rmw_ret_t rmw_zerodds_take_response_adapted(
  const rmw_client_t * client, rmw_service_info_t * header, void * custom_response,
  bool * taken)
{
  if (taken) { *taken = false; }
  if (!client || !client->data || !custom_response) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_client_data_t * cd = (zerodds_client_data_t *) client->data;
  zerodds_svc_adapters_t * a = zerodds_svc_adapters(client->data, 0);
  if (!a || !a->resp) { return RMW_RET_INVALID_ARGUMENT; }
  unsigned char * buf = NULL; size_t len = 0; unsigned char be = 0; int64_t s = 0;
  if (rmw_zerodds_take_response(cd->bridge_client, &buf, &len, &be, &s) != 0 || !buf) {
    return RMW_RET_OK;
  }
  if (len < 4) { rmw_zerodds_buffer_free(buf, len); return RMW_RET_ERROR; }
  int dr = a->resp->deserialize(buf + 4, len - 4, be ? true : false, custom_response);
  rmw_zerodds_buffer_free(buf, len);
  if (dr != 0) { return RMW_RET_ERROR; }
  if (header) {
    memcpy(header->request_id.writer_guid, cd->gid, 16);
    header->request_id.sequence_number = s;
    header->source_timestamp = 0;
    header->received_timestamp = 0;
  }
  if (taken) { *taken = true; }
  return RMW_RET_OK;
}

rmw_ret_t rmw_zerodds_take_request_adapted(
  const rmw_service_t * service, rmw_service_info_t * header, void * custom_request,
  bool * taken)
{
  if (taken) { *taken = false; }
  if (!service || !service->data || !custom_request) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_service_data_t * sd = (zerodds_service_data_t *) service->data;
  zerodds_svc_adapters_t * a = zerodds_svc_adapters(service->data, 0);
  if (!a || !a->req) { return RMW_RET_INVALID_ARGUMENT; }
  unsigned char * buf = NULL; size_t len = 0; unsigned char be = 0;
  unsigned char guid[16]; int64_t s_seq = 0;
  if (rmw_zerodds_take_request(sd->bridge_service, &buf, &len, &be, guid, &s_seq) != 0 || !buf) {
    return RMW_RET_OK;
  }
  if (len < 4) { rmw_zerodds_buffer_free(buf, len); return RMW_RET_ERROR; }
  int dr = a->req->deserialize(buf + 4, len - 4, be ? true : false, custom_request);
  rmw_zerodds_buffer_free(buf, len);
  if (dr != 0) { return RMW_RET_ERROR; }
  if (header) {
    memcpy(header->request_id.writer_guid, guid, 16);
    header->request_id.sequence_number = s_seq;
    header->source_timestamp = 0;
    header->received_timestamp = 0;
  }
  if (taken) { *taken = true; }
  return RMW_RET_OK;
}

rmw_ret_t rmw_zerodds_send_response_adapted(
  const rmw_service_t * service, rmw_request_id_t * header, const void * custom_response)
{
  if (!service || !service->data || !header || !custom_response) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_service_data_t * sd = (zerodds_service_data_t *) service->data;
  zerodds_svc_adapters_t * a = zerodds_svc_adapters(service->data, 0);
  if (!a || !a->resp) { return RMW_RET_INVALID_ARGUMENT; }
  unsigned char * wire = NULL; size_t total = 0;
  rmw_ret_t r = zerodds_adapted_wire(a->resp, custom_response, &wire, &total);
  if (r != RMW_RET_OK) { return r; }
  int rc = rmw_zerodds_send_response(sd->bridge_service, (const unsigned char *) header->writer_guid,
                                     header->sequence_number, wire, total);
  free(wire);
  return (rmw_ret_t) rc;
}

// ===========================================================================
// REP-2008 — Hardware Acceleration, the rmw-layer half
// ===========================================================================
//
// REP-2008 §"Kernel levels in ROS 2" places the middleware at level 3 ("ROS 2
// underlayers [...] including the communication middleware (e.g. DDS)") and rmw
// itself at level 2 ("rclcpp, rcl, rmw, and the corresponding rmw_adapters").
// §"Methodology" step 3.2 asks to "accelerate inter-Node exchanges and reduce
// the overhead of the ROS 2 message-passing system across all its abstraction
// layers. To do so, consider leveraging REP-2007 and REP-2009."
//
// The concrete thing a DDS implementation owes an accelerator is memory
// provenance. CUDA, ROCm and XRT can only DMA out of host memory they have
// pinned or registered, and they pin whole pages. Without a descriptor the
// application must allocate its own pinned buffer and copy into the middleware
// on every publish — the staging copy that step 3.2 asks to remove. With it,
// the application pins the loaned buffer once and the accelerator writes
// directly into the memory DDS sends.

uint32_t rmw_zerodds_acceleration_kernel_levels(void)
{
  // Level 3 unconditionally: this rmw implementation *is* the ROS 2 underlayer.
  // Level 2 because the loan surface below hands rmw-level code a registerable
  // buffer. Level 1 is the application's, not ours.
  return RMW_ZERODDS_ACCEL_LEVEL_2 | RMW_ZERODDS_ACCEL_LEVEL_3;
}

// Fills the page-aligned registration extent covering [ptr, ptr + size).
static void zerodds_fill_registration(rmw_zerodds_loan_memory_t * out, void * ptr, size_t size)
{
  size_t ps = zerodds_page_size();
  uintptr_t p = (uintptr_t) ptr;
  uintptr_t base = p & ~(uintptr_t) (ps - 1);
  uintptr_t end = p + size;
  uintptr_t end_aligned = (end + ps - 1) & ~(uintptr_t) (ps - 1);
  out->host_ptr = ptr;
  out->size = size;
  out->registration_base = (void *) base;
  out->registration_size = (size_t) (end_aligned - base);
  out->page_size = ps;
}

rmw_ret_t rmw_zerodds_publisher_get_loan_memory(
  const rmw_publisher_t * publisher, const void * loaned_message,
  rmw_zerodds_loan_memory_t * out)
{
  if (!publisher || !publisher->data || !loaned_message || !out) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_pub_data_t * pd = (zerodds_pub_data_t *) publisher->data;
  if (pd->struct_size == 0) { return RMW_RET_UNSUPPORTED; }
  memset(out, 0, sizeof(*out));
  zerodds_fill_registration(out, (void *) (uintptr_t) loaned_message, pd->struct_size);
  if (pd->mode == 1) {
    out->kind = RMW_ZERODDS_LOAN_MEMORY_SHARED_SEGMENT;
    out->cross_process = true;
    snprintf(out->segment_link, sizeof(out->segment_link), "%s", pd->shm_path);
  } else if (pd->mode == 2) {
    out->kind = RMW_ZERODDS_LOAN_MEMORY_ICEORYX;
    out->cross_process = true;
    snprintf(out->segment_link, sizeof(out->segment_link), "%s", pd->shm_path);
  } else {
    out->kind = RMW_ZERODDS_LOAN_MEMORY_PROCESS_HEAP;
    out->cross_process = false;
    out->segment_link[0] = '\0';
  }
  return RMW_RET_OK;
}

rmw_ret_t rmw_zerodds_subscription_get_loan_memory(
  const rmw_subscription_t * subscription, const void * loaned_message,
  rmw_zerodds_loan_memory_t * out)
{
  if (!subscription || !subscription->data || !loaned_message || !out) {
    return RMW_RET_INVALID_ARGUMENT;
  }
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) subscription->data;
  if (sd->struct_size == 0) { return RMW_RET_UNSUPPORTED; }
  memset(out, 0, sizeof(*out));
  zerodds_fill_registration(out, (void *) (uintptr_t) loaned_message, sd->struct_size);
  if (sd->mode == 1) {
    out->kind = RMW_ZERODDS_LOAN_MEMORY_SHARED_SEGMENT;
    out->cross_process = true;
    snprintf(out->segment_link, sizeof(out->segment_link), "%s", sd->shm_path);
  } else if (sd->mode == 2) {
    out->kind = RMW_ZERODDS_LOAN_MEMORY_ICEORYX;
    out->cross_process = true;
    snprintf(out->segment_link, sizeof(out->segment_link), "%s", sd->shm_path);
  } else {
    out->kind = RMW_ZERODDS_LOAN_MEMORY_PROCESS_HEAP;
    out->cross_process = false;
    out->segment_link[0] = '\0';
  }
  return RMW_RET_OK;
}

rmw_ret_t rmw_zerodds_publisher_set_loan_alignment(rmw_publisher_t * publisher, size_t alignment)
{
  if (!publisher || !publisher->data) { return RMW_RET_INVALID_ARGUMENT; }
  // aligned_alloc is undefined for a non-power-of-two or sub-pointer alignment:
  // reject loudly instead of quietly handing back a differently aligned buffer.
  if (alignment != 0 && (alignment < sizeof(void *) || (alignment & (alignment - 1)) != 0)) {
    return RMW_RET_INVALID_ARGUMENT;
  }
  ((zerodds_pub_data_t *) publisher->data)->loan_align = alignment;
  return RMW_RET_OK;
}

// ROS 2 Jazzy's dynamic message API (rmw.h, rosidl_dynamic_typesupport):
// `rmw_serialization_support_init`, `rmw_take_dynamic_message`,
// `rmw_take_dynamic_message_with_info`, and the
// `rosidl_dynamic_typesupport_serialization_support_interface_t` backing
// them, are implemented in rmw_zerodds_dynamic.cpp — a
// `rosidl_dynamic_typesupport_zerodds`, the way
// `rosidl_dynamic_typesupport_fastrtps` is one for Fast DDS, over
// ZeroDDS' own reflective DynamicType/DynamicData
// (crates/rmw-zerodds-shim/src/dynamic.rs). That file carries the same
// `__has_include(<rosidl_dynamic_typesupport/types.h>)` guard this
// comment used to sit under; Humble has neither the header nor the three
// rmw.h declarations, and it compiles to nothing there.

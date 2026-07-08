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
// STATUS: incremental bring-up. The load + identifier + init/options/context
// path is real; node/pub/sub/service/wait are being wired against the bridge.
// Functions not yet wired return RMW_RET_UNSUPPORTED rather than crash, so the
// loader and the implemented path stay well-defined.

#include <stdlib.h>
#include <string.h>

#include <rcutils/allocator.h>
#include <rcutils/strdup.h>
#include <rcutils/types/uint8_array.h>
#include <rmw/rmw.h>
#include <rmw/error_handling.h>
#include <rmw/init.h>
#include <rmw/init_options.h>
#include <rmw/allocators.h>
#include <rmw/features.h>
#include <rmw/names_and_types.h>
#include <rmw/network_flow_endpoint_array.h>
#include <rmw/get_network_flow_endpoints.h>

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
  init_options->localhost_only = RMW_LOCALHOST_ONLY_DEFAULT;
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
  *init_options = rmw_get_zero_initialized_init_options();
  return RMW_RET_OK;
}

// ---------------------------------------------------------------------------
// init / shutdown / context — wires the ZeroDDS runtime via the Rust bridge.
// ---------------------------------------------------------------------------

// Rust bridge (librmw_zerodds.so): create/destroy a ZeroDDS runtime context.
extern void * rmw_zerodds_init(unsigned int domain_id);
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
  context->impl->bridge_ctx = rmw_zerodds_init((unsigned int) domain);
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
extern void * rmw_zerodds_create_publisher(void * node, const char * type_name,
                                           const char * topic_name, int reliable);
extern int rmw_zerodds_destroy_publisher(void * pub);
extern size_t rmw_zerodds_publisher_matched_count(void * pub);
extern size_t rmw_zerodds_subscription_matched_count(void * sub);
extern int rmw_zerodds_publish(void * pub, const unsigned char * data, size_t len);
extern void * rmw_zerodds_create_subscription(void * node, const char * type_name,
                                              const char * topic_name, int reliable);
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
static const rosidl_typesupport_introspection_c__MessageMembers *
zerodds_introspect(const rosidl_message_type_support_t * ts)
{
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
    // … then fall back to the C++ introspection (rclcpp messages).
    h = ts->func(ts, ZERODDS_INTROSPECTION_CPP_ID);
    if (h && h->data) {
      return (const rosidl_typesupport_introspection_c__MessageMembers *) h->data;
    }
  }
  // The handle itself may be the cpp introspection (identifier matched by string).
  if (ts->typesupport_identifier && ts->data &&
      strcmp(ts->typesupport_identifier, ZERODDS_INTROSPECTION_CPP_ID) == 0)
  {
    return (const rosidl_typesupport_introspection_c__MessageMembers *) ts->data;
  }
  return NULL;
}

static char *
zerodds_type_name(const rosidl_message_type_support_t * ts)
{
  const rosidl_typesupport_introspection_c__MessageMembers * m = zerodds_introspect(ts);
  const char * ns = (m && m->message_namespace_) ? m->message_namespace_ : "rosidl";
  const char * nm = (m && m->message_name_) ? m->message_name_ : "Msg";
  size_t len = strlen(ns) + 2 + strlen(nm) + 1;
  char * out = (char *) malloc(len);
  if (out) { snprintf(out, len, "%s::%s", ns, nm); }
  return out;
}

#include <stdint.h>
#include <rosidl_typesupport_introspection_c/service_introspection.h>

// Bridge externs (librmw_zerodds.so) for the service request-reply path: raw
// byte transport over a request writer + reply reader (client) / request reader
// + reply writer (service); the readers are listener-fed inboxes so has_data is
// non-consuming and wakes the executor wait.
extern void * rmw_zerodds_create_client(void * node, const char * service, const char * type_name);
extern int rmw_zerodds_destroy_client(void * client);
extern int rmw_zerodds_send_request(void * client, const unsigned char * data, size_t len);
extern int rmw_zerodds_take_response(void * client, unsigned char ** out_buf, size_t * out_len, unsigned char * out_big_endian);
extern int rmw_zerodds_client_has_data(void * client);
extern int rmw_zerodds_client_server_available(void * client);
extern void * rmw_zerodds_create_service(void * node, const char * service, const char * type_name);
extern int rmw_zerodds_destroy_service(void * service);
extern int rmw_zerodds_take_request(void * service, unsigned char ** out_buf, size_t * out_len, unsigned char * out_big_endian);
extern int rmw_zerodds_send_response(void * service, const unsigned char * data, size_t len);
extern int rmw_zerodds_service_has_data(void * service);
// on_new_* event callbacks (EventsExecutor) — fired on each arrival.
extern int rmw_zerodds_subscription_set_event_callback(void * sub, rmw_event_callback_t cb, const void * ud);
extern int rmw_zerodds_service_set_event_callback(void * service, rmw_event_callback_t cb, const void * ud);
extern int rmw_zerodds_client_set_event_callback(void * client, rmw_event_callback_t cb, const void * ud);

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
extern int rmw_zerodds_subscription_enable_iceoryx(void * sub, const char * name);
// Starts the raw-mode doorbell (event-driven rmw_wait wakeups). Idempotent.
extern int rmw_zerodds_subscription_start_doorbell(void * sub);

// Delivery mode (zerodds-delivery-modes-1.0 §4): participant default from the
// env `ZERODDS_DELIVERY_MODE`. 0 = Portable (default, interop-safe),
// 1 = RawSameHost (same-host zero-copy SHM, no wire), 2 = Iceoryx (same-host
// cross-stack via iceoryx2). Modes 1/2 share the loan/commit/take_shm surface;
// they differ only in the writer/reader enable call.
static int zerodds_delivery_mode(void)
{
  static int cached = -1;
  if (cached < 0) {
    const char * e = getenv("ZERODDS_DELIVERY_MODE");
    if (e && (strcmp(e, "raw-same-host") == 0 || strcmp(e, "raw_same_host") == 0)) { cached = 1; }
    else if (e && strcmp(e, "iceoryx") == 0) { cached = 2; }
    else { cached = 0; }
  }
  return cached;
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
} zerodds_node_data_t;

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
  int mode;            // 0 = Portable, 1 = RawSameHost (SHM loan active)
  size_t struct_size;  // in-memory message struct size (the SHM slot size)
} zerodds_pub_data_t;
typedef struct {
  void * bridge_sub; rmw_qos_profile_t qos; const void * members;
  int mode;            // 0 = Portable, 1 = RawSameHost, 2 = Iceoryx
  size_t struct_size;
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
} zerodds_sub_data_t;

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

static int qos_reliable(const rmw_qos_profile_t * q)
{
  return (q && q->reliability == RMW_QOS_POLICY_RELIABILITY_BEST_EFFORT) ? 0 : 1;
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
  void * bp = rmw_zerodds_create_publisher(nd->bridge_node, tn, topic_name, qos_reliable(qos_policies));
  free(tn);
  if (!bp) { return NULL; }
  rmw_publisher_t * pub = rmw_publisher_allocate();
  zerodds_pub_data_t * pd = (zerodds_pub_data_t *) calloc(1, sizeof(zerodds_pub_data_t));
  if (!pub || !pd) { free(pd); if (pub) { rmw_publisher_free(pub); } rmw_zerodds_destroy_publisher(bp); return NULL; }
  pd->bridge_pub = bp;
  pd->qos = *qos_policies;
  pd->members = zerodds_introspect(type_support);
  static unsigned int gid_counter = 1;
  pd->gid.implementation_identifier = ZERODDS_IDENTIFIER;
  memcpy(pd->gid.data, &gid_counter, sizeof(gid_counter));
  gid_counter++;
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
  int dmode = zerodds_delivery_mode();
  if (dmode != 0 && pub->can_loan_messages && pd->struct_size > 0) {
    char name[256];
    if (dmode == 2) {
      zerodds_iceoryx_service(topic_name, name, sizeof(name));
      if (rmw_zerodds_publisher_enable_iceoryx(pd->bridge_pub, name, pd->struct_size) == RMW_RET_OK) {
        pd->mode = 2;
      }
    } else {
      zerodds_shm_path(topic_name, name, sizeof(name));
      if (rmw_zerodds_publisher_enable_raw_loan(pd->bridge_pub, name, ZERODDS_MAX_SHM_LOANS,
                                                pd->struct_size) == RMW_RET_OK) {
        pd->mode = 1;
      }
    }
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
rmw_publisher_assert_liveliness(const rmw_publisher_t * publisher) { (void) publisher; return RMW_RET_OK; }

rmw_ret_t
rmw_publisher_wait_for_all_acked(const rmw_publisher_t * publisher, rmw_time_t wait_timeout)
{
  (void) publisher; (void) wait_timeout; return RMW_RET_OK;
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
  void * bs = rmw_zerodds_create_subscription(nd->bridge_node, tn, topic_name, qos_reliable(qos_policies));
  free(tn);
  if (!bs) { return NULL; }
  rmw_subscription_t * sub = rmw_subscription_allocate();
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) calloc(1, sizeof(zerodds_sub_data_t));
  if (!sub || !sd) { free(sd); if (sub) { rmw_subscription_free(sub); } rmw_zerodds_destroy_subscription(bs); return NULL; }
  sd->bridge_sub = bs;
  sd->qos = *qos_policies;
  sd->members = zerodds_introspect(type_support);
  sub->implementation_identifier = ZERODDS_IDENTIFIER;
  sub->data = sd;
  sub->topic_name = rcutils_strdup(topic_name, node->context->options.allocator);
  if (subscription_options) { sub->options = *subscription_options; }
  sub->can_loan_messages = zerodds_msg_can_loan(sd->members) ? true : false;
  sub->is_cft_enabled = false;
  // Raw delivery modes. Mode 1 (SHM): remember the mode + flink path; the
  // mapping is lazy (the writer must create the segment first). Mode 2
  // (Iceoryx): the iceoryx2 subscribe is order-independent, so attach eagerly —
  // and if iceoryx is unavailable (shim built without `delivery-iceoryx`),
  // degrade to Portable now so the normal RTPS take still receives the writer's
  // Portable fallback (both sides degrade identically for the same build).
  sd->struct_size = zerodds_msg_struct_size(sd->members);
  int dmode = zerodds_delivery_mode();
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
  return sub;
}

rmw_ret_t
rmw_destroy_subscription(rmw_node_t * node, rmw_subscription_t * subscription)
{
  if (!subscription || !subscription->data) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) subscription->data;
  rmw_zerodds_destroy_subscription(sd->bridge_sub);
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
  unsigned char gid[16];
  int64_t seq;
} zerodds_client_data_t;

typedef struct {
  void * bridge_service;
  const ms_t * req_members;
  const ms_t * resp_members;
} zerodds_service_data_t;

typedef struct { unsigned char * buf; size_t len; size_t cap; } cdr_t;
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
typedef struct { const unsigned char * buf; size_t len; size_t pos; int big_endian; } rdr_t;
static void rdr_align(rdr_t * r, size_t a) { size_t off = (r->pos - 4) % a; if (off) { r->pos += (a - off); } }
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
  const uint16_t * const * dp = (const uint16_t * const *) p;
  const size_t * szp = (const size_t *) (p + sizeof(uint16_t *));
  const uint16_t * data = *dp;
  uint32_t u16len = (uint32_t) (data ? *szp : 0);
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
    const char * const * sp = (const char * const *) p;
    const size_t * szp = (const size_t *) (p + sizeof(char *));
    const char * data = *sp ? *sp : "";
    uint32_t n = (uint32_t) (*szp) + 1;
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
  bool ok = rosidl_runtime_c__U16String__assignn(p, u16, count);
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
    bool ok = rosidl_runtime_c__String__assignn(p, (const char *) (r->buf + r->pos), n - 1);
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
      if (!mem->resize_function(fp, cnt)) { return -1; }
      for (size_t k = 0; k < cnt; ++k) {
        unsigned char * ep = (unsigned char *) mem->get_function(fp, k);
        if (cdr_de_one(r, mem, ep)) { return -1; }
      }
    }
  }
  return 0;
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
  if (cdr_reserve(&c, 4)) { return RMW_RET_ERROR; }
  c.buf[0] = 0x00; c.buf[1] = 0x01; c.buf[2] = 0x00; c.buf[3] = 0x00;
  c.len = 4;
  if (cdr_ser_msg(&c, (const ms_t *) pd->members, (const unsigned char *) ros_message)) { free(c.buf); return RMW_RET_ERROR; }
  int rc = rmw_zerodds_publish(pd->bridge_pub, c.buf, c.len);
  free(c.buf);
  return rc == 0 ? RMW_RET_OK : RMW_RET_ERROR;
}

rmw_ret_t
rmw_take_with_info(const rmw_subscription_t * subscription, void * ros_message,
                   bool * taken, rmw_message_info_t * message_info,
                   rmw_subscription_allocation_t * allocation)
{
  (void) message_info; (void) allocation;
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
    // rmw_take is called without a preceding rmw_wait.
    if (!zerodds_sub_prefetch(sd)) { return RMW_RET_OK; }  // no sample
    size_t slen = sd->pending_len;
    size_t n = (sd->struct_size && slen >= sd->struct_size) ? sd->struct_size : slen;
    memcpy(ros_message, sd->pending_ptr, n);
    rmw_zerodds_subscription_release_shm(sd->bridge_sub, sd->pending_slot);
    sd->has_pending = 0;
    if (message_info) { memset(message_info, 0, sizeof(*message_info)); }
    if (taken) { *taken = true; }
    return RMW_RET_OK;
  }
  unsigned char * buf = NULL; size_t len = 0; unsigned char be = 0;
  int rc = rmw_zerodds_take(sd->bridge_sub, &buf, &len, &be);
  if (rc != 0 || !buf || len < 4) { if (buf) { rmw_zerodds_buffer_free(buf, len); } return RMW_RET_OK; }
  rdr_t r = { buf, len, 4, be };
  int dr = sd->members ? cdr_de_msg(&r, (const ms_t *) sd->members, (unsigned char *) ros_message) : -1;
  rmw_zerodds_buffer_free(buf, len);
  if (dr == 0 && taken) { *taken = true; }
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

    size_t ready = 0;
    if (subscriptions) {
      for (size_t i = 0; i < subscriptions->subscriber_count; ++i) {
        zerodds_sub_data_t * sd = (zerodds_sub_data_t *) subscriptions->subscribers[i];
        if (zerodds_sub_ready(sd)) { ready++; }
      }
    }
    if (guard_conditions) {
      for (size_t i = 0; i < guard_conditions->guard_condition_count; ++i) {
        rmw_guard_condition_t * gc = (rmw_guard_condition_t *) guard_conditions->guard_conditions[i];
        // Only our own guard conditions carry an `unsigned int *` flag in `data`.
        // rcl creates internal guard conditions that bypass rmw_create_guard_condition,
        // so their `data` is foreign memory — casting/dereferencing it is UB (segfault).
        unsigned int * f = (gc && gc->implementation_identifier == ZERODDS_IDENTIFIER)
                             ? (unsigned int *) gc->data : NULL;
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
    // events are not yet wired into wait (P3 / status events).

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
          rmw_guard_condition_t * gc =
            (rmw_guard_condition_t *) guard_conditions->guard_conditions[i];
          // Same foreign-guard-condition guard as the readiness loop above: never
          // cast/deref/write `data` of a guard condition we did not create. Here the
          // stakes are higher — `*f = 0` would corrupt rcl's foreign memory, not just read it.
          unsigned int * f = (gc && gc->implementation_identifier == ZERODDS_IDENTIFIER)
                               ? (unsigned int *) gc->data : NULL;
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
      if (events) { for (size_t i = 0; i < events->event_count; ++i) { events->events[i] = NULL; } }
      return (ready > 0) ? RMW_RET_OK : RMW_RET_TIMEOUT;
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
  const ms_t * members = zerodds_introspect(type_support);
  if (!members) { return RMW_RET_ERROR; }
  cdr_t c = {0};
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
  const ms_t * members = zerodds_introspect(type_support);
  if (!members || serialized_message->buffer_length < 4) { return RMW_RET_ERROR; }
  rdr_t r = { serialized_message->buffer, serialized_message->buffer_length, 4, (unsigned char) (serialized_message->buffer[1] == 0x00) };
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

rmw_ret_t rmw_publisher_event_init(rmw_event_t * e, const rmw_publisher_t * p, rmw_event_type_t t)
{ (void) p; if (e) { e->implementation_identifier = ZERODDS_IDENTIFIER; e->data = NULL; e->event_type = t; } return RMW_RET_OK; }
rmw_ret_t rmw_subscription_event_init(rmw_event_t * e, const rmw_subscription_t * s, rmw_event_type_t t)
{ (void) s; if (e) { e->implementation_identifier = ZERODDS_IDENTIFIER; e->data = NULL; e->event_type = t; } return RMW_RET_OK; }
rmw_ret_t rmw_take_event(const rmw_event_t * e, void * d, bool * taken)
{ (void) e; (void) d; if (taken) { *taken = false; } return RMW_RET_OK; }
rmw_ret_t rmw_event_set_callback(rmw_event_t * e, rmw_event_callback_t cb, const void * u)
{ (void) e; (void) cb; (void) u; return RMW_RET_UNSUPPORTED; }

bool rmw_feature_supported(rmw_feature_t feature) { (void) feature; return false; }

rmw_ret_t rmw_compare_gids_equal(const rmw_gid_t * a, const rmw_gid_t * b, bool * result)
{ if (!a || !b || !result) { return RMW_RET_INVALID_ARGUMENT; } *result = (memcmp(a->data, b->data, RMW_GID_STORAGE_SIZE) == 0); return RMW_RET_OK; }

// ---------------------------------------------------------------------------
// Graph introspection (P4a) — discovered topics via the node's runtime SEDP.
// rmw_get_node_names needs the rmw_dds_common graph cache (ros_discovery_info)
// and stays empty (see ros2-rmw.md, P4b).
// ---------------------------------------------------------------------------
#include <rcutils/types/string_array.h>
typedef void (*zerodds_topic_cb_t)(void * ud, const char * topic, const char * type);
extern int rmw_zerodds_node_for_each_publication(void * node, zerodds_topic_cb_t cb, void * ud);
extern int rmw_zerodds_node_for_each_subscription(void * node, zerodds_topic_cb_t cb, void * ud);

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
// "std_msgs__msg::String" → "std_msgs/msg/String". The rosidl introspection
// namespace uses "__" as the package/msg separator and the bridge joins it to
// the message name with "::"; both map to '/'. Single underscores are kept.
static char * zerodds_demangle_type(const char * ty)
{
  size_t n = strlen(ty);
  char * out = (char *) malloc(n + 1);
  if (!out) { return NULL; }
  size_t j = 0;
  for (size_t i = 0; i < n; ) {
    if (ty[i] == ':' && i + 1 < n && ty[i + 1] == ':') { out[j++] = '/'; i += 2; }
    else if (ty[i] == '_' && i + 1 < n && ty[i + 1] == '_') { out[j++] = '/'; i += 2; }
    else { out[j++] = ty[i++]; }
  }
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
rmw_ret_t rmw_count_clients(const rmw_node_t * n, const char * t, size_t * c)
{ (void) n; (void) t; if (c) { *c = 0; } return RMW_RET_OK; }
rmw_ret_t rmw_count_services(const rmw_node_t * n, const char * t, size_t * c)
{ (void) n; (void) t; if (c) { *c = 0; } return RMW_RET_OK; }

// ---------------------------------------------------------------------------
// Services / clients — request/reply over the bridge with a 24-byte
// correlation header [client_gid:16][sequence:8 LE] prepended to the CDR
// (encap + body). The server echoes the header in the reply; the client filters
// replies by its gid. Self-consistent for rmw_zerodds <-> rmw_zerodds.
// ---------------------------------------------------------------------------
#define ZERODDS_SVC_HDR 24  // 16 gid + 8 sequence
// (zerodds_client_data_t / zerodds_service_data_t are declared above rmw_wait.)

// Resolve the introspection ServiceMembers (request/response MessageMembers).
typedef rosidl_typesupport_introspection_c__ServiceMembers svc_ms_t;
static const svc_ms_t * zerodds_service_introspect(const rosidl_service_type_support_t * ts)
{
  if (!ts) { return NULL; }
  const rosidl_service_type_support_t * h = ts;
  if (ts->typesupport_identifier != rosidl_typesupport_introspection_c__identifier && ts->func) {
    h = ts->func(ts, rosidl_typesupport_introspection_c__identifier);
  }
  if (!h || !h->data) { return NULL; }
  return (const svc_ms_t *) h->data;
}

// Map a ROS service name (possibly "/ns/name") to the bridge's accepted form
// [A-Za-z_][A-Za-z0-9_]*: drop a leading '/', replace remaining '/' with '_'.
// Client and service use the same mapping → matching request/reply topics.
static char * zerodds_sanitize_service(const char * name)
{
  if (!name) { return NULL; }
  while (*name == '/') { name++; }
  size_t n = strlen(name);
  char * out = (char *) malloc(n + 1);
  if (!out) { return NULL; }
  for (size_t i = 0; i < n; ++i) { out[i] = (name[i] == '/') ? '_' : name[i]; }
  out[n] = '\0';
  return out;
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
static int zerodds_cdr_message(cdr_t * out, const ms_t * members, const void * msg)
{
  if (cdr_reserve(out, 4)) { return -1; }
  out->buf[0] = 0x00; out->buf[1] = 0x01; out->buf[2] = 0x00; out->buf[3] = 0x00;
  out->len = 4;
  return cdr_ser_msg(out, members, (const unsigned char *) msg);
}

static const char * zerodds_service_type_string(const svc_ms_t * sm, char * buf, size_t cap)
{
  const char * ns = (sm->request_members_ && sm->request_members_->message_namespace_)
                      ? sm->request_members_->message_namespace_ : "rosidl";
  const char * nm = sm->service_name_ ? sm->service_name_ : "Srv";
  snprintf(buf, cap, "%s::%s", ns, nm);
  return buf;
}

rmw_client_t * rmw_create_client(const rmw_node_t * n, const rosidl_service_type_support_t * ts,
  const char * name, const rmw_qos_profile_t * q)
{
  (void) q;
  if (!n || !n->data || !ts || !name) { return NULL; }
  const svc_ms_t * sm = zerodds_service_introspect(ts);
  if (!sm) { return NULL; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;
  char tn[512]; zerodds_service_type_string(sm, tn, sizeof(tn));
  char * svc = zerodds_sanitize_service(name);
  void * bc = svc ? rmw_zerodds_create_client(nd->bridge_node, svc, tn) : NULL;
  free(svc);
  if (!bc) { return NULL; }
  rmw_client_t * c = rmw_client_allocate();
  zerodds_client_data_t * cd = (zerodds_client_data_t *) calloc(1, sizeof(zerodds_client_data_t));
  if (!c || !cd) { free(cd); if (c) { rmw_client_free(c); } rmw_zerodds_destroy_client(bc); return NULL; }
  cd->bridge_client = bc;
  cd->req_members = sm->request_members_;
  cd->resp_members = sm->response_members_;
  cd->seq = 0;
  zerodds_make_gid(cd->gid, cd);
  c->implementation_identifier = ZERODDS_IDENTIFIER;
  c->data = cd;
  c->service_name = rcutils_strdup(name, n->context->options.allocator);
  return c;
}

rmw_ret_t rmw_destroy_client(rmw_node_t * n, rmw_client_t * c)
{
  if (!c || !c->data) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_client_data_t * cd = (zerodds_client_data_t *) c->data;
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
  if (!c || !c->data || !req) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_client_data_t * cd = (zerodds_client_data_t *) c->data;
  if (!cd->req_members) { return RMW_RET_ERROR; }
  cdr_t cc = {0};
  if (zerodds_cdr_message(&cc, cd->req_members, req)) { free(cc.buf); return RMW_RET_ERROR; }
  int64_t s = cd->seq++;
  size_t total = ZERODDS_SVC_HDR + cc.len;
  unsigned char * wire = (unsigned char *) malloc(total);
  if (!wire) { free(cc.buf); return RMW_RET_ERROR; }
  memcpy(wire, cd->gid, 16);
  memcpy(wire + 16, &s, 8);
  memcpy(wire + ZERODDS_SVC_HDR, cc.buf, cc.len);
  free(cc.buf);
  int rc = rmw_zerodds_send_request(cd->bridge_client, wire, total);
  free(wire);
  if (rc != 0) { return RMW_RET_ERROR; }
  if (seq) { *seq = s; }
  return RMW_RET_OK;
}

rmw_ret_t rmw_take_response(const rmw_client_t * c, rmw_service_info_t * h, void * r, bool * taken)
{
  if (taken) { *taken = false; }
  if (!c || !c->data || !r) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_client_data_t * cd = (zerodds_client_data_t *) c->data;
  for (;;) {
    unsigned char * buf = NULL; size_t len = 0; unsigned char be = 0;
    if (rmw_zerodds_take_response(cd->bridge_client, &buf, &len, &be) != 0 || !buf) { return RMW_RET_OK; }
    if (len < ZERODDS_SVC_HDR + 4 || memcmp(buf, cd->gid, 16) != 0) {
      rmw_zerodds_buffer_free(buf, len);  // not ours / malformed — skip
      continue;
    }
    int64_t s; memcpy(&s, buf + 16, 8);
    rdr_t rr = { buf + ZERODDS_SVC_HDR, len - ZERODDS_SVC_HDR, 4, be };
    int dr = cd->resp_members ? cdr_de_msg(&rr, cd->resp_members, (unsigned char *) r) : -1;
    rmw_zerodds_buffer_free(buf, len);
    if (dr != 0) { return RMW_RET_OK; }
    if (h) {
      memcpy(h->request_id.writer_guid, cd->gid, 16);
      h->request_id.sequence_number = s;
      h->source_timestamp = 0;
      h->received_timestamp = 0;
    }
    if (taken) { *taken = true; }
    return RMW_RET_OK;
  }
}

rmw_ret_t rmw_client_request_publisher_get_actual_qos(const rmw_client_t * c, rmw_qos_profile_t * q)
{ (void) c; if (q) { *q = rmw_qos_profile_services_default; } return RMW_RET_OK; }
rmw_ret_t rmw_client_response_subscription_get_actual_qos(const rmw_client_t * c, rmw_qos_profile_t * q)
{ (void) c; if (q) { *q = rmw_qos_profile_services_default; } return RMW_RET_OK; }
rmw_ret_t rmw_client_set_on_new_response_callback(rmw_client_t * c, rmw_event_callback_t cb, const void * u)
{
  if (!c || !c->data) { return RMW_RET_INVALID_ARGUMENT; }
  rmw_zerodds_client_set_event_callback(((zerodds_client_data_t *) c->data)->bridge_client, cb, u);
  return RMW_RET_OK;
}

rmw_service_t * rmw_create_service(const rmw_node_t * n, const rosidl_service_type_support_t * ts,
  const char * name, const rmw_qos_profile_t * q)
{
  (void) q;
  if (!n || !n->data || !ts || !name) { return NULL; }
  const svc_ms_t * sm = zerodds_service_introspect(ts);
  if (!sm) { return NULL; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;
  char tn[512]; zerodds_service_type_string(sm, tn, sizeof(tn));
  char * svc = zerodds_sanitize_service(name);
  void * bs = svc ? rmw_zerodds_create_service(nd->bridge_node, svc, tn) : NULL;
  free(svc);
  if (!bs) { return NULL; }
  rmw_service_t * sv = rmw_service_allocate();
  zerodds_service_data_t * sd = (zerodds_service_data_t *) calloc(1, sizeof(zerodds_service_data_t));
  if (!sv || !sd) { free(sd); if (sv) { rmw_service_free(sv); } rmw_zerodds_destroy_service(bs); return NULL; }
  sd->bridge_service = bs;
  sd->req_members = sm->request_members_;
  sd->resp_members = sm->response_members_;
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
  if (rmw_zerodds_take_request(sd->bridge_service, &buf, &len, &be) != 0 || !buf) { return RMW_RET_OK; }
  if (len < ZERODDS_SVC_HDR + 4) { rmw_zerodds_buffer_free(buf, len); return RMW_RET_OK; }
  int64_t s_seq; memcpy(&s_seq, buf + 16, 8);
  rdr_t rr = { buf + ZERODDS_SVC_HDR, len - ZERODDS_SVC_HDR, 4, be };
  int dr = sd->req_members ? cdr_de_msg(&rr, sd->req_members, (unsigned char *) r) : -1;
  if (dr == 0 && h) {
    memcpy(h->request_id.writer_guid, buf, 16);  // client gid → route the reply
    h->request_id.sequence_number = s_seq;
    h->source_timestamp = 0;
    h->received_timestamp = 0;
  }
  rmw_zerodds_buffer_free(buf, len);
  if (dr == 0 && taken) { *taken = true; }
  return RMW_RET_OK;
}

rmw_ret_t rmw_send_response(const rmw_service_t * s, rmw_request_id_t * id, void * r)
{
  if (!s || !s->data || !id || !r) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_service_data_t * sd = (zerodds_service_data_t *) s->data;
  if (!sd->resp_members) { return RMW_RET_ERROR; }
  cdr_t cc = {0};
  if (zerodds_cdr_message(&cc, sd->resp_members, r)) { free(cc.buf); return RMW_RET_ERROR; }
  size_t total = ZERODDS_SVC_HDR + cc.len;
  unsigned char * wire = (unsigned char *) malloc(total);
  if (!wire) { free(cc.buf); return RMW_RET_ERROR; }
  memcpy(wire, id->writer_guid, 16);                 // echo client gid
  memcpy(wire + 16, &id->sequence_number, 8);        // echo sequence
  memcpy(wire + ZERODDS_SVC_HDR, cc.buf, cc.len);
  free(cc.buf);
  int rc = rmw_zerodds_send_response(sd->bridge_service, wire, total);
  free(wire);
  return rc == 0 ? RMW_RET_OK : RMW_RET_ERROR;
}

rmw_ret_t rmw_service_request_subscription_get_actual_qos(const rmw_service_t * s, rmw_qos_profile_t * q)
{ (void) s; if (q) { *q = rmw_qos_profile_services_default; } return RMW_RET_OK; }
rmw_ret_t rmw_service_response_publisher_get_actual_qos(const rmw_service_t * s, rmw_qos_profile_t * q)
{ (void) s; if (q) { *q = rmw_qos_profile_services_default; } return RMW_RET_OK; }
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
{ (void) s; (void) o; return RMW_RET_UNSUPPORTED; }
rmw_ret_t rmw_subscription_get_content_filter(const rmw_subscription_t * s, rcutils_allocator_t * a,
  rmw_subscription_content_filter_options_t * o)
{ (void) s; (void) a; (void) o; return RMW_RET_UNSUPPORTED; }

// ---------------------------------------------------------------------------
// Graph — empty results so introspection succeeds without crashing.
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
rmw_ret_t rmw_get_topic_names_and_types(const rmw_node_t * n, rcutils_allocator_t * a, bool d, rmw_names_and_types_t * nt)
{
  (void) d;
  if (!n || !n->data || !a || !nt) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_node_data_t * nd = (zerodds_node_data_t *) n->data;
  zerodds_tt_t acc = {0};
  rmw_zerodds_node_for_each_publication(nd->bridge_node, zerodds_tt_cb, &acc);
  rmw_zerodds_node_for_each_subscription(nd->bridge_node, zerodds_tt_cb, &acc);
  rmw_ret_t ret = rmw_names_and_types_init(nt, acc.count, a);
  if (ret == RMW_RET_OK) {
    for (size_t i = 0; i < acc.count; ++i) {
      nt->names.data[i] = rcutils_strdup(acc.topics[i], *a);
      if (rcutils_string_array_init(&nt->types[i], acc.ntypes[i], a) == RCUTILS_RET_OK) {
        for (size_t j = 0; j < acc.ntypes[i]; ++j) {
          nt->types[i].data[j] = rcutils_strdup(acc.types[i][j], *a);
        }
      }
    }
  }
  for (size_t i = 0; i < acc.count; ++i) {
    for (size_t j = 0; j < acc.ntypes[i]; ++j) { free(acc.types[i][j]); }
    free(acc.types[i]);
    free(acc.topics[i]);
  }
  free(acc.topics); free(acc.types); free(acc.ntypes);
  return ret;
}
rmw_ret_t rmw_get_service_names_and_types(const rmw_node_t * n, rcutils_allocator_t * a, rmw_names_and_types_t * nt)
{ (void) n; (void) a; (void) nt; return RMW_RET_OK; }
rmw_ret_t rmw_get_publisher_names_and_types_by_node(const rmw_node_t * n, rcutils_allocator_t * a,
  const char * nn, const char * ns, bool d, rmw_names_and_types_t * nt)
{ (void) n; (void) a; (void) nn; (void) ns; (void) d; (void) nt; return RMW_RET_OK; }
rmw_ret_t rmw_get_subscriber_names_and_types_by_node(const rmw_node_t * n, rcutils_allocator_t * a,
  const char * nn, const char * ns, bool d, rmw_names_and_types_t * nt)
{ (void) n; (void) a; (void) nn; (void) ns; (void) d; (void) nt; return RMW_RET_OK; }
rmw_ret_t rmw_get_service_names_and_types_by_node(const rmw_node_t * n, rcutils_allocator_t * a,
  const char * nn, const char * ns, rmw_names_and_types_t * nt)
{ (void) n; (void) a; (void) nn; (void) ns; (void) nt; return RMW_RET_OK; }
rmw_ret_t rmw_get_client_names_and_types_by_node(const rmw_node_t * n, rcutils_allocator_t * a,
  const char * nn, const char * ns, rmw_names_and_types_t * nt)
{ (void) n; (void) a; (void) nn; (void) ns; (void) nt; return RMW_RET_OK; }
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
  int32_t deadline_seconds;
  int32_t lifespan_seconds;
  int32_t liveliness_lease_seconds;
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
  int32_t deadline_s;
  int32_t lifespan_s;
  int32_t liveliness_s;
} zerodds_ep_row_t;

typedef struct {
  const char * target_topic;  // demangled ROS topic to match (e.g. "/chatter")
  zerodds_ep_row_t * rows;
  size_t count;
  size_t cap;
} zerodds_ep_collect_t;

static void zerodds_ep_collect_cb(void * ud, const zerodds_endpoint_info_t * e)
{
  zerodds_ep_collect_t * c = (zerodds_ep_collect_t *) ud;
  char * dtopic = zerodds_demangle_topic(e->topic_name);
  if (!dtopic) { return; }                       // non-"rt/" (service/internal)
  if (strcmp(dtopic, c->target_topic) != 0) { free(dtopic); return; }
  free(dtopic);
  if (c->count == c->cap) {
    size_t ncap = c->cap ? c->cap * 2 : 4;
    zerodds_ep_row_t * nr = (zerodds_ep_row_t *) realloc(c->rows, ncap * sizeof(*nr));
    if (!nr) { return; }                          // OOM: drop this row, keep prior
    c->rows = nr; c->cap = ncap;
  }
  zerodds_ep_row_t * row = &c->rows[c->count];
  memcpy(row->gid, e->endpoint_guid, 16);
  row->type = zerodds_demangle_type(e->type_name);
  row->reliable = e->reliable;
  row->transient_local = e->transient_local;
  row->deadline_s = e->deadline_seconds;
  row->lifespan_s = e->lifespan_seconds;
  row->liveliness_s = e->liveliness_lease_seconds;
  c->count++;
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

  zerodds_ep_collect_t cx = { t, NULL, 0, 0 };
  if (is_pub) {
    rmw_zerodds_node_for_each_publication_endpoint(nd->bridge_node, zerodds_ep_collect_cb, &cx);
  } else {
    rmw_zerodds_node_for_each_subscription_endpoint(nd->bridge_node, zerodds_ep_collect_cb, &cx);
  }

  rmw_ret_t rc = rmw_topic_endpoint_info_array_init_with_size(info, cx.count, a);
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

    rmw_qos_profile_t qos = rmw_qos_profile_default;
    qos.reliability = row->reliable ?
      RMW_QOS_POLICY_RELIABILITY_RELIABLE : RMW_QOS_POLICY_RELIABILITY_BEST_EFFORT;
    qos.durability = row->transient_local ?
      RMW_QOS_POLICY_DURABILITY_TRANSIENT_LOCAL : RMW_QOS_POLICY_DURABILITY_VOLATILE;
    // History/depth are not carried on the wire — leave them as "unknown".
    qos.history = RMW_QOS_POLICY_HISTORY_UNKNOWN;
    qos.depth = 0;
    qos.deadline.sec = (uint64_t) (row->deadline_s > 0 ? row->deadline_s : 0);
    qos.deadline.nsec = 0;
    qos.lifespan.sec = (uint64_t) (row->lifespan_s > 0 ? row->lifespan_s : 0);
    qos.lifespan.nsec = 0;
    qos.liveliness_lease_duration.sec = (uint64_t) (row->liveliness_s > 0 ? row->liveliness_s : 0);
    qos.liveliness_lease_duration.nsec = 0;

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

rmw_ret_t rmw_qos_profile_check_compatible(const rmw_qos_profile_t p, const rmw_qos_profile_t s,
  rmw_qos_compatibility_type_t * compat, char * reason, size_t reason_size)
{ (void) p; (void) s; (void) reason; (void) reason_size; if (compat) { *compat = RMW_QOS_COMPATIBILITY_OK; } return RMW_RET_OK; }

// ---------------------------------------------------------------------------
// Typed-message loaning (`zerodds-delivery-modes-1.0`). A message is loanable
// iff it is a fixed POD (only primitives, fixed arrays of POD, nested fixed-POD
// structs — no strings, no sequences). The loaned buffer is the ROS message
// struct (the in-memory C layout). Two delivery modes, selected per the env
// `ZERODDS_DELIVERY_MODE` (participant default):
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
  void * m = calloc(1, sz);  // Portable: heap struct, serialized at publish
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
    if (mi) { memset(mi, 0, sizeof(*mi)); }
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
  (void) mi; (void) a;
  if (taken) { *taken = false; }
  if (!s || !s->data || !m) { return RMW_RET_INVALID_ARGUMENT; }
  zerodds_sub_data_t * sd = (zerodds_sub_data_t *) s->data;
  unsigned char * buf = NULL; size_t len = 0;
  if (rmw_zerodds_take(sd->bridge_sub, &buf, &len, NULL) != 0 || !buf || len == 0) {
    if (buf) { rmw_zerodds_buffer_free(buf, len); }
    return RMW_RET_OK;
  }
  if (rcutils_uint8_array_resize(m, len) != RCUTILS_RET_OK) {
    rmw_zerodds_buffer_free(buf, len);
    return RMW_RET_BAD_ALLOC;
  }
  memcpy(m->buffer, buf, len);
  m->buffer_length = len;
  rmw_zerodds_buffer_free(buf, len);
  if (taken) { *taken = true; }
  return RMW_RET_OK;
}
rmw_ret_t rmw_take_serialized_message(const rmw_subscription_t * s, rmw_serialized_message_t * m, bool * taken, rmw_subscription_allocation_t * a)
{ return rmw_take_serialized_message_with_info(s, m, taken, NULL, a); }
rmw_ret_t rmw_take_sequence(const rmw_subscription_t * s, size_t count, rmw_message_sequence_t * seq,
  rmw_message_info_sequence_t * iseq, size_t * taken, rmw_subscription_allocation_t * a)
{ (void) s; (void) count; (void) seq; (void) iseq; if (taken) { *taken = 0; } (void) a; return RMW_RET_OK; }

rmw_ret_t rmw_get_gid_for_client(const rmw_client_t * c, rmw_gid_t * gid)
{ (void) c; (void) gid; return RMW_RET_UNSUPPORTED; }
rmw_ret_t rmw_publisher_get_network_flow_endpoints(const rmw_publisher_t * p, rcutils_allocator_t * a, rmw_network_flow_endpoint_array_t * arr)
{ (void) p; (void) a; (void) arr; return RMW_RET_UNSUPPORTED; }
rmw_ret_t rmw_subscription_get_network_flow_endpoints(const rmw_subscription_t * s, rcutils_allocator_t * a, rmw_network_flow_endpoint_array_t * arr)
{ (void) s; (void) a; (void) arr; return RMW_RET_UNSUPPORTED; }

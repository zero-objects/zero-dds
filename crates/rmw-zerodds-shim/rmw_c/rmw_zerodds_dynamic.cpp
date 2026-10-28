// rmw_zerodds_dynamic.cpp — ROS 2 Jazzy dynamic type support.
//
// A `rosidl_dynamic_typesupport_serialization_support_interface_t`
// implementation backed by ZeroDDS' own reflective DynamicType/DynamicData
// (crates/rmw-zerodds-shim/src/dynamic.rs), the way
// `rosidl_dynamic_typesupport_fastrtps` is one for Fast DDS — plus the
// three rmw.h entry points that hand it to rclcpp/rclpy:
// `rmw_serialization_support_init`, `rmw_take_dynamic_message`,
// `rmw_take_dynamic_message_with_info`.
//
// Guarded on the header exactly as rmw_zerodds.c's three (now removed)
// stub functions were: Humble has neither `<rosidl_dynamic_typesupport/
// types.h>` nor the three rmw.h declarations, and this translation unit
// compiles to nothing there.
//
// # Design
//
// Every `..._impl_s` struct this interface's callers embed *by value*
// (`rosidl_dynamic_typesupport_dynamic_type_builder_impl_s`, `..._type_
// impl_s`, `..._data_impl_s`) is generically defined by ROS itself as
// `{ rcutils_allocator_t allocator; void * handle; }` — see
// `rosidl_dynamic_typesupport/api/dynamic_type.h` and `.../dynamic_data.h`.
// `handle` is exactly the slot every wrapper below uses: it holds one of
// dynamic.rs's three opaque Rust pointers (`ZddsDynBuilder*`,
// `ZddsDynType*`, `ZddsDynData*`), cast to `void*`. Nothing in this file
// needs to know those Rust types' layout — every call crosses straight
// into the `extern "C"` functions declared below.
//
// # What is not implemented, and why
//
// A member's `default_value` (every `add_*_member` in the primitive/string
// families below) *is* implemented — forwarded to
// `zerodds_dyn_builder_add_member`, which translates it against the
// member's own kind and attaches it as the member's `@default` (see
// dynamic.rs's module note on `ros_default_literal`). The `add_complex_*`
// family still takes and ignores the same two parameters: `.msg`'s grammar
// has no syntax for a nested-message-typed field's default, so
// `TypeDescription` generation never fills them for one.
//
// - **A writer-negotiated `DataRepresentation` QoS**, for
//   `dynamic_data_serialize`. The interface hands that call a bare
//   `dynamic_data_impl_t*` with no writer/QoS context at all; every
//   struct this module builds is `@final`, little-endian, XCDR1 — see
//   dynamic.rs's module note.
// - **32-bit ARM and PowerPC `long double`.** `get`/`set_float128_*`
//   below pick the x86-64 80-bit-extended or the true-binary128 layout
//   at compile time (`ZDDS_DYN_F128_X87`); a host whose `long double` is
//   neither refuses the call rather than silently reading the wrong
//   layout — named because PowerPC is a real ZeroDDS target elsewhere
//   (`docs/`'s native endpoint SDK note), not a hypothetical.

#if defined(__has_include)
#if __has_include(<rosidl_dynamic_typesupport/types.h>)

#include <cstdint>
#include <cstring>
#include <string>
#include <type_traits>

#include <rcutils/allocator.h>
#include <rcutils/types/rcutils_ret.h>
#include <rcutils/types/uint8_array.h>
#include <rmw/convert_rcutils_ret_to_rmw_ret.h>
#include <rmw/dynamic_message_type_support.h>
#include <rmw/error_handling.h>
#include <rmw/rmw.h>
#include <rosidl_dynamic_typesupport/api/dynamic_data.h>
#include <rosidl_dynamic_typesupport/api/dynamic_type.h>
#include <rosidl_dynamic_typesupport/api/serialization_support.h>
#include <rosidl_dynamic_typesupport/api/serialization_support_interface.h>
#include <rosidl_dynamic_typesupport/types.h>

// The dynamic takes go through rmw_zerodds.c's own
// `rmw_take_serialized_message_with_info` (declared in <rmw/rmw.h>): it
// applies the subscription's content filter and fills `message_info`
// from the sample's origin, the same as the static take, and hands back
// the stream with its encapsulation header, which is what
// `zerodds_dyn_data_deserialize` reads the representation from.

// =================================================================================================
// Rust FFI (crates/rmw-zerodds-shim/src/dynamic.rs)
// =================================================================================================
extern "C" {

// -- builder --
void * zerodds_dyn_builder_init(const char * name, size_t name_len);
void * zerodds_dyn_builder_clone(const void * src);
void zerodds_dyn_builder_free(void * b);
int32_t zerodds_dyn_builder_get_name(const void * b, const char ** out_ptr, size_t * out_len);
int32_t zerodds_dyn_builder_set_name(void * b, const char * name, size_t name_len);
int32_t zerodds_dyn_builder_add_member(
  void * b, uint32_t kind_tag, uint32_t shape_tag, uint64_t id, const char * name,
  size_t name_len, uint64_t str_bound, uint64_t container_bound, const char * default_value,
  size_t default_value_len);
int32_t zerodds_dyn_builder_add_nested_member(
  void * b, uint32_t shape_tag, uint64_t id, const char * name, size_t name_len,
  uint64_t container_bound, const void * nested);
int32_t zerodds_dyn_builder_add_nested_member_from_builder(
  void * b, uint32_t shape_tag, uint64_t id, const char * name, size_t name_len,
  uint64_t container_bound, const void * nested);
void * zerodds_dyn_builder_finish(void * b);
void * zerodds_dyn_data_from_builder(void * b);

// -- type --
void zerodds_dyn_type_free(void * t);
void * zerodds_dyn_type_clone(const void * t);
uint8_t zerodds_dyn_type_equals(const void * a, const void * b);
uint64_t zerodds_dyn_type_member_count(const void * t);
int32_t zerodds_dyn_type_get_name(const void * t, const char ** out_ptr, size_t * out_len);

// -- data: lifecycle --
void * zerodds_dyn_data_from_type(const void * t);
void zerodds_dyn_data_free(void * d);
void * zerodds_dyn_data_clone(const void * d);
uint8_t zerodds_dyn_data_equals(const void * a, const void * b);
uint64_t zerodds_dyn_data_item_count(const void * d);
int32_t zerodds_dyn_data_member_id_by_name(
  const void * d, const char * name, size_t name_len, uint64_t * out_id);
int32_t zerodds_dyn_data_id_at_index(const void * d, uint64_t index, uint64_t * out_id);
int32_t zerodds_dyn_data_get_name(const void * d, const char ** out_ptr, size_t * out_len);
int32_t zerodds_dyn_data_clear_all_values(void * d);
int32_t zerodds_dyn_data_clear_nonkey_values(void * d);
int32_t zerodds_dyn_data_clear_value(void * d, uint64_t id);

// -- data: scalar getters/setters/inserts --
int32_t zerodds_dyn_data_get_bool(const void * d, uint64_t id, uint8_t * out);
int32_t zerodds_dyn_data_set_bool(void * d, uint64_t id, uint8_t value);
int32_t zerodds_dyn_data_insert_bool(void * d, uint8_t value, uint64_t * out_id);

#define ZDDS_DECLARE_INT(suffix, ctype) \
  int32_t zerodds_dyn_data_get_##suffix(const void * d, uint64_t id, ctype * out); \
  int32_t zerodds_dyn_data_set_##suffix(void * d, uint64_t id, ctype value); \
  int32_t zerodds_dyn_data_insert_##suffix(void * d, ctype value, uint64_t * out_id)

ZDDS_DECLARE_INT(int8, int8_t);
ZDDS_DECLARE_INT(uint8, uint8_t);
ZDDS_DECLARE_INT(byte, uint8_t);
ZDDS_DECLARE_INT(int16, int16_t);
ZDDS_DECLARE_INT(uint16, uint16_t);
ZDDS_DECLARE_INT(int32, int32_t);
ZDDS_DECLARE_INT(uint32, uint32_t);
ZDDS_DECLARE_INT(int64, int64_t);
ZDDS_DECLARE_INT(uint64, uint64_t);
ZDDS_DECLARE_INT(char, uint32_t);
ZDDS_DECLARE_INT(wchar, uint32_t);
ZDDS_DECLARE_INT(float32, float);
ZDDS_DECLARE_INT(float64, double);

#undef ZDDS_DECLARE_INT

int32_t zerodds_dyn_data_get_float128_x87(const void * d, uint64_t id, uint8_t * out16);
int32_t zerodds_dyn_data_set_float128_x87(void * d, uint64_t id, const uint8_t * in16);
int32_t zerodds_dyn_data_insert_float128_x87(void * d, const uint8_t * in16, uint64_t * out_id);
int32_t zerodds_dyn_data_get_float128_ieee(const void * d, uint64_t id, uint8_t * out16);
int32_t zerodds_dyn_data_set_float128_ieee(void * d, uint64_t id, const uint8_t * in16);
int32_t zerodds_dyn_data_insert_float128_ieee(void * d, const uint8_t * in16, uint64_t * out_id);

// -- data: strings --
int32_t zerodds_dyn_data_get_string(void * d, uint64_t id, const char ** out_ptr, size_t * out_len);
int32_t zerodds_dyn_data_set_string(
  void * d, uint64_t id, const uint8_t * ptr, size_t len, uint64_t bound);
int32_t zerodds_dyn_data_insert_string(
  void * d, const uint8_t * ptr, size_t len, uint64_t bound, uint64_t * out_id);
int32_t zerodds_dyn_data_get_wstring(
  void * d, uint64_t id, const uint16_t ** out_ptr, size_t * out_len);
int32_t zerodds_dyn_data_set_wstring(
  void * d, uint64_t id, const uint16_t * ptr, size_t len, uint64_t bound);
int32_t zerodds_dyn_data_insert_wstring(
  void * d, const uint16_t * ptr, size_t len, uint64_t bound, uint64_t * out_id);

// -- data: sequences --
int32_t zerodds_dyn_data_seq_clear(void * d);
int32_t zerodds_dyn_data_seq_remove(void * d, uint64_t id);
int32_t zerodds_dyn_data_seq_insert_default(void * d, uint64_t * out_id);

// -- data: nested --
void * zerodds_dyn_data_get_complex(const void * d, uint64_t id);
int32_t zerodds_dyn_data_set_complex(void * d, uint64_t id, const void * value);
int32_t zerodds_dyn_data_insert_complex_copy(void * d, const void * value, uint64_t * out_id);
int32_t zerodds_dyn_data_insert_complex(void * d, void * value, uint64_t * out_id);
void * zerodds_dyn_data_loan(void * d, uint64_t id);
int32_t zerodds_dyn_data_return_loan(void * d, void * loaned);

// -- data: (de)serialize --
int32_t zerodds_dyn_data_serialize(const void * d, uint8_t ** out_ptr, size_t * out_len);
int32_t zerodds_dyn_data_deserialize(void * d, const uint8_t * bytes_ptr, size_t bytes_len);
void zerodds_dyn_bytes_free(uint8_t * ptr, size_t len);

}  // extern "C"

// =================================================================================================
// Kind / shape tags — must match `ZddsDynKindTag`/`ZddsDynShapeTag` in
// dynamic.rs exactly; there is no generated header for them (cbindgen
// does not expand `macro_rules!`, so these two enums — never named in a
// `pub extern "C" fn` signature — never reach the generated one either).
// =================================================================================================
#define ZDDS_K_BOOL 0u
#define ZDDS_K_BYTE 1u
#define ZDDS_K_CHAR8 2u
#define ZDDS_K_CHAR16 3u
#define ZDDS_K_FLOAT32 4u
#define ZDDS_K_FLOAT64 5u
#define ZDDS_K_FLOAT128 6u
#define ZDDS_K_INT8 7u
#define ZDDS_K_UINT8 8u
#define ZDDS_K_INT16 9u
#define ZDDS_K_UINT16 10u
#define ZDDS_K_INT32 11u
#define ZDDS_K_UINT32 12u
#define ZDDS_K_INT64 13u
#define ZDDS_K_UINT64 14u
#define ZDDS_K_STRING8 15u
#define ZDDS_K_STRING16 16u

#define ZDDS_SHAPE_SCALAR 0u
#define ZDDS_SHAPE_ARRAY 1u
#define ZDDS_SHAPE_BOUNDED_SEQ 2u
#define ZDDS_SHAPE_UNBOUNDED_SEQ 3u

// The x86-64 80-bit-extended-padded-to-16-bytes layout `long double`
// occupies there, versus true binary128 (aarch64 glibc). See the module
// note; `Float128::{to,from}_x87_padded_bytes`/`{to,from}_le_bytes` do
// the real conversion, entirely on the Rust side.
#if defined(__x86_64__) || defined(__i386__)
#define ZDDS_DYN_F128_GET zerodds_dyn_data_get_float128_x87
#define ZDDS_DYN_F128_SET zerodds_dyn_data_set_float128_x87
#define ZDDS_DYN_F128_INSERT zerodds_dyn_data_insert_float128_x87
#elif defined(__aarch64__)
#define ZDDS_DYN_F128_GET zerodds_dyn_data_get_float128_ieee
#define ZDDS_DYN_F128_SET zerodds_dyn_data_set_float128_ieee
#define ZDDS_DYN_F128_INSERT zerodds_dyn_data_insert_float128_ieee
#else
// Neither layout this module bridges (32-bit ARM, PowerPC's
// double-double, …) — refused rather than guessed; see the module note.
#define ZDDS_DYN_F128_UNSUPPORTED 1
#endif

namespace
{

rcutils_ret_t to_rcutils(int32_t rc)
{
  // dynamic.rs's own `OK`/`ERR`/`INVALID` constants, which are already
  // exactly RCUTILS_RET_OK/RCUTILS_RET_ERROR/RCUTILS_RET_INVALID_ARGUMENT
  // — see that module's header comment.
  return static_cast<rcutils_ret_t>(rc);
}

const char ZERODDS_DYNAMIC_TYPESUPPORT_ID[] = "rmw_zerodds_cpp";

}  // namespace

// =================================================================================================
// serialization_support_impl_fini / interface_fini
// =================================================================================================
extern "C" {

static rcutils_ret_t zdds_support_impl_fini(
  rosidl_dynamic_typesupport_serialization_support_impl_t * impl)
{
  // No per-session state: every dynamic_type_builder_init/dynamic_data_
  // init_from_* call is self-sufficient (see dynamic.rs's module note on
  // why grafting replaces a shared model). `impl->handle` is never set.
  (void) impl;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_interface_fini(
  rosidl_dynamic_typesupport_serialization_support_interface_t * methods)
{
  (void) methods;
  return RCUTILS_RET_OK;
}

// =================================================================================================
// DYNAMIC TYPE — core
// =================================================================================================

static rcutils_ret_t zdds_type_equals(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_type_impl_t * type,
  const rosidl_dynamic_typesupport_dynamic_type_impl_t * other,
  bool * equals)
{
  (void) support;
  if (!type || !other || !equals) { return RCUTILS_RET_INVALID_ARGUMENT; }
  *equals = zerodds_dyn_type_equals(type->handle, other->handle) != 0;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_type_get_member_count(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_type_impl_t * type,
  size_t * member_count)
{
  (void) support;
  if (!type || !member_count) { return RCUTILS_RET_INVALID_ARGUMENT; }
  *member_count = static_cast<size_t>(zerodds_dyn_type_member_count(type->handle));
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_builder_init(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const char * name, size_t name_length,
  rcutils_allocator_t * allocator,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder)
{
  (void) support;
  (void) allocator;
  if (!builder) { return RCUTILS_RET_INVALID_ARGUMENT; }
  void * handle = zerodds_dyn_builder_init(name, name_length);
  if (!handle) { return RCUTILS_RET_ERROR; }
  builder->handle = handle;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_builder_clone(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * other,
  rcutils_allocator_t * allocator,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder)
{
  (void) support;
  (void) allocator;
  if (!other || !builder) { return RCUTILS_RET_INVALID_ARGUMENT; }
  void * handle = zerodds_dyn_builder_clone(other->handle);
  if (!handle) { return RCUTILS_RET_ERROR; }
  builder->handle = handle;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_builder_fini(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder)
{
  (void) support;
  if (!builder) { return RCUTILS_RET_INVALID_ARGUMENT; }
  zerodds_dyn_builder_free(builder->handle);
  builder->handle = nullptr;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_type_init_from_builder(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder,
  rcutils_allocator_t * allocator,
  rosidl_dynamic_typesupport_dynamic_type_impl_t * type)
{
  (void) support;
  (void) allocator;
  if (!builder || !type) { return RCUTILS_RET_INVALID_ARGUMENT; }
  void * handle = zerodds_dyn_builder_finish(builder->handle);
  // `finish` consumes the builder's Rust allocation regardless of
  // outcome (see dynamic.rs); the C-side handle is nulled either way so
  // a matching `dynamic_type_builder_fini` is a no-op, not a double-free.
  builder->handle = nullptr;
  if (!handle) { return RCUTILS_RET_ERROR; }
  type->handle = handle;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_type_clone(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_type_impl_t * other,
  rcutils_allocator_t * allocator,
  rosidl_dynamic_typesupport_dynamic_type_impl_t * type)
{
  (void) support;
  (void) allocator;
  if (!other || !type) { return RCUTILS_RET_INVALID_ARGUMENT; }
  void * handle = zerodds_dyn_type_clone(other->handle);
  if (!handle) { return RCUTILS_RET_ERROR; }
  type->handle = handle;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_type_fini(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_impl_t * type)
{
  (void) support;
  if (!type) { return RCUTILS_RET_INVALID_ARGUMENT; }
  zerodds_dyn_type_free(type->handle);
  type->handle = nullptr;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_type_get_name(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_type_impl_t * type,
  const char ** name, size_t * name_length)
{
  (void) support;
  if (!type) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_type_get_name(type->handle, name, name_length));
}

static rcutils_ret_t zdds_builder_get_name(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder,
  const char ** name, size_t * name_length)
{
  (void) support;
  if (!builder) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_builder_get_name(builder->handle, name, name_length));
}

static rcutils_ret_t zdds_builder_set_name(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder,
  const char * name, size_t name_length)
{
  (void) support;
  if (!builder) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_builder_set_name(builder->handle, name, name_length));
}

// =================================================================================================
// DYNAMIC TYPE — add member (primitive + string, every shape)
// =================================================================================================

static rcutils_ret_t zdds_add_member_impl(
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder,
  uint32_t kind, uint32_t shape, rosidl_dynamic_typesupport_member_id_t id,
  const char * name, size_t name_length, uint64_t str_bound, uint64_t container_bound,
  const char * default_value, size_t default_value_length)
{
  if (!builder) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_builder_add_member(
    builder->handle, kind, shape, static_cast<uint64_t>(id), name, name_length, str_bound,
    container_bound, default_value, default_value_length));
}

// One primitive kind, all four shapes. `default_value`/`default_value_
// length` reach `zerodds_dyn_builder_add_member`, which translates and
// attaches them — see dynamic.rs's module note.
#define ZDDS_ADD_PRIMITIVE_FAMILY(suffix, kind) \
  static rcutils_ret_t zdds_add_##suffix##_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_SCALAR, id, name, name_length, 0, 0, default_value, \
      default_value_length); \
  } \
  static rcutils_ret_t zdds_add_##suffix##_array_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length, size_t array_length) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_ARRAY, id, name, name_length, 0, array_length, default_value, \
      default_value_length); \
  } \
  static rcutils_ret_t zdds_add_##suffix##_bounded_sequence_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length, size_t sequence_bound) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_BOUNDED_SEQ, id, name, name_length, 0, sequence_bound, \
      default_value, default_value_length); \
  } \
  static rcutils_ret_t zdds_add_##suffix##_unbounded_sequence_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_UNBOUNDED_SEQ, id, name, name_length, 0, 0, default_value, \
      default_value_length); \
  }

ZDDS_ADD_PRIMITIVE_FAMILY(bool, ZDDS_K_BOOL)
ZDDS_ADD_PRIMITIVE_FAMILY(byte, ZDDS_K_BYTE)
ZDDS_ADD_PRIMITIVE_FAMILY(char, ZDDS_K_CHAR8)
ZDDS_ADD_PRIMITIVE_FAMILY(wchar, ZDDS_K_CHAR16)
ZDDS_ADD_PRIMITIVE_FAMILY(float32, ZDDS_K_FLOAT32)
ZDDS_ADD_PRIMITIVE_FAMILY(float64, ZDDS_K_FLOAT64)
ZDDS_ADD_PRIMITIVE_FAMILY(float128, ZDDS_K_FLOAT128)
ZDDS_ADD_PRIMITIVE_FAMILY(int8, ZDDS_K_INT8)
ZDDS_ADD_PRIMITIVE_FAMILY(uint8, ZDDS_K_UINT8)
ZDDS_ADD_PRIMITIVE_FAMILY(int16, ZDDS_K_INT16)
ZDDS_ADD_PRIMITIVE_FAMILY(uint16, ZDDS_K_UINT16)
ZDDS_ADD_PRIMITIVE_FAMILY(int32, ZDDS_K_INT32)
ZDDS_ADD_PRIMITIVE_FAMILY(uint32, ZDDS_K_UINT32)
ZDDS_ADD_PRIMITIVE_FAMILY(int64, ZDDS_K_INT64)
ZDDS_ADD_PRIMITIVE_FAMILY(uint64, ZDDS_K_UINT64)

#undef ZDDS_ADD_PRIMITIVE_FAMILY

// One string kind (string8/string16), all four shapes and all three
// bound variants (plain/fixed/bounded — see the crate README: this
// interface's `fixed_*` and `bounded_*` member families both reach a
// `string<N>`/`wstring<N>` identically, since XTypes has one such
// construct and not two). `default_value`/`default_value_length` reach
// `zerodds_dyn_builder_add_member` unchanged, exactly as a `string`'s
// own default text already reads in this crate's `@default` parser.
#define ZDDS_ADD_STRING_FAMILY(suffix, kind) \
  static rcutils_ret_t zdds_add_##suffix##_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_SCALAR, id, name, name_length, 0, 0, default_value, \
      default_value_length); \
  } \
  static rcutils_ret_t zdds_add_fixed_##suffix##_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length, size_t string_length) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_SCALAR, id, name, name_length, string_length, 0, default_value, \
      default_value_length); \
  } \
  static rcutils_ret_t zdds_add_bounded_##suffix##_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length, size_t string_bound) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_SCALAR, id, name, name_length, string_bound, 0, default_value, \
      default_value_length); \
  } \
  static rcutils_ret_t zdds_add_##suffix##_array_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length, size_t array_length) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_ARRAY, id, name, name_length, 0, array_length, default_value, \
      default_value_length); \
  } \
  static rcutils_ret_t zdds_add_fixed_##suffix##_array_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length, size_t string_length, \
    size_t array_length) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_ARRAY, id, name, name_length, string_length, array_length, \
      default_value, default_value_length); \
  } \
  static rcutils_ret_t zdds_add_bounded_##suffix##_array_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length, size_t string_bound, \
    size_t array_length) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_ARRAY, id, name, name_length, string_bound, array_length, \
      default_value, default_value_length); \
  } \
  static rcutils_ret_t zdds_add_##suffix##_bounded_sequence_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length, size_t sequence_bound) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_BOUNDED_SEQ, id, name, name_length, 0, sequence_bound, \
      default_value, default_value_length); \
  } \
  static rcutils_ret_t zdds_add_fixed_##suffix##_bounded_sequence_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length, size_t string_length, \
    size_t sequence_bound) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_BOUNDED_SEQ, id, name, name_length, string_length, \
      sequence_bound, default_value, default_value_length); \
  } \
  static rcutils_ret_t zdds_add_bounded_##suffix##_bounded_sequence_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length, size_t string_bound, \
    size_t sequence_bound) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_BOUNDED_SEQ, id, name, name_length, string_bound, \
      sequence_bound, default_value, default_value_length); \
  } \
  static rcutils_ret_t zdds_add_##suffix##_unbounded_sequence_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_UNBOUNDED_SEQ, id, name, name_length, 0, 0, default_value, \
      default_value_length); \
  } \
  static rcutils_ret_t zdds_add_fixed_##suffix##_unbounded_sequence_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length, size_t string_length) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_UNBOUNDED_SEQ, id, name, name_length, string_length, 0, \
      default_value, default_value_length); \
  } \
  static rcutils_ret_t zdds_add_bounded_##suffix##_unbounded_sequence_member( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder, \
    rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length, \
    const char * default_value, size_t default_value_length, size_t string_bound) \
  { \
    (void) support; \
    return zdds_add_member_impl( \
      builder, kind, ZDDS_SHAPE_UNBOUNDED_SEQ, id, name, name_length, string_bound, 0, \
      default_value, default_value_length); \
  }

ZDDS_ADD_STRING_FAMILY(string, ZDDS_K_STRING8)
ZDDS_ADD_STRING_FAMILY(wstring, ZDDS_K_STRING16)

#undef ZDDS_ADD_STRING_FAMILY

// -- nested / complex members --

static rcutils_ret_t zdds_add_complex_member(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder,
  rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length,
  const char * default_value, size_t default_value_length,
  rosidl_dynamic_typesupport_dynamic_type_impl_t * nested_struct)
{
  (void) support; (void) default_value; (void) default_value_length;
  if (!builder || !nested_struct) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_builder_add_nested_member(
    builder->handle, ZDDS_SHAPE_SCALAR, static_cast<uint64_t>(id), name, name_length, 0,
    nested_struct->handle));
}

static rcutils_ret_t zdds_add_complex_array_member(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder,
  rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length,
  const char * default_value, size_t default_value_length,
  rosidl_dynamic_typesupport_dynamic_type_impl_t * nested_struct, size_t array_length)
{
  (void) support; (void) default_value; (void) default_value_length;
  if (!builder || !nested_struct) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_builder_add_nested_member(
    builder->handle, ZDDS_SHAPE_ARRAY, static_cast<uint64_t>(id), name, name_length,
    array_length, nested_struct->handle));
}

static rcutils_ret_t zdds_add_complex_unbounded_sequence_member(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder,
  rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length,
  const char * default_value, size_t default_value_length,
  rosidl_dynamic_typesupport_dynamic_type_impl_t * nested_struct)
{
  (void) support; (void) default_value; (void) default_value_length;
  if (!builder || !nested_struct) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_builder_add_nested_member(
    builder->handle, ZDDS_SHAPE_UNBOUNDED_SEQ, static_cast<uint64_t>(id), name, name_length, 0,
    nested_struct->handle));
}

static rcutils_ret_t zdds_add_complex_bounded_sequence_member(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder,
  rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length,
  const char * default_value, size_t default_value_length,
  rosidl_dynamic_typesupport_dynamic_type_impl_t * nested_struct, size_t sequence_bound)
{
  (void) support; (void) default_value; (void) default_value_length;
  if (!builder || !nested_struct) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_builder_add_nested_member(
    builder->handle, ZDDS_SHAPE_BOUNDED_SEQ, static_cast<uint64_t>(id), name, name_length,
    sequence_bound, nested_struct->handle));
}

static rcutils_ret_t zdds_add_complex_member_builder(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder,
  rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length,
  const char * default_value, size_t default_value_length,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * nested_struct_builder)
{
  (void) support; (void) default_value; (void) default_value_length;
  if (!builder || !nested_struct_builder) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_builder_add_nested_member_from_builder(
    builder->handle, ZDDS_SHAPE_SCALAR, static_cast<uint64_t>(id), name, name_length, 0,
    nested_struct_builder->handle));
}

static rcutils_ret_t zdds_add_complex_array_member_builder(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder,
  rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length,
  const char * default_value, size_t default_value_length,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * nested_struct_builder,
  size_t array_length)
{
  (void) support; (void) default_value; (void) default_value_length;
  if (!builder || !nested_struct_builder) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_builder_add_nested_member_from_builder(
    builder->handle, ZDDS_SHAPE_ARRAY, static_cast<uint64_t>(id), name, name_length,
    array_length, nested_struct_builder->handle));
}

static rcutils_ret_t zdds_add_complex_unbounded_sequence_member_builder(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder,
  rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length,
  const char * default_value, size_t default_value_length,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * nested_struct_builder)
{
  (void) support; (void) default_value; (void) default_value_length;
  if (!builder || !nested_struct_builder) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_builder_add_nested_member_from_builder(
    builder->handle, ZDDS_SHAPE_UNBOUNDED_SEQ, static_cast<uint64_t>(id), name, name_length, 0,
    nested_struct_builder->handle));
}

static rcutils_ret_t zdds_add_complex_bounded_sequence_member_builder(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder,
  rosidl_dynamic_typesupport_member_id_t id, const char * name, size_t name_length,
  const char * default_value, size_t default_value_length,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * nested_struct_builder,
  size_t sequence_bound)
{
  (void) support; (void) default_value; (void) default_value_length;
  if (!builder || !nested_struct_builder) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_builder_add_nested_member_from_builder(
    builder->handle, ZDDS_SHAPE_BOUNDED_SEQ, static_cast<uint64_t>(id), name, name_length,
    sequence_bound, nested_struct_builder->handle));
}

// =================================================================================================
// DYNAMIC DATA — utils / construction / (de)serialize
// =================================================================================================

static rcutils_ret_t zdds_data_clear_all_values(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data)
{
  (void) support;
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_data_clear_all_values(data->handle));
}

// XTypes 1.3 §7.5.2.11.3: clear_nonkey_values is clear_all_values except
// that the key members keep their values.
static rcutils_ret_t zdds_data_clear_nonkey_values(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data)
{
  (void) support;
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_data_clear_nonkey_values(data->handle));
}

static rcutils_ret_t zdds_data_clear_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id)
{
  (void) support;
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_data_clear_value(data->handle, static_cast<uint64_t>(id)));
}

static rcutils_ret_t zdds_data_equals(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * other,
  bool * equals)
{
  (void) support;
  if (!data || !other || !equals) { return RCUTILS_RET_INVALID_ARGUMENT; }
  *equals = zerodds_dyn_data_equals(data->handle, other->handle) != 0;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_data_get_item_count(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  size_t * item_count)
{
  (void) support;
  if (!data || !item_count) { return RCUTILS_RET_INVALID_ARGUMENT; }
  *item_count = static_cast<size_t>(zerodds_dyn_data_item_count(data->handle));
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_data_get_member_id_by_name(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  const char * name, size_t name_length,
  rosidl_dynamic_typesupport_member_id_t * member_id)
{
  (void) support;
  if (!data || !member_id) { return RCUTILS_RET_INVALID_ARGUMENT; }
  uint64_t id = 0;
  rcutils_ret_t rc =
    to_rcutils(zerodds_dyn_data_member_id_by_name(data->handle, name, name_length, &id));
  if (rc == RCUTILS_RET_OK) { *member_id = static_cast<rosidl_dynamic_typesupport_member_id_t>(id); }
  return rc;
}

static rcutils_ret_t zdds_data_get_member_id_at_index(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  size_t index, rosidl_dynamic_typesupport_member_id_t * member_id)
{
  (void) support;
  if (!data || !member_id) { return RCUTILS_RET_INVALID_ARGUMENT; }
  uint64_t id = 0;
  rcutils_ret_t rc = to_rcutils(zerodds_dyn_data_id_at_index(data->handle, index, &id));
  if (rc == RCUTILS_RET_OK) { *member_id = static_cast<rosidl_dynamic_typesupport_member_id_t>(id); }
  return rc;
}

static rcutils_ret_t zdds_data_get_array_index(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  size_t index, rosidl_dynamic_typesupport_member_id_t * array_index)
{
  // A collection's member id *is* its element index — dynamic.rs's
  // `zerodds_dyn_data_id_at_index` already answers both this and
  // `get_member_id_at_index` with the same rule.
  return zdds_data_get_member_id_at_index(support, data, index, array_index);
}

static rcutils_ret_t zdds_data_get_name(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  const char ** name, size_t * name_length)
{
  (void) support;
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_data_get_name(data->handle, name, name_length));
}

static rcutils_ret_t zdds_data_init_from_builder(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_builder_impl_t * builder,
  rcutils_allocator_t * allocator,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data)
{
  (void) support; (void) allocator;
  if (!builder || !data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  void * handle = zerodds_dyn_data_from_builder(builder->handle);
  builder->handle = nullptr;  // consumed either way, see zdds_type_init_from_builder
  if (!handle) { return RCUTILS_RET_ERROR; }
  data->handle = handle;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_data_init_from_type(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_type_impl_t * type,
  rcutils_allocator_t * allocator,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data)
{
  (void) support; (void) allocator;
  if (!type || !data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  void * handle = zerodds_dyn_data_from_type(type->handle);
  if (!handle) { return RCUTILS_RET_ERROR; }
  data->handle = handle;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_data_clone(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * other,
  rcutils_allocator_t * allocator,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data)
{
  (void) support; (void) allocator;
  if (!other || !data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  void * handle = zerodds_dyn_data_clone(other->handle);
  if (!handle) { return RCUTILS_RET_ERROR; }
  data->handle = handle;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_data_fini(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data)
{
  (void) support;
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  zerodds_dyn_data_free(data->handle);
  data->handle = nullptr;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_data_serialize(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rcutils_uint8_array_t * buffer)
{
  (void) support;
  if (!data || !buffer) { return RCUTILS_RET_INVALID_ARGUMENT; }
  uint8_t * bytes = nullptr;
  size_t len = 0;
  int32_t rc = zerodds_dyn_data_serialize(data->handle, &bytes, &len);
  if (rc != 0) { return to_rcutils(rc); }
  if (buffer->buffer_capacity < len) {
    uint8_t * grown = static_cast<uint8_t *>(
      buffer->allocator.reallocate(buffer->buffer, len, buffer->allocator.state));
    if (!grown) {
      zerodds_dyn_bytes_free(bytes, len);
      return RCUTILS_RET_BAD_ALLOC;
    }
    buffer->buffer = grown;
    buffer->buffer_capacity = len;
  }
  std::memcpy(buffer->buffer, bytes, len);
  buffer->buffer_length = len;
  zerodds_dyn_bytes_free(bytes, len);
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_data_deserialize(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rcutils_uint8_array_t * buffer)
{
  (void) support;
  if (!data || !buffer) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(
    zerodds_dyn_data_deserialize(data->handle, buffer->buffer, buffer->buffer_length));
}

// =================================================================================================
// DYNAMIC DATA — scalar getters / setters / inserts
// =================================================================================================

static rcutils_ret_t zdds_data_get_bool_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, bool * value)
{
  (void) support;
  if (!data || !value) { return RCUTILS_RET_INVALID_ARGUMENT; }
  uint8_t out = 0;
  rcutils_ret_t rc =
    to_rcutils(zerodds_dyn_data_get_bool(data->handle, static_cast<uint64_t>(id), &out));
  if (rc == RCUTILS_RET_OK) { *value = out != 0; }
  return rc;
}

static rcutils_ret_t zdds_data_set_bool_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, bool value)
{
  (void) support;
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(
    zerodds_dyn_data_set_bool(data->handle, static_cast<uint64_t>(id), value ? 1 : 0));
}

static rcutils_ret_t zdds_data_insert_bool_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data, bool value,
  rosidl_dynamic_typesupport_member_id_t * out_id)
{
  (void) support;
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  uint64_t id = 0;
  rcutils_ret_t rc =
    to_rcutils(zerodds_dyn_data_insert_bool(data->handle, value ? 1 : 0, &id));
  if (rc == RCUTILS_RET_OK && out_id) {
    *out_id = static_cast<rosidl_dynamic_typesupport_member_id_t>(id);
  }
  return rc;
}

#define ZDDS_DATA_SCALAR_FAMILY(suffix, ctype) \
  static rcutils_ret_t zdds_data_get_##suffix##_value( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    const rosidl_dynamic_typesupport_dynamic_data_impl_t * data, \
    rosidl_dynamic_typesupport_member_id_t id, ctype * value) \
  { \
    (void) support; \
    if (!data || !value) { return RCUTILS_RET_INVALID_ARGUMENT; } \
    return to_rcutils( \
      zerodds_dyn_data_get_##suffix(data->handle, static_cast<uint64_t>(id), value)); \
  } \
  static rcutils_ret_t zdds_data_set_##suffix##_value( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_data_impl_t * data, \
    rosidl_dynamic_typesupport_member_id_t id, ctype value) \
  { \
    (void) support; \
    if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; } \
    return to_rcutils( \
      zerodds_dyn_data_set_##suffix(data->handle, static_cast<uint64_t>(id), value)); \
  } \
  static rcutils_ret_t zdds_data_insert_##suffix##_value( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_data_impl_t * data, ctype value, \
    rosidl_dynamic_typesupport_member_id_t * out_id) \
  { \
    (void) support; \
    if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; } \
    uint64_t id = 0; \
    rcutils_ret_t rc = to_rcutils(zerodds_dyn_data_insert_##suffix(data->handle, value, &id)); \
    if (rc == RCUTILS_RET_OK && out_id) { \
      *out_id = static_cast<rosidl_dynamic_typesupport_member_id_t>(id); \
    } \
    return rc; \
  }

ZDDS_DATA_SCALAR_FAMILY(int8, int8_t)
ZDDS_DATA_SCALAR_FAMILY(uint8, uint8_t)
ZDDS_DATA_SCALAR_FAMILY(byte, uint8_t)
ZDDS_DATA_SCALAR_FAMILY(int16, int16_t)
ZDDS_DATA_SCALAR_FAMILY(uint16, uint16_t)
ZDDS_DATA_SCALAR_FAMILY(int32, int32_t)
ZDDS_DATA_SCALAR_FAMILY(uint32, uint32_t)
ZDDS_DATA_SCALAR_FAMILY(int64, int64_t)
ZDDS_DATA_SCALAR_FAMILY(uint64, uint64_t)
ZDDS_DATA_SCALAR_FAMILY(float32, float)
ZDDS_DATA_SCALAR_FAMILY(float64, double)

#undef ZDDS_DATA_SCALAR_FAMILY

// char/wchar: the interface's `char`/`char16_t` versus dynamic.rs's
// UTF-32 code-point `uint32_t` getter/setter.
#define ZDDS_DATA_CHAR_FAMILY(suffix, rust_suffix, ctype) \
  static rcutils_ret_t zdds_data_get_##suffix##_value( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    const rosidl_dynamic_typesupport_dynamic_data_impl_t * data, \
    rosidl_dynamic_typesupport_member_id_t id, ctype * value) \
  { \
    (void) support; \
    if (!data || !value) { return RCUTILS_RET_INVALID_ARGUMENT; } \
    uint32_t out = 0; \
    rcutils_ret_t rc = to_rcutils( \
      zerodds_dyn_data_get_##rust_suffix(data->handle, static_cast<uint64_t>(id), &out)); \
    if (rc == RCUTILS_RET_OK) { *value = static_cast<ctype>(out); } \
    return rc; \
  } \
  static rcutils_ret_t zdds_data_set_##suffix##_value( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_data_impl_t * data, \
    rosidl_dynamic_typesupport_member_id_t id, ctype value) \
  { \
    (void) support; \
    if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; } \
    return to_rcutils(zerodds_dyn_data_set_##rust_suffix( \
      data->handle, static_cast<uint64_t>(id), static_cast<uint32_t>(value))); \
  } \
  static rcutils_ret_t zdds_data_insert_##suffix##_value( \
    rosidl_dynamic_typesupport_serialization_support_impl_t * support, \
    rosidl_dynamic_typesupport_dynamic_data_impl_t * data, ctype value, \
    rosidl_dynamic_typesupport_member_id_t * out_id) \
  { \
    (void) support; \
    if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; } \
    uint64_t id = 0; \
    rcutils_ret_t rc = to_rcutils(zerodds_dyn_data_insert_##rust_suffix( \
      data->handle, static_cast<uint32_t>(value), &id)); \
    if (rc == RCUTILS_RET_OK && out_id) { \
      *out_id = static_cast<rosidl_dynamic_typesupport_member_id_t>(id); \
    } \
    return rc; \
  }

ZDDS_DATA_CHAR_FAMILY(char, char, char)
ZDDS_DATA_CHAR_FAMILY(wchar, wchar, char16_t)

#undef ZDDS_DATA_CHAR_FAMILY

// float128: the two host layouts, chosen at compile time — see
// `ZDDS_DYN_F128_GET`/`_SET`/`_INSERT` above.
static rcutils_ret_t zdds_data_get_float128_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, long double * value)
{
  (void) support;
#if defined(ZDDS_DYN_F128_UNSUPPORTED)
  (void) data; (void) id; (void) value;
  return RCUTILS_RET_UNSUPPORTED;
#else
  if (!data || !value) { return RCUTILS_RET_INVALID_ARGUMENT; }
  uint8_t bytes[16];
  rcutils_ret_t rc = to_rcutils(
    ZDDS_DYN_F128_GET(data->handle, static_cast<uint64_t>(id), bytes));
  if (rc == RCUTILS_RET_OK) { std::memcpy(value, bytes, sizeof(*value) < 16 ? sizeof(*value) : 16); }
  return rc;
#endif
}

static rcutils_ret_t zdds_data_set_float128_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, long double value)
{
  (void) support;
#if defined(ZDDS_DYN_F128_UNSUPPORTED)
  (void) data; (void) id; (void) value;
  return RCUTILS_RET_UNSUPPORTED;
#else
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  uint8_t bytes[16] = {0};
  std::memcpy(bytes, &value, sizeof(value) < 16 ? sizeof(value) : 16);
  return to_rcutils(ZDDS_DYN_F128_SET(data->handle, static_cast<uint64_t>(id), bytes));
#endif
}

static rcutils_ret_t zdds_data_insert_float128_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data, long double value,
  rosidl_dynamic_typesupport_member_id_t * out_id)
{
  (void) support;
#if defined(ZDDS_DYN_F128_UNSUPPORTED)
  (void) data; (void) value; (void) out_id;
  return RCUTILS_RET_UNSUPPORTED;
#else
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  uint8_t bytes[16] = {0};
  std::memcpy(bytes, &value, sizeof(value) < 16 ? sizeof(value) : 16);
  uint64_t id = 0;
  rcutils_ret_t rc = to_rcutils(ZDDS_DYN_F128_INSERT(data->handle, bytes, &id));
  if (rc == RCUTILS_RET_OK && out_id) {
    *out_id = static_cast<rosidl_dynamic_typesupport_member_id_t>(id);
  }
  return rc;
#endif
}

// =================================================================================================
// DYNAMIC DATA — strings (plain / fixed / bounded, narrow + wide)
// =================================================================================================

static rcutils_ret_t zdds_data_get_string_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, char ** value, size_t * value_length)
{
  (void) support;
  if (!data || !value || !value_length) { return RCUTILS_RET_INVALID_ARGUMENT; }
  const char * borrowed = nullptr;
  size_t len = 0;
  rcutils_ret_t rc = to_rcutils(zerodds_dyn_data_get_string(
    const_cast<void *>(data->handle), static_cast<uint64_t>(id), &borrowed, &len));
  if (rc == RCUTILS_RET_OK) { *value = const_cast<char *>(borrowed); *value_length = len; }
  return rc;
}

static rcutils_ret_t zdds_data_set_string_value_impl(
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, const char * value, size_t value_length,
  size_t bound)
{
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_data_set_string(
    const_cast<void *>(data->handle), static_cast<uint64_t>(id),
    reinterpret_cast<const uint8_t *>(value), value_length, bound));
}

static rcutils_ret_t zdds_data_set_string_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, const char * value, size_t value_length)
{
  (void) support;
  return zdds_data_set_string_value_impl(data, id, value, value_length, 0);
}

static rcutils_ret_t zdds_data_set_fixed_string_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, const char * value, size_t value_length,
  size_t string_length)
{
  (void) support;
  return zdds_data_set_string_value_impl(data, id, value, value_length, string_length);
}

static rcutils_ret_t zdds_data_set_bounded_string_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, const char * value, size_t value_length,
  size_t string_bound)
{
  (void) support;
  return zdds_data_set_string_value_impl(data, id, value, value_length, string_bound);
}

static rcutils_ret_t zdds_data_insert_string_value_impl(
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data, const char * value, size_t value_length,
  size_t bound, rosidl_dynamic_typesupport_member_id_t * out_id)
{
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  uint64_t id = 0;
  rcutils_ret_t rc = to_rcutils(zerodds_dyn_data_insert_string(
    data->handle, reinterpret_cast<const uint8_t *>(value), value_length, bound, &id));
  if (rc == RCUTILS_RET_OK && out_id) {
    *out_id = static_cast<rosidl_dynamic_typesupport_member_id_t>(id);
  }
  return rc;
}

static rcutils_ret_t zdds_data_insert_string_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data, const char * value, size_t value_length,
  rosidl_dynamic_typesupport_member_id_t * out_id)
{
  (void) support;
  return zdds_data_insert_string_value_impl(data, value, value_length, 0, out_id);
}

static rcutils_ret_t zdds_data_insert_fixed_string_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data, const char * value, size_t value_length,
  size_t string_length, rosidl_dynamic_typesupport_member_id_t * out_id)
{
  (void) support;
  return zdds_data_insert_string_value_impl(data, value, value_length, string_length, out_id);
}

static rcutils_ret_t zdds_data_insert_bounded_string_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data, const char * value, size_t value_length,
  size_t string_bound, rosidl_dynamic_typesupport_member_id_t * out_id)
{
  (void) support;
  return zdds_data_insert_string_value_impl(data, value, value_length, string_bound, out_id);
}

static rcutils_ret_t zdds_data_get_wstring_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, char16_t ** value, size_t * value_length)
{
  (void) support;
  if (!data || !value || !value_length) { return RCUTILS_RET_INVALID_ARGUMENT; }
  const uint16_t * borrowed = nullptr;
  size_t len = 0;
  rcutils_ret_t rc = to_rcutils(zerodds_dyn_data_get_wstring(
    const_cast<void *>(data->handle), static_cast<uint64_t>(id), &borrowed, &len));
  if (rc == RCUTILS_RET_OK) {
    *value = reinterpret_cast<char16_t *>(const_cast<uint16_t *>(borrowed));
    *value_length = len;
  }
  return rc;
}

static rcutils_ret_t zdds_data_set_wstring_value_impl(
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, const char16_t * value, size_t value_length,
  size_t bound)
{
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_data_set_wstring(
    const_cast<void *>(data->handle), static_cast<uint64_t>(id),
    reinterpret_cast<const uint16_t *>(value), value_length, bound));
}

static rcutils_ret_t zdds_data_set_wstring_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, const char16_t * value, size_t value_length)
{
  (void) support;
  return zdds_data_set_wstring_value_impl(data, id, value, value_length, 0);
}

static rcutils_ret_t zdds_data_set_fixed_wstring_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, const char16_t * value, size_t value_length,
  size_t wstring_length)
{
  (void) support;
  return zdds_data_set_wstring_value_impl(data, id, value, value_length, wstring_length);
}

static rcutils_ret_t zdds_data_set_bounded_wstring_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, const char16_t * value, size_t value_length,
  size_t wstring_bound)
{
  (void) support;
  return zdds_data_set_wstring_value_impl(data, id, value, value_length, wstring_bound);
}

static rcutils_ret_t zdds_data_insert_wstring_value_impl(
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data, const char16_t * value,
  size_t value_length, size_t bound, rosidl_dynamic_typesupport_member_id_t * out_id)
{
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  uint64_t id = 0;
  rcutils_ret_t rc = to_rcutils(zerodds_dyn_data_insert_wstring(
    data->handle, reinterpret_cast<const uint16_t *>(value), value_length, bound, &id));
  if (rc == RCUTILS_RET_OK && out_id) {
    *out_id = static_cast<rosidl_dynamic_typesupport_member_id_t>(id);
  }
  return rc;
}

static rcutils_ret_t zdds_data_insert_wstring_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data, const char16_t * value,
  size_t value_length, rosidl_dynamic_typesupport_member_id_t * out_id)
{
  (void) support;
  return zdds_data_insert_wstring_value_impl(data, value, value_length, 0, out_id);
}

static rcutils_ret_t zdds_data_insert_fixed_wstring_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data, const char16_t * value,
  size_t value_length, size_t wstring_length,
  rosidl_dynamic_typesupport_member_id_t * out_id)
{
  (void) support;
  return zdds_data_insert_wstring_value_impl(data, value, value_length, wstring_length, out_id);
}

static rcutils_ret_t zdds_data_insert_bounded_wstring_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data, const char16_t * value,
  size_t value_length, size_t wstring_bound,
  rosidl_dynamic_typesupport_member_id_t * out_id)
{
  (void) support;
  return zdds_data_insert_wstring_value_impl(data, value, value_length, wstring_bound, out_id);
}

// The interface declares `get_fixed_string_value`/`get_bounded_string_value`
// (and the wide kin) as their own function slots, with a `string_length`/
// `string_bound` parameter the plain getter lacks. Reading never needs to
// enforce that bound — only writing does, see `zdds_data_set_string_value_
// impl` — so all three read back identically.
static rcutils_ret_t zdds_data_get_fixed_string_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, char ** value, size_t * value_length,
  size_t string_length)
{
  (void) string_length;
  return zdds_data_get_string_value(support, data, id, value, value_length);
}

static rcutils_ret_t zdds_data_get_bounded_string_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, char ** value, size_t * value_length,
  size_t string_bound)
{
  (void) string_bound;
  return zdds_data_get_string_value(support, data, id, value, value_length);
}

static rcutils_ret_t zdds_data_get_fixed_wstring_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, char16_t ** value, size_t * value_length,
  size_t wstring_length)
{
  (void) wstring_length;
  return zdds_data_get_wstring_value(support, data, id, value, value_length);
}

static rcutils_ret_t zdds_data_get_bounded_wstring_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, char16_t ** value, size_t * value_length,
  size_t wstring_bound)
{
  (void) wstring_bound;
  return zdds_data_get_wstring_value(support, data, id, value, value_length);
}

// =================================================================================================
// DYNAMIC DATA — sequences
// =================================================================================================

static rcutils_ret_t zdds_data_clear_sequence_data(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data)
{
  (void) support;
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_data_seq_clear(data->handle));
}

static rcutils_ret_t zdds_data_remove_sequence_data(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id)
{
  (void) support;
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(zerodds_dyn_data_seq_remove(data->handle, static_cast<uint64_t>(id)));
}

static rcutils_ret_t zdds_data_insert_sequence_data(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t * out_id)
{
  (void) support;
  if (!data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  uint64_t id = 0;
  rcutils_ret_t rc = to_rcutils(zerodds_dyn_data_seq_insert_default(data->handle, &id));
  if (rc == RCUTILS_RET_OK && out_id) {
    *out_id = static_cast<rosidl_dynamic_typesupport_member_id_t>(id);
  }
  return rc;
}

// =================================================================================================
// DYNAMIC DATA — nested
// =================================================================================================

static rcutils_ret_t zdds_data_get_complex_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, rcutils_allocator_t * allocator,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * value)
{
  (void) support; (void) allocator;
  if (!data || !value) { return RCUTILS_RET_INVALID_ARGUMENT; }
  void * handle = zerodds_dyn_data_get_complex(data->handle, static_cast<uint64_t>(id));
  if (!handle) { return RCUTILS_RET_ERROR; }
  value->handle = handle;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_data_set_complex_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * value)
{
  (void) support;
  if (!data || !value) { return RCUTILS_RET_INVALID_ARGUMENT; }
  return to_rcutils(
    zerodds_dyn_data_set_complex(data->handle, static_cast<uint64_t>(id), value->handle));
}

static rcutils_ret_t zdds_data_insert_complex_value_copy(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * value,
  rosidl_dynamic_typesupport_member_id_t * out_id)
{
  (void) support;
  if (!data || !value) { return RCUTILS_RET_INVALID_ARGUMENT; }
  uint64_t id = 0;
  rcutils_ret_t rc =
    to_rcutils(zerodds_dyn_data_insert_complex_copy(data->handle, value->handle, &id));
  if (rc == RCUTILS_RET_OK && out_id) {
    *out_id = static_cast<rosidl_dynamic_typesupport_member_id_t>(id);
  }
  return rc;
}

static rcutils_ret_t zdds_data_insert_complex_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * value,
  rosidl_dynamic_typesupport_member_id_t * out_id)
{
  (void) support;
  if (!data || !value) { return RCUTILS_RET_INVALID_ARGUMENT; }
  uint64_t id = 0;
  rcutils_ret_t rc = to_rcutils(zerodds_dyn_data_insert_complex(data->handle, value->handle, &id));
  value->handle = nullptr;  // consumed by the Rust call regardless of outcome
  if (rc == RCUTILS_RET_OK && out_id) {
    *out_id = static_cast<rosidl_dynamic_typesupport_member_id_t>(id);
  }
  return rc;
}

static rcutils_ret_t zdds_data_loan_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  rosidl_dynamic_typesupport_member_id_t id, rcutils_allocator_t * allocator,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * loaned_dynamic_data)
{
  (void) support; (void) allocator;
  if (!data || !loaned_dynamic_data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  void * handle = zerodds_dyn_data_loan(data->handle, static_cast<uint64_t>(id));
  if (!handle) { return RCUTILS_RET_ERROR; }
  loaned_dynamic_data->handle = handle;
  return RCUTILS_RET_OK;
}

static rcutils_ret_t zdds_data_return_loaned_value(
  rosidl_dynamic_typesupport_serialization_support_impl_t * support,
  rosidl_dynamic_typesupport_dynamic_data_impl_t * data,
  const rosidl_dynamic_typesupport_dynamic_data_impl_t * inner_data)
{
  (void) support;
  if (!data || !inner_data) { return RCUTILS_RET_INVALID_ARGUMENT; }
  // The interface's own signature carries no member id here — only the
  // parent and the loaned value — so the loaned value has to remember
  // where it came from. `zerodds_dyn_data_loan` tags it
  // (`ZddsDynData::loan_origin`); `zerodds_dyn_data_return_loan` reads
  // the tag, writes the (possibly mutated) value back to that slot in
  // `data`, and frees `inner_data`'s Rust allocation. A value this
  // backend did not itself loan (no tag) is refused rather than written
  // to slot 0 by default.
  auto * mutable_inner = const_cast<rosidl_dynamic_typesupport_dynamic_data_impl_t *>(inner_data);
  rcutils_ret_t rc = to_rcutils(zerodds_dyn_data_return_loan(data->handle, mutable_inner->handle));
  // Consumed by the Rust call above regardless of outcome (freed on
  // success; on the one failure path — a value this backend did not
  // itself loan — nothing else references it either, so it is still
  // freed rather than leaked). Nulled so a caller's later
  // `dynamic_data_fini` on `inner_data`'s owning struct is a no-op, not
  // a double free — the same pattern `zdds_type_init_from_builder` uses.
  mutable_inner->handle = nullptr;
  return rc;
}

}  // extern "C"

// =================================================================================================
// Table assembly
// =================================================================================================
namespace
{

// Kilted removed the float128 entries from the serialization support
// interface (Jazzy has them). Whether the distro's interface has them is
// asked of the type, so one source builds on both.
template<typename I, typename = void>
struct zdds_has_float128 : std::false_type {};
template<typename I>
struct zdds_has_float128<I,
  std::void_t<decltype(&I::dynamic_type_builder_add_float128_member)>>
  : std::true_type {};

template<typename I>
void zdds_set_float128(I & methods)
{
  if constexpr (zdds_has_float128<I>::value) {
    methods.dynamic_type_builder_add_float128_member = zdds_add_float128_member;
    methods.dynamic_type_builder_add_float128_array_member = zdds_add_float128_array_member;
    methods.dynamic_type_builder_add_float128_bounded_sequence_member =
      zdds_add_float128_bounded_sequence_member;
    methods.dynamic_type_builder_add_float128_unbounded_sequence_member =
      zdds_add_float128_unbounded_sequence_member;
    methods.dynamic_data_get_float128_value = zdds_data_get_float128_value;
    methods.dynamic_data_set_float128_value = zdds_data_set_float128_value;
    methods.dynamic_data_insert_float128_value = zdds_data_insert_float128_value;
  }
}

rosidl_dynamic_typesupport_serialization_support_interface_t build_interface()
{
  rosidl_dynamic_typesupport_serialization_support_interface_t methods =
    rosidl_dynamic_typesupport_get_zero_initialized_serialization_support_interface();
  methods.serialization_library_identifier = ZERODDS_DYNAMIC_TYPESUPPORT_ID;

  methods.serialization_support_impl_fini = zdds_support_impl_fini;
  methods.serialization_support_interface_fini = zdds_interface_fini;

  methods.dynamic_type_equals = zdds_type_equals;
  methods.dynamic_type_get_member_count = zdds_type_get_member_count;

  methods.dynamic_type_builder_init = zdds_builder_init;
  methods.dynamic_type_builder_clone = zdds_builder_clone;
  methods.dynamic_type_builder_fini = zdds_builder_fini;
  methods.dynamic_type_init_from_dynamic_type_builder = zdds_type_init_from_builder;
  methods.dynamic_type_clone = zdds_type_clone;
  methods.dynamic_type_fini = zdds_type_fini;
  methods.dynamic_type_get_name = zdds_type_get_name;
  methods.dynamic_type_builder_get_name = zdds_builder_get_name;
  methods.dynamic_type_builder_set_name = zdds_builder_set_name;

#define ZDDS_WIRE_PRIMITIVE(suffix) \
  methods.dynamic_type_builder_add_##suffix##_member = zdds_add_##suffix##_member; \
  methods.dynamic_type_builder_add_##suffix##_array_member = zdds_add_##suffix##_array_member; \
  methods.dynamic_type_builder_add_##suffix##_bounded_sequence_member = \
    zdds_add_##suffix##_bounded_sequence_member; \
  methods.dynamic_type_builder_add_##suffix##_unbounded_sequence_member = \
    zdds_add_##suffix##_unbounded_sequence_member;

  ZDDS_WIRE_PRIMITIVE(bool)
  ZDDS_WIRE_PRIMITIVE(byte)
  ZDDS_WIRE_PRIMITIVE(char)
  ZDDS_WIRE_PRIMITIVE(wchar)
  ZDDS_WIRE_PRIMITIVE(float32)
  ZDDS_WIRE_PRIMITIVE(float64)
  // float128: `zdds_set_float128` below, where the interface has it.
  ZDDS_WIRE_PRIMITIVE(int8)
  ZDDS_WIRE_PRIMITIVE(uint8)
  ZDDS_WIRE_PRIMITIVE(int16)
  ZDDS_WIRE_PRIMITIVE(uint16)
  ZDDS_WIRE_PRIMITIVE(int32)
  ZDDS_WIRE_PRIMITIVE(uint32)
  ZDDS_WIRE_PRIMITIVE(int64)
  ZDDS_WIRE_PRIMITIVE(uint64)
#undef ZDDS_WIRE_PRIMITIVE

#define ZDDS_WIRE_STRING(suffix) \
  methods.dynamic_type_builder_add_##suffix##_member = zdds_add_##suffix##_member; \
  methods.dynamic_type_builder_add_fixed_##suffix##_member = zdds_add_fixed_##suffix##_member; \
  methods.dynamic_type_builder_add_bounded_##suffix##_member = \
    zdds_add_bounded_##suffix##_member; \
  methods.dynamic_type_builder_add_##suffix##_array_member = zdds_add_##suffix##_array_member; \
  methods.dynamic_type_builder_add_fixed_##suffix##_array_member = \
    zdds_add_fixed_##suffix##_array_member; \
  methods.dynamic_type_builder_add_bounded_##suffix##_array_member = \
    zdds_add_bounded_##suffix##_array_member; \
  methods.dynamic_type_builder_add_##suffix##_bounded_sequence_member = \
    zdds_add_##suffix##_bounded_sequence_member; \
  methods.dynamic_type_builder_add_fixed_##suffix##_bounded_sequence_member = \
    zdds_add_fixed_##suffix##_bounded_sequence_member; \
  methods.dynamic_type_builder_add_bounded_##suffix##_bounded_sequence_member = \
    zdds_add_bounded_##suffix##_bounded_sequence_member; \
  methods.dynamic_type_builder_add_##suffix##_unbounded_sequence_member = \
    zdds_add_##suffix##_unbounded_sequence_member; \
  methods.dynamic_type_builder_add_fixed_##suffix##_unbounded_sequence_member = \
    zdds_add_fixed_##suffix##_unbounded_sequence_member; \
  methods.dynamic_type_builder_add_bounded_##suffix##_unbounded_sequence_member = \
    zdds_add_bounded_##suffix##_unbounded_sequence_member;

  ZDDS_WIRE_STRING(string)
  ZDDS_WIRE_STRING(wstring)
#undef ZDDS_WIRE_STRING

  methods.dynamic_type_builder_add_complex_member = zdds_add_complex_member;
  methods.dynamic_type_builder_add_complex_array_member = zdds_add_complex_array_member;
  methods.dynamic_type_builder_add_complex_unbounded_sequence_member =
    zdds_add_complex_unbounded_sequence_member;
  methods.dynamic_type_builder_add_complex_bounded_sequence_member =
    zdds_add_complex_bounded_sequence_member;
  methods.dynamic_type_builder_add_complex_member_builder = zdds_add_complex_member_builder;
  methods.dynamic_type_builder_add_complex_array_member_builder =
    zdds_add_complex_array_member_builder;
  methods.dynamic_type_builder_add_complex_unbounded_sequence_member_builder =
    zdds_add_complex_unbounded_sequence_member_builder;
  methods.dynamic_type_builder_add_complex_bounded_sequence_member_builder =
    zdds_add_complex_bounded_sequence_member_builder;

  methods.dynamic_data_clear_all_values = zdds_data_clear_all_values;
  methods.dynamic_data_clear_nonkey_values = zdds_data_clear_nonkey_values;
  methods.dynamic_data_clear_value = zdds_data_clear_value;
  methods.dynamic_data_equals = zdds_data_equals;
  methods.dynamic_data_get_item_count = zdds_data_get_item_count;
  methods.dynamic_data_get_member_id_by_name = zdds_data_get_member_id_by_name;
  methods.dynamic_data_get_member_id_at_index = zdds_data_get_member_id_at_index;
  methods.dynamic_data_get_array_index = zdds_data_get_array_index;
  methods.dynamic_data_loan_value = zdds_data_loan_value;
  methods.dynamic_data_return_loaned_value = zdds_data_return_loaned_value;
  methods.dynamic_data_get_name = zdds_data_get_name;

  methods.dynamic_data_init_from_dynamic_type_builder = zdds_data_init_from_builder;
  methods.dynamic_data_init_from_dynamic_type = zdds_data_init_from_type;
  methods.dynamic_data_clone = zdds_data_clone;
  methods.dynamic_data_fini = zdds_data_fini;

  methods.dynamic_data_serialize = zdds_data_serialize;
  methods.dynamic_data_deserialize = zdds_data_deserialize;

  methods.dynamic_data_get_bool_value = zdds_data_get_bool_value;
  methods.dynamic_data_set_bool_value = zdds_data_set_bool_value;
  methods.dynamic_data_insert_bool_value = zdds_data_insert_bool_value;

#define ZDDS_DATA_PRIMITIVE(suffix) \
  methods.dynamic_data_get_##suffix##_value = zdds_data_get_##suffix##_value; \
  methods.dynamic_data_set_##suffix##_value = zdds_data_set_##suffix##_value; \
  methods.dynamic_data_insert_##suffix##_value = zdds_data_insert_##suffix##_value;

  ZDDS_DATA_PRIMITIVE(byte)
  ZDDS_DATA_PRIMITIVE(char)
  ZDDS_DATA_PRIMITIVE(wchar)
  ZDDS_DATA_PRIMITIVE(float32)
  ZDDS_DATA_PRIMITIVE(float64)
  zdds_set_float128(methods);
  ZDDS_DATA_PRIMITIVE(int8)
  ZDDS_DATA_PRIMITIVE(uint8)
  ZDDS_DATA_PRIMITIVE(int16)
  ZDDS_DATA_PRIMITIVE(uint16)
  ZDDS_DATA_PRIMITIVE(int32)
  ZDDS_DATA_PRIMITIVE(uint32)
  ZDDS_DATA_PRIMITIVE(int64)
  ZDDS_DATA_PRIMITIVE(uint64)
#undef ZDDS_DATA_PRIMITIVE

  methods.dynamic_data_get_string_value = zdds_data_get_string_value;
  methods.dynamic_data_set_string_value = zdds_data_set_string_value;
  methods.dynamic_data_insert_string_value = zdds_data_insert_string_value;
  methods.dynamic_data_get_fixed_string_value = zdds_data_get_fixed_string_value;
  methods.dynamic_data_set_fixed_string_value = zdds_data_set_fixed_string_value;
  methods.dynamic_data_insert_fixed_string_value = zdds_data_insert_fixed_string_value;
  methods.dynamic_data_get_bounded_string_value = zdds_data_get_bounded_string_value;
  methods.dynamic_data_set_bounded_string_value = zdds_data_set_bounded_string_value;
  methods.dynamic_data_insert_bounded_string_value = zdds_data_insert_bounded_string_value;

  methods.dynamic_data_get_wstring_value = zdds_data_get_wstring_value;
  methods.dynamic_data_set_wstring_value = zdds_data_set_wstring_value;
  methods.dynamic_data_insert_wstring_value = zdds_data_insert_wstring_value;
  methods.dynamic_data_get_fixed_wstring_value = zdds_data_get_fixed_wstring_value;
  methods.dynamic_data_set_fixed_wstring_value = zdds_data_set_fixed_wstring_value;
  methods.dynamic_data_insert_fixed_wstring_value = zdds_data_insert_fixed_wstring_value;
  methods.dynamic_data_get_bounded_wstring_value = zdds_data_get_bounded_wstring_value;
  methods.dynamic_data_set_bounded_wstring_value = zdds_data_set_bounded_wstring_value;
  methods.dynamic_data_insert_bounded_wstring_value = zdds_data_insert_bounded_wstring_value;

  methods.dynamic_data_clear_sequence_data = zdds_data_clear_sequence_data;
  methods.dynamic_data_remove_sequence_data = zdds_data_remove_sequence_data;
  methods.dynamic_data_insert_sequence_data = zdds_data_insert_sequence_data;

  methods.dynamic_data_get_complex_value = zdds_data_get_complex_value;
  methods.dynamic_data_set_complex_value = zdds_data_set_complex_value;
  methods.dynamic_data_insert_complex_value_copy = zdds_data_insert_complex_value_copy;
  methods.dynamic_data_insert_complex_value = zdds_data_insert_complex_value;

  return methods;
}

}  // namespace

// =================================================================================================
// rmw.h entry points
// =================================================================================================
extern "C" {

rmw_ret_t rmw_serialization_support_init(
  const char * serialization_lib_name,
  rcutils_allocator_t * allocator,
  rosidl_dynamic_typesupport_serialization_support_t * serialization_support)
{
  if (!serialization_support) {
    RMW_SET_ERROR_MSG("rmw_serialization_support_init: serialization_support is NULL");
    return RMW_RET_INVALID_ARGUMENT;
  }
  if (serialization_lib_name &&
    std::strcmp(serialization_lib_name, ZERODDS_DYNAMIC_TYPESUPPORT_ID) != 0)
  {
    RMW_SET_ERROR_MSG(
      "rmw_serialization_support_init: serialization_lib_name names a different library");
    return RMW_RET_INCORRECT_RMW_IMPLEMENTATION;
  }
  rosidl_dynamic_typesupport_serialization_support_impl_t impl =
    rosidl_dynamic_typesupport_get_zero_initialized_serialization_support_impl();
  impl.serialization_library_identifier = ZERODDS_DYNAMIC_TYPESUPPORT_ID;
  rosidl_dynamic_typesupport_serialization_support_interface_t methods = build_interface();
  rcutils_ret_t rc = rosidl_dynamic_typesupport_serialization_support_init(
    &impl, &methods, allocator, serialization_support);
  return rmw_convert_rcutils_ret_to_rmw_ret(rc);
}

// Takes one sample as `[encapsulation][body]` and decodes it into
// `dynamic_message`. A sample that does not decode is consumed and
// reported as RMW_RET_ERROR, not skipped.
static rmw_ret_t zerodds_take_dynamic(
  const rmw_subscription_t * subscription,
  rosidl_dynamic_typesupport_dynamic_data_t * dynamic_message,
  bool * taken,
  rmw_message_info_t * message_info,
  rmw_subscription_allocation_t * allocation,
  const char * who)
{
  if (taken) { *taken = false; }
  if (!subscription || !subscription->data || !dynamic_message || !dynamic_message->impl.handle ||
    !taken)
  {
    RMW_SET_ERROR_MSG_WITH_FORMAT_STRING("%s: invalid argument", who);
    return RMW_RET_INVALID_ARGUMENT;
  }
  rcutils_allocator_t allocator = rcutils_get_default_allocator();
  rmw_serialized_message_t serialized = rcutils_get_zero_initialized_uint8_array();
  if (rcutils_uint8_array_init(&serialized, 0, &allocator) != RCUTILS_RET_OK) {
    return RMW_RET_BAD_ALLOC;
  }
  bool got = false;
  rmw_ret_t ret = rmw_take_serialized_message_with_info(
    subscription, &serialized, &got, message_info, allocation);
  if (ret == RMW_RET_OK && got) {
    int32_t decode_rc = zerodds_dyn_data_deserialize(
      dynamic_message->impl.handle, serialized.buffer, serialized.buffer_length);
    if (decode_rc == 0) {
      *taken = true;
    } else {
      RMW_SET_ERROR_MSG_WITH_FORMAT_STRING(
        "%s: a %zu-byte sample on '%s' does not decode as the dynamic type", who,
        serialized.buffer_length, subscription->topic_name);
      ret = RMW_RET_ERROR;
    }
  }
  if (rcutils_uint8_array_fini(&serialized) != RCUTILS_RET_OK && ret == RMW_RET_OK) {
    ret = RMW_RET_ERROR;
  }
  return ret;
}

rmw_ret_t rmw_take_dynamic_message(
  const rmw_subscription_t * subscription,
  rosidl_dynamic_typesupport_dynamic_data_t * dynamic_message,
  bool * taken,
  rmw_subscription_allocation_t * allocation)
{
  return zerodds_take_dynamic(
    subscription, dynamic_message, taken, nullptr, allocation, "rmw_take_dynamic_message");
}

rmw_ret_t rmw_take_dynamic_message_with_info(
  const rmw_subscription_t * subscription,
  rosidl_dynamic_typesupport_dynamic_data_t * dynamic_message,
  bool * taken,
  rmw_message_info_t * message_info,
  rmw_subscription_allocation_t * allocation)
{
  if (!message_info) {
    RMW_SET_ERROR_MSG("rmw_take_dynamic_message_with_info: message_info is NULL");
    return RMW_RET_INVALID_ARGUMENT;
  }
  return zerodds_take_dynamic(
    subscription, dynamic_message, taken, message_info, allocation,
    "rmw_take_dynamic_message_with_info");
}

}  // extern "C"

#endif  // __has_include(<rosidl_dynamic_typesupport/types.h>)
#endif  // defined(__has_include)

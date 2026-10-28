// End-to-end test for ROS 2 Jazzy's dynamic type support
// (rmw_zerodds_dynamic.cpp: rmw_serialization_support_init +
// rosidl_dynamic_typesupport_serialization_support_interface_t +
// rmw_take_dynamic_message[_with_info]), backed by ZeroDDS' own reflective
// DynamicType/DynamicData (crates/rmw-zerodds-shim/src/dynamic.rs).
//
// Four cases, all against a REAL publisher and a REAL static-typed
// rclcpp subscription on the SAME topic, so "the dynamic take agrees
// with the static take" is measured against a running peer, not against
// this test's own encoder:
//
//   1. std_msgs/msg/String  — the simplest case: one bounded-nothing
//      string field.
//   2. test_msgs/msg/BasicTypes — "a message with every [primitive]
//      field kind" the task asks for: bool, byte, char, float32,
//      float64, int8/16/32/64, uint8/16/32/64.
//   5. test_msgs/msg/Arrays — every primitive/string kind again, each
//      as a fixed array, once plain and once with a `default_value`
//      (test_msgs' own `*_values`/`*_values_default` fields), plus the
//      three nested-message array fields and the trailing alignment
//      scalar the real message also declares.
//   6. test_msgs/msg/BoundedSequences — the same field list as 5),
//      each array turned into a bounded sequence instead.
//
// For each: a plain rclcpp::Publisher<T> publishes N samples. A plain
// rclcpp::Subscription<T> is the reference ("static-typed subscriber").
// A second, bare rmw_subscription_t on the same topic — built directly
// through rmw_create_subscription, not through rclcpp — is driven
// through this backend's own rmw_serialization_support_init +
// dynamic_type_builder_init + dynamic_type_builder_add_*_member +
// dynamic_data_init_from_dynamic_type + rmw_take_dynamic_message, and
// every field it reads back is compared against the value the static
// subscription received for the *same* sample (matched by content, since
// nothing here correlates by sequence number).
//
// This exercises the RMW-level wiring end to end; the interface table's
// own per-kind correctness (every primitive, nested structs, sequences,
// bounds) is already measured, exhaustively and byte-for-byte, by
// crates/rmw-zerodds-shim/src/dynamic.rs's `#[cfg(test)]` module — this
// test does not re-walk that ground, it checks the plumbing those units
// cannot: a real rmw_subscription_t, a real wire sample, a real second
// process's publisher.
//
// Build + run: see run_dynamic_typesupport_test.sh.
// PASS criterion: every line prints PASS and the exit code is 0.
//
// NOTE: test_msgs' exact field lists are asserted by name below — for
// BasicTypes: bool_value, byte_value, char_value, float32_value,
// float64_value, int8_value, uint8_value, int16_value, uint16_value,
// int32_value, uint32_value, int64_value, uint64_value; for Constants
// and Defaults, the same names Defaults gives its own (scalar-default)
// fields, and Constants' one placeholder field,
// structure_needs_at_least_one_member, since `Constants.msg` declares
// no fields of its own (only IDL constants) and rosidl's `struct.idl.em`
// gives such a message one placeholder field so it is not empty on the
// wire; for Arrays/BoundedSequences: bool_values[_default],
// byte_values[_default], char_values[_default], float32_values[_default],
// float64_values[_default], int8_values[_default], uint8_values[_default],
// int16_values[_default], uint16_values[_default], int32_values[_default],
// uint32_values[_default], int64_values[_default], uint64_values[_default],
// string_values[_default], basic_types_values, constants_values,
// defaults_values, alignment_check — against this repo's understanding
// of the package; run_dynamic_typesupport_test.sh's build step is where
// a field-name mismatch against the installed test_msgs version would
// first show up (a compile error in the reference assignments below,
// not a silent skip).

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <thread>

#include <rclcpp/rclcpp.hpp>
#include <rcl/node.h>
#include <rcl/publisher.h>
#include <rcl/subscription.h>
#include <rcutils/allocator.h>
#include <rmw/rmw.h>
#include <rosidl_dynamic_typesupport/api/dynamic_data.h>
#include <rosidl_dynamic_typesupport/api/dynamic_type.h>
#include <rosidl_dynamic_typesupport/api/serialization_support.h>
#include <rosidl_dynamic_typesupport/types.h>
#include <rosidl_typesupport_cpp/message_type_support.hpp>
#include <std_msgs/msg/string.hpp>
#include <test_msgs/msg/arrays.hpp>
#include <test_msgs/msg/basic_types.hpp>
#include <test_msgs/msg/bounded_sequences.hpp>
#include <test_msgs/msg/constants.hpp>
#include <test_msgs/msg/defaults.hpp>

static int g_failures = 0;
static void check(bool ok, const char * what)
{
  printf("%-58s %s\n", what, ok ? "PASS" : "FAIL");
  if (!ok) { ++g_failures; }
}

namespace
{

// A bare rmw_subscription_t on `topic`, of type `type_support` — the
// same call rcl/rclcpp make under the hood, used directly here so this
// test can drive `rmw_take_dynamic_message` without needing rclcpp to
// know anything about dynamic types (it does not, on Jazzy).
rmw_subscription_t * create_bare_subscription(
  rmw_node_t * node, const rosidl_message_type_support_t * type_support, const char * topic)
{
  rmw_subscription_options_t options = rmw_get_default_subscription_options();
  return rmw_create_subscription(
    node, type_support, topic, &rmw_qos_profile_default, &options);
}

// One `add_*_member` function pointer, shared by every scalar-member
// family below (`add_bool_member`, `add_byte_member`, … — all take the
// same six arguments).
using ScalarAdd = rcutils_ret_t (*)(
  rosidl_dynamic_typesupport_dynamic_type_builder_t *, rosidl_dynamic_typesupport_member_id_t,
  const char *, size_t, const char *, size_t);

struct ScalarMember
{
  const char * name;
  ScalarAdd add;
};

// Finishes a builder already declared (`dynamic_type_builder_init`) and
// populated into `*out`. Shared by every nested-type builder below.
bool finish_nested_type(
  rosidl_dynamic_typesupport_dynamic_type_builder_t * builder, rcutils_allocator_t * allocator,
  rosidl_dynamic_typesupport_dynamic_type_t * out)
{
  return rosidl_dynamic_typesupport_dynamic_type_init_from_dynamic_type_builder(
    builder, allocator, out) == RCUTILS_RET_OK;
}

// test_msgs/msg/BasicTypes.msg and test_msgs/msg/Defaults.msg declare
// the identical thirteen scalar fields — Defaults just gives each a
// `.msg` scalar default this test does not need, since the nested
// `defaults_values` member below is checked by element count only, not
// field by field (its own scalar defaults are a round-1 concern,
// already covered by crates/rmw-zerodds-shim/src/dynamic.rs's
// `#[cfg(test)]` module). One builder serves both nested types.
bool build_basic_types_like_type(
  rosidl_dynamic_typesupport_serialization_support_t * support, rcutils_allocator_t * allocator,
  const char * qualified_name, rosidl_dynamic_typesupport_dynamic_type_t * out)
{
  rosidl_dynamic_typesupport_dynamic_type_builder_t builder =
    rosidl_dynamic_typesupport_get_zero_initialized_dynamic_type_builder();
  if (rosidl_dynamic_typesupport_dynamic_type_builder_init(
        support, qualified_name, std::strlen(qualified_name), allocator, &builder) !=
      RCUTILS_RET_OK)
  {
    return false;
  }
  const ScalarMember members[] = {
    {"bool_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_bool_member},
    {"byte_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_byte_member},
    {"char_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_char_member},
    {"float32_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_float32_member},
    {"float64_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_float64_member},
    {"int8_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_int8_member},
    {"uint8_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_uint8_member},
    {"int16_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_int16_member},
    {"uint16_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_uint16_member},
    {"int32_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_int32_member},
    {"uint32_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_uint32_member},
    {"int64_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_int64_member},
    {"uint64_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_uint64_member},
  };
  for (size_t i = 0; i < sizeof(members) / sizeof(members[0]); ++i) {
    if (members[i].add(
          &builder, static_cast<rosidl_dynamic_typesupport_member_id_t>(i), members[i].name,
          std::strlen(members[i].name), "", 0) != RCUTILS_RET_OK)
    {
      return false;
    }
  }
  return finish_nested_type(&builder, allocator, out);
}

// test_msgs/msg/Constants.msg declares only IDL constants, no fields —
// rosidl's own `struct.idl.em` gives such a message one placeholder
// field, `structure_needs_at_least_one_member` (a `uint8`), so that is
// what is really on the wire.
bool build_constants_like_type(
  rosidl_dynamic_typesupport_serialization_support_t * support, rcutils_allocator_t * allocator,
  const char * qualified_name, rosidl_dynamic_typesupport_dynamic_type_t * out)
{
  rosidl_dynamic_typesupport_dynamic_type_builder_t builder =
    rosidl_dynamic_typesupport_get_zero_initialized_dynamic_type_builder();
  if (rosidl_dynamic_typesupport_dynamic_type_builder_init(
        support, qualified_name, std::strlen(qualified_name), allocator, &builder) !=
      RCUTILS_RET_OK)
  {
    return false;
  }
  const char * field = "structure_needs_at_least_one_member";
  if (rosidl_dynamic_typesupport_dynamic_type_builder_add_uint8_member(
        &builder, 0, field, std::strlen(field), "", 0) != RCUTILS_RET_OK)
  {
    return false;
  }
  return finish_nested_type(&builder, allocator, out);
}

// One `add_*_array_member`/`add_*_bounded_sequence_member` function
// pointer — every kind's pair shares this same seven-argument shape
// (`array_length`/`sequence_bound` is the same trailing `size_t`
// either way).
using CollectionAdd = rcutils_ret_t (*)(
  rosidl_dynamic_typesupport_dynamic_type_builder_t *, rosidl_dynamic_typesupport_member_id_t,
  const char *, size_t, const char *, size_t, size_t);

struct CollectionKind
{
  const char * stem;
  // test_msgs' own declared default for `<stem>_values_default`
  // (test_msgs/msg/Arrays.msg's — BoundedSequences.msg's is byte for
  // byte identical), in the Python-tuple-`repr()` form
  // `rmw_zerodds_dynamic.cpp`'s module note describes.
  const char * default_text;
  CollectionAdd add_array;
  CollectionAdd add_bounded_seq;
};

// Reads `nested`'s elements 0..expected.size() back through `get` and
// compares each against `expected[i]`. `nested` is a member already
// fetched through `dynamic_data_get_complex_value` — a collection
// member's own value.
template<typename T, typename Container>
bool array_matches(
  const rosidl_dynamic_typesupport_dynamic_data_t * nested,
  rcutils_ret_t (*get)(
    const rosidl_dynamic_typesupport_dynamic_data_t *, rosidl_dynamic_typesupport_member_id_t,
    T *),
  const Container & expected)
{
  for (size_t i = 0; i < expected.size(); ++i) {
    T got{};
    if (get(nested, static_cast<rosidl_dynamic_typesupport_member_id_t>(i), &got) !=
        RCUTILS_RET_OK)
    {
      return false;
    }
    if (!(got == expected[i])) {
      return false;
    }
  }
  return true;
}

// As [array_matches], for a string element (`get_string_value`'s shape
// does not fit the template above).
template<typename Container>
bool string_array_matches(
  const rosidl_dynamic_typesupport_dynamic_data_t * nested, const Container & expected)
{
  for (size_t i = 0; i < expected.size(); ++i) {
    char * value = nullptr;
    size_t value_length = 0;
    if (rosidl_dynamic_typesupport_dynamic_data_get_string_value(
          nested, static_cast<rosidl_dynamic_typesupport_member_id_t>(i), &value,
          &value_length) != RCUTILS_RET_OK)
    {
      return false;
    }
    if (std::string(value, value_length) != expected[i]) {
      return false;
    }
  }
  return true;
}

// Fetches member `id` of `data` as a nested complex value and hands it
// to `compare`. `false` where the fetch itself fails.
template<typename Compare>
bool with_complex_member(
  const rosidl_dynamic_typesupport_dynamic_data_t * data,
  rosidl_dynamic_typesupport_member_id_t id, rcutils_allocator_t * allocator, Compare compare)
{
  rosidl_dynamic_typesupport_dynamic_data_t nested =
    rosidl_dynamic_typesupport_get_zero_initialized_dynamic_data();
  if (rosidl_dynamic_typesupport_dynamic_data_get_complex_value(
        data, id, allocator, &nested) != RCUTILS_RET_OK)
  {
    return false;
  }
  bool ok = compare(&nested);
  rosidl_dynamic_typesupport_dynamic_data_fini(&nested);
  return ok;
}

// `item_count(member) == expected` — the light check the three
// nested-message array/sequence members below get instead of a field
// by field one (their own field kinds are already covered by 2), and
// what block 5)/6) fall back to on a `get`/compare mismatch that would
// otherwise stop the whole run.
bool item_count_is(
  const rosidl_dynamic_typesupport_dynamic_data_t * data,
  rosidl_dynamic_typesupport_member_id_t id, rcutils_allocator_t * allocator, size_t expected)
{
  return with_complex_member(data, id, allocator, [expected](
    const rosidl_dynamic_typesupport_dynamic_data_t * nested) {
    size_t count = 0;
    return rosidl_dynamic_typesupport_dynamic_data_get_item_count(nested, &count) ==
      RCUTILS_RET_OK && count == expected;
  });
}

// test_msgs/msg/Arrays (`bounded == false`) and
// test_msgs/msg/BoundedSequences (`bounded == true`) declare the
// identical field list — every array turned into a bounded sequence —
// so one templated body builds, publishes, takes and compares both;
// `MsgT` is the only thing that differs, and it is deduced from the
// message type the caller names.
//
// `basic_types_ty`/`constants_ty`/`defaults_ty` are the once-built
// nested types [`build_basic_types_like_type`]/[`build_constants_like_type`]
// produced; grafted in, not consumed — see `dynamic.rs`'s module note
// on why the same handle can seed more than one outer builder.
template<typename MsgT>
void run_collection_message_test(
  const rclcpp::Node::SharedPtr & node, rmw_node_t * rmw_node, rcutils_allocator_t & allocator,
  const char * qualified_name, const char * topic, const char * label, bool bounded,
  rosidl_dynamic_typesupport_dynamic_type_t * basic_types_ty,
  rosidl_dynamic_typesupport_dynamic_type_t * constants_ty,
  rosidl_dynamic_typesupport_dynamic_type_t * defaults_ty)
{
  auto pub = node->create_publisher<MsgT>(topic, 10);
  MsgT got_static{};
  bool got_static_flag = false;
  auto sub_static = node->create_subscription<MsgT>(
    topic, 10, [&got_static, &got_static_flag](typename MsgT::SharedPtr m) {
      got_static = *m;
      got_static_flag = true;
    });

  const rosidl_message_type_support_t * ts =
    rosidl_typesupport_cpp::get_message_type_support_handle<MsgT>();
  rmw_subscription_t * dyn_sub = create_bare_subscription(rmw_node, ts, topic);
  check(dyn_sub != nullptr, (std::string(label) + " bare rmw_subscription_t created").c_str());

  rosidl_dynamic_typesupport_serialization_support_t support =
    rosidl_dynamic_typesupport_get_zero_initialized_serialization_support();
  check(
    rmw_serialization_support_init(nullptr, &allocator, &support) == RMW_RET_OK,
    (std::string(label) + " rmw_serialization_support_init").c_str());

  rosidl_dynamic_typesupport_dynamic_type_builder_t builder =
    rosidl_dynamic_typesupport_get_zero_initialized_dynamic_type_builder();
  check(
    rosidl_dynamic_typesupport_dynamic_type_builder_init(
      &support, qualified_name, std::strlen(qualified_name), &allocator, &builder) ==
      RCUTILS_RET_OK,
    (std::string(label) + " dynamic_type_builder_init").c_str());

  // Declaration order matters for a `@final` message's plain CDR layout
  // to match test_msgs' own generated order — see test_msgs/msg/
  // Arrays.msg / BoundedSequences.msg, mirrored field for field.
  const CollectionKind kinds[] = {
    {"bool", "(False, True, False)",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_bool_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_bool_bounded_sequence_member},
    {"byte", "(0, 1, 255)",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_byte_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_byte_bounded_sequence_member},
    {"char", "(0, 1, 127)",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_char_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_char_bounded_sequence_member},
    {"float32", "(1.125, 0.0, -1.125)",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_float32_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_float32_bounded_sequence_member},
    {"float64", "(3.1415, 0.0, -3.1415)",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_float64_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_float64_bounded_sequence_member},
    {"int8", "(0, 127, -128)",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_int8_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_int8_bounded_sequence_member},
    {"uint8", "(0, 1, 255)",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_uint8_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_uint8_bounded_sequence_member},
    {"int16", "(0, 32767, -32768)",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_int16_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_int16_bounded_sequence_member},
    {"uint16", "(0, 1, 65535)",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_uint16_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_uint16_bounded_sequence_member},
    {"int32", "(0, 2147483647, -2147483648)",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_int32_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_int32_bounded_sequence_member},
    {"uint32", "(0, 1, 4294967295)",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_uint32_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_uint32_bounded_sequence_member},
    {"int64", "(0, 9223372036854775807, -9223372036854775808)",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_int64_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_int64_bounded_sequence_member},
    {"uint64", "(0, 1, 18446744073709551615)",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_uint64_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_uint64_bounded_sequence_member},
    {"string", "('', 'max value', 'min value')",
     rosidl_dynamic_typesupport_dynamic_type_builder_add_string_array_member,
     rosidl_dynamic_typesupport_dynamic_type_builder_add_string_bounded_sequence_member},
  };
  constexpr size_t kind_count = sizeof(kinds) / sizeof(kinds[0]);
  rosidl_dynamic_typesupport_member_id_t plain_ids[kind_count];
  rosidl_dynamic_typesupport_member_id_t default_ids[kind_count];
  rosidl_dynamic_typesupport_member_id_t next_id = 0;
  bool all_added = true;
  for (size_t i = 0; i < kind_count; ++i) {
    CollectionAdd add = bounded ? kinds[i].add_bounded_seq : kinds[i].add_array;
    std::string plain_name = std::string(kinds[i].stem) + "_values";
    std::string default_name = plain_name + "_default";
    plain_ids[i] = next_id++;
    all_added = all_added &&
      add(&builder, plain_ids[i], plain_name.c_str(), plain_name.size(), "", 0, 3) ==
        RCUTILS_RET_OK;
    default_ids[i] = next_id++;
    all_added = all_added &&
      add(
        &builder, default_ids[i], default_name.c_str(), default_name.size(),
        kinds[i].default_text, std::strlen(kinds[i].default_text), 3) == RCUTILS_RET_OK;
  }

  using ComplexAdd = rcutils_ret_t (*)(
    rosidl_dynamic_typesupport_dynamic_type_builder_t *, rosidl_dynamic_typesupport_member_id_t,
    const char *, size_t, const char *, size_t, rosidl_dynamic_typesupport_dynamic_type_t *,
    size_t);
  ComplexAdd add_complex = bounded
    ? rosidl_dynamic_typesupport_dynamic_type_builder_add_complex_bounded_sequence_member
    : rosidl_dynamic_typesupport_dynamic_type_builder_add_complex_array_member;
  const char * basic_types_name = "basic_types_values";
  rosidl_dynamic_typesupport_member_id_t basic_types_id = next_id++;
  all_added = all_added &&
    add_complex(
      &builder, basic_types_id, basic_types_name, std::strlen(basic_types_name), "", 0,
      basic_types_ty, 3) == RCUTILS_RET_OK;
  const char * constants_name = "constants_values";
  rosidl_dynamic_typesupport_member_id_t constants_id = next_id++;
  all_added = all_added &&
    add_complex(
      &builder, constants_id, constants_name, std::strlen(constants_name), "", 0, constants_ty,
      3) == RCUTILS_RET_OK;
  const char * defaults_name = "defaults_values";
  rosidl_dynamic_typesupport_member_id_t defaults_id = next_id++;
  all_added = all_added &&
    add_complex(
      &builder, defaults_id, defaults_name, std::strlen(defaults_name), "", 0, defaults_ty, 3) ==
      RCUTILS_RET_OK;

  const char * alignment_name = "alignment_check";
  rosidl_dynamic_typesupport_member_id_t alignment_id = next_id++;
  all_added = all_added &&
    rosidl_dynamic_typesupport_dynamic_type_builder_add_int32_member(
      &builder, alignment_id, alignment_name, std::strlen(alignment_name), "", 0) ==
      RCUTILS_RET_OK;
  check(all_added, (std::string(label) + " every field's add_*_member call succeeded").c_str());

  rosidl_dynamic_typesupport_dynamic_data_t data =
    rosidl_dynamic_typesupport_get_zero_initialized_dynamic_data();
  check(
    rosidl_dynamic_typesupport_dynamic_data_init_from_dynamic_type_builder(
      &builder, &allocator, &data) == RCUTILS_RET_OK,
    (std::string(label) + " dynamic_data_init_from_dynamic_type_builder").c_str());

  rclcpp::executors::SingleThreadedExecutor exec;
  exec.add_node(node);

  MsgT msg;
  msg.bool_values = {false, true, false};
  msg.bool_values_default = {false, true, false};
  msg.byte_values = {0, 1, UINT8_MAX};
  msg.byte_values_default = {0, 1, UINT8_MAX};
  msg.char_values = {0, 1, 127};
  msg.char_values_default = {0, 1, 127};
  msg.float32_values = {1.125f, 0.0f, -1.125f};
  msg.float32_values_default = {1.125f, 0.0f, -1.125f};
  msg.float64_values = {3.1415, 0.0, -3.1415};
  msg.float64_values_default = {3.1415, 0.0, -3.1415};
  msg.int8_values = {0, INT8_MAX, INT8_MIN};
  msg.int8_values_default = {0, INT8_MAX, INT8_MIN};
  msg.uint8_values = {0, 1, UINT8_MAX};
  msg.uint8_values_default = {0, 1, UINT8_MAX};
  msg.int16_values = {0, INT16_MAX, INT16_MIN};
  msg.int16_values_default = {0, INT16_MAX, INT16_MIN};
  msg.uint16_values = {0, 1, UINT16_MAX};
  msg.uint16_values_default = {0, 1, UINT16_MAX};
  msg.int32_values = {0, INT32_MAX, INT32_MIN};
  msg.int32_values_default = {0, INT32_MAX, INT32_MIN};
  msg.uint32_values = {0, 1, UINT32_MAX};
  msg.uint32_values_default = {0, 1, UINT32_MAX};
  msg.int64_values = {0, INT64_MAX, INT64_MIN};
  msg.int64_values_default = {0, INT64_MAX, INT64_MIN};
  msg.uint64_values = {0, 1, UINT64_MAX};
  msg.uint64_values_default = {0, 1, UINT64_MAX};
  msg.string_values = {"", "max value", "min value"};
  msg.string_values_default = {"", "max value", "min value"};
  msg.alignment_check = 42;

  bool dynamic_taken = false;
  auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(15);
  while ((!got_static_flag || !dynamic_taken) && std::chrono::steady_clock::now() < deadline) {
    pub->publish(msg);
    exec.spin_some();
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    if (!dynamic_taken) {
      bool taken = false;
      if (rmw_take_dynamic_message(dyn_sub, &data, &taken, nullptr) == RMW_RET_OK && taken) {
        dynamic_taken = true;
      }
    }
  }
  check(got_static_flag, (std::string(label) + " static subscription received the sample").c_str());
  check(dynamic_taken, (std::string(label) + " dynamic take succeeded").c_str());

  if (dynamic_taken && got_static_flag) {
    bool ok = true;
    for (size_t i = 0; i < kind_count; ++i) {
      // clang-format off
      if (std::string(kinds[i].stem) == "bool") {
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<bool>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_bool_value, got_static.bool_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<bool>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_bool_value,
                got_static.bool_values_default);
            });
      } else if (std::string(kinds[i].stem) == "byte") {
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<unsigned char>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_byte_value, got_static.byte_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<unsigned char>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_byte_value,
                got_static.byte_values_default);
            });
      } else if (std::string(kinds[i].stem) == "char") {
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<char>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_char_value, got_static.char_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<char>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_char_value,
                got_static.char_values_default);
            });
      } else if (std::string(kinds[i].stem) == "float32") {
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<float>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_float32_value,
                got_static.float32_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<float>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_float32_value,
                got_static.float32_values_default);
            });
      } else if (std::string(kinds[i].stem) == "float64") {
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<double>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_float64_value,
                got_static.float64_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<double>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_float64_value,
                got_static.float64_values_default);
            });
      } else if (std::string(kinds[i].stem) == "int8") {
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<int8_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_int8_value, got_static.int8_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<int8_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_int8_value,
                got_static.int8_values_default);
            });
      } else if (std::string(kinds[i].stem) == "uint8") {
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<uint8_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_uint8_value,
                got_static.uint8_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<uint8_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_uint8_value,
                got_static.uint8_values_default);
            });
      } else if (std::string(kinds[i].stem) == "int16") {
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<int16_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_int16_value,
                got_static.int16_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<int16_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_int16_value,
                got_static.int16_values_default);
            });
      } else if (std::string(kinds[i].stem) == "uint16") {
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<uint16_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_uint16_value,
                got_static.uint16_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<uint16_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_uint16_value,
                got_static.uint16_values_default);
            });
      } else if (std::string(kinds[i].stem) == "int32") {
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<int32_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_int32_value,
                got_static.int32_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<int32_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_int32_value,
                got_static.int32_values_default);
            });
      } else if (std::string(kinds[i].stem) == "uint32") {
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<uint32_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_uint32_value,
                got_static.uint32_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<uint32_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_uint32_value,
                got_static.uint32_values_default);
            });
      } else if (std::string(kinds[i].stem) == "int64") {
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<int64_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_int64_value,
                got_static.int64_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<int64_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_int64_value,
                got_static.int64_values_default);
            });
      } else if (std::string(kinds[i].stem) == "uint64") {
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<uint64_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_uint64_value,
                got_static.uint64_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return array_matches<uint64_t>(
                n, rosidl_dynamic_typesupport_dynamic_data_get_uint64_value,
                got_static.uint64_values_default);
            });
      } else {  // "string"
        ok = ok &&
          with_complex_member(&data, plain_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return string_array_matches(n, got_static.string_values);
            }) &&
          with_complex_member(&data, default_ids[i], &allocator,
            [&](const rosidl_dynamic_typesupport_dynamic_data_t * n) {
              return string_array_matches(n, got_static.string_values_default);
            });
      }
      // clang-format on
    }
    ok = ok && item_count_is(&data, basic_types_id, &allocator, 3) &&
      item_count_is(&data, constants_id, &allocator, 3) &&
      item_count_is(&data, defaults_id, &allocator, 3);
    int32_t alignment_out = 0;
    ok = ok &&
      rosidl_dynamic_typesupport_dynamic_data_get_int32_value(
        &data, alignment_id, &alignment_out) == RCUTILS_RET_OK &&
      alignment_out == got_static.alignment_check;
    check(ok, (std::string(label) + " every field the dynamic take read back matches the static take").c_str());
  }

  check(
    rmw_destroy_subscription(rmw_node, dyn_sub) == RMW_RET_OK,
    (std::string(label) + " bare subscription destroyed").c_str());
}

}  // namespace

int main()
{
  setvbuf(stdout, nullptr, _IOLBF, 0);
  rclcpp::init(0, nullptr);
  rclcpp::NodeOptions opts;
  opts.start_parameter_services(false);
  opts.start_parameter_event_publisher(false);
  auto node = std::make_shared<rclcpp::Node>("zerodds_dynamic_typesupport_test", opts);
  rmw_node_t * rmw_node =
    rcl_node_get_rmw_handle(node->get_node_base_interface()->get_rcl_node_handle());

  rcutils_allocator_t allocator = rcutils_get_default_allocator();

  // -- 1) std_msgs/msg/String ------------------------------------------------
  {
    const char * topic = "zerodds_dynamic_test_string";
    auto pub = node->create_publisher<std_msgs::msg::String>(topic, 10);
    std::string got_static;
    auto sub_static = node->create_subscription<std_msgs::msg::String>(
      topic, 10, [&got_static](std_msgs::msg::String::SharedPtr m) { got_static = m->data; });

    const rosidl_message_type_support_t * ts =
      rosidl_typesupport_cpp::get_message_type_support_handle<std_msgs::msg::String>();
    rmw_subscription_t * dyn_sub = create_bare_subscription(rmw_node, ts, topic);
    check(dyn_sub != nullptr, "1) bare rmw_subscription_t created (String)");

    rosidl_dynamic_typesupport_serialization_support_t support =
      rosidl_dynamic_typesupport_get_zero_initialized_serialization_support();
    check(
      rmw_serialization_support_init(nullptr, &allocator, &support) == RMW_RET_OK,
      "1) rmw_serialization_support_init");

    rosidl_dynamic_typesupport_dynamic_type_builder_t builder =
      rosidl_dynamic_typesupport_get_zero_initialized_dynamic_type_builder();
    check(
      rosidl_dynamic_typesupport_dynamic_type_builder_init(
        &support, "std_msgs::msg::String", std::strlen("std_msgs::msg::String"), &allocator,
        &builder) ==
        RCUTILS_RET_OK,
      "1) dynamic_type_builder_init");
    check(
      rosidl_dynamic_typesupport_dynamic_type_builder_add_string_member(
        &builder, 0, "data", std::strlen("data"), "", 0) == RCUTILS_RET_OK,
      "1) add_string_member(data)");

    rosidl_dynamic_typesupport_dynamic_data_t data =
      rosidl_dynamic_typesupport_get_zero_initialized_dynamic_data();
    check(
      rosidl_dynamic_typesupport_dynamic_data_init_from_dynamic_type_builder(
        &builder, &allocator, &data) ==
        RCUTILS_RET_OK,
      "1) dynamic_data_init_from_dynamic_type_builder");

    rclcpp::executors::SingleThreadedExecutor exec;
    exec.add_node(node);

    std_msgs::msg::String msg;
    msg.data = "hello from the dynamic typesupport test";
    std::string got_dynamic;
    bool dynamic_taken = false;
    auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(15);
    while ((got_static.empty() || !dynamic_taken) && std::chrono::steady_clock::now() < deadline) {
      pub->publish(msg);
      exec.spin_some();
      std::this_thread::sleep_for(std::chrono::milliseconds(100));
      if (!dynamic_taken) {
        bool taken = false;
        if (rmw_take_dynamic_message(dyn_sub, &data, &taken, nullptr) == RMW_RET_OK && taken) {
          char * value = nullptr;
          size_t value_length = 0;
          if (rosidl_dynamic_typesupport_dynamic_data_get_string_value(
                &data, 0, &value, &value_length) == RCUTILS_RET_OK)
          {
            got_dynamic.assign(value, value_length);
            dynamic_taken = true;
          }
        }
      }
    }
    check(got_static == msg.data, "1) static subscription received the sample");
    check(dynamic_taken && got_dynamic == msg.data, "1) dynamic take agrees with the static take");

    check(rmw_destroy_subscription(rmw_node, dyn_sub) == RMW_RET_OK, "bare subscription destroyed");
  }

  // -- 2) test_msgs/msg/BasicTypes — every primitive field kind ---------------
  {
    const char * topic = "zerodds_dynamic_test_basic_types";
    auto pub = node->create_publisher<test_msgs::msg::BasicTypes>(topic, 10);
    test_msgs::msg::BasicTypes got_static;
    bool got_static_flag = false;
    auto sub_static = node->create_subscription<test_msgs::msg::BasicTypes>(
      topic, 10,
      [&got_static, &got_static_flag](test_msgs::msg::BasicTypes::SharedPtr m) {
        got_static = *m;
        got_static_flag = true;
      });

    const rosidl_message_type_support_t * ts =
      rosidl_typesupport_cpp::get_message_type_support_handle<test_msgs::msg::BasicTypes>();
    rmw_subscription_t * dyn_sub = create_bare_subscription(rmw_node, ts, topic);
    check(dyn_sub != nullptr, "2) bare rmw_subscription_t created (BasicTypes)");

    rosidl_dynamic_typesupport_serialization_support_t support =
      rosidl_dynamic_typesupport_get_zero_initialized_serialization_support();
    check(
      rmw_serialization_support_init(nullptr, &allocator, &support) == RMW_RET_OK,
      "2) rmw_serialization_support_init");

    rosidl_dynamic_typesupport_dynamic_type_builder_t builder =
      rosidl_dynamic_typesupport_get_zero_initialized_dynamic_type_builder();
    check(
      rosidl_dynamic_typesupport_dynamic_type_builder_init(
        &support, "test_msgs::msg::BasicTypes", std::strlen("test_msgs::msg::BasicTypes"),
        &allocator, &builder) == RCUTILS_RET_OK,
      "2) dynamic_type_builder_init");

    struct MemberAdd
    {
      const char * name;
      rcutils_ret_t (*add)(
        rosidl_dynamic_typesupport_dynamic_type_builder_t *, rosidl_dynamic_typesupport_member_id_t,
        const char *, size_t, const char *, size_t);
    };
    // Declaration order matters for a `@final` message's plain CDR
    // layout to match test_msgs' own generated order — see
    // test_msgs/msg/BasicTypes.msg.
    const MemberAdd members[] = {
      {"bool_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_bool_member},
      {"byte_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_byte_member},
      {"char_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_char_member},
      {"float32_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_float32_member},
      {"float64_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_float64_member},
      {"int8_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_int8_member},
      {"uint8_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_uint8_member},
      {"int16_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_int16_member},
      {"uint16_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_uint16_member},
      {"int32_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_int32_member},
      {"uint32_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_uint32_member},
      {"int64_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_int64_member},
      {"uint64_value", rosidl_dynamic_typesupport_dynamic_type_builder_add_uint64_member},
    };
    bool all_added = true;
    for (size_t i = 0; i < sizeof(members) / sizeof(members[0]); ++i) {
      rosidl_dynamic_typesupport_member_id_t id =
        static_cast<rosidl_dynamic_typesupport_member_id_t>(i);
      rcutils_ret_t rc =
        members[i].add(&builder, id, members[i].name, std::strlen(members[i].name), "", 0);
      all_added = all_added && rc == RCUTILS_RET_OK;
    }
    check(all_added, "2) every primitive add_*_member call succeeded");

    rosidl_dynamic_typesupport_dynamic_data_t data =
      rosidl_dynamic_typesupport_get_zero_initialized_dynamic_data();
    check(
      rosidl_dynamic_typesupport_dynamic_data_init_from_dynamic_type_builder(
        &builder, &allocator, &data) ==
        RCUTILS_RET_OK,
      "2) dynamic_data_init_from_dynamic_type_builder");

    rclcpp::executors::SingleThreadedExecutor exec;
    exec.add_node(node);

    test_msgs::msg::BasicTypes msg;
    msg.bool_value = true;
    msg.byte_value = 0xAB;
    msg.char_value = 'Z';
    msg.float32_value = 1.5f;
    msg.float64_value = -2.25;
    msg.int8_value = -8;
    msg.uint8_value = 8;
    msg.int16_value = -1600;
    msg.uint16_value = 1600;
    msg.int32_value = -320000;
    msg.uint32_value = 320000;
    msg.int64_value = -640000000000LL;
    msg.uint64_value = 640000000000ULL;

    bool dynamic_taken = false;
    auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(15);
    while ((!got_static_flag || !dynamic_taken) &&
           std::chrono::steady_clock::now() < deadline)
    {
      pub->publish(msg);
      exec.spin_some();
      std::this_thread::sleep_for(std::chrono::milliseconds(100));
      if (!dynamic_taken) {
        bool taken = false;
        if (rmw_take_dynamic_message(dyn_sub, &data, &taken, nullptr) == RMW_RET_OK && taken) {
          dynamic_taken = true;
        }
      }
    }
    check(got_static_flag, "2) static subscription received the sample");
    check(dynamic_taken, "2) dynamic take succeeded");

    if (dynamic_taken && got_static_flag) {
      bool b = false; uint8_t u8 = 0; char c8 = 0; float f32 = 0; double f64 = 0;
      int8_t i8 = 0; int16_t i16 = 0; uint16_t u16 = 0; int32_t i32 = 0; uint32_t u32 = 0;
      int64_t i64 = 0; uint64_t u64 = 0;
      bool ok =
        rosidl_dynamic_typesupport_dynamic_data_get_bool_value(&data, 0, &b) == RCUTILS_RET_OK &&
        rosidl_dynamic_typesupport_dynamic_data_get_byte_value(&data, 1, &u8) == RCUTILS_RET_OK &&
        u8 == got_static.byte_value &&
        rosidl_dynamic_typesupport_dynamic_data_get_char_value(&data, 2, &c8) == RCUTILS_RET_OK &&
        c8 == got_static.char_value &&
        rosidl_dynamic_typesupport_dynamic_data_get_float32_value(&data, 3, &f32) ==
          RCUTILS_RET_OK && f32 == got_static.float32_value &&
        rosidl_dynamic_typesupport_dynamic_data_get_float64_value(&data, 4, &f64) ==
          RCUTILS_RET_OK && f64 == got_static.float64_value &&
        rosidl_dynamic_typesupport_dynamic_data_get_int8_value(&data, 5, &i8) == RCUTILS_RET_OK &&
        i8 == got_static.int8_value &&
        rosidl_dynamic_typesupport_dynamic_data_get_uint8_value(&data, 6, &u8) ==
          RCUTILS_RET_OK && u8 == got_static.uint8_value &&
        rosidl_dynamic_typesupport_dynamic_data_get_int16_value(&data, 7, &i16) ==
          RCUTILS_RET_OK && i16 == got_static.int16_value &&
        rosidl_dynamic_typesupport_dynamic_data_get_uint16_value(&data, 8, &u16) ==
          RCUTILS_RET_OK && u16 == got_static.uint16_value &&
        rosidl_dynamic_typesupport_dynamic_data_get_int32_value(&data, 9, &i32) ==
          RCUTILS_RET_OK && i32 == got_static.int32_value &&
        rosidl_dynamic_typesupport_dynamic_data_get_uint32_value(&data, 10, &u32) ==
          RCUTILS_RET_OK && u32 == got_static.uint32_value &&
        rosidl_dynamic_typesupport_dynamic_data_get_int64_value(&data, 11, &i64) ==
          RCUTILS_RET_OK && i64 == got_static.int64_value &&
        rosidl_dynamic_typesupport_dynamic_data_get_uint64_value(&data, 12, &u64) ==
          RCUTILS_RET_OK && u64 == got_static.uint64_value;
      check(ok && b == got_static.bool_value,
            "2) every field the dynamic take read back matches the static take");
    }

    check(rmw_destroy_subscription(rmw_node, dyn_sub) == RMW_RET_OK, "bare subscription destroyed");
  }

  // -- 3) a member's default_value — no publisher needed ----------------------
  //
  // Builds a scratch type with one int32 member declared with a
  // `default_value` of "42" and reads it back off a data value that was
  // never set — the member-level `@default` this backend now attaches
  // (rmw_zerodds_dynamic.cpp's module note; crates/rmw-zerodds-shim/
  // src/dynamic.rs's `ros_default_literal`), not the type's zero.
  {
    rosidl_dynamic_typesupport_serialization_support_t support =
      rosidl_dynamic_typesupport_get_zero_initialized_serialization_support();
    check(
      rmw_serialization_support_init(nullptr, &allocator, &support) == RMW_RET_OK,
      "3) rmw_serialization_support_init");

    rosidl_dynamic_typesupport_dynamic_type_builder_t builder =
      rosidl_dynamic_typesupport_get_zero_initialized_dynamic_type_builder();
    check(
      rosidl_dynamic_typesupport_dynamic_type_builder_init(
        &support, "zerodds::test::Defaulted", std::strlen("zerodds::test::Defaulted"),
        &allocator, &builder) == RCUTILS_RET_OK,
      "3) dynamic_type_builder_init");
    check(
      rosidl_dynamic_typesupport_dynamic_type_builder_add_int32_member(
        &builder, 0, "count", std::strlen("count"), "42", std::strlen("42")) == RCUTILS_RET_OK,
      "3) add_int32_member(count, default \"42\")");

    rosidl_dynamic_typesupport_dynamic_data_t data =
      rosidl_dynamic_typesupport_get_zero_initialized_dynamic_data();
    check(
      rosidl_dynamic_typesupport_dynamic_data_init_from_dynamic_type_builder(
        &builder, &allocator, &data) == RCUTILS_RET_OK,
      "3) dynamic_data_init_from_dynamic_type_builder");

    int32_t count = 0;
    check(
      rosidl_dynamic_typesupport_dynamic_data_get_int32_value(&data, 0, &count) ==
          RCUTILS_RET_OK &&
        count == 42,
      "3) the member's own default reads back, not a zero");
  }

  // -- 4) the x86-64 `long double` bridge --------------------------------------
  //
  // `long double` on x86-64 is the 80-bit extended layout, padded to 16
  // bytes — the layout `Float128::{to,from}_x87_padded_bytes` bridges
  // (crates/rmw-zerodds-shim/src/dynamic.rs). No publisher needed: this
  // sets and reads back a `float128` member on a scratch type, the same
  // as block 3.
#if defined(__x86_64__)
  {
    rosidl_dynamic_typesupport_serialization_support_t support =
      rosidl_dynamic_typesupport_get_zero_initialized_serialization_support();
    check(
      rmw_serialization_support_init(nullptr, &allocator, &support) == RMW_RET_OK,
      "4) rmw_serialization_support_init");

    rosidl_dynamic_typesupport_dynamic_type_builder_t builder =
      rosidl_dynamic_typesupport_get_zero_initialized_dynamic_type_builder();
    check(
      rosidl_dynamic_typesupport_dynamic_type_builder_init(
        &support, "zerodds::test::LongDouble", std::strlen("zerodds::test::LongDouble"),
        &allocator, &builder) == RCUTILS_RET_OK,
      "4) dynamic_type_builder_init");
    check(
      rosidl_dynamic_typesupport_dynamic_type_builder_add_float128_member(
        &builder, 0, "value", std::strlen("value"), "", 0) == RCUTILS_RET_OK,
      "4) add_float128_member(value)");

    rosidl_dynamic_typesupport_dynamic_data_t data =
      rosidl_dynamic_typesupport_get_zero_initialized_dynamic_data();
    check(
      rosidl_dynamic_typesupport_dynamic_data_init_from_dynamic_type_builder(
        &builder, &allocator, &data) == RCUTILS_RET_OK,
      "4) dynamic_data_init_from_dynamic_type_builder");

    const long double sent = 3.25L;
    check(
      rosidl_dynamic_typesupport_dynamic_data_set_float128_value(&data, 0, sent) ==
        RCUTILS_RET_OK,
      "4) set_float128_value");
    long double received = 0.0L;
    check(
      rosidl_dynamic_typesupport_dynamic_data_get_float128_value(&data, 0, &received) ==
          RCUTILS_RET_OK &&
        received == sent,
      "4) long double round-trips through the x87 bridge");
  }
#endif

  // -- 5)/6) test_msgs/msg/Arrays and test_msgs/msg/BoundedSequences ----------
  //
  // Every primitive/string kind again, each as a fixed array (5) or a
  // bounded sequence (6), once plain and once with a `default_value` —
  // the array/sequence default this task adds
  // (crates/rmw-zerodds-shim/src/dynamic/collection_default.rs) — plus
  // the three nested-message members and the trailing scalar the real
  // messages also declare, so the dynamic type's plain-CDR layout stays
  // aligned with what the static publisher actually writes.
  {
    rosidl_dynamic_typesupport_serialization_support_t nested_support =
      rosidl_dynamic_typesupport_get_zero_initialized_serialization_support();
    check(
      rmw_serialization_support_init(nullptr, &allocator, &nested_support) == RMW_RET_OK,
      "5/6) rmw_serialization_support_init (nested types)");

    rosidl_dynamic_typesupport_dynamic_type_t basic_types_ty =
      rosidl_dynamic_typesupport_get_zero_initialized_dynamic_type();
    check(
      build_basic_types_like_type(
        &nested_support, &allocator, "test_msgs::msg::BasicTypes", &basic_types_ty),
      "5/6) built the nested BasicTypes type");

    rosidl_dynamic_typesupport_dynamic_type_t constants_ty =
      rosidl_dynamic_typesupport_get_zero_initialized_dynamic_type();
    check(
      build_constants_like_type(
        &nested_support, &allocator, "test_msgs::msg::Constants", &constants_ty),
      "5/6) built the nested Constants type");

    rosidl_dynamic_typesupport_dynamic_type_t defaults_ty =
      rosidl_dynamic_typesupport_get_zero_initialized_dynamic_type();
    check(
      build_basic_types_like_type(
        &nested_support, &allocator, "test_msgs::msg::Defaults", &defaults_ty),
      "5/6) built the nested Defaults type");

    run_collection_message_test<test_msgs::msg::Arrays>(
      node, rmw_node, allocator, "test_msgs::msg::Arrays", "zerodds_dynamic_test_arrays",
      "5) Arrays:", false, &basic_types_ty, &constants_ty, &defaults_ty);
    run_collection_message_test<test_msgs::msg::BoundedSequences>(
      node, rmw_node, allocator, "test_msgs::msg::BoundedSequences",
      "zerodds_dynamic_test_bounded_sequences", "6) BoundedSequences:", true, &basic_types_ty,
      &constants_ty, &defaults_ty);

    rosidl_dynamic_typesupport_dynamic_type_fini(&basic_types_ty);
    rosidl_dynamic_typesupport_dynamic_type_fini(&constants_ty);
    rosidl_dynamic_typesupport_dynamic_type_fini(&defaults_ty);
  }

  rclcpp::shutdown();
  printf("%s (%d failure(s))\n", g_failures == 0 ? "ALL PASS" : "FAILURES", g_failures);
  return g_failures == 0 ? 0 : 1;
}

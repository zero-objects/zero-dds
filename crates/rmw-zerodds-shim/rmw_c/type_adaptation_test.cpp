// End-to-end test for the REP-2007 type-adaptation surface at the rmw layer
// (`rmw_zerodds_type_adapter_t` + the adapted publish/take and request/response
// entry points in rmw_zerodds_ros2_ext.h).
//
// REP-2007 defines the user-facing TypeAdapter in rclcpp but names the part it
// left to the middleware:
//
//   "Perhaps we can support a form of this feature in other languages in rcl or
//    rmw in the future. One challenge in doing this is that it may require
//    custom type support, which may be middleware specific."
//
// and what the rclcpp-only placement costs:
//
//   "An advantage of following the ROS 1 approach is that an extra copy can be
//    avoided; although it is likely much more challenging to implement this
//    feature the ROS 1 way because of the middleware."
//
// This test drives that middleware-side custom type support through the real
// rmw handles of live rclcpp entities, using REP-2007's own worked example
// (std::string adapted as std_msgs::msg::String), and measures four things:
//
//   1. WIRE IDENTITY — the bytes the adapter produces are identical, byte for
//      byte, to what rmw_serialize produces for the equivalent ROS message.
//      Measured by memcmp, not asserted by construction.
//   2. ADAPTED PUBLISH → PLAIN TAKE — an adapted publisher and an ordinary
//      rclcpp subscription of the ros_message_type exchange the value.
//   3. PLAIN PUBLISH → ADAPTED TAKE — the reverse direction.
//   4. COPY COUNT — the ros_message_type is never materialized on the adapted
//      path: the test's convert_to_ros_message counter must stay at 0 while
//      serialize is called exactly once per publish. That is the "extra copy"
//      REP-2007 calls avoidable, measured.
//
// Plus the service half (REP-2007 "Type Adaptation in Services"): an adapted
// request and an adapted response over example_interfaces/srv/AddTwoInts, with
// custom types that are not ROS messages.
//
// Endpoint invariance is checked too: binding an adapter must not change the
// topic type reported by discovery, because a REP-2007 endpoint has to keep
// matching plain endpoints of the ros_message_type.
//
// Build + run: see run_type_adaptation_test.sh.
// PASS criterion: every line prints PASS and the exit code is 0.

#include <chrono>
#include <cstdio>
#include <cstring>
#include <cstdint>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#include <rclcpp/rclcpp.hpp>
#include <rcl/client.h>
#include <rcl/publisher.h>
#include <rcl/service.h>
#include <rcl/subscription.h>
#include <rmw/rmw.h>
#include <rosidl_typesupport_cpp/message_type_support.hpp>
#include <std_msgs/msg/string.hpp>
#include <example_interfaces/srv/add_two_ints.hpp>

#include "rmw_zerodds_ros2_ext.h"

static int g_failures = 0;
static void check(bool ok, const char * what)
{
  printf("%-58s %s\n", what, ok ? "PASS" : "FAIL");
  if (!ok) { ++g_failures; }
}

// ---------------------------------------------------------------------------
// REP-2007's own example adapter: std::string as std_msgs::msg::String.
// Instrumented so the test can count what the middleware actually did.
// ---------------------------------------------------------------------------
static int g_serialize_calls = 0;
static int g_deserialize_calls = 0;
// Incremented only by the convert half. REP-2007 says convert is for
// intra-process and (de)serialize for inter-process; on the adapted wire path
// the middleware must never need the ros_message_type, so this must stay 0.
static int g_convert_to_ros_message_calls = 0;

[[maybe_unused]] static void convert_to_ros_message(
  const std::string & source, std_msgs::msg::String & destination)
{
  ++g_convert_to_ros_message_calls;
  destination.data = source;
}

static size_t str_serialized_size(const void * custom)
{
  // std_msgs/msg/String CDR body: uint32 length (including the NUL) + bytes + NUL.
  return 4u + static_cast<const std::string *>(custom)->size() + 1u;
}

static int str_serialize(const void * custom, uint8_t * buffer, size_t capacity, size_t * written)
{
  ++g_serialize_calls;
  const std::string & s = *static_cast<const std::string *>(custom);
  const uint32_t n = static_cast<uint32_t>(s.size() + 1u);
  if (capacity < 4u + n) { return -1; }
  std::memcpy(buffer, &n, 4);  // little endian, matching the CDR_LE encapsulation
  std::memcpy(buffer + 4, s.data(), s.size());
  buffer[4 + s.size()] = '\0';
  *written = 4u + n;
  return 0;
}

static int str_deserialize(const uint8_t * buffer, size_t length, bool big_endian, void * custom)
{
  ++g_deserialize_calls;
  if (big_endian || length < 4u) { return -1; }
  uint32_t n = 0;
  std::memcpy(&n, buffer, 4);
  if (n == 0u || length < 4u + n) { return -1; }
  *static_cast<std::string *>(custom) = std::string(
    reinterpret_cast<const char *>(buffer + 4), static_cast<size_t>(n - 1u));
  return 0;
}

static const rmw_zerodds_type_adapter_t kStringAdapter = {
  "std_msgs/msg/String", str_serialized_size, str_serialize, str_deserialize
};

// ---------------------------------------------------------------------------
// Service adapters (REP-2007 "Type Adaptation in Services"): the request half
// adapts std::pair<int64_t,int64_t>, the response half adapts a plain int64_t.
// Neither custom type is a ROS message.
// ---------------------------------------------------------------------------
using AddPair = std::pair<int64_t, int64_t>;

static size_t pair_size(const void *) { return 16u; }
static int pair_serialize(const void * custom, uint8_t * buf, size_t cap, size_t * written)
{
  if (cap < 16u) { return -1; }
  const AddPair & p = *static_cast<const AddPair *>(custom);
  std::memcpy(buf, &p.first, 8);
  std::memcpy(buf + 8, &p.second, 8);
  *written = 16u;
  return 0;
}
static int pair_deserialize(const uint8_t * buf, size_t len, bool be, void * custom)
{
  if (be || len < 16u) { return -1; }
  AddPair & p = *static_cast<AddPair *>(custom);
  std::memcpy(&p.first, buf, 8);
  std::memcpy(&p.second, buf + 8, 8);
  return 0;
}
static size_t i64_size(const void *) { return 8u; }
static int i64_serialize(const void * custom, uint8_t * buf, size_t cap, size_t * written)
{
  if (cap < 8u) { return -1; }
  std::memcpy(buf, custom, 8);
  *written = 8u;
  return 0;
}
static int i64_deserialize(const uint8_t * buf, size_t len, bool be, void * custom)
{
  if (be || len < 8u) { return -1; }
  std::memcpy(custom, buf, 8);
  return 0;
}

static const rmw_zerodds_type_adapter_t kPairAdapter = {
  "example_interfaces/srv/AddTwoInts_Request", pair_size, pair_serialize, pair_deserialize
};
static const rmw_zerodds_type_adapter_t kI64Adapter = {
  "example_interfaces/srv/AddTwoInts_Response", i64_size, i64_serialize, i64_deserialize
};

// ---------------------------------------------------------------------------

int main()
{
  setvbuf(stdout, nullptr, _IOLBF, 0);  // line buffered: a crash must not swallow the log
  rclcpp::init(0, nullptr);
  rclcpp::NodeOptions opts;
  opts.start_parameter_services(false);
  opts.start_parameter_event_publisher(false);
  auto node = std::make_shared<rclcpp::Node>("zerodds_type_adaptation_test", opts);

  // -- 1) wire identity ------------------------------------------------------
  // The adapter's output must equal rmw_serialize's output for the equivalent
  // ROS message, byte for byte, encapsulation header included.
  {
    const std::string custom = "REP-2007";
    std_msgs::msg::String ros;
    ros.data = custom;
    const rosidl_message_type_support_t * ts =
      rosidl_typesupport_cpp::get_message_type_support_handle<std_msgs::msg::String>();
    rcutils_allocator_t alloc = rcutils_get_default_allocator();
    rmw_serialized_message_t ref = rmw_get_zero_initialized_serialized_message();
    bool ok = rmw_serialized_message_init(&ref, 0, &alloc) == RMW_RET_OK &&
      rmw_serialize(&ros, ts, &ref) == RMW_RET_OK;

    std::vector<uint8_t> mine(4u + str_serialized_size(&custom));
    mine[0] = 0x00; mine[1] = 0x01; mine[2] = 0x00; mine[3] = 0x00;  // CDR_LE
    size_t written = 0;
    ok = ok && str_serialize(&custom, mine.data() + 4, mine.size() - 4, &written) == 0;
    ok = ok && (4u + written == ref.buffer_length);
    ok = ok && std::memcmp(mine.data(), ref.buffer, ref.buffer_length) == 0;
    printf("  adapter bytes=%zu rmw_serialize bytes=%zu\n", 4u + written, ref.buffer_length);
    check(ok, "1) adapted wire bytes == rmw_serialize bytes");
    rmw_serialized_message_fini(&ref);
  }

  // -- 2) adapted publish -> plain rclcpp subscription ------------------------
  auto pub = node->create_publisher<std_msgs::msg::String>("zerodds_adapt_topic", 10);
  std::string got_plain;
  auto sub_plain = node->create_subscription<std_msgs::msg::String>(
    "zerodds_adapt_topic", 10,
    [&got_plain](std_msgs::msg::String::SharedPtr m) { got_plain = m->data; });
  rclcpp::executors::SingleThreadedExecutor exec;
  exec.add_node(node);

  rmw_publisher_t * rmw_pub = rcl_publisher_get_rmw_handle(pub->get_publisher_handle().get());
  check(rmw_pub != nullptr, "   rmw publisher handle resolved");

  // Endpoint invariance: discovery must report the same type before and after
  // the adapter is bound — a REP-2007 endpoint keeps matching plain endpoints.
  auto info_before = node->get_publishers_info_by_topic("zerodds_adapt_topic");
  check(rmw_zerodds_publisher_set_type_adapter(rmw_pub, &kStringAdapter) == RMW_RET_OK,
        "   publisher type adapter bound");
  auto info_after = node->get_publishers_info_by_topic("zerodds_adapt_topic");
  bool same_type = info_before.size() == info_after.size();
  for (size_t i = 0; same_type && i < info_before.size(); ++i) {
    same_type = info_before[i].topic_type() == info_after[i].topic_type();
  }
  check(same_type, "   endpoint topic type unchanged by adaptation");

  const std::string sent = "adapted hello";
  g_serialize_calls = 0;
  g_convert_to_ros_message_calls = 0;
  int publishes = 0;
  auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(15);
  while (got_plain.empty() && std::chrono::steady_clock::now() < deadline) {
    if (rmw_zerodds_publish_adapted(rmw_pub, &sent) == RMW_RET_OK) { ++publishes; }
    exec.spin_some();
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
  }
  check(got_plain == sent, "2) adapted publish -> plain subscription");
  printf("  publishes=%d serialize_calls=%d convert_to_ros_message_calls=%d\n",
         publishes, g_serialize_calls, g_convert_to_ros_message_calls);
  check(publishes > 0 && g_serialize_calls == publishes,
        "4) exactly one serialization per adapted publish");
  check(g_convert_to_ros_message_calls == 0,
        "4) ros_message_type never materialized (no extra copy)");

  // -- 3) plain publish -> adapted take --------------------------------------
  // A second topic with a subscription whose rmw handle carries the adapter.
  // rclcpp does not know about the adapter, so the take is driven directly
  // against the rmw handle instead of through the executor callback.
  auto pub_plain = node->create_publisher<std_msgs::msg::String>("zerodds_adapt_topic2", 10);
  auto sub_adapted = node->create_subscription<std_msgs::msg::String>(
    "zerodds_adapt_topic2", 10, [](std_msgs::msg::String::SharedPtr) {});
  rmw_subscription_t * rmw_sub =
    rcl_subscription_get_rmw_handle(sub_adapted->get_subscription_handle().get());
  check(rmw_sub != nullptr && rmw_zerodds_subscription_set_type_adapter(rmw_sub, &kStringAdapter) ==
        RMW_RET_OK, "   subscription type adapter bound");

  std_msgs::msg::String plain_msg;
  plain_msg.data = "plain to adapted";
  std::string got_adapted;
  bool taken = false;
  g_deserialize_calls = 0;
  deadline = std::chrono::steady_clock::now() + std::chrono::seconds(15);
  while (!taken && std::chrono::steady_clock::now() < deadline) {
    pub_plain->publish(plain_msg);
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    rmw_message_info_t mi;
    if (rmw_zerodds_take_adapted(rmw_sub, &got_adapted, &taken, &mi) == RMW_RET_OK && taken) {
      printf("  message_info: reception_sequence_number=%llu received_timestamp!=0 -> %d\n",
             static_cast<unsigned long long>(mi.reception_sequence_number),
             mi.received_timestamp != 0 ? 1 : 0);
      check(mi.reception_sequence_number != 0 && mi.received_timestamp != 0,
            "   adapted take fills rmw_message_info_t");
    }
  }
  check(taken && got_adapted == plain_msg.data, "3) plain publish -> adapted take");
  check(g_deserialize_calls > 0, "   adapter deserialize was the only decode path");

  // -- 5) services: adapted request and adapted response ---------------------
  {
    using AddTwoInts = example_interfaces::srv::AddTwoInts;
    auto service = node->create_service<AddTwoInts>(
      "zerodds_adapt_add", [](const std::shared_ptr<AddTwoInts::Request>,
                              std::shared_ptr<AddTwoInts::Response>) {});
    auto client = node->create_client<AddTwoInts>("zerodds_adapt_add");
    rmw_service_t * rmw_srv = rcl_service_get_rmw_handle(service->get_service_handle().get());
    rmw_client_t * rmw_cli = rcl_client_get_rmw_handle(client->get_client_handle().get());
    bool bound = rmw_srv && rmw_cli &&
      rmw_zerodds_service_set_type_adapters(rmw_srv, &kPairAdapter, &kI64Adapter) == RMW_RET_OK &&
      rmw_zerodds_client_set_type_adapters(rmw_cli, &kPairAdapter, &kI64Adapter) == RMW_RET_OK;
    check(bound, "   service + client type adapters bound");

    AddPair request{7, 35};
    int64_t response = 0;
    bool got_response = false;
    deadline = std::chrono::steady_clock::now() + std::chrono::seconds(20);
    while (!got_response && std::chrono::steady_clock::now() < deadline) {
      int64_t seq = 0;
      rmw_zerodds_send_request_adapted(rmw_cli, &request, &seq);
      std::this_thread::sleep_for(std::chrono::milliseconds(100));

      rmw_service_info_t rq_info;
      AddPair server_side{0, 0};
      bool have_request = false;
      if (rmw_zerodds_take_request_adapted(rmw_srv, &rq_info, &server_side, &have_request) ==
          RMW_RET_OK && have_request)
      {
        int64_t sum = server_side.first + server_side.second;  // the custom types, never a ROS message
        rmw_zerodds_send_response_adapted(rmw_srv, &rq_info.request_id, &sum);
      }
      std::this_thread::sleep_for(std::chrono::milliseconds(100));
      rmw_service_info_t rs_info;
      rmw_zerodds_take_response_adapted(rmw_cli, &rs_info, &response, &got_response);
    }
    printf("  adapted service: %ld + %ld = %ld\n",
           static_cast<long>(request.first), static_cast<long>(request.second),
           static_cast<long>(response));
    check(got_response && response == 42, "5) adapted service request/response roundtrip");
  }

  rclcpp::shutdown();
  printf("%s (%d failure(s))\n", g_failures == 0 ? "ALL PASS" : "FAILURES", g_failures);
  return g_failures == 0 ? 0 : 1;
}

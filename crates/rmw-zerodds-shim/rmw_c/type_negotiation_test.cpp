// End-to-end test for the rmw-layer half of REP-2009 (Type Negotiation).
//
// REP-2009 §"Implementation in the ROS 2 Core or in a Separate Project" states
// where the negotiation state machine belongs and what it assumes of the layers
// below it:
//
//   "While there are some advantages to having this project in the ROS 2 core,
//    all of the needed APIs are available to make this a separate package.
//    Thus, in the spirit of keeping the ROS 2 core smaller, this should be
//    implemented in a separate project."
//
// So the rmw obligation is not to run the algorithm — it is to make that
// sentence true. This test runs REP-2009's algorithm by hand over rmw_zerodds,
// step for step as §"Negotiation Algorithm" and §"Renegotiating Selected
// Message Types" describe it, and measures that each API the algorithm needs
// actually delivers:
//
//   1. NEGOTIATION TOPIC — "Receive the supported message priority maps from
//      all connected subscriptions." The priority map is an ordinary ROS
//      message on the negotiation topic: a sequence of nested structs carrying
//      strings. diagnostic_msgs/msg/DiagnosticArray has exactly that shape
//      (DiagnosticStatus[] with three strings and a nested KeyValue[]) and is
//      round-tripped field by field.
//   2. DISCOVERY — "Use discovery to find all the connected subscriptions that
//      are using the user specified negotiation topic." Measured through
//      count_subscribers and get_subscriptions_info_by_topic.
//   3. MATCHED COUNTS — the publisher side has to see that a negotiating
//      subscription is connected before it decides.
//   4. SELECTED MESSAGE TYPES — "The negotiating publisher then creates a
//      regular publisher for each supported message type." Two selected types
//      on two topics; discovery must report the right ROS type per topic,
//      because "the combination of the ROS message type and the string
//      identifier must form a unique tuple" (§"Defining Supported Message
//      Types") and the rmw half of that tuple is the type on the wire.
//   5. RENEGOTIATION — "If the selected messages must change, the negotiating
//      publisher destroys its publishers. […] create new publishers for the
//      selected message types." Destroy, verify the graph drops them, create a
//      different selected type, verify it appears and delivers.
//   6. GRAPH EVENT — a negotiating publisher has to *learn* that the network
//      changed, not poll for it. Measured with rclcpp's graph event on the
//      node's graph guard condition.
//
// Build + run: see run_type_negotiation_test.sh.
// PASS criterion: every line prints PASS and the exit code is 0.

#include <chrono>
#include <cstdio>
#include <cstring>
#include <memory>
#include <string>
#include <thread>
#include <vector>

#include <rclcpp/rclcpp.hpp>
#include <rcl/node.h>
#include <rmw/rmw.h>
#include <rosidl_typesupport_cpp/message_type_support.hpp>
#include <diagnostic_msgs/msg/diagnostic_array.hpp>
#include <std_msgs/msg/int32.hpp>
#include <std_msgs/msg/string.hpp>

static int g_failures = 0;
static void check(bool ok, const char * what)
{
  printf("%-58s %s\n", what, ok ? "PASS" : "FAIL");
  if (!ok) { ++g_failures; }
}

static const char * kNegotiationTopic = "zerodds_negotiation";
static const char * kTopicX = "zerodds_selected_x";
static const char * kTopicY = "zerodds_selected_y";

// One entry of a REP-2009 supported message priority map, carried in a
// DiagnosticStatus: name = ROS message type, message = supported type
// identifier, hardware_id = topic, values[0] = the priority.
static diagnostic_msgs::msg::DiagnosticStatus supported_type(
  const std::string & ros_type, const std::string & identifier,
  const std::string & topic, const std::string & priority)
{
  diagnostic_msgs::msg::DiagnosticStatus st;
  st.level = diagnostic_msgs::msg::DiagnosticStatus::OK;
  st.name = ros_type;
  st.message = identifier;
  st.hardware_id = topic;
  diagnostic_msgs::msg::KeyValue kv;
  kv.key = "priority";
  kv.value = priority;
  st.values.push_back(kv);
  return st;
}

static bool spin_until(
  rclcpp::executors::SingleThreadedExecutor & exec, std::chrono::seconds budget,
  const std::function<bool()> & done)
{
  auto deadline = std::chrono::steady_clock::now() + budget;
  while (std::chrono::steady_clock::now() < deadline) {
    if (done()) { return true; }
    exec.spin_some();
    std::this_thread::sleep_for(std::chrono::milliseconds(50));
  }
  return done();
}

int main()
{
  setvbuf(stdout, nullptr, _IOLBF, 0);  // line buffered: a crash must not swallow the log
  rclcpp::init(0, nullptr);
  rclcpp::NodeOptions opts;
  opts.start_parameter_services(false);
  opts.start_parameter_event_publisher(false);
  auto node = std::make_shared<rclcpp::Node>("zerodds_type_negotiation_test", opts);
  rclcpp::executors::SingleThreadedExecutor exec;
  exec.add_node(node);

  // -- 1) the negotiation topic carries the supported message priority map ----
  auto neg_pub = node->create_publisher<diagnostic_msgs::msg::DiagnosticArray>(
    kNegotiationTopic, 10);
  diagnostic_msgs::msg::DiagnosticArray received;
  bool got_map = false;
  auto neg_sub = node->create_subscription<diagnostic_msgs::msg::DiagnosticArray>(
    kNegotiationTopic, 10,
    [&received, &got_map](diagnostic_msgs::msg::DiagnosticArray::SharedPtr m) {
      received = *m;
      got_map = true;
    });

  diagnostic_msgs::msg::DiagnosticArray priority_map;
  priority_map.header.frame_id = "zerodds_type_negotiation_test";
  priority_map.status.push_back(
    supported_type("std_msgs/msg/String", "x", kTopicX, "10"));
  priority_map.status.push_back(
    supported_type("std_msgs/msg/Int32", "y", kTopicY, "5"));

  // Codec identity first: if the negotiation payload does not survive
  // serialize/deserialize, a delivery failure below would be ambiguous.
  {
    const rosidl_message_type_support_t * ts =
      rosidl_typesupport_cpp::get_message_type_support_handle<
        diagnostic_msgs::msg::DiagnosticArray>();
    rcutils_allocator_t alloc = rcutils_get_default_allocator();
    rmw_serialized_message_t buf = rmw_get_zero_initialized_serialized_message();
    bool ok = rmw_serialized_message_init(&buf, 0, &alloc) == RMW_RET_OK;
    ok = ok && rmw_serialize(&priority_map, ts, &buf) == RMW_RET_OK;
    diagnostic_msgs::msg::DiagnosticArray back;
    ok = ok && rmw_deserialize(&buf, ts, &back) == RMW_RET_OK;
    printf("  priority map CDR bytes=%zu, decoded entries=%zu\n",
           buf.buffer_length, back.status.size());
    for (size_t i = 0; i < back.status.size(); ++i) {
      printf("    [%zu] name='%s' identifier='%s' topic='%s' values=%zu\n", i,
             back.status[i].name.c_str(), back.status[i].message.c_str(),
             back.status[i].hardware_id.c_str(), back.status[i].values.size());
    }
    check(ok && back.status.size() == priority_map.status.size() &&
          !back.status.empty() && back.status[0].name == priority_map.status[0].name &&
          !back.status[0].values.empty() &&
          back.status[0].values[0].value == priority_map.status[0].values[0].value,
          "1) priority map survives serialize/deserialize");
    rmw_serialized_message_fini(&buf);
  }

  bool delivered = spin_until(exec, std::chrono::seconds(20), [&]() {
    if (!got_map) { neg_pub->publish(priority_map); }
    return got_map;
  });
  check(delivered, "1) supported message priority map delivered");
  bool same = delivered && received.status.size() == 2 &&
    received.header.frame_id == priority_map.header.frame_id;
  for (size_t i = 0; same && i < received.status.size(); ++i) {
    const auto & a = received.status[i];
    const auto & b = priority_map.status[i];
    same = a.level == b.level && a.name == b.name && a.message == b.message &&
      a.hardware_id == b.hardware_id && a.values.size() == b.values.size() &&
      !a.values.empty() && a.values[0].key == b.values[0].key &&
      a.values[0].value == b.values[0].value;
  }
  if (delivered) {
    printf("  map entries=%zu first=(%s, %s, prio %s)\n", received.status.size(),
           received.status.empty() ? "-" : received.status[0].name.c_str(),
           received.status.empty() ? "-" : received.status[0].message.c_str(),
           (received.status.empty() || received.status[0].values.empty())
             ? "-" : received.status[0].values[0].value.c_str());
  }
  check(same, "1) priority map round-trips field for field");

  // -- 2) discovery of the negotiation peers ---------------------------------
  bool discovered = spin_until(exec, std::chrono::seconds(10), [&]() {
    return node->count_subscribers(std::string("/") + kNegotiationTopic) >= 1;
  });
  auto sub_info = node->get_subscriptions_info_by_topic(std::string("/") + kNegotiationTopic);
  printf("  negotiation topic: count_subscribers=%zu endpoint_info=%zu type=%s\n",
         node->count_subscribers(std::string("/") + kNegotiationTopic), sub_info.size(),
         sub_info.empty() ? "-" : sub_info[0].topic_type().c_str());
  check(discovered, "2) negotiation subscription found through discovery");
  check(!sub_info.empty() &&
        sub_info[0].topic_type() == "diagnostic_msgs/msg/DiagnosticArray",
        "2) endpoint info names the negotiation message type");

  // -- 3) matched counts on the negotiation topic ----------------------------
  bool matched = spin_until(exec, std::chrono::seconds(10), [&]() {
    return neg_pub->get_subscription_count() >= 1;
  });
  printf("  negotiation publisher matched subscriptions=%zu\n",
         neg_pub->get_subscription_count());
  check(matched, "3) negotiating publisher sees the connected subscription");

  // -- 4) a regular publisher per selected message type ----------------------
  // Two selected types, two ROS message types, two topics — the case REP-2009
  // draws as N_1(x, y) publishing to N_2(x) and N_3(y).
  auto pub_x = node->create_publisher<std_msgs::msg::String>(kTopicX, 10);
  auto pub_y = node->create_publisher<std_msgs::msg::Int32>(kTopicY, 10);
  std::string got_x;
  int got_y = -1;
  auto sub_x = node->create_subscription<std_msgs::msg::String>(
    kTopicX, 10, [&got_x](std_msgs::msg::String::SharedPtr m) { got_x = m->data; });
  auto sub_y = node->create_subscription<std_msgs::msg::Int32>(
    kTopicY, 10, [&got_y](std_msgs::msg::Int32::SharedPtr m) { got_y = m->data; });

  std_msgs::msg::String msg_x;
  msg_x.data = "selected-x";
  std_msgs::msg::Int32 msg_y;
  msg_y.data = 7;
  bool both = spin_until(exec, std::chrono::seconds(20), [&]() {
    if (got_x.empty()) { pub_x->publish(msg_x); }
    if (got_y < 0) { pub_y->publish(msg_y); }
    return !got_x.empty() && got_y >= 0;
  });
  check(both && got_x == "selected-x" && got_y == 7,
        "4) both selected message types deliver on their topics");

  auto info_x = node->get_publishers_info_by_topic(std::string("/") + kTopicX);
  auto info_y = node->get_publishers_info_by_topic(std::string("/") + kTopicY);
  printf("  selected types: %s -> %s, %s -> %s\n", kTopicX,
         info_x.empty() ? "-" : info_x[0].topic_type().c_str(), kTopicY,
         info_y.empty() ? "-" : info_y[0].topic_type().c_str());
  check(!info_x.empty() && info_x[0].topic_type() == "std_msgs/msg/String" &&
        !info_y.empty() && info_y[0].topic_type() == "std_msgs/msg/Int32",
        "4) discovery reports the right ROS type per selected type");

  // -- 6) graph event: the network change has to be announced ----------------
  // Armed here, checked after the renegotiation below destroys and recreates
  // publishers — that is the change a negotiating publisher must react to.
  auto graph_event = node->get_graph_event();

  // -- 5) renegotiation: destroy the publishers, select a different type ------
  // Driven on the rmw handles rather than through rclcpp: an rclcpp publisher
  // stays alive as long as its QoS event handlers sit in a callback group, so
  // resetting the shared_ptr would not reach rmw_destroy_publisher at all. The
  // REP-2009 renegotiation step *is* rmw_create_publisher/rmw_destroy_publisher,
  // so that is what is measured.
  rcl_node_t * rcl_node = node->get_node_base_interface()->get_rcl_node_handle();
  rmw_node_t * rmw_node = rcl_node_get_rmw_handle(rcl_node);
  check(rmw_node != nullptr, "   rmw node handle resolved");

  const char * kTopicZ = "/zerodds_renegotiated_z";
  rmw_qos_profile_t qos = rmw_qos_profile_default;
  rmw_publisher_options_t pub_opts = rmw_get_default_publisher_options();
  const rosidl_message_type_support_t * ts_i32 =
    rosidl_typesupport_cpp::get_message_type_support_handle<std_msgs::msg::Int32>();
  const rosidl_message_type_support_t * ts_str =
    rosidl_typesupport_cpp::get_message_type_support_handle<std_msgs::msg::String>();

  rmw_publisher_t * sel_a = rmw_create_publisher(rmw_node, ts_i32, kTopicZ, &qos, &pub_opts);
  check(sel_a != nullptr, "5) a regular publisher per selected message type");
  size_t before = 0;
  spin_until(exec, std::chrono::seconds(10), [&]() {
    before = node->count_publishers(kTopicZ);
    return before >= 1;
  });
  auto info_a = node->get_publishers_info_by_topic(kTopicZ);
  printf("  selected type on %s: count=%zu type=%s\n", kTopicZ, before,
         info_a.empty() ? "-" : info_a[0].topic_type().c_str());
  check(before >= 1 && !info_a.empty() && info_a[0].topic_type() == "std_msgs/msg/Int32",
        "5) the selected type appears in the graph with its ROS type");

  // Let discovery settle so a late announcement cannot be mistaken for a
  // leftover of the destroy below.
  spin_until(exec, std::chrono::seconds(3), []() { return false; });
  check(rmw_destroy_publisher(rmw_node, sel_a) == RMW_RET_OK, "   selected publisher destroyed");
  bool dropped = spin_until(exec, std::chrono::seconds(15), [&]() {
    return node->count_publishers(kTopicZ) == 0;
  });
  printf("  after destroy: count_publishers(%s)=%zu, endpoint_info=%zu\n", kTopicZ,
         node->count_publishers(kTopicZ),
         node->get_publishers_info_by_topic(kTopicZ).size());
  check(dropped, "5) destroyed publishers leave the graph");

  // The renegotiated selection: the same topic now carries a String.
  rmw_publisher_t * sel_b = rmw_create_publisher(rmw_node, ts_str, kTopicZ, &qos, &pub_opts);
  bool reappeared = sel_b != nullptr && spin_until(exec, std::chrono::seconds(10), [&]() {
    return node->count_publishers(kTopicZ) >= 1;
  });
  auto info_b = node->get_publishers_info_by_topic(kTopicZ);
  printf("  after renegotiation: %s -> %s\n", kTopicZ,
         info_b.empty() ? "-" : info_b[0].topic_type().c_str());
  check(reappeared && !info_b.empty() && info_b[0].topic_type() == "std_msgs/msg/String",
        "5) discovery reports the renegotiated ROS type");

  // The renegotiated publisher still delivers to a matching subscription.
  std::string got_z;
  auto sub_z = node->create_subscription<std_msgs::msg::String>(
    kTopicZ, 10, [&got_z](std_msgs::msg::String::SharedPtr m) { got_z = m->data; });
  std_msgs::msg::String msg_z;
  msg_z.data = "renegotiated-z";
  bool renegotiated = spin_until(exec, std::chrono::seconds(20), [&]() {
    if (got_z.empty() && sel_b) { rmw_publish(sel_b, &msg_z, nullptr); }
    return got_z == "renegotiated-z";
  });
  check(renegotiated, "5) the renegotiated selected type delivers");

  // -- 6) the graph change was announced, not polled -------------------------
  // The event was armed before the renegotiation. wait_for_graph_change blocks
  // on the node's graph guard condition and returns when the rmw implementation
  // raises it (it throws only on an rcl error, never on a plain timeout), so the
  // event flag afterwards is the measurement.
  bool announced = graph_event->check_and_clear();
  auto graph_deadline = std::chrono::steady_clock::now() + std::chrono::seconds(15);
  while (!announced && std::chrono::steady_clock::now() < graph_deadline) {
    node->wait_for_graph_change(graph_event, std::chrono::seconds(1));
    announced = graph_event->check_and_clear();
  }
  printf("  graph event raised: %d\n", announced ? 1 : 0);
  check(announced, "6) the graph change raised the node graph guard");

  rclcpp::shutdown();
  printf("%s (%d failure(s))\n", g_failures == 0 ? "ALL PASS" : "FAILURES", g_failures);
  return g_failures == 0 ? 0 : 1;
}

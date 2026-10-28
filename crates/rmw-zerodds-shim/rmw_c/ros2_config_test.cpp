// End-to-end test: the ros2 section of the runtime configuration reaches the
// DDS endpoints the RMW creates.
//
// The runner installs a zerodds.d fragment with
//   ros2.topic_mangling: none
//   ros2.topic_overrides: /zerodds_cfg_topic -> durability transient_local,
//                         history keep_last, depth 1
// and runs, one process per role:
//   ros2_config_test pub        requests VOLATILE KEEP_ALL; the override makes
//                               it TRANSIENT_LOCAL KEEP_LAST(1), which
//                               get_actual_qos must report; publishes 1..5,
//                               then keeps the node alive for 8 s
//   ros2_config_test pub-avoid  TRANSIENT_LOCAL KEEP_LAST(1) with
//                               avoid_ros_namespace_conventions, 1..5
//   ros2_config_test sub        the pub request under the same override: a
//                               late joiner that must receive exactly [5]
//   ros2_config_test sub-avoid  TRANSIENT_LOCAL KEEP_LAST(1) with
//                               avoid_ros_namespace_conventions: [5]
//   ros2_config_test list-raw NAME
//                               the DDS topic names discovery reports, not
//                               demangled; NAME must be among them
// Build + run: see run_ros2_config_test.sh.
#include <chrono>
#include <cstdio>
#include <cstring>
#include <thread>
#include <vector>
#include <rclcpp/rclcpp.hpp>
#include <rcl/graph.h>
#include <rcl/publisher.h>
#include <rcl/subscription.h>
#include <rmw/rmw.h>
#include <std_msgs/msg/int32.hpp>

static int g_failures = 0;
static void check(bool ok, const char * what)
{
  printf("%-58s %s\n", what, ok ? "PASS" : "FAIL");
  if (!ok) { ++g_failures; }
}

static const char * kTopic = "zerodds_cfg_topic";

static rclcpp::QoS volatile_keep_all()
{
  return rclcpp::QoS(rclcpp::KeepAll()).reliable().durability_volatile();
}

static void check_overridden(const rmw_qos_profile_t & actual, const char * who)
{
  char what[96];
  snprintf(what, sizeof what, "%s actual durability TRANSIENT_LOCAL (override)", who);
  check(actual.durability == RMW_QOS_POLICY_DURABILITY_TRANSIENT_LOCAL, what);
  snprintf(what, sizeof what, "%s actual KEEP_LAST(1) (override)", who);
  check(actual.history == RMW_QOS_POLICY_HISTORY_KEEP_LAST && actual.depth == 1, what);
  snprintf(what, sizeof what, "%s actual RELIABLE (requested, not overridden)", who);
  check(actual.reliability == RMW_QOS_POLICY_RELIABILITY_RELIABLE, what);
}

static void spin_for(const rclcpp::Node::SharedPtr & node, int seconds)
{
  rclcpp::executors::SingleThreadedExecutor exec;
  exec.add_node(node);
  auto end = std::chrono::steady_clock::now() + std::chrono::seconds(seconds);
  while (std::chrono::steady_clock::now() < end) {
    exec.spin_some();
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
  }
}

static rclcpp::QoS tl_keep_last_1_avoid()
{
  return rclcpp::QoS(rclcpp::KeepLast(1)).reliable().transient_local()
         .avoid_ros_namespace_conventions(true);
}

static void run_pub(bool avoid)
{
  auto node = std::make_shared<rclcpp::Node>("zerodds_cfg_pub");
  auto pub = node->create_publisher<std_msgs::msg::Int32>(
    kTopic, avoid ? tl_keep_last_1_avoid() : volatile_keep_all());
  if (!avoid) {
    rmw_qos_profile_t actual;
    rmw_publisher_t * rp = rcl_publisher_get_rmw_handle(pub->get_publisher_handle().get());
    check(rmw_publisher_get_actual_qos(rp, &actual) == RMW_RET_OK, "pub actual qos returned");
    check_overridden(actual, "pub");
  }
  for (int n = 1; n <= 5; ++n) {
    std_msgs::msg::Int32 m;
    m.data = n;
    pub->publish(m);
  }
  printf("published 1..5\n");
  fflush(stdout);
  spin_for(node, 8);
}

static void run_sub(bool avoid)
{
  auto node = std::make_shared<rclcpp::Node>("zerodds_cfg_sub");
  std::vector<int> got;
  rclcpp::QoS qos = avoid ? tl_keep_last_1_avoid() : volatile_keep_all();
  auto sub = node->create_subscription<std_msgs::msg::Int32>(
    kTopic, qos, [&got](std_msgs::msg::Int32::SharedPtr m) { got.push_back(m->data); });
  if (!avoid) {
    rmw_qos_profile_t actual;
    rmw_subscription_t * rs = rcl_subscription_get_rmw_handle(sub->get_subscription_handle().get());
    check(rmw_subscription_get_actual_qos(rs, &actual) == RMW_RET_OK, "sub actual qos returned");
    check_overridden(actual, "sub");
  }
  spin_for(node, 4);
  printf("received:");
  for (int v : got) { printf(" %d", v); }
  printf("\n");
  check(got.size() == 1 && got[0] == 5, "late joiner receives exactly the newest sample [5]");
}

static void run_list_raw(const char * expect)
{
  auto node = std::make_shared<rclcpp::Node>("zerodds_cfg_list");
  spin_for(node, 3);
  rcl_allocator_t alloc = rcl_get_default_allocator();
  rcl_names_and_types_t nt = rmw_get_zero_initialized_names_and_types();
  const rcl_ret_t ret = rcl_get_topic_names_and_types(
    node->get_node_base_interface()->get_rcl_node_handle(), &alloc, true, &nt);
  check(ret == RCL_RET_OK, "topic names and types returned");
  bool found = false;
  printf("raw topics:");
  for (size_t i = 0; ret == RCL_RET_OK && i < nt.names.size; ++i) {
    printf(" %s", nt.names.data[i]);
    found = found || std::strcmp(nt.names.data[i], expect) == 0;
  }
  printf("\n");
  char what[96];
  snprintf(what, sizeof what, "DDS topic %s discovered", expect);
  check(found, what);
  (void) rcl_names_and_types_fini(&nt);
}

int main(int argc, char ** argv)
{
  setvbuf(stdout, nullptr, _IOLBF, 0);
  rclcpp::init(0, nullptr);
  const char * mode = argc > 1 ? argv[1] : "sub";
  if (std::strcmp(mode, "pub") == 0 || std::strcmp(mode, "pub-avoid") == 0) {
    run_pub(std::strcmp(mode, "pub-avoid") == 0);
  } else if (std::strcmp(mode, "list-raw") == 0) {
    run_list_raw(argc > 2 ? argv[2] : kTopic);
  } else {
    run_sub(std::strcmp(mode, "sub-avoid") == 0);
  }
  rclcpp::shutdown();
  if (g_failures == 0) {
    printf("ALL PASS (0 failure(s))\n");
    return 0;
  }
  printf("FAILURES (%d failure(s))\n", g_failures);
  return 1;
}

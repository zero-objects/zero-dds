// End-to-end test: an rclcpp QoS profile reaches the DDS endpoint whole.
//
// The RMW used to pass only reliability to the DDS side; durability, history,
// deadline, lifespan and liveliness were dropped, and get_actual_qos reported
// the request as if it had been applied. A TRANSIENT_LOCAL KEEP_LAST(1)
// publisher therefore served a late joiner nothing.
//
// Two processes, so the late joiner is replayed over RTPS and not through the
// same-runtime path:
//   qos_translation_test pub   publishes 1..5 TRANSIENT_LOCAL KEEP_LAST(1),
//                              then keeps the node alive for 8 s
//   qos_translation_test sub   started after the publisher has written; a
//                              TRANSIENT_LOCAL KEEP_LAST(1) subscription must
//                              receive exactly [5]
// Both check rmw_*_get_actual_qos; pub also checks wait_for_all_acked.
//
// Build + run: see run_qos_translation_test.sh.
#include <chrono>
#include <cstdio>
#include <cstring>
#include <thread>
#include <vector>
#include <rclcpp/rclcpp.hpp>
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

static rclcpp::QoS tl_keep_last_1()
{
  return rclcpp::QoS(rclcpp::KeepLast(1)).reliable().transient_local();
}

static int run_pub()
{
  auto node = std::make_shared<rclcpp::Node>("zerodds_qos_pub");
  auto pub = node->create_publisher<std_msgs::msg::Int32>("zerodds_qos_topic", tl_keep_last_1());
  rmw_qos_profile_t actual;
  rmw_publisher_t * rp = rcl_publisher_get_rmw_handle(pub->get_publisher_handle().get());
  check(rmw_publisher_get_actual_qos(rp, &actual) == RMW_RET_OK, "pub actual qos returned");
  check(actual.durability == RMW_QOS_POLICY_DURABILITY_TRANSIENT_LOCAL, "pub actual durability TRANSIENT_LOCAL");
  check(actual.history == RMW_QOS_POLICY_HISTORY_KEEP_LAST && actual.depth == 1, "pub actual KEEP_LAST(1)");
  for (int n = 1; n <= 5; ++n) {
    std_msgs::msg::Int32 m;
    m.data = n;
    pub->publish(m);
  }
  check(rmw_publisher_wait_for_all_acked(rp, rmw_time_t{1, 0}) == RMW_RET_OK,
        "wait_for_all_acked with no subscription returns OK");
  printf("published 1..5\n");
  fflush(stdout);
  rclcpp::executors::SingleThreadedExecutor exec;
  exec.add_node(node);
  auto end = std::chrono::steady_clock::now() + std::chrono::seconds(8);
  while (std::chrono::steady_clock::now() < end) {
    exec.spin_some();
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
  }
  return 0;
}

static int run_sub()
{
  auto node = std::make_shared<rclcpp::Node>("zerodds_qos_sub");
  std::vector<int> got;
  auto sub = node->create_subscription<std_msgs::msg::Int32>(
    "zerodds_qos_topic", tl_keep_last_1(),
    [&got](std_msgs::msg::Int32::SharedPtr m) { got.push_back(m->data); });
  rmw_qos_profile_t actual;
  rmw_subscription_t * rs = rcl_subscription_get_rmw_handle(sub->get_subscription_handle().get());
  check(rmw_subscription_get_actual_qos(rs, &actual) == RMW_RET_OK, "sub actual qos returned");
  check(actual.durability == RMW_QOS_POLICY_DURABILITY_TRANSIENT_LOCAL, "sub actual durability TRANSIENT_LOCAL");
  rclcpp::executors::SingleThreadedExecutor exec;
  exec.add_node(node);
  auto end = std::chrono::steady_clock::now() + std::chrono::seconds(4);
  while (std::chrono::steady_clock::now() < end) {
    exec.spin_some();
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
  }
  printf("received:");
  for (int v : got) { printf(" %d", v); }
  printf("\n");
  check(got.size() == 1 && got[0] == 5, "late joiner receives exactly the newest sample [5]");
  return 0;
}

int main(int argc, char ** argv)
{
  setvbuf(stdout, nullptr, _IOLBF, 0);
  rclcpp::init(0, nullptr);
  const bool publisher = argc > 1 && std::strcmp(argv[1], "pub") == 0;
  if (publisher) { run_pub(); } else { run_sub(); }
  rclcpp::shutdown();
  if (g_failures == 0) {
    printf("ALL PASS (0 failure(s))\n");
    return 0;
  }
  printf("FAILURES (%d failure(s))\n", g_failures);
  return 1;
}

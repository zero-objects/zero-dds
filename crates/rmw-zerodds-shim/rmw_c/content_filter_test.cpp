// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// rmw_zerodds_cpp content filters through rclcpp (rclpy of Humble has
// none): a filter given at creation, one set later with
// set_content_filter, get_content_filter, and the message info the take
// reports (source timestamp, publication sequence number) for a message
// from another context, i.e. over RTPS.
//
// Run by run_content_filter_test.sh.

#include <chrono>
#include <cstdio>
#include <memory>
#include <string>
#include <thread>
#include <vector>

#include "rclcpp/rclcpp.hpp"
#include "std_msgs/msg/int32.hpp"

using namespace std::chrono_literals;

static int failures = 0;
#define EXPECT(cond, what)                                              \
  do {                                                                  \
    if (!(cond)) {                                                      \
      std::fprintf(stderr, "FAIL %s (%s:%d)\n", what, __FILE__, __LINE__); \
      ++failures;                                                       \
    } else {                                                            \
      std::printf("ok   %s\n", what);                                   \
    }                                                                   \
  } while (0)

template <typename Pred>
static bool spin_until(rclcpp::executors::SingleThreadedExecutor & exec, Pred pred,
                       std::chrono::milliseconds timeout)
{
  auto deadline = std::chrono::steady_clock::now() + timeout;
  while (std::chrono::steady_clock::now() < deadline) {
    exec.spin_some(20ms);
    if (pred()) { return true; }
    std::this_thread::sleep_for(5ms);
  }
  return pred();
}

int main(int argc, char ** argv)
{
  // Two contexts: the publisher's samples reach the subscription over RTPS,
  // with an INFO_TS source timestamp and a writer sequence number.
  auto ctx_pub = std::make_shared<rclcpp::Context>();
  auto ctx_sub = std::make_shared<rclcpp::Context>();
  ctx_pub->init(argc, argv);
  ctx_sub->init(argc, argv);
  rclcpp::NodeOptions opt_pub;
  opt_pub.context(ctx_pub);
  rclcpp::NodeOptions opt_sub;
  opt_sub.context(ctx_sub);
  auto talker = std::make_shared<rclcpp::Node>("zerodds_cft_talker", opt_pub);
  auto listener = std::make_shared<rclcpp::Node>("zerodds_cft_listener", opt_sub);
  rclcpp::ExecutorOptions eo_pub;
  eo_pub.context = ctx_pub;
  rclcpp::ExecutorOptions eo_sub;
  eo_sub.context = ctx_sub;
  rclcpp::executors::SingleThreadedExecutor exec_pub(eo_pub);
  rclcpp::executors::SingleThreadedExecutor exec_sub(eo_sub);
  exec_pub.add_node(talker);
  exec_sub.add_node(listener);

  auto pub = talker->create_publisher<std_msgs::msg::Int32>("zerodds_cft_topic", 20);

  std::vector<int> received;
  std::vector<rclcpp::MessageInfo> infos;
  rclcpp::SubscriptionOptions so;
  so.content_filter_options.filter_expression = "data > %0";
  so.content_filter_options.expression_parameters = {"5"};
  auto sub = listener->create_subscription<std_msgs::msg::Int32>(
    "zerodds_cft_topic", 20,
    [&](const std_msgs::msg::Int32 & m, const rclcpp::MessageInfo & info) {
      received.push_back(m.data);
      infos.push_back(info);
    },
    so);

  EXPECT(sub->is_cft_enabled(), "the subscription reports its content filter");
  EXPECT(spin_until(exec_sub, [&] { return pub->get_subscription_count() >= 1; }, 10s),
         "publisher and subscription match");

  const int64_t before = std::chrono::duration_cast<std::chrono::nanoseconds>(
    std::chrono::system_clock::now().time_since_epoch()).count();
  for (int i = 0; i < 10; ++i) {
    std_msgs::msg::Int32 m;
    m.data = i;
    pub->publish(m);
  }
  spin_until(exec_sub, [&] { return received.size() >= 4; }, 5s);
  spin_until(exec_sub, [] { return false; }, 300ms);
  EXPECT((received == std::vector<int>{6, 7, 8, 9}), "data > 5 lets 6..9 through");
  if (received != std::vector<int>{6, 7, 8, 9}) {
    for (int v : received) { std::fprintf(stderr, "  received %d\n", v); }
  }

  auto options = sub->get_content_filter();
  EXPECT(options.filter_expression == "data > %0", "get_content_filter returns the expression");
  EXPECT((options.expression_parameters == std::vector<std::string>{"5"}),
         "get_content_filter returns the parameters");

  bool timestamps = !infos.empty();
  bool sequences = !infos.empty();
  for (const auto & info : infos) {
    const auto & rmw = info.get_rmw_message_info();
    timestamps = timestamps && rmw.source_timestamp > before - 5000000000LL &&
      rmw.source_timestamp <= rmw.received_timestamp;
    sequences = sequences &&
      rmw.publication_sequence_number != RMW_MESSAGE_INFO_SEQUENCE_NUMBER_UNSUPPORTED;
  }
  EXPECT(timestamps, "message info carries the writer's source timestamp");
  EXPECT(sequences, "message info carries the writer's sequence number");

  // A filter set later applies to what arrives after it.
  sub->set_content_filter("data < %0", {"2"});
  received.clear();
  for (int i = 0; i < 10; ++i) {
    std_msgs::msg::Int32 m;
    m.data = i;
    pub->publish(m);
  }
  spin_until(exec_sub, [&] { return received.size() >= 2; }, 5s);
  spin_until(exec_sub, [] { return false; }, 300ms);
  EXPECT((received == std::vector<int>{0, 1}), "set_content_filter(data < 2) lets 0..1 through");

  // An empty expression removes the filter.
  sub->set_content_filter("");
  EXPECT(!sub->is_cft_enabled(), "an empty expression removes the filter");

  rclcpp::shutdown(ctx_pub);
  rclcpp::shutdown(ctx_sub);
  std::printf(failures == 0 ? "ALL OK\n" : "FAILURES: %d\n", failures);
  return failures == 0 ? 0 : 1;
}

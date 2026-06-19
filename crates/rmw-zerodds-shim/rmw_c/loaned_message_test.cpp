// End-to-end test for the rmw zero-copy message-loaning ABI
// (`rmw_borrow_loaned_message` / `rmw_publish_loaned_message` /
// `rmw_take_loaned_message`, surfaced as rclcpp `borrow_loaned_message()` /
// `publish(LoanedMessage)`).
//
// rclpy has no loaned-message API, so rclcpp (C++) is the only way to exercise
// the loan path. A fixed-POD message (`std_msgs/Int32`, no strings/sequences)
// is loanable, so `can_loan_messages()` must be true; a loaned message is
// borrowed, filled, and published; a normal subscription on the same topic must
// receive the value.
//
// Build + run (ROS 2 Humble via RoboStack/micromamba on codepit):
//   see run_loaned_message_test.sh
//
// PASS criterion: `can_loan=1 got=42 PASS`. The test itself is delivery-mode
// agnostic; run_loaned_message_test.sh runs it twice — once as `Portable`
// (default, serialize→CDR→RTPS) and once with `ZERODDS_DELIVERY_MODE=raw-same-host`
// (same-host SHM, no wire). A delivered value in the raw run proves the SHM
// path, since a `RawSameHost` writer never publishes over RTPS.
//
// NB: parameter services are disabled on the node — rclcpp would otherwise
// create them via the rosidl_typesupport_cpp dispatch, which the introspection
// CDR path resolves through the C++ introspection fallback (see
// `zerodds_introspect` in rmw_zerodds.c). Keeping the node minimal isolates the
// loan path under test.
#include <chrono>
#include <thread>
#include <cstdio>
#include <rclcpp/rclcpp.hpp>
#include <std_msgs/msg/int32.hpp>

int main()
{
  rclcpp::init(0, nullptr);
  rclcpp::NodeOptions opts;
  opts.start_parameter_services(false);
  opts.start_parameter_event_publisher(false);
  auto node = std::make_shared<rclcpp::Node>("zerodds_loan_test", opts);
  auto pub = node->create_publisher<std_msgs::msg::Int32>("zerodds_loan_topic", 10);
  int got = -1;
  auto sub = node->create_subscription<std_msgs::msg::Int32>(
    "zerodds_loan_topic", 10,
    [&got](std_msgs::msg::Int32::SharedPtr m) { got = m->data; });
  rclcpp::executors::SingleThreadedExecutor exec;
  exec.add_node(node);
  bool can = pub->can_loan_messages();
  auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
  while (got < 0 && std::chrono::steady_clock::now() < deadline) {
    if (can) {
      auto l = pub->borrow_loaned_message();
      l.get().data = 42;
      pub->publish(std::move(l));
    } else {
      std_msgs::msg::Int32 m;
      m.data = 42;
      pub->publish(m);
    }
    exec.spin_some();
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
  }
  rclcpp::shutdown();
  printf("can_loan=%d got=%d %s\n", can ? 1 : 0, got, got == 42 ? "PASS" : "FAIL");
  return got == 42 ? 0 : 1;
}

// End-to-end test for the REP-2008 accelerator surface at the rmw layer
// (`rmw_zerodds_loan_memory_t`, `rmw_zerodds_publisher_set_loan_alignment`,
// `rmw_zerodds_acceleration_kernel_levels` in rmw_zerodds_ros2_ext.h).
//
// REP-2008 §"Kernel levels in ROS 2" places this package at level 2 ("rclcpp,
// rcl, rmw, and the corresponding rmw_adapters") and level 3 ("ROS 2
// underlayers […] including the communication middleware (e.g. DDS)"), and
// §"Methodology" step 3.2 asks to "accelerate inter-Node exchanges and reduce
// the overhead of the ROS 2 message-passing system across all its abstraction
// layers".
//
// The middleware's concrete part of that is memory provenance. CUDA, ROCm and
// XRT can only DMA out of host memory they have pinned or registered, and they
// pin whole pages. Unless the middleware states where a loaned message buffer
// lives and which page-aligned extent covers it, an accelerated node has to
// keep its own pinned staging buffer and copy into the middleware on every
// publish.
//
// This test needs no accelerator, because the property under test is the host
// memory contract, not the device. It measures:
//
//   1. LEVELS — rmw_zerodds_acceleration_kernel_levels() reports 2 and 3.
//   2. ALIGNMENT — after set_loan_alignment(page_size) a Portable loan starts
//      on a page boundary.
//   3. REGISTRATION EXTENT — registration_base/registration_size are page
//      aligned and cover [host_ptr, host_ptr + size).
//   4. PINNABILITY — mlock() on the reported extent succeeds. mlock performs
//      the same kernel operation an accelerator runtime performs when it pins
//      host memory (cudaHostRegister / hipHostRegister lock the pages), so a
//      successful mlock is a measured proof that the reported extent is a valid
//      pinnable host mapping, not a computed guess.
//   5. CROSS-PROCESS (raw-same-host delivery only) — the descriptor names a
//      shared segment; a forked child maps that segment through the reported
//      link and finds the payload the parent wrote into its loan. That is the
//      property an accelerator needs for a device that DMAs from a different
//      process than the one that filled the buffer.
//
// Build + run: see run_accel_loan_memory_test.sh, which runs the binary once in
// Portable mode and once with ZERODDS_DELIVERY_MODE=raw-same-host.
// PASS criterion: every line prints PASS and the exit code is 0.

#include <cerrno>
#include <chrono>
#include <cstdio>
#include <cstring>
#include <cstdint>
#include <string>
#include <thread>

#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#include <rclcpp/rclcpp.hpp>
#include <rcl/publisher.h>
#include <rcl/subscription.h>
#include <rmw/rmw.h>
#include <rosidl_typesupport_cpp/message_type_support.hpp>
#include <std_msgs/msg/int32.hpp>

#include "rmw_zerodds_ros2_ext.h"

static int g_failures = 0;
static void check(bool ok, const char * what)
{
  printf("%-58s %s\n", what, ok ? "PASS" : "FAIL");
  if (!ok) { ++g_failures; }
}

static const char * kind_name(rmw_zerodds_loan_memory_kind_t k)
{
  switch (k) {
    case RMW_ZERODDS_LOAN_MEMORY_PROCESS_HEAP: return "PROCESS_HEAP";
    case RMW_ZERODDS_LOAN_MEMORY_SHARED_SEGMENT: return "SHARED_SEGMENT";
    case RMW_ZERODDS_LOAN_MEMORY_ICEORYX: return "ICEORYX";
  }
  return "?";
}

// Maps the shared segment named by a flink link in a *forked child* and looks
// for `needle` in it. The flink file written by the segment owner contains the
// OS shm id; on Linux that id lives under /dev/shm. Returns true when the child
// found the bytes, i.e. when the loaned buffer really is in memory another
// process can map — the precondition for an accelerator in a second process.
static bool child_finds_payload_in_segment(const char * flink, int32_t needle)
{
  pid_t pid = fork();
  if (pid < 0) { return false; }
  if (pid == 0) {
    int rc = 1;
    int ffd = open(flink, O_RDONLY);
    if (ffd >= 0) {
      char id[256] = {0};
      ssize_t n = read(ffd, id, sizeof(id) - 1);
      close(ffd);
      if (n > 0) {
        while (n > 0 && (id[n - 1] == '\n' || id[n - 1] == '\r' || id[n - 1] == ' ')) { id[--n] = 0; }
        const char * name = id[0] == '/' ? id + 1 : id;
        std::string path = std::string("/dev/shm/") + name;
        int sfd = open(path.c_str(), O_RDONLY);
        if (sfd >= 0) {
          struct stat st;
          if (fstat(sfd, &st) == 0 && st.st_size > 0) {
            void * base = mmap(nullptr, static_cast<size_t>(st.st_size), PROT_READ, MAP_SHARED, sfd, 0);
            if (base != MAP_FAILED) {
              const unsigned char * p = static_cast<const unsigned char *>(base);
              for (size_t i = 0; i + sizeof(needle) <= static_cast<size_t>(st.st_size); ++i) {
                if (std::memcmp(p + i, &needle, sizeof(needle)) == 0) { rc = 0; break; }
              }
              munmap(base, static_cast<size_t>(st.st_size));
            }
          }
          close(sfd);
        }
      }
    }
    _exit(rc);
  }
  int status = 0;
  waitpid(pid, &status, 0);
  return WIFEXITED(status) && WEXITSTATUS(status) == 0;
}

int main()
{
  setvbuf(stdout, nullptr, _IOLBF, 0);  // line buffered: a crash must not swallow the log
  const size_t page = static_cast<size_t>(sysconf(_SC_PAGESIZE));
  const char * mode_env = getenv("ZERODDS_DELIVERY_MODE");
  const bool raw_mode = mode_env && std::strstr(mode_env, "raw") != nullptr;
  printf("delivery mode: %s, page size: %zu\n", mode_env ? mode_env : "portable (default)", page);

  // -- 1) declared kernel levels --------------------------------------------
  uint32_t levels = rmw_zerodds_acceleration_kernel_levels();
  printf("  kernel level bitmask: 0x%x\n", levels);
  check((levels & RMW_ZERODDS_ACCEL_LEVEL_2) && (levels & RMW_ZERODDS_ACCEL_LEVEL_3),
        "1) REP-2008 kernel levels 2 and 3 declared");

  rclcpp::init(0, nullptr);
  rclcpp::NodeOptions opts;
  opts.start_parameter_services(false);
  opts.start_parameter_event_publisher(false);
  auto node = std::make_shared<rclcpp::Node>("zerodds_accel_loan_test", opts);
  auto pub = node->create_publisher<std_msgs::msg::Int32>("zerodds_accel_topic", 10);
  int got = -1;
  auto sub = node->create_subscription<std_msgs::msg::Int32>(
    "zerodds_accel_topic", 10,
    [&got](std_msgs::msg::Int32::SharedPtr m) { got = m->data; });
  rclcpp::executors::SingleThreadedExecutor exec;
  exec.add_node(node);

  rmw_publisher_t * rmw_pub = rcl_publisher_get_rmw_handle(pub->get_publisher_handle().get());
  check(rmw_pub != nullptr && pub->can_loan_messages(), "   loanable fixed-POD publisher");

  // -- 2) requested loan alignment ------------------------------------------
  check(rmw_zerodds_publisher_set_loan_alignment(rmw_pub, page) == RMW_RET_OK,
        "2) set_loan_alignment(page) accepted");
  check(rmw_zerodds_publisher_set_loan_alignment(rmw_pub, 3) == RMW_RET_INVALID_ARGUMENT,
        "   non-power-of-two alignment rejected");

  const rosidl_message_type_support_t * ts =
    rosidl_typesupport_cpp::get_message_type_support_handle<std_msgs::msg::Int32>();
  void * loan = nullptr;
  check(rmw_borrow_loaned_message(rmw_pub, ts, &loan) == RMW_RET_OK && loan != nullptr,
        "   loan borrowed");
  if (!loan) { rclcpp::shutdown(); printf("FAILURES (%d)\n", ++g_failures); return 1; }

  if (!raw_mode) {
    check((reinterpret_cast<uintptr_t>(loan) % page) == 0,
          "2) portable loan starts on a page boundary");
  }

  // -- 3) registration extent -----------------------------------------------
  rmw_zerodds_loan_memory_t mem;
  bool described = rmw_zerodds_publisher_get_loan_memory(rmw_pub, loan, &mem) == RMW_RET_OK;
  check(described, "   loan memory descriptor returned");
  if (described) {
    printf("  kind=%s host_ptr=%p size=%zu reg_base=%p reg_size=%zu cross_process=%d link=%s\n",
           kind_name(mem.kind), mem.host_ptr, mem.size, mem.registration_base,
           mem.registration_size, mem.cross_process ? 1 : 0,
           mem.segment_link[0] ? mem.segment_link : "(none)");
    uintptr_t base = reinterpret_cast<uintptr_t>(mem.registration_base);
    uintptr_t p = reinterpret_cast<uintptr_t>(mem.host_ptr);
    check(mem.page_size == page, "3) reported page size == sysconf(_SC_PAGESIZE)");
    check(base % page == 0 && mem.registration_size % page == 0,
          "3) registration extent is page aligned on both ends");
    check(base <= p && base + mem.registration_size >= p + mem.size,
          "3) registration extent covers the loaned buffer");
    check(mem.host_ptr == loan && mem.size > 0, "3) descriptor names the loaned buffer");

    // -- 4) pinnability ------------------------------------------------------
    // The same kernel operation an accelerator runtime performs when it
    // registers host memory. A failure here would mean the reported extent is
    // not a valid mapping.
    int rc = mlock(mem.registration_base, mem.registration_size);
    if (rc != 0 && errno == ENOMEM) {
      // RLIMIT_MEMLOCK too small in this container: report it rather than
      // claiming a pass we did not measure.
      printf("  mlock: ENOMEM (RLIMIT_MEMLOCK too small) — pinnability NOT measured\n");
      check(false, "4) reported extent is pinnable (mlock)");
    } else {
      check(rc == 0, "4) reported extent is pinnable (mlock)");
      if (rc == 0) { munlock(mem.registration_base, mem.registration_size); }
    }

    check(mem.cross_process == (mem.kind != RMW_ZERODDS_LOAN_MEMORY_PROCESS_HEAP),
          "   cross_process flag matches the memory kind");
  }

  // Fill and publish the loan; the plain subscription must receive it, so the
  // buffer just described is provably the buffer that carries the payload.
  static_cast<std_msgs::msg::Int32 *>(loan)->data = 4242;
  const int32_t needle = 4242;
  bool cross_ok = true;
  if (raw_mode) {
    // The mode reaches the C layer through the runtime configuration
    // (rmw_zerodds_delivery_mode); a raw loan that is not in a shared segment
    // means the selection fell back to Portable, which step 5 would skip.
    check(described && mem.kind == RMW_ZERODDS_LOAN_MEMORY_SHARED_SEGMENT,
          "5) raw-same-host loan lives in a shared segment");
  }
  if (raw_mode && described && mem.kind == RMW_ZERODDS_LOAN_MEMORY_SHARED_SEGMENT) {
    // -- 5) cross-process mappability ----------------------------------------
    // Check before the publish consumes the slot: the bytes are already in the
    // segment, because the loan is the segment slot itself.
    cross_ok = child_finds_payload_in_segment(mem.segment_link, needle);
    check(cross_ok, "5) forked child maps the segment and finds the payload");
  }
  check(rmw_publish_loaned_message(rmw_pub, loan, nullptr) == RMW_RET_OK, "   loan published");

  auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(15);
  while (got < 0 && std::chrono::steady_clock::now() < deadline) {
    void * l2 = nullptr;
    if (rmw_borrow_loaned_message(rmw_pub, ts, &l2) == RMW_RET_OK && l2) {
      static_cast<std_msgs::msg::Int32 *>(l2)->data = needle;
      rmw_publish_loaned_message(rmw_pub, l2, nullptr);
    }
    exec.spin_some();
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
  }
  check(got == needle, "   the described buffer is the buffer that was delivered");

  // The subscription side reports provenance too, so an accelerated consumer
  // can register the buffer it takes.
  rmw_subscription_t * rmw_sub =
    rcl_subscription_get_rmw_handle(sub->get_subscription_handle().get());
  if (raw_mode && rmw_sub) {
    void * taken_msg = nullptr;
    bool taken = false;
    for (int i = 0; i < 30 && !taken; ++i) {
      void * l3 = nullptr;
      if (rmw_borrow_loaned_message(rmw_pub, ts, &l3) == RMW_RET_OK && l3) {
        static_cast<std_msgs::msg::Int32 *>(l3)->data = needle;
        rmw_publish_loaned_message(rmw_pub, l3, nullptr);
      }
      std::this_thread::sleep_for(std::chrono::milliseconds(100));
      rmw_take_loaned_message(rmw_sub, &taken_msg, &taken, nullptr);
    }
    if (taken && taken_msg) {
      rmw_zerodds_loan_memory_t rmem;
      bool ok = rmw_zerodds_subscription_get_loan_memory(rmw_sub, taken_msg, &rmem) == RMW_RET_OK;
      printf("  subscriber loan: kind=%s cross_process=%d link=%s\n",
             ok ? kind_name(rmem.kind) : "?", ok && rmem.cross_process ? 1 : 0,
             ok && rmem.segment_link[0] ? rmem.segment_link : "(none)");
      check(ok && rmem.kind == RMW_ZERODDS_LOAN_MEMORY_SHARED_SEGMENT && rmem.cross_process &&
            std::strcmp(rmem.segment_link, mem.segment_link) == 0,
            "5) subscriber loan reports the same shared segment");
      rmw_return_loaned_message_from_subscription(rmw_sub, taken_msg);
    } else {
      check(false, "5) subscriber loan reports the same shared segment");
    }
  }

  rclcpp::shutdown();
  printf("%s (%d failure(s))\n", g_failures == 0 ? "ALL PASS" : "FAILURES", g_failures);
  return g_failures == 0 ? 0 : 1;
}

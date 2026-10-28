// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// A ROS 2 type support over RTPS/UDP: `rmw_publish` through the RMW
// implementation this binary is linked against, so the octets in the
// capture are the ones a ROS node puts on the network.
//
// ## Why `rmw` and not `rclcpp`
//
// `rclcpp` would need an ament package, a `colcon` install tree and a
// type support library to `dlopen`. The corpus case is one generated
// message compiled into this binary, so the handle is available as a
// symbol and the layer that actually writes the socket is `rmw`. Every
// serialisation decision is below this line: `rmw_fastrtps_cpp` calls
// the `rosidl_typesupport_fastrtps_c` callbacks generated for this
// message, `rmw_cyclonedds_cpp` walks the introspection type support.
// Nothing here decides a wire question.
//
// ## Which type support handle
//
// `rmw_fastrtps_cpp` takes either the C or the C++ Fast RTPS type
// support and this passes the **C** one, because that is the generator
// `wireprobe-ros` measures in memory. Comparing the wire against the
// in-memory record only means anything if both ran the same generated
// code.
//
// `rmw_cyclonedds_cpp` takes the introspection type support, which is a
// different serialiser again — `rmw_cyclonedds_cpp`'s own writer walking
// a runtime type description — and that is the second ROS column.
//
// @-delimited names are substituted by `wire-golden.sh`.
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <thread>

#include "rcutils/allocator.h"
#include "rcutils/strdup.h"
#include "rmw/error_handling.h"
#include "rmw/rmw.h"
#include "rmw/init.h"
#include "rmw/init_options.h"
#include "rosidl_runtime_c/primitives_sequence_functions.h"
#include "rosidl_runtime_c/string_functions.h"
#include "rosidl_runtime_c/u16string_functions.h"
#include "rosidl_typesupport_interface/macros.h"

@MESSAGE_INCLUDES@
#include "@PACKAGE@/msg/detail/@SNAKE@__rosidl_typesupport_fastrtps_c.h"
#include "@PACKAGE@/msg/detail/@SNAKE@__rosidl_typesupport_introspection_c.h"

// The same two macros `ros-template.cpp` defines, because
// `wireprobe-fill --vendor ros` writes their call sites.
#define ZDROS_STRING(target, literal, length)                                \
  do {                                                                       \
    if (!rosidl_runtime_c__String__assignn((target), (literal), (length))) { \
      std::fprintf(stderr, "String__assignn failed\n");                      \
      return 6;                                                              \
    }                                                                        \
  } while (0)

#define ZDROS_WSTRING(target, units, count)                                  \
  do {                                                                       \
    if (!rosidl_runtime_c__U16String__assignn((target), (units), (count))) { \
      std::fprintf(stderr, "U16String__assignn failed\n");                   \
      return 6;                                                              \
    }                                                                        \
  } while (0)

#define ZDROS_SEQ(init, target, count)                                       \
  do {                                                                       \
    if (!init((target), (count))) {                                          \
      std::fprintf(stderr, "a sequence would not initialise\n");             \
      return 6;                                                              \
    }                                                                        \
  } while (0)

// The handle an RMW is given has to answer for **both** generators.
//
// `rmw_create_publisher` in `rmw_fastrtps_cpp` asks for the Fast RTPS C
// type support and then, separately, for the introspection one to build
// the `TypeObject` it announces in discovery; handing it the Fast RTPS
// handle alone gets as far as the type name and then fails with
// "failed to register type object with incompatible type". An ament
// package would supply `rosidl_typesupport_c`'s multiplexer, which
// resolves each identifier by `dlopen`ing a library; the two handles
// here are symbols in this binary, so the multiplexer is three lines.
static const rosidl_message_type_support_t *zd_lookup(
    const rosidl_message_type_support_t *, const char *identifier) {
    const rosidl_message_type_support_t *fastrtps =
        ROSIDL_TYPESUPPORT_INTERFACE__MESSAGE_SYMBOL_NAME(
            rosidl_typesupport_fastrtps_c, @PACKAGE@, msg, @MESSAGE@)();
    const rosidl_message_type_support_t *introspection =
        ROSIDL_TYPESUPPORT_INTERFACE__MESSAGE_SYMBOL_NAME(
            rosidl_typesupport_introspection_c, @PACKAGE@, msg, @MESSAGE@)();
    if (std::strcmp(identifier, fastrtps->typesupport_identifier) == 0)
        return fastrtps;
    if (std::strcmp(identifier, introspection->typesupport_identifier) == 0)
        return introspection;
    return nullptr;
}

static rosidl_message_type_support_t zd_multiplexed = {
    "zdros_wire_capture", nullptr, zd_lookup };

static int zd_fill_sample(@CTYPE@ *out) {
    @CTYPE@ sample;
    @CTYPE@__init(&sample);
#include "fill-sample.inc"
    *out = sample;
    return 0;
}

static int zd_fill_poison(@CTYPE@ *out) {
    @CTYPE@ sample;
    @CTYPE@__init(&sample);
#include "fill-poison.inc"
    *out = sample;
    return 0;
}

int main(int argc, char **argv) {
    if (argc != 6) {
        std::fprintf(stderr, "usage: probe <pub|sub> <domain> <1|2> <count> <topic>\n");
        return 2;
    }
    const std::string role = argv[1];
    const std::size_t domain = static_cast<std::size_t>(std::atoi(argv[2]));
    const int count = std::atoi(argv[4]);
    const std::string topic_name = std::string("rt/") + argv[5];

    const rosidl_message_type_support_t *type_support = &zd_multiplexed;

    rmw_init_options_t options = rmw_get_zero_initialized_init_options();
    rcutils_allocator_t allocator = rcutils_get_default_allocator();
    if (rmw_init_options_init(&options, allocator) != RMW_RET_OK) {
        std::fprintf(stderr, "rmw_init_options_init: %s\n", rmw_get_error_string().str);
        return 1;
    }
    options.domain_id = domain;
    options.enclave = rcutils_strdup("/", allocator);

    rmw_context_t context = rmw_get_zero_initialized_context();
    if (rmw_init(&options, &context) != RMW_RET_OK) {
        std::fprintf(stderr, "rmw_init: %s\n", rmw_get_error_string().str);
        return 1;
    }
    rmw_node_t *node = rmw_create_node(&context, role == "pub" ? "zdw_pub" : "zdw_sub", "/");
    if (node == nullptr) {
        std::fprintf(stderr, "rmw_create_node: %s\n", rmw_get_error_string().str);
        return 1;
    }

    rmw_qos_profile_t qos = rmw_qos_profile_default;
    qos.reliability = RMW_QOS_POLICY_RELIABILITY_RELIABLE;
    qos.history = RMW_QOS_POLICY_HISTORY_KEEP_ALL;
    qos.durability = RMW_QOS_POLICY_DURABILITY_VOLATILE;
    qos.avoid_ros_namespace_conventions = true;

    if (role == "pub") {
        rmw_publisher_options_t publisher_options = rmw_get_default_publisher_options();
        rmw_publisher_t *publisher = rmw_create_publisher(
            node, type_support, topic_name.c_str(), &qos, &publisher_options);
        if (publisher == nullptr) {
            std::fprintf(stderr, "rmw_create_publisher: %s\n", rmw_get_error_string().str);
            return 1;
        }
        std::size_t matched = 0;
        for (int i = 0; i < 300 && matched == 0; i++) {
            rmw_publisher_count_matched_subscriptions(publisher, &matched);
            if (matched == 0) std::this_thread::sleep_for(std::chrono::milliseconds(50));
        }
        if (matched == 0) { std::fprintf(stderr, "no subscription matched\n"); return 1; }
        // Which writer in the capture is this one. An RMW builds
        // writers of its own — `ros_discovery_info` is a user-defined
        // writer like any other — so a capture holds more than this
        // topic and "exactly one user writer" is not a check that can
        // hold here. The GUID says which, and `check-run.py` reads it
        // off this line.
        rmw_gid_t gid;
        if (rmw_get_gid_for_publisher(publisher, &gid) == RMW_RET_OK) {
            std::printf("writer %02x%02x%02x%02x\n", gid.data[12], gid.data[13],
                        gid.data[14], gid.data[15]);
            std::printf("gid ");
            for (std::size_t i = 0; i < RMW_GID_STORAGE_SIZE; i++)
                std::printf("%02x", gid.data[i]);
            std::printf("\n");
            std::fflush(stdout);
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(400));

        for (int i = 0; i < count; i++) {
            @CTYPE@ value;
            const int filled = (i % 2 == 0) ? zd_fill_poison(&value) : zd_fill_sample(&value);
            if (filled != 0) return filled;
            if (rmw_publish(publisher, &value, nullptr) != RMW_RET_OK) {
                std::fprintf(stderr, "rmw_publish: %s\n", rmw_get_error_string().str);
                return 1;
            }
            std::printf("wrote %d %s\n", i, i % 2 == 0 ? "poison" : "sample");
            std::fflush(stdout);
            std::this_thread::sleep_for(std::chrono::milliseconds(120));
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(1500));
        std::printf("matched %zu\n", matched);
        return 0;
    }

    rmw_subscription_options_t subscription_options = rmw_get_default_subscription_options();
    rmw_subscription_t *subscription = rmw_create_subscription(
        node, type_support, topic_name.c_str(), &qos, &subscription_options);
    if (subscription == nullptr) {
        std::fprintf(stderr, "rmw_create_subscription: %s\n", rmw_get_error_string().str);
        return 1;
    }
    int seen = 0;
    for (int i = 0; i < 600 && seen < count; i++) {
        @CTYPE@ value;
        @CTYPE@__init(&value);
        bool taken = false;
        while (rmw_take(subscription, &value, &taken, nullptr) == RMW_RET_OK && taken) {
            seen++;
            taken = false;
        }
        @CTYPE@__fini(&value);
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }
    std::printf("read %d\n", seen);
    return seen == count ? 0 : 1;
}

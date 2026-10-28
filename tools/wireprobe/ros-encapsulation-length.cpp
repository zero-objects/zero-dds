// Does either ROS type support pad the payload to four?
//
// A one-off, not a template: `wireprobe-ros` generates its probes from
// `ros-template.cpp` and `ros-rmw-template.cpp`, and both of those append
// their own padding to `--emit payload+encapsulation` output
// (`ros-template.cpp:149-156`, `ros-rmw-template.cpp:138-142`). That
// padding was once recorded as the peer's. This program asks the two
// encoders directly and prints the length each one reports for itself,
// so the question is settled without a template in the way.
//
// `std_msgs/msg/Byte` is exactly `@final struct { octet a; }`, the shape
// `docs/peer-defects.md` uses for this field. The buffer is filled with
// `0xAA` first: anything the encoder did not write shows as `aa`.
//
// Result on the measurement host, ROS 2 Humble at
// /root/micromamba/envs/ros2 — rmw_cyclonedds_cpp 1.3.4,
// rosidl_typesupport_fastrtps_c 2.2.2, Fast CDR 1.0.24:
//
//   fastrtps_c serialized        len=5  00010000ab
//     + 4 octets beyond          len=9  00010000abaaaaaaaa
//   rmw_serialize                buffer_length=5 buffer_capacity=5
//   cyclone rmw_serialize        len=5  00010000ab
//
// Five octets from both, and the marker survives at the first octet past
// the payload. Neither pads. Both leave the encapsulation options
// `0000`, which is the half of the recorded finding that holds.
//
// Build (P is the ROS prefix):
//
//   P=/root/micromamba/envs/ros2
//   g++ -std=c++17 -o ros-encapsulation-length ros-encapsulation-length.cpp \
//     -I$P/include -I$P/include/std_msgs -I$P/include/rmw \
//     -I$P/include/rcutils -I$P/include/rosidl_runtime_c \
//     -I$P/include/rosidl_typesupport_interface \
//     -I$P/include/rosidl_typesupport_fastrtps_c \
//     -I$P/include/rosidl_typesupport_fastrtps_cpp -I$P/include/fastcdr \
//     -L$P/lib -lstd_msgs__rosidl_typesupport_c \
//     -lstd_msgs__rosidl_typesupport_fastrtps_c \
//     -lstd_msgs__rosidl_generator_c -lrosidl_runtime_c \
//     -lrmw -lrmw_cyclonedds_cpp -lrcutils -lfastcdr -Wl,-rpath,$P/lib
//
// `-lrmw_cyclonedds_cpp` is linked directly rather than through
// RMW_IMPLEMENTATION, so `rmw_serialize` below is Cyclone's.
#include <cstdio>
#include <cstring>
#include <fastcdr/Cdr.h>
#include <fastcdr/FastBuffer.h>
#include "rosidl_runtime_c/message_type_support_struct.h"
#include "rosidl_typesupport_fastrtps_c/identifier.h"
#include "rosidl_typesupport_fastrtps_cpp/message_type_support.h"
#include "std_msgs/msg/byte.h"
#include "rmw/rmw.h"
#include "rmw/serialized_message.h"

extern "C" const rosidl_message_type_support_t *
ROSIDL_TYPESUPPORT_INTERFACE__MESSAGE_SYMBOL_NAME(
  rosidl_typesupport_c, std_msgs, msg, Byte)();

static void hexdump(const char * tag, const void * p, size_t n)
{
  std::printf("%-28s len=%zu  ", tag, n);
  const unsigned char * b = (const unsigned char *)p;
  for (size_t i = 0; i < n; i++) {
    std::printf("%02x", b[i]);
  }
  std::printf("\n");
}

int main()
{
  std_msgs__msg__Byte sample;
  sample.data = 0xAB;

  const rosidl_message_type_support_t * ts =
    ROSIDL_TYPESUPPORT_INTERFACE__MESSAGE_SYMBOL_NAME(
    rosidl_typesupport_c, std_msgs, msg, Byte)();

  // rosidl_typesupport_fastrtps_c, the path `wireprobe-ros` takes for
  // the `ros/fastrtps` column, through the same `Cdr(…, DDS_CDR)` and
  // `serialize_encapsulation()` that
  // `rmw_fastrtps_cpp/src/type_support_common.cpp:84` uses.
  {
    const rosidl_message_type_support_t * fts =
      get_message_typesupport_handle(ts, "rosidl_typesupport_fastrtps_c");
    if (!fts) {
      std::printf("no fastrtps_c handle\n");
      return 1;
    }
    auto cb = reinterpret_cast<const message_type_support_callbacks_t *>(fts->data);
    char buf[64];
    // The marker. Anything the encoder does not write stays 0xAA.
    std::memset(buf, 0xAA, sizeof(buf));
    eprosima::fastcdr::FastBuffer fb(buf, sizeof(buf));
    eprosima::fastcdr::Cdr ser(
      fb, eprosima::fastcdr::Cdr::DEFAULT_ENDIAN,
      eprosima::fastcdr::Cdr::DDS_CDR);
    ser.serialize_encapsulation();
    cb->cdr_serialize(&sample, ser);
    // `m_currentPosition - m_cdrBuffer.begin()`, Cdr.h:230 — the write
    // cursor, so it counts what was written and nothing beyond it.
    size_t len = ser.getSerializedDataLength();
    hexdump("fastrtps_c serialized", buf, len);
    hexdump("  + 4 octets beyond", buf, len + 4);
  }

  // rmw_serialize, the path `wireprobe-ros` takes for the `ros/cyclone`
  // column. `buffer_length` is what `rmw_cyclonedds_cpp`'s own
  // `get_serialized_size` returned (rmw_node.cpp:1680-1688).
  {
    rcutils_allocator_t alloc = rcutils_get_default_allocator();
    rmw_serialized_message_t msg = rmw_get_zero_initialized_serialized_message();
    if (rmw_serialized_message_init(&msg, 0, &alloc) != RMW_RET_OK) {
      return 1;
    }
    if (rmw_serialize(&sample, ts, &msg) != RMW_RET_OK) {
      std::printf("rmw_serialize failed: %s\n", rcutils_get_error_string().str);
      return 1;
    }
    std::printf(
      "%-28s buffer_length=%zu buffer_capacity=%zu\n",
      "rmw_serialize", msg.buffer_length, msg.buffer_capacity);
    hexdump("cyclone rmw_serialize", msg.buffer, msg.buffer_length);
  }
  return 0;
}

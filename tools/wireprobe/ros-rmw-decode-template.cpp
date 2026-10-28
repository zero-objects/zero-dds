// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The read half of `wireprobe-ros --typesupport cyclone`. Bytes on
// stdin, through `rmw_deserialize` in `librmw_cyclonedds_cpp`, out as
// the canonical JSON every column prints.
//
// `@`-delimited names are substituted by the shell script.

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

#include "rcutils/allocator.h"
#include "rmw/error_handling.h"
#include "rmw/rmw.h"
#include "rmw/serialized_message.h"
// The generated struct header pulls in `string.h` and `u16string.h`
// for the *types*. The `__assignn` and `__Sequence__init` the fill
// statements call are declared in the `_functions.h` beside them,
// which nothing else includes.
#include "rosidl_runtime_c/primitives_sequence_functions.h"
#include "rosidl_runtime_c/string_functions.h"
#include "rosidl_runtime_c/u16string_functions.h"
#include "rosidl_typesupport_interface/macros.h"
// One pair per message in the package, dependencies first. The
// root's header alone is not enough: a `sequence<Cell>` calls
// `Cell__Sequence__init`, which is declared in Cell's own
// `__functions.h` and nowhere else.
@MESSAGE_INCLUDES@

#include "dumpjson.h"

// The same guard `ros-decode-template.cpp` uses, and for the same
// reason: `zdj_string` stops at a terminator and a
// `rosidl_runtime_c__String` carries a size beside one.
#define ZDROS_STRING_OUT(str)                                                 \
  do {                                                                        \
    const rosidl_runtime_c__String * zd_s = (str);                            \
    if (zd_s->data == NULL) {                                                 \
      std::fprintf(stderr, "wireprobe-ros: a string came back with no data\n"); \
      return 6;                                                               \
    }                                                                         \
    if (std::strlen(zd_s->data) != zd_s->size) {                              \
      std::fprintf(stderr,                                                    \
        "wireprobe-ros: a string of size %zu holds an interior NUL at %zu\n", \
        zd_s->size, std::strlen(zd_s->data));                                 \
      return 6;                                                               \
    }                                                                         \
    zdj_string(zd_s->data);                                                   \
  } while (0)

extern "C" const rosidl_message_type_support_t *
ROSIDL_TYPESUPPORT_INTERFACE__MESSAGE_SYMBOL_NAME(
  rosidl_typesupport_introspection_c, @PACKAGE@, msg, @MESSAGE@)();

// The byte order comes in on the command line and not from `sed`: it
// used to be part of the binary's identity, so the two orders of one
// case meant two runs of the ROS code generator over identical code.
int main(int argc, char **argv)
{
  if (argc < 2) {
    std::fprintf(stderr, "usage: probe <le|be> [<1|2>]\n");
    return 2;
  }
  const bool zd_big = std::strcmp(argv[1], "be") == 0;

  // The payload arrives without an encapsulation header. `rmw_deserialize`
  // expects one and takes the byte order from it, which is the only way
  // to tell it the order at all.
  std::vector<unsigned char> bytes;
  bytes.push_back(0x00);
  bytes.push_back(zd_big ? 0x00 : 0x01);
  bytes.push_back(0x00);
  bytes.push_back(0x00);
  int c;
  while ((c = std::fgetc(stdin)) != EOF) {
    bytes.push_back(static_cast<unsigned char>(c));
  }

  @CTYPE@ sample;
  if (!@CTYPE@__init(&sample)) {
    std::fprintf(stderr, "wireprobe-ros: __init failed\n");
    return 6;
  }

  const rosidl_message_type_support_t * ts =
    ROSIDL_TYPESUPPORT_INTERFACE__MESSAGE_SYMBOL_NAME(
      rosidl_typesupport_introspection_c, @PACKAGE@, msg, @MESSAGE@)();

  rcutils_allocator_t allocator = rcutils_get_default_allocator();
  rmw_serialized_message_t message = rmw_get_zero_initialized_serialized_message();
  if (rmw_serialized_message_init(&message, bytes.size(), &allocator) != RMW_RET_OK) {
    std::fprintf(stderr, "wireprobe-ros: serialized_message_init failed\n");
    return 6;
  }
  std::memcpy(message.buffer, bytes.data(), bytes.size());
  message.buffer_length = bytes.size();

  if (rmw_deserialize(&message, ts, &sample) != RMW_RET_OK) {
    std::fprintf(stderr, "wireprobe-ros: rmw_deserialize: %s\n",
                 rmw_get_error_string().str);
    return 3;
  }

#include "dump.inc"

  return 0;
}

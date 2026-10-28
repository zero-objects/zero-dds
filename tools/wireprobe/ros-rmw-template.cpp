// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The second ROS encoder. `wireprobe-ros --typesupport cyclone` runs
// this; `--typesupport fastrtps` runs `ros-template.cpp`.
//
// `rmw_cyclonedds_cpp` does not use Cyclone's `idlc` output and does not
// use `rosidl_typesupport_fastrtps` either. It walks the *introspection*
// type support — a runtime description of the members — with a CDR
// writer of its own, in `serdata.cpp` and `TypeSupport2.hpp`. So the
// same `.idl` reaches the wire through two independently written ROS
// serialisers, and this is the other one.
//
// It is reached through `rmw_serialize`, which is the public RMW entry
// point and the one every ROS publisher's serialised path goes through.
// Nothing here decides a wire question.
//
// `@`-delimited names are substituted by the shell script.

#include <cstdio>
#include <cstdlib>
#include <cstring>

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

// The same three macros `ros-template.cpp` defines, for the same
// `fill.inc`. One fill emitter serves both encoders: the structure is
// `rosidl_generator_c`'s and does not depend on which serialiser reads
// it, which is exactly what makes the two comparable.
#define ZDROS_STRING(target, literal, length)                                \
  do {                                                                       \
    if (!rosidl_runtime_c__String__assignn((target), (literal), (length))) { \
      std::fprintf(stderr, "wireprobe-ros: String__assignn failed\n");       \
      return 6;                                                              \
    }                                                                        \
  } while (0)

#define ZDROS_WSTRING(target, units, count)                                  \
  do {                                                                       \
    if (!rosidl_runtime_c__U16String__assignn((target), (units), (count))) { \
      std::fprintf(stderr, "wireprobe-ros: U16String__assignn failed\n");    \
      return 6;                                                              \
    }                                                                        \
  } while (0)

#define ZDROS_SEQ(init, target, count)                                       \
  do {                                                                       \
    if (!init((target), (count))) {                                          \
      std::fprintf(stderr, "wireprobe-ros: Sequence__init failed\n");        \
      return 6;                                                              \
    }                                                                        \
  } while (0)

extern "C" const rosidl_message_type_support_t *
ROSIDL_TYPESUPPORT_INTERFACE__MESSAGE_SYMBOL_NAME(
  rosidl_typesupport_introspection_c, @PACKAGE@, msg, @MESSAGE@)();

// The byte order and the emit form come in on the command line and not
// from `sed`. They used to be part of the binary's identity, so the two
// byte orders of one case meant two runs of the ROS code generator and
// two compiles of identical generated code; `--matrix` now prints every
// row this type support can answer from one build.
int main(int argc, char **argv)
{
  if (argc != 3) {
    std::fprintf(stderr, "usage: probe <le|be> <payload|encapsulated>\n");
    return 2;
  }
  const bool zd_big = std::strcmp(argv[1], "be") == 0;
  const bool zd_encapsulated = std::strcmp(argv[2], "encapsulated") == 0;
  // Accepted and ignored, deliberately. `rmw_serialize` writes the
  // host's byte order and takes no argument for it, which is why
  // `wireprobe-ros` refuses `--endian be` on a little-endian host
  // rather than letting this decide. Taking the argument here keeps one
  // command line across both type supports; acting on it would be this
  // probe answering a question the type support does not.
  (void)zd_big;

  @CTYPE@ sample;
  if (!@CTYPE@__init(&sample)) {
    std::fprintf(stderr, "wireprobe-ros: __init failed\n");
    return 6;
  }

#include "fill.inc"

  const rosidl_message_type_support_t * ts =
    ROSIDL_TYPESUPPORT_INTERFACE__MESSAGE_SYMBOL_NAME(
      rosidl_typesupport_introspection_c, @PACKAGE@, msg, @MESSAGE@)();

  rcutils_allocator_t allocator = rcutils_get_default_allocator();
  rmw_serialized_message_t message = rmw_get_zero_initialized_serialized_message();
  if (rmw_serialized_message_init(&message, 0, &allocator) != RMW_RET_OK) {
    std::fprintf(stderr, "wireprobe-ros: serialized_message_init failed\n");
    return 6;
  }
  if (rmw_serialize(&sample, ts, &message) != RMW_RET_OK) {
    // `rmw_serialize` reports a refusal the same way it reports a
    // failure, so the two cannot be told apart here. Exit 3 — the
    // DISCARD code — because a sample the serialiser will not take is
    // the outcome the run is measuring, and the message is on stderr
    // either way.
    std::fprintf(stderr, "wireprobe-ros: rmw_serialize: %s\n",
                 rmw_get_error_string().str);
    return 3;
  }

  if (message.buffer_length < 4) {
    std::fprintf(stderr, "wireprobe-ros: no encapsulation header was written\n");
    return 6;
  }

  const char * start = zd_encapsulated
    ? reinterpret_cast<const char *>(message.buffer)
    : reinterpret_cast<const char *>(message.buffer) + 4;
  size_t count = zd_encapsulated
    ? message.buffer_length : message.buffer_length - 4;
  if (std::fwrite(start, 1, count, stdout) != count) {
    std::fprintf(stderr, "wireprobe-ros: short write\n");
    return 6;
  }
  if (zd_encapsulated) {
    static const char zero[4] = {0, 0, 0, 0};
    size_t padding = (4 - (count % 4)) % 4;
    if (padding != 0 && std::fwrite(zero, 1, padding, stdout) != padding) {
      std::fprintf(stderr, "wireprobe-ros: short write\n");
      return 6;
    }
  }
  return 0;
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The read half of `wireprobe-ros`. Bytes on stdin, through the
// deserialiser `rosidl_typesupport_fastrtps_c` generated, out as the
// canonical JSON every column prints.
//
// The mirror of `ros-template.cpp`, and it runs what
// rmw_fastrtps_shared_cpp's `TypeSupport::deserialize` runs: a
// `FastBuffer` over the payload, a `Cdr` in `DDS_CDR` mode,
// `read_encapsulation()`, then the type support's `cdr_deserialize`.

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

#include <fastcdr/Cdr.h>
#include <fastcdr/FastBuffer.h>

#include "rosidl_typesupport_fastrtps_cpp/message_type_support.h"
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

// A `rosidl_runtime_c__String` carries a `size` and a NUL terminator.
// `zdj_string` takes only the pointer, so it stops at the terminator —
// and a payload holding an interior NUL makes the two disagree. Printing
// the shorter one would be this printer losing bytes the deserialiser
// did read, so the two are compared and the run fails instead.
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
  rosidl_typesupport_fastrtps_c, @PACKAGE@, msg, @MESSAGE@)();

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

  // The payload arrives without an encapsulation header — that is what
  // every column's `--emit payload` produces — so one is put back in
  // front of it. Fast CDR has no way to be told the byte order except
  // through `read_encapsulation`, and rmw_fastrtps does the same.
  std::vector<char> bytes;
  bytes.push_back(0x00);
  bytes.push_back(zd_big ? 0x00 : 0x01);
  bytes.push_back(0x00);
  bytes.push_back(0x00);
  int c;
  while ((c = std::fgetc(stdin)) != EOF) {
    bytes.push_back(static_cast<char>(c));
  }

  @CTYPE@ sample;
  if (!@CTYPE@__init(&sample)) {
    std::fprintf(stderr, "wireprobe-ros: __init failed\n");
    return 6;
  }

  const rosidl_message_type_support_t * ts =
    ROSIDL_TYPESUPPORT_INTERFACE__MESSAGE_SYMBOL_NAME(
      rosidl_typesupport_fastrtps_c, @PACKAGE@, msg, @MESSAGE@)();
  const message_type_support_callbacks_t * callbacks =
    static_cast<const message_type_support_callbacks_t *>(ts->data);

  try {
    eprosima::fastcdr::FastBuffer buffer(bytes.data(), bytes.size());
    eprosima::fastcdr::Cdr deserializer(
      buffer,
      eprosima::fastcdr::Cdr::DEFAULT_ENDIAN,
      eprosima::fastcdr::Cdr::DDS_CDR);
    deserializer.read_encapsulation();
    if (!callbacks->cdr_deserialize(deserializer, &sample)) {
      std::fprintf(stderr, "wireprobe-ros: cdr_deserialize refused the bytes\n");
      return 3;
    }
  } catch (const std::exception & error) {
    // Fast CDR throws `NotEnoughMemoryException` when the payload runs
    // out under it. A short stream is a refusal, not a value.
    std::fprintf(stderr, "wireprobe-ros: %s\n", error.what());
    return 3;
  }

#include "dump.inc"

  return 0;
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The write half of `wireprobe-ros`. One sample, through the
// serialiser `rosidl_typesupport_fastrtps_c` generated for it.
//
// `@`-delimited names are substituted by the shell script.
//
// The three lines that matter are the ones rmw_fastrtps_shared_cpp's
// `TypeSupport::serialize` runs, in the same order and with the same
// arguments: a `FastBuffer` over the caller's memory, a `Cdr` in
// `DDS_CDR` mode, `serialize_encapsulation()`, and then the type
// support's own `cdr_serialize` callback. Nothing here decides a wire
// question — the callback is the generated code, unread.

#include <cstdio>
#include <cstdlib>
#include <cstring>

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

// `wireprobe-fill --vendor ros` writes these two macros' call sites.
// They are macros and not plain calls so that a failed allocation or a
// refused assignment stops the run instead of leaving a member holding
// whatever `__init` left there. A probe that reported bytes for a
// half-filled sample would be reporting a wire format nobody asked for.
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
  rosidl_typesupport_fastrtps_c, @PACKAGE@, msg, @MESSAGE@)();

// 1 MiB. The corpus's largest case is three orders of magnitude under
// it, and Fast CDR throws rather than overruns, so an overflow is a
// caught exception and not a wrong answer.
static char zd_buffer[1024 * 1024];

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

  @CTYPE@ sample;
  if (!@CTYPE@__init(&sample)) {
    std::fprintf(stderr, "wireprobe-ros: __init failed\n");
    return 6;
  }

#include "fill.inc"

  const rosidl_message_type_support_t * ts =
    ROSIDL_TYPESUPPORT_INTERFACE__MESSAGE_SYMBOL_NAME(
      rosidl_typesupport_fastrtps_c, @PACKAGE@, msg, @MESSAGE@)();
  const message_type_support_callbacks_t * callbacks =
    static_cast<const message_type_support_callbacks_t *>(ts->data);

  size_t length = 0;
  try {
    eprosima::fastcdr::FastBuffer buffer(zd_buffer, sizeof(zd_buffer));
    eprosima::fastcdr::Cdr serializer(
      buffer,
      zd_big ? eprosima::fastcdr::Cdr::BIG_ENDIANNESS
             : eprosima::fastcdr::Cdr::LITTLE_ENDIANNESS,
      eprosima::fastcdr::Cdr::DDS_CDR);
    // The four-byte encapsulation header, and the alignment origin it
    // pushes. `--emit payload` drops the header again below; it is
    // written here because dropping it and *not* writing it are
    // different alignments, and rmw_fastrtps writes it.
    serializer.serialize_encapsulation();
    if (!callbacks->cdr_serialize(&sample, serializer)) {
      // The generated code returns false for a bounded-sequence
      // overrun and for a string whose capacity does not exceed its
      // size. Both are §7.2.2.7 DISCARD outcomes, which the common
      // command line reports as exit 3.
      std::fprintf(stderr, "wireprobe-ros: cdr_serialize refused the sample\n");
      return 3;
    }
    length = serializer.getSerializedDataLength();
  } catch (const std::exception & error) {
    std::fprintf(stderr, "wireprobe-ros: %s\n", error.what());
    return 6;
  }

  if (length < 4) {
    std::fprintf(stderr, "wireprobe-ros: no encapsulation header was written\n");
    return 6;
  }

  // `--emit payload` is the serialised data alone. `--emit
  // payload+encapsulation` is the header, the payload, and zero padding
  // to the next four-byte offset — §7.4.1.1, the same shape every other
  // column emits.
  // Raw bytes on stdout, as every other probe writes them; the shell
  // script turns them into hex when no `--out` was given. Hex here
  // would make a payload holding a newline indistinguishable from two.
  const char * start = zd_encapsulated ? zd_buffer : zd_buffer + 4;
  size_t count = zd_encapsulated ? length : length - 4;
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

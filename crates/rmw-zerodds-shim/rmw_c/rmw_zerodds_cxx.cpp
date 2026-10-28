// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// rmw_zerodds_cxx.cpp — the std::string / std::u16string half of the
// introspection CDR codec.
//
// rosidl ships two introspection typesupports with the *same* MessageMembers
// layout but different runtime types for the variable-length members:
//
//   rosidl_typesupport_introspection_c    string  -> rosidl_runtime_c__String
//                                         wstring -> rosidl_runtime_c__U16String
//   rosidl_typesupport_introspection_cpp  string  -> std::string
//                                         wstring -> std::u16string
//
// rclpy resolves to the C flavour, rclcpp to the C++ one. Treating a
// std::string as a rosidl_runtime_c__String corrupts memory: the C assign
// reallocs what it believes is a heap pointer but is, for a short string, the
// std::string's inline SSO buffer.
//
// rmw_zerodds.c is a C translation unit and cannot touch std::string, so the
// four accessors below are compiled as C++ and linked into
// librmw_zerodds_cpp.so alongside it.

#include <cstddef>
#include <cstdint>
#include <string>

extern "C" {

/// Borrows the bytes of a std::string. The pointer stays valid until the string
/// is modified — long enough for one serialize pass.
const char * zerodds_cxx_string_data(const void * s, size_t * out_len)
{
  const std::string * p = static_cast<const std::string *>(s);
  *out_len = p->size();
  return p->data();
}

/// Replaces the contents of a std::string with `n` bytes from `data`.
void zerodds_cxx_string_assign(void * s, const char * data, size_t n)
{
  static_cast<std::string *>(s)->assign(data, n);
}

/// Borrows the UTF-16 code units of a std::u16string.
const uint16_t * zerodds_cxx_u16string_data(const void * s, size_t * out_len)
{
  const std::u16string * p = static_cast<const std::u16string *>(s);
  *out_len = p->size();
  return reinterpret_cast<const uint16_t *>(p->data());
}

/// Replaces the contents of a std::u16string with `n` UTF-16 code units.
void zerodds_cxx_u16string_assign(void * s, const uint16_t * data, size_t n)
{
  static_cast<std::u16string *>(s)->assign(reinterpret_cast<const char16_t *>(data), n);
}

}  // extern "C"

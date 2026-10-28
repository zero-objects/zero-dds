// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The primitive types of the Plain Language Binding in C++.
//
// DDS-XTypes 1.3 §7.5.1.1.2, Table 43: "The Service shall provide typedefs
// with the following names to types available on the underlying platform
// that have the appropriate sizes and representations." Where the table
// names a C99 type the typedef is that type. `Float128` is the runtime's
// `zerodds::wire::Float128`, because `long double` is x87 extended on
// x86-64 and a plain `double` on macOS arm64. `Char16` is `std::uint16_t`,
// which the table allows ("they may map Char16 to an equivalent integer
// type") and which has the same size everywhere, unlike `wchar_t`.
//
// `Boolean`, `Char8` and `Byte` map to three different C++ types here (`Byte` is
// `zerodds::wire::Octet`, which is not `UInt8`'s `unsigned char`), and `Char16` to an integer
// type: the table permits the first three to coincide, says `Char16` may not be
// distinguishable from integers for overloading, and asks that every other type be
// distinguishable from every other, `UInt8` from `Byte` included (§7.5.1.1.2).
#ifndef DDS_TYPES_HPP
#define DDS_TYPES_HPP

#include <cstdint>

#include <dds/return_code.hpp>
#include <zerodds/wire.hpp>

namespace DDS {

typedef std::int32_t Int32;
typedef std::uint32_t UInt32;
typedef std::int8_t Int8;
typedef std::uint8_t UInt8;
typedef std::int16_t Int16;
typedef std::uint16_t UInt16;
typedef std::int64_t Int64;
typedef std::uint64_t UInt64;
typedef float Float32;
typedef double Float64;
typedef zerodds::wire::Float128 Float128;
typedef char Char8;
typedef std::uint16_t Char16;
typedef bool Boolean;
typedef zerodds::wire::Octet Byte;

}  // namespace DDS

#endif  // DDS_TYPES_HPP

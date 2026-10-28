/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The C helpers, plus the one the C++ binding needs on top of them.
 *
 * XTypes 1.3 §7.5.2 fixes `wstring` as `std::wstring`, whose element is
 * a code point — four bytes on Linux — where the C binding leaves the
 * width of `Char16` to the vendor and the units are already UTF-16. So
 * the C++ side is the one that has to split a code point above the Basic
 * Multilingual Plane back into the surrogate pair the canonical form
 * carries, and this is the only place that difference lives.
 */
#ifndef ZERODDS_WIREPROBE_DUMPJSON_HPP
#define ZERODDS_WIREPROBE_DUMPJSON_HPP

#include <string>

#include "dumpjson.h"

/* `inline` and not `static`: a `static` function no translation unit
 * calls is a `-Wunused-function` error, and this header is now included
 * by a probe whose `wstring` is not a `std::wstring` — the ZeroDDS C++
 * column holds both string widths as UTF-8 in a `std::string`, so it
 * reaches `zdj_f128_string_cpp` below and never this. An inline function
 * that goes uncalled is not diagnosed, and the one definition rule is
 * satisfied the same way. */
inline void zdj_wstring_cpp(const std::wstring &text) {
    unsigned short units[ZDJ_WSTRING_MAX];
    size_t count = 0;
    for (std::wstring::const_iterator it = text.begin(); it != text.end(); ++it) {
        unsigned long point = static_cast<unsigned long>(*it);
        if (point > 0xFFFFul) {
            if (count + 2 > ZDJ_WSTRING_MAX) {
                break;
            }
            unsigned long rest = point - 0x10000ul;
            units[count++] = static_cast<unsigned short>(0xD800ul + (rest >> 10));
            units[count++] = static_cast<unsigned short>(0xDC00ul + (rest & 0x3FFul));
            continue;
        }
        if (count + 1 > ZDJ_WSTRING_MAX) {
            break;
        }
        units[count++] = static_cast<unsigned short>(point);
    }
    zdj_wstring(units, count);
}

/* A `long double` through the ZeroDDS C++ runtime.
 *
 * The counterpart of `zdj_f128_string` in the C header, which is guarded
 * on `ZERODDS_WIRE_H` so a vendor probe that includes this file without
 * the ZeroDDS runtime still compiles. The same guard on the C++ side:
 * only a translation unit that has already included `zerodds/wire.hpp`
 * gets this.
 *
 * Every digit the value has, not the shortest round-trip: the canonical
 * form for a `long double` is the exact expansion, and printing it as a
 * `double` would report agreement for a value this column never carried.
 *
 * `inline` and not `static`, unlike `zdj_wstring_cpp` above: a `static`
 * function no translation unit calls is a `-Wunused-function` error, and
 * the ZeroDDS C++ probe reaches this one only for a model that has a
 * `long double` in it.
 */
#ifdef ZERODDS_WIRE_HPP
inline void zdj_f128_string_cpp(const zerodds::wire::Float128 &value) {
    printf("\"%s\"", value.to_exact_decimal().c_str());
}
#endif /* ZERODDS_WIRE_HPP */

#endif /* ZERODDS_WIREPROBE_DUMPJSON_HPP */

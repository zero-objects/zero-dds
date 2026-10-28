/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The two helpers the OMG IDL-to-C++ mapping needs on top of
 * `dumpjson.h`, for the omniORB and TAO probes.
 *
 * Everything else in the canonical form is already in `dumpjson.h` and
 * is shared with the DDS vendors' probes, which is the point: a
 * difference between an ORB's answer and a DDS implementation's must not
 * be a difference in how the two were printed.
 */
#ifndef ZERODDS_WIREPROBE_CORBAJSON_HPP
#define ZERODDS_WIREPROBE_CORBAJSON_HPP

#include <cstddef>

#include "dumpjson.h"

/* A `wstring`, as the UTF-16 code units the canonical form carries.
 *
 * `CORBA::WChar` is not one width. CORBA 3.3 part 3 §1.2.4 fixes only
 * that it holds a wide character, and the two ORBs made different
 * choices: omniORB's is two bytes and TAO's is `ACE_CDR::WChar`, which
 * is `wchar_t` — four bytes on Linux. So where omniORB hands back UTF-16
 * units already, TAO hands back code points, and a code point above the
 * Basic Multilingual Plane has to be split into the surrogate pair
 * §7.4.1.1.2 puts on the wire.
 *
 * The `sizeof` test is a compile-time constant, so the branch that does
 * not apply to this ORB is not compiled in. It is a test and not a
 * `#ifdef` because both ORBs include this header and neither defines a
 * macro saying which one it is. */
static void zdj_wstring_corba(const CORBA::WChar *text) {
    unsigned short units[ZDJ_WSTRING_MAX];
    std::size_t count = 0;
    if (text == 0) {
        zdj_raw("null");
        return;
    }
    for (; *text != 0; ++text) {
        unsigned long point = (unsigned long)*text;
        if (sizeof(CORBA::WChar) > 2 && point > 0xFFFFul) {
            if (count + 2 > ZDJ_WSTRING_MAX) {
                break;
            }
            unsigned long rest = point - 0x10000ul;
            units[count++] = (unsigned short)(0xD800ul + (rest >> 10));
            units[count++] = (unsigned short)(0xDC00ul + (rest & 0x3FFul));
            continue;
        }
        if (count + 1 > ZDJ_WSTRING_MAX) {
            break;
        }
        units[count++] = (unsigned short)point;
    }
    zdj_wstring(units, count);
}

/* A `fixed<P,S>`, as the decimal string the canonical form carries.
 *
 * Behind `ZDJ_HAS_CORBA_FIXED`, which the omniORB templates define and
 * the TAO ones do not: TAO has no `CORBA::Fixed` at all — `tao_idl`
 * refuses the keyword with "fixed data types are not supported" — so an
 * unconditional declaration stops every TAO probe compiling for a
 * function no TAO probe can call. Measured 2026-08-10, on the whole
 * read direction.
 *
 * CORBA 3.3 part 3 §1.14 gives `CORBA::Fixed` a `to_string` that
 * allocates through the ORB's own allocator, so the result is freed
 * here rather than leaked once per member of every sample. OpenDDS's
 * FACE mapping allocates it with `FACE::string_dup`;
 * `opendds-face-fixed.h` names that type `CORBA::Fixed` and sets
 * `ZDJ_FIXED_STRING_FREE` to the matching `FACE::string_free`.
 *
 * The digits are the ORB's. `crates/wire-probe/src/canonical.rs`
 * normalises the decimal to the declared scale before anything is
 * compared, so an ORB that writes `0.10` and one that writes `0.1` are
 * not recorded as a divergence they are not. */
#ifdef ZDJ_HAS_CORBA_FIXED
#ifndef ZDJ_FIXED_STRING_FREE
#define ZDJ_FIXED_STRING_FREE CORBA::string_free
#endif
static void zdj_fixed_corba(const CORBA::Fixed &value) {
    char *text = value.to_string();
    if (text == 0) {
        zdj_raw("null");
        return;
    }
    zdj_string(text);
    ZDJ_FIXED_STRING_FREE(text);
}
#endif /* ZDJ_HAS_CORBA_FIXED */

#endif /* ZERODDS_WIREPROBE_CORBAJSON_HPP */

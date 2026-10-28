/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Force-included (`g++ -include`) into an OpenDDS probe whose IDL holds
 * `fixed<P,S>`, which `wireprobe-opendds` and `wireprobe-batch` generate
 * with `opendds_idl -Lface` and not with `tao_idl -Sa -St`.
 *
 * The classic mapping has no `fixed` at all: `tao_idl` prints "fixed
 * data types are not supported" and the member is missing from the
 * generated struct. OpenDDS 3.34 carries the type in its FACE mapping,
 * as `OpenDDS::FaceTypes::Fixed_T<P,S>`, whose `operator<<` writes
 * `(P + 2) / 2` octets — the declared count. The rest of that mapping
 * keeps the classic member assignment, so the fill and the dump stay
 * the `wireprobe-fill --vendor omniorb` / `wireprobe-dump --vendor
 * omniorb` statements, which name `CORBA::Fixed`. This header gives
 * them that name for the FACE type.
 *
 * `Fixed::to_string` allocates with `FACE::string_dup`, so the dump
 * frees with `FACE::string_free` and not with the ORB's function.
 */
#ifndef ZERODDS_WIREPROBE_OPENDDS_FACE_FIXED_H
#define ZERODDS_WIREPROBE_OPENDDS_FACE_FIXED_H

#include <FACE/Fixed.h>
#include <FACE/StringManager.h>

namespace CORBA {
typedef ::OpenDDS::FaceTypes::Fixed Fixed;
}

#define ZDJ_HAS_CORBA_FIXED 1
#define ZDJ_FIXED_STRING_FREE FACE::string_free

#endif /* ZERODDS_WIREPROBE_OPENDDS_FACE_FIXED_H */

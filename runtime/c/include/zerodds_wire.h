/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The C wire runtime: the counterpart of `crates/wire`, not a binding to
 * it. Every rule below is implemented here from the specification text,
 * so that a difference between this and the Rust runtime is a difference
 * in how the two read DDS-XTypes 1.3 and not a difference in how one of
 * them calls the other.
 *
 * C99, no dependency beyond the freestanding headers, and no allocation
 * anywhere in the write path: the caller owns the buffer and is told
 * when it is short. That is what lets an embedded target link it.
 *
 * Spec references are to OMG DDS-XTypes 1.3 unless another document is
 * named.
 */
#ifndef ZERODDS_WIRE_H
#define ZERODDS_WIRE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* -- byte order ---------------------------------------------------------- */

typedef enum zdw_endianness {
    ZDW_LITTLE = 0,
    ZDW_BIG = 1
} zdw_endianness;

/* -- `long double` -------------------------------------------------------
 *
 * IEEE 754 binary128: one sign bit, a fifteen-bit exponent biased by
 * 16383, and a 112-bit stored fraction. CORBA 3.3 part 2 §9.3.1.3 with
 * Figure 9.2 gives that layout for CDR `long double`, and DDS-XTypes 1.3
 * Table 31 names it — quadruple precision, sixteen bytes, aligned to
 * eight.
 *
 * Held as two 64-bit halves in *value* order, high first, so the type
 * means the same thing on a big-endian and a little-endian host and the
 * writer is the only place a byte order is chosen.
 *
 * ## Why this is not a `long double`
 *
 * Because the C type is not the wire type, and on the platforms this
 * project runs it is not even one type. Measured:
 *
 *   Linux x86-64, gcc 14.2   `LDBL_MANT_DIG` 64   x87 80-bit extended,
 *                                                 padded to sixteen with
 *                                                 six bytes never written
 *   Linux aarch64, gcc 14.2  `LDBL_MANT_DIG` 113  binary128
 *   macOS arm64, clang       `LDBL_MANT_DIG` 53   the same as `double`
 *
 * Fast DDS and OpenDDS both copy the object's bytes onto the wire, so
 * the same IDL and the same value give three different payloads on those
 * three platforms — and on x86-64 the payload also carries six bytes of
 * whatever the stack held, which changes between runs of one writer.
 * `docs/float128-column.md` §2 has the measurements.
 *
 * So this runtime never memcpys a `long double`. It converts, through
 * `zdw_f128_from_long_double`, which is exact on every platform above
 * because 53, 64 and 113 significand bits all fit inside 113 — and the
 * reverse conversion reports when it is not.
 */
typedef struct zdw_f128 {
    /* Sign, exponent, and the top 48 fraction bits. */
    uint64_t high;
    /* The low 64 fraction bits. */
    uint64_t low;
} zdw_f128;

/* -- the primitive types of the Plain Language Binding -------------------
 *
 * DDS-XTypes 1.3 §7.5.1.1.1, Table 42: "The Service shall provide
 * typedefs with the following names to types available on the underlying
 * platform that have the appropriate sizes and representations." Where
 * the table names a C99 type the typedef is that type. `DDS_Float32` and
 * `DDS_Float64` are `float` and `double`; `DDS_Float128` is the runtime's
 * `zdw_f128`, for the reason given above; `DDS_Char16` is `uint16_t`,
 * which Table 42 allows ("they may map Char16 to an equivalent integer
 * type") and which is the same size on every platform, unlike
 * `wchar_t`. Only the values 0 and 1 of `DDS_Boolean` are defined.
 */
typedef int32_t DDS_Int32;
typedef uint32_t DDS_UInt32;
typedef int8_t DDS_Int8;
typedef uint8_t DDS_UInt8;
typedef int16_t DDS_Int16;
typedef uint16_t DDS_UInt16;
typedef int64_t DDS_Int64;
typedef uint64_t DDS_UInt64;
typedef float DDS_Float32;
typedef double DDS_Float64;
typedef zdw_f128 DDS_Float128;
typedef char DDS_Char8;
typedef uint16_t DDS_Char16;
typedef _Bool DDS_Boolean;
typedef uint8_t DDS_Byte;

/* -- outcomes ------------------------------------------------------------
 *
 * A writer records the first thing that went wrong and then does
 * nothing further. The alternative — carrying on and truncating — is
 * the failure mode this project treats as the worst one: bytes that
 * look like a sample and are not.
 */
typedef enum zdw_status {
    ZDW_OK = 0,
    /* The caller's buffer cannot hold what was asked for. */
    ZDW_SHORT_BUFFER,
    /* A value outside what its holder can represent. */
    ZDW_RANGE,
    /* A parameter the runtime has no rule for — a bit bound of 0, an
     * anchor that is not a power of two, a length code above 7. */
    ZDW_BAD_ARGUMENT,
    /* Text that is not well-formed UTF-8. */
    ZDW_NOT_UTF8,
    /* A decimal that `fixed<P,S>` cannot represent. */
    ZDW_BAD_DECIMAL,
    /* Reader: fewer bytes remain than the read needs. */
    ZDW_TRUNCATED,
    /* Reader: a length the stream cannot possibly hold. */
    ZDW_IMPOSSIBLE_LENGTH,
    /* Reader: a value the type does not admit — a boolean of 2. */
    ZDW_OUT_OF_RANGE,
    /* Reader: the stream is well formed and framed the way another
     * implementation frames it, not the way this reader is configured.
     *
     * Apart from ZDW_OUT_OF_RANGE on purpose, and the same split the
     * Rust runtime draws with `WireError::WrongFraming`. A reader given
     * the wrong dialect and a reader given a corrupt stream both stop,
     * and only one of them is fixed by changing a flag. Telling a user
     * "expected a four-byte parameter length" when the answer is
     * `--dialect fastdds` costs an afternoon; `zdw_reader_fits` names
     * the setting the bytes do fit and costs nothing. */
    ZDW_WRONG_FRAMING,
    /* Reader: the stream is a well-formed sample of a compatible type
     * and no object of *this* type can be constructed from it — a
     * string longer than this bound, a sequence with more elements than
     * this one holds, an enumerator this type does not declare — where
     * the governing `@try_construct` is DISCARD.
     *
     * DDS-XTypes 1.3 §7.2.2.7. Apart from every status above it on
     * purpose. Those say the stream is wrong and the peer needs fixing;
     * this says one sample does not fit a type that is otherwise
     * perfectly compatible, and the next sample may well fit. §7.2.4.1
     * calls for the receiver to "discard the O2 object", and §7.2.3 for
     * the sample to be "safely ignored (filtered out)" — an absence,
     * not an error. A subscriber that logged every narrowed sample as a
     * protocol fault would drown; one that read a malformed stream as a
     * narrowing would never notice a broken peer. */
    ZDW_DISCARDED,
    /* Writer: a member that is `@external` and not `@optional` has no
     * value (its pointer is NULL) at the time of the write.
     *
     * DDS-XTypes 1.3 §7.5.1.2.3.1.1: "If the member is only external but
     * not optional, then it is not allowed for the member to be NULL at
     * the time of a write." Apart from ZDW_RANGE: the value is not out of
     * range, it is not there. */
    ZDW_UNSET_EXTERNAL
} zdw_status;

/* What a receiving type does with a value it cannot hold.
 * DDS-XTypes 1.3 §7.2.2.7, Table 11.
 *
 * Passed to every generated decoder, because the behaviour belongs to
 * the *position* — this member, this collection's element, this map's
 * key, this union's discriminator — and not to the type, which is
 * shared between positions that may each want a different answer. */
typedef enum zdw_try_construct {
    /* The failure propagates and the sample is discarded. The default
     * where no annotation was written. */
    ZDW_TC_DISCARD = 0,
    /* The member or element takes the default value of its type
     * (Table 9) and the container is constructed. */
    ZDW_TC_USE_DEFAULT,
    /* A string, wstring, sequence or map is cut to the bound, order
     * preserved. Undefined on anything else, which the IDL frontend
     * refuses rather than passing on. */
    ZDW_TC_TRIM
} zdw_try_construct;

/* A human-readable name, for a probe's diagnostics. */
const char *zdw_status_name(zdw_status status);

/* -- the dialect settings ------------------------------------------------
 *
 * Two questions the clause does not settle the same way the field does.
 * They are the runtime's because they are about the bytes, not about
 * the layout; which *form* a PL_CDR1 header takes is the layout plan's,
 * because it changes the member's extent.
 *
 * Set both from `zerodds_ast_layout`'s `Dialect`, whose documentation
 * carries the measurement, rather than assembling them by hand.
 */

/* How a `wstring`'s length is counted, and how wide its code units are
 * — C1 x C2 x C3, one cell per enumerator.
 *
 * Three independent questions: what the length prefix counts (octets or
 * characters), whether a terminating NUL is written and counted, and
 * how wide one UTF-16 code unit is (two octets or four). All eight
 * crossings are named, including the three no measured implementation
 * writes, because a cell that has no name cannot be reached from a
 * command line to be measured against — which is exactly how the ROS
 * form went unreachable while two implementations were writing it.
 *
 * Mirrors `zerodds_ast_layout::WstringLengthPolicy`, whose
 * documentation carries the nine-implementation table. */
typedef enum zdw_wstring_policy {
    /* Octets, no terminator, two-octet units.
     * XTypes §7.4.1.1.2 and CORBA §9.3.1.6's GIOP 1.2 `wstring` over a
     * two-octet code set. Cyclone and OpenDDS under both versions, RTI
     * under XCDR2, omniORB, JacORB. The default. */
    ZDW_WSTRING_SPECIFICATION = 0,
    /* Characters, no terminator, two-octet units. Fast DDS.
     * A code point outside the BMP is refused rather than truncated:
     * reproducing a vendor's length convention is a policy,
     * reproducing its data loss is not. */
    ZDW_WSTRING_CHARACTER_COUNT,
    /* Characters including a NUL, four-octet units.
     * CORBA §9.3.1.6's GIOP 1.1 `wstring` over a four-octet code set,
     * and RTI Connext 7.7 under XCDR1 — measured 2026-08-12 at the
     * member and the collection element alike, byte for byte, with no
     * RTI-specific rule. The four-octet characters are UTF-16 code
     * units zero-extended, so a surrogate pair survives. */
    ZDW_WSTRING_CHARACTER_COUNT_TERMINATED,
    /* Octets, no terminator, four-octet units.
     *
     * CORBA §9.3.1.6's GIOP 1.2 `wstring` over a four-octet
     * transmission code set — what TAO 2.5.24 writes. Measured
     * 2026-08-10, `wstring w = "a<U+1D11E>b"` little endian:
     * `10000000 61000000 34d80000 1edd0000 62000000` against this
     * project's `08000000 6100 34d8 1edd 6200`. */
    ZDW_WSTRING_SPECIFICATION_WIDE_UNITS,
    /* Characters, no terminator, four-octet units.
     *
     * The cell the previous enumeration could not reach, and two
     * implementations write it: `rosidl_typesupport_fastrtps` counts
     * characters like Fast CDR and hands each over as a four-octet
     * `wchar_t`, and `rmw_cyclonedds_cpp` arrives at the same octets
     * through the introspection type support. Measured 2026-08-12,
     * `wstring-unbounded-utf8` little endian, "aouo <U+1234> Z" as
     * seven characters:
     * `07000000 e4000000 f6000000 fc000000 20000000 34120000 20000000
     *  5a000000` from both. */
    ZDW_WSTRING_ROS_TYPE_SUPPORT,
    /* The three cells no measured implementation writes. Named so that
     * the product is complete and every cell is reachable; a cell that
     * cannot be named cannot be tested against a peer that turns out to
     * write it. */
    ZDW_WSTRING_OCTETS_TERMINATED_2,
    ZDW_WSTRING_OCTETS_TERMINATED_4,
    ZDW_WSTRING_CHARACTERS_TERMINATED_2
} zdw_wstring_policy;

/* How a scalar `wchar` is framed.
 *
 * A `wstring` is delimited as a whole and a scalar `wchar` is not, and
 * CORBA §9.3.1.6 gives the scalar its own framing. All five DDS
 * implementations write XTypes Table 31's bare code unit; all three
 * ORBs write an octet count and then that many octets, aligned to one.
 * Measured 2026-08-10, `wchar a = U+00E4; wchar b = U+1234`, little
 * endian: `e400 3412` here, `02 00e4 02 1234` from omniORB,
 * `04 e4000000 04 34120000` from TAO.
 *
 * It is not a width. A `wchar` that carries its own count is one octet
 * longer and aligns to one, so every member behind it moves. */
typedef enum zdw_wchar_framing {
    /* A bare code unit, aligned to its own width. XTypes Table 31. */
    ZDW_WCHAR_FIXED = 0,
    /* One octet of length, then that many octets, aligned to one.
     * CORBA §9.3.1.6's GIOP 1.2 `wchar`. */
    ZDW_WCHAR_GIOP_LENGTH_PREFIXED
} zdw_wchar_framing;

/* In what byte order a `wstring`'s code units go out.
 *
 * A third axis, because CORBA makes it one: omniORB and JacORB decode a
 * `wstring`'s units as UTF-16 big-endian whatever the stream says, and
 * omniORB writes a byte-order mark so that its own reader knows. */
typedef enum zdw_wstring_unit_order {
    /* The stream's own order. §7.4.1.1.2 through ESWAP. */
    ZDW_WSTRING_UNITS_STREAM = 0,
    /* UTF-16 big-endian whatever the stream says, no mark. What
     * omniORB and JacORB decode in the absence of one. */
    ZDW_WSTRING_UNITS_BIG_ENDIAN_ALWAYS,
    /* A byte-order mark, then the units, both little-endian. omniORB.
     * The mark's two octets are counted in the length and it is not a
     * character of the string. On read the mark decides: FEFF in the
     * stream's order means the units follow in it, FFFE means they are
     * the other way round, and no mark at all means big-endian. */
    ZDW_WSTRING_UNITS_BYTE_ORDER_MARK,
    /* The units always little-endian, whatever the stream says, and no
     * mark.
     *
     * This reproduces a peer misreading its own output, and it is on
     * this axis because it is a byte order. It arrived as a value of
     * the length enum — a byte order wearing a length's name — and
     * moved here, which also retired the rule that refused it beside a
     * non-default order: one axis cannot conflict with itself.
     *
     * Cyclone swaps a `wstring`'s length word for a big-endian stream
     * and does not swap its content, so its big-endian `"ab"` is
     * `00000004 61006200` where RTI and Fast DDS write
     * `00000004 00610062` — which a conforming reader returns as U+6100
     * U+6200, two different characters with nothing in the frame saying
     * so. `rosidl_typesupport_fastrtps` does the same, measured
     * 2026-08-12. Never a default and no dialect selects it; it is here
     * because without it nothing can exchange a `wstring` with Cyclone
     * over big endian. In little endian it is ZDW_WSTRING_UNITS_STREAM
     * byte for byte. */
    ZDW_WSTRING_UNITS_HOST_UNSWAPPED
} zdw_wstring_unit_order;

/* C1 — whether a `wstring`'s length prefix counts characters rather
 * than the octets they occupy. Non-zero for characters. */
int zdw_wstring_counts_characters(zdw_wstring_policy policy);

/* C2 — whether a terminating NUL code unit is written and counted.
 * Non-zero when it is. */
int zdw_wstring_terminated(zdw_wstring_policy policy);

/* C3 — how wide one of a `wstring`'s code units is under this policy.
 *
 * **Not** the width of a scalar `wchar`, which is `zdw_wchar_width`
 * below. The two agree in eight of the nine measured implementations
 * and `rmw_cyclonedds_cpp` 1.3.4 is the ninth: two octets for a `wchar`
 * member, four for a `wstring`'s units. This function used to be called
 * `zdw_wchar_width` and answered both, which is why a dialect for that
 * type support could not be built. */
size_t zdw_wstring_unit_width(zdw_wstring_policy policy);

/* How wide a scalar `wchar` is.
 *
 * Two octets under XTypes 1.3 Table 31 and for seven of the nine
 * measured implementations; four under RTI Connext's XCDR1 — measured
 * 2026-08-09, `wchar c = U+0061` gives `61000000` under XCDR1 and
 * `6100` under XCDR2 — under TAO, and under
 * `rosidl_typesupport_fastrtps_c`.
 *
 * The values are the widths, so that a `switch` on this enumeration and
 * an arithmetic use of it cannot disagree. */
typedef enum zdw_wchar_width {
    ZDW_WCHAR_WIDTH_2 = 2,
    ZDW_WCHAR_WIDTH_4 = 4
} zdw_wchar_width;

/* Where the value being written or read sits.
 *
 * The C half of `zerodds_ast_layout::WirePosition`. It is an argument
 * and not a field, because one stream carries several positions at
 * once: a setting made when the writer was built could not answer for
 * all of them. */
typedef enum zdw_wire_position {
    /* A member of a structure, a union branch, or the whole value. For
     * `zdw_pid_flags_at` this is a member's own parameter id. */
    ZDW_POSITION_MEMBER = 0,
    /* An element of an array or a sequence. */
    ZDW_POSITION_ELEMENT = 1,
    /* The `PID_LIST_END` that closes a PL_CDR1 parameter list. */
    ZDW_POSITION_SENTINEL = 2,
    /* The `PID_EXTENDED` marker that opens a parameter's extended
     * header. Cyclone and RTI write `7f01` under two different framing
     * forms, Fast DDS `3f01` and OpenDDS `ff01` under one, which is why
     * the marker's flags are read here and not off the form. */
    ZDW_POSITION_EXTENDED_MARKER = 3
} zdw_wire_position;

/* What flags a PL_CDR1 parameter id carries, at a position.
 *
 * XTypes 1.3 §7.4.1.2 splits the sixteen-bit field: the low fourteen
 * bits are the id, bit 14 is `ZDW_PID_MUST_UNDERSTAND` and bit 15 is
 * `ZDW_PID_IMPL_EXTENSION`. The clause fixes neither flag on
 * `PID_LIST_END`, and the four measured implementations do not agree —
 * `struct-mutable` and `evolution-mutable-mustunderstand` under XCDR1:
 *
 *     Cyclone   member as declared    sentinel 7f02
 *     RTI       member as declared    sentinel 7f02
 *     Fast DDS  member never          sentinel 3f02
 *     OpenDDS   member c0xx, always   sentinel 3f02
 *
 * The sentinel's answer cannot be read off the member's, which is what
 * makes `ZDW_POSITION_SENTINEL` a position on this axis rather than a
 * property of the list. */
typedef enum zdw_pid_flag_policy {
    /* A bare member id, `ZDW_PID_MUST_UNDERSTAND` on the sentinel.
     * Cyclone DDS and RTI Connext 7.7. */
    ZDW_PID_FLAGS_SENTINEL_MUST_UNDERSTAND = 0,
    /* Bare everywhere. Fast DDS. */
    ZDW_PID_FLAGS_BARE = 1,
    /* Both flags on every member id whatever the member declared, a
     * bare sentinel, and both flags on the extended marker. OpenDDS.
     * The `eMemberHeader` behind the marker stays bare: OpenDDS's
     * `annotation-hashid-implicit` is `ff010008 0343172c 00000004`. */
    ZDW_PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS = 2
} zdw_pid_flag_policy;

/* The flag bits a parameter id carries at `position`.
 *
 * A member's own `must_understand` is not on this axis; it is
 * `zdw_pl_cdr1_framing::member_must_understand`, and the writer ORs the
 * two. Only the member, the sentinel and the extended marker have a
 * parameter id at all; `ZDW_POSITION_ELEMENT` answers zero. */
uint16_t zdw_pid_flags_at(zdw_pid_flag_policy policy,
                          zdw_wire_position position);

/* How a PL_CDR1 parameter list is framed. */
typedef struct zdw_pl_cdr1_framing {
    /* What flags a parameter id carries, asked per position — a
     * member's id, `PID_LIST_END` and the `PID_EXTENDED` marker. */
    zdw_pid_flag_policy pid_flags;
    /* The highest member id a short parameter header carries.
     * §7.4.1.2.2 stops at 0x3F00 inclusive and reserves the rest of the
     * low fourteen bits; one implementation carries all fourteen, and a
     * reader holding the clause's threshold as a constant reports
     * `@id(16383)` as reserved and drops the member.
     *
     * Both sides: `zdw_write_pid` refuses an id above it rather than
     * putting a reserved parameter on the wire. The layout plan chooses
     * the form and this is the floor under a caller that did not. */
    uint16_t highest_short_id;
    /* Whether a `@mutable` member's declared length covers the padding
     * that follows its value. §7.4.1.2 says it does not; RTI counts it. */
    bool pad_declared_length;
    /* The same question for an `@optional` member of a type that is not
     * `@mutable`, which is a separate axis because Cyclone answers the
     * two differently: `04000000` for the two-byte `short c` of
     * `ext_mutable` and `01000000` for the one-byte `@optional octet b`
     * of `opt_final_present`. RTI pads both, Fast DDS pads neither. */
    bool pad_declared_optional_length;
    /* Whether a member's own must-understand bit goes out. §7.4.1.2.2
     * makes it unconditional and two of the three implementations write
     * it: Cyclone as `FLAG_2` of the extended `eMemberHeader`, RTI as
     * bit 14 of the short id, Fast DDS not at all. */
    bool member_must_understand;
    /* Whether the parameter that just ended writes its own trailing
     * padding to the next four-byte boundary. Every header writer
     * aligns itself, so the two readings only part at the end of the
     * stream: RTI pads after a final `@optional` parameter, Fast DDS
     * does not (`annotation-must-understand-unannotated`, 24 against
     * 22 octets). */
    bool pad_after_value;
} zdw_pl_cdr1_framing;

/* Which of §7.4.1.2's two roles a parameter header stands in.
 *
 * The header is the same header. The role decides what the length field
 * counts, and whether a member that took no value writes one at all: a
 * `@mutable` member that is absent leaves no trace in the parameter
 * list, an `@optional` member of a non-mutable type writes its header
 * with a declared length of zero. */
typedef enum zdw_parameter_role {
    /* A member of a `@mutable` type. Rules (24) and (25). */
    ZDW_PARAM_MEMBER,
    /* An `@optional` member of a type that is not `@mutable`. Rule (19). */
    ZDW_PARAM_OPTIONAL
} zdw_parameter_role;

/* The clause's framing, which is this project's default dialect —
 * `Dialect::Specification`. It was Cyclone's; see the definition. */
zdw_pl_cdr1_framing zdw_pl_cdr1_default(void);

/* -- the writer ----------------------------------------------------------
 *
 * `origin` is where alignment is counted from. Zero for a sample, and
 * still zero inside a DHEADER, because XCDR counts alignment from the
 * encapsulation and not from a nested value's own start (§7.4.1.1). A
 * PL_CDR1 parameter value is the one exception and moves it; see
 * `zdw_write_pid`.
 *
 * `max_align` is the encoding's MAXALIGN, which caps every alignment
 * request: eight under XCDR1, four under XCDR2 (§7.4.1, the XCDR stream
 * operations table — `ALIGN(N)` aligns to `MIN(N, XCDR.maxalign)`).
 */
typedef struct zdw_writer {
    uint8_t *out;
    size_t capacity;
    size_t len;
    size_t origin;
    size_t max_align;
    zdw_endianness endianness;
    zdw_status status;
    zdw_pl_cdr1_framing pl_cdr1;
    /* Whether an LC4 header whose member turns out to be 1, 2, 4 or 8
     * bytes is rewritten with a short length code. See
     * `zdw_writer_set_short_length_codes`. */
    bool short_length_codes;
} zdw_writer;

void zdw_writer_init(zdw_writer *w, uint8_t *buffer, size_t capacity,
                     zdw_endianness endianness, size_t max_align);

/* The same writer framing a PL_CDR1 parameter list another way. */
void zdw_writer_set_pl_cdr1(zdw_writer *w, zdw_pl_cdr1_framing framing);

/* Whether an LC4 header is shortened when its member fits a code.
 *
 * Set it from `zerodds_ast_layout::LengthCodePolicy::
 * short_code_where_it_fits`. The layout plan already picks a short code
 * for every member whose type fixes the size; this is the other half,
 * for a sequence or a map whose length is in the value. Fast DDS makes
 * the same choice the same way: its `sequence<short>` of two elements
 * is eight bytes and goes out as `02000030`, where three elements would
 * be ten and go out as LC4. */
void zdw_writer_set_short_length_codes(zdw_writer *w, bool shorten);

/* How many bytes have been written. Meaningless once the status is not
 * ZDW_OK, which is why a caller must check the status first. */
size_t zdw_writer_length(const zdw_writer *w);
zdw_status zdw_writer_status(const zdw_writer *w);

/* Record a refusal the generated code makes for the type: the first one
 * sticks, like every internal failure, and the writer does nothing
 * further. `ZDW_OK` records nothing. The writer-side counterpart of
 * `zdw_reader_refuse`; a generated encoder calls it with
 * `ZDW_UNSET_EXTERNAL` for an `@external` member that is NULL
 * (DDS-XTypes 1.3 §7.5.1.2.3.1.1). */
void zdw_writer_refuse(zdw_writer *w, zdw_status status);

/* Pad to `align`, counted from the stream's origin and capped at
 * MAXALIGN. */
void zdw_align(zdw_writer *w, size_t align);

/* -- packed collection elements ------------------------------------------
 *
 * `ElementPadding::Packed` is HDDS's measured departure from §7.4.1,
 * which counts every element's alignment from the encapsulation: each
 * collection element and each aggregate value counts its own alignment
 * from its own first octet instead. `array-1d-struct-final` is
 * `01000000 0200 03000000 0400 77`, the second `Cell`'s `long` at offset
 * six where the clause puts it at eight, and `typedef-of-struct` is the
 * same inside two `@final` members.
 *
 * `zdw_begin_packed_element` moves the origin to the current position
 * and returns the origin it displaced; the generated codec holds that in
 * a local and restores it with `zdw_finish_packed_element`.
 */
size_t zdw_begin_packed_element(zdw_writer *w);
void zdw_finish_packed_element(zdw_writer *w, size_t saved);

/* -- primitives ---------------------------------------------------------- */

void zdw_write_bool(zdw_writer *w, bool value);
void zdw_write_u8(zdw_writer *w, uint8_t value);
void zdw_write_i8(zdw_writer *w, int8_t value);
/* `char`, one byte on the wire, and `wchar`, two or four.
 *
 * The argument is a code point so that a caller cannot lose the high
 * bits before the runtime sees them, and one that does not fit is
 * refused: §7.2.2.2.1.2.2 says a `CHAR_8_TYPE` "shall be representable
 * using 8 bits", and §7.2.2.2.1.2.5 restricts a `CHAR_16_TYPE` to the
 * Basic Multilingual Plane. Writing the low bits would put a different
 * character on the wire with nothing said. */
void zdw_write_char(zdw_writer *w, uint32_t code_point);
/* `wchar`, at `width`. See `zdw_wchar_width` for which implementation
 * writes which, and why the width is an argument of its own rather than
 * a reading of the `wstring` policy. */
void zdw_write_wchar(zdw_writer *w, uint32_t code_point,
                     zdw_wchar_width width,
                     zdw_wchar_framing framing,
                     zdw_wstring_unit_order order);
void zdw_write_u16(zdw_writer *w, uint16_t value);
/* A 16-bit value in little-endian order whatever the stream's order is,
 * aligned to two like any other. One caller: the code units of
 * `ZDW_WSTRING_UNITS_HOST_UNSWAPPED`. */
void zdw_write_u16_little(zdw_writer *w, uint16_t value);
/* A 16-bit value in big-endian order whatever the stream's order is.
 * The other side of the same axis: the code units an ORB reads. */
void zdw_write_u16_big(zdw_writer *w, uint16_t value);
/* The same pair at 32 bits, for a four-octet transmission code set
 * crossed with a fixed unit order. No dialect selects the combination;
 * it exists so that two orthogonal settings have an answer for every
 * pairing rather than a hole one of them falls into. */
void zdw_write_u32_little(zdw_writer *w, uint32_t value);
void zdw_write_u32_big(zdw_writer *w, uint32_t value);
void zdw_write_i16(zdw_writer *w, int16_t value);
void zdw_write_u32(zdw_writer *w, uint32_t value);
void zdw_write_i32(zdw_writer *w, int32_t value);
void zdw_write_u64(zdw_writer *w, uint64_t value);
void zdw_write_i64(zdw_writer *w, int64_t value);
void zdw_write_f32(zdw_writer *w, float value);
void zdw_write_f64(zdw_writer *w, double value);
/* `long double`: sixteen bytes of IEEE 754 binary128.
 *
 * The value is a `zdw_f128` and never a `long double`, on purpose. See
 * the section at the end of this header for why, and for the explicit
 * conversions that replace the memcpy two DDS vendors do. */
void zdw_write_f128(zdw_writer *w, zdw_f128 value);

/* -- strings -------------------------------------------------------------
 *
 * §7.4.1.1: a four-byte length *including* the terminator, the bytes,
 * then a NUL. The included terminator is the one place XCDR differs
 * from every other length-prefixed encoding.
 */
void zdw_write_string(zdw_writer *w, const char *text);

/* A bounded `string`: the same bytes `zdw_write_string` writes, refused
 * with `ZDW_RANGE` rather than cut at a character boundary when `text`
 * holds more than `bound` octets. The bound is a promise about the
 * source and not a difference on the wire, so this is the only place it
 * can be enforced; XTypes 1.3 §7.2.2.7's TRIM is a *read*-side
 * behaviour and belongs to the receiver's own bound.
 */
void zdw_write_string_bounded(zdw_writer *w, const char *text, uint32_t bound);

/* A bounded `sequence`'s count, checked against `bound` before the
 * DHEADER or the length word goes out. Table 17 makes
 * `O2.length <= T1.length` the condition for constructing one, and the
 * wire carries no bound to compare against: refused with `ZDW_RANGE`.
 */
void zdw_check_sequence_bound(zdw_writer *w, uint32_t actual, uint32_t bound);

/* A bounded `map`'s entry count, as `zdw_check_sequence_bound` for a
 * `sequence` — the unit is an entry and not an element, per §7.4.1.1.4's
 * map-as-a-sequence-of-pairs equivalence.
 */
void zdw_check_map_bound(zdw_writer *w, uint32_t actual, uint32_t bound);

/* A `wstring`, given as UTF-8 and written in whichever length
 * convention the policy names.
 *
 * §7.4.1.1.2 fixes three things the specification form reimplements
 * rather than assumes: the length counts *bytes* — "twice the number of
 * characters" — because a code point outside the Basic Multilingual
 * Plane takes two units; no BOM; no terminating NUL. The conversion is
 * here so that the surrogate rule is exercised by this runtime and not
 * by its caller.
 */
void zdw_write_wstring_utf8(zdw_writer *w, const char *utf8,
                            zdw_wstring_policy policy,
                            zdw_wstring_unit_order order);

/* A bounded `wstring`, as `zdw_write_string_bounded` is to
 * `zdw_write_string`: the same units `zdw_write_wstring_utf8` writes,
 * refused with `ZDW_RANGE` when `utf8` holds more than `bound` UTF-16
 * code units — not characters and not octets, whatever `policy` puts on
 * the wire.
 */
void zdw_write_wstring_bounded(zdw_writer *w, const char *utf8, uint32_t bound,
                               zdw_wstring_policy policy,
                               zdw_wstring_unit_order order);

/* A `wstring` whose characters go out as their own UTF-8 octets.
 *
 * `WstringCodeUnits::Utf8` is the measured departure of one product's
 * generated serialiser: XTypes 1.3 Table 31 gives a `wstring`'s code
 * unit as `Char16` and §7.4.1.1.2 makes the length "twice the number of
 * characters", so the clause has no UTF-8 form at all. The declared
 * length is the UTF-8 octet count and only C2 — the terminator — still
 * reaches the wire. `wstring-member` is `04 00 00 00 61 62 63 00` for
 * `"abc"`: three octets, a NUL, and the four declares both. */
void zdw_write_wstring_utf8_octets(zdw_writer *w, const char *utf8,
                                   zdw_wstring_policy policy);

/* A bounded UTF-8-octet `wstring`, as `zdw_write_wstring_bounded` is to
 * `zdw_write_wstring_utf8`: the bound still counts UTF-16 code units,
 * which is what every `wstring<N>` bound means, while the wire carries
 * the UTF-8 octets. */
void zdw_write_wstring_utf8_octets_bounded(zdw_writer *w, const char *utf8,
                                           uint32_t bound,
                                           zdw_wstring_policy policy);

/* -- framing -------------------------------------------------------------
 *
 * A DHEADER's length is not known until the members are written, so it
 * goes out as a placeholder and is patched on finish.
 */
typedef struct zdw_dheader {
    size_t at;
    bool present;
} zdw_dheader;

zdw_dheader zdw_write_dheader(zdw_writer *w);
/* A DHEADER that is not written at all, so that generated code can hold
 * one unconditionally and let the plan decide. */
zdw_dheader zdw_no_dheader(void);
void zdw_finish_dheader(zdw_writer *w, zdw_dheader header);

/* An XCDR2 EMHEADER: must-understand, length code and member id in one
 * word. §7.4.3.4.2.
 *
 * The length code is the layout plan's decision and arrives here as a
 * number; this runtime does not choose one, for the same reason the
 * Rust one does not — a second chooser is a second thing to get wrong.
 *
 * Codes 0..3 carry the length in the code. Code 4 is followed by a
 * NEXTINT holding the member's length. Codes 5..7 also have a NEXTINT,
 * but §7.4.3.4.2 says it "shall be reused also as part of the
 * serialized member", so no placeholder is reserved: the member's own
 * leading length word lands in that position and *is* the NEXTINT.
 *
 * A member id above `ZDW_E_MEMBER_ID_MASK` is refused with ZDW_RANGE.
 * §7.2.2.4.4.4.4 runs the ids from 0 to 0x0FFFFFFF and the word holds
 * exactly those bits, so masking a larger one would write a well-formed
 * header naming a member the caller did not.
 */
typedef struct zdw_emheader {
    /* Where the EMHEADER word itself sits, so a short length code can
     * be patched into it once the value's length is known. */
    size_t word;
    size_t next_int;
    bool has_next_int;
    /* The code the header was written with, so that `zdw_finish_emheader`
     * can hold the member's value to the length the code declares. `0xff`
     * marks a header no call succeeded in writing. */
    uint8_t length_code;
} zdw_emheader;

/* The member length a length code declares: 1, 2, 4 or 8 octets for
 * codes 0..3. Codes 4..7 carry a NEXTINT instead and declare nothing on
 * their own. §7.4.3.4.2. */
#define ZDW_EMHEADER_NO_CODE 0xffu

zdw_emheader zdw_write_emheader(zdw_writer *w, uint32_t member_id,
                                uint8_t length_code, bool must_understand);
void zdw_finish_emheader(zdw_writer *w, zdw_emheader header);

/* A PL_CDR1 parameter header. §7.4.1.2.
 *
 * Short form `[id:u16][length:u16]`, extended form
 * `[PID_EXTENDED][8][id:u32][length:u32]`. The length field carries the
 * **unpadded** value length; the padding to the next four-byte boundary
 * follows the value and is not counted.
 *
 * Each form refuses with ZDW_RANGE the ids its own field cannot carry:
 * `zdw_write_pid` an id above `highest_short_id`, whose short header
 * would land in the range §7.4.1.2.2 reserves for the protocol's own
 * parameters, and `zdw_write_pid_extended` an id above
 * `ZDW_E_MEMBER_ID_MASK`, which its `eMemberHeader` would mask.
 */
typedef struct zdw_pid {
    size_t at;
    size_t saved_origin;
    bool extended;
    /* The member id and its must-understand, kept so that a short
     * header can be rewritten as an extended one when the value turns
     * out not to fit sixteen bits. The bit is in a different word in
     * the two forms. */
    uint32_t pid;
    bool must_understand;
    /* Which length convention `zdw_finish_pid` patches by. */
    zdw_parameter_role role;
} zdw_pid;

zdw_pid zdw_write_pid(zdw_writer *w, uint16_t pid, zdw_parameter_role role,
                      bool must_understand);
zdw_pid zdw_write_pid_extended(zdw_writer *w, uint32_t member_id,
                               zdw_parameter_role role, bool must_understand);
void zdw_finish_pid(zdw_writer *w, zdw_pid header);

/* PID_LIST_END. A reader that stops at the last member it recognises
 * instead of at the sentinel misreads whatever follows. §7.4.1.2.4. */
void zdw_write_sentinel(zdw_writer *w);

#define ZDW_PID_EXTENDED 0x3f01u
#define ZDW_PID_LIST_END 0x3f02u
#define ZDW_PID_SHORT_MAX_ID 0x3f00u
/* FLAG_IMPL_EXTENSION, bit 15 of a short parameter id. */
#define ZDW_PID_IMPL_EXTENSION 0x8000u
/* PID_IGNORE. Every consumer shall ignore it. §7.4.1.2.2. */
#define ZDW_PID_IGNORE 0x3f03u
/* The must-understand bit of a parameter id. DDSI-RTPS 2.5 §9.4.2.11.2. */
#define ZDW_PID_MUST_UNDERSTAND 0x4000u

/* `FLAG_2` of the extended `eMemberHeader`, which is the member's own
 * must-understand, and the mask of the 28-bit member id beside it.
 * XTypes 1.3 §7.4.1.2.1. */
#define ZDW_E_FLAG_MUST_UNDERSTAND 0x40000000u
/* `FLAG_1`, which is FLAG_IMPL_EXTENSION. */
#define ZDW_E_FLAG_IMPL_EXTENSION 0x80000000u
#define ZDW_E_MEMBER_ID_MASK 0x0fffffffu

/* -- the constructed scalars ---------------------------------------------
 *
 * These carry a rule of their own, so they are the runtime's and not
 * the generated code's: a generator that computed the bit pattern would
 * put the rule in a place no second implementation can be diffed
 * against.
 */

/* An enumeration, as the integer its bit bound calls for.
 *
 * §7.4.3.5.3 rule (5) writes `O.value` as `O.holder_type`, and the
 * ENUM_TYPE row of the same clause makes the holder signed: Int8, Int16
 * or Int32 by the `@bit_bound`. So `@value(-1)` at the default bound is
 * `ffffffff`, and the accepted range is everything the bound's bits can
 * express in either signedness — `-2^(n-1) ..= 2^n - 1`. §7.3.1.2.1.5
 * caps the bound itself at 1..32.
 */
void zdw_write_enum(zdw_writer *w, uint16_t bit_bound, int64_t value);

/* A bitmask, as the unsigned integer Table 33 of §7.4.1.1.3.2 gives for
 * its bound. A bit at or past the bound is refused: the clause says
 * those bits "are not considered to be part of the bitmask", so setting
 * one would put a value on the wire no peer must read back. */
void zdw_write_bitmask(zdw_writer *w, uint16_t bit_bound, uint64_t bits);

/* A bitset, as the anchor integer its bitfields pack into. The anchor
 * width is the layout plan's answer and arrives as a byte count.
 *
 * XTypes 1.3 has no serialisation rule for a bitset; §7.4.3.5.3 runs
 * from (1) to (30) and none matches one. What is implemented is the
 * only reading under which an IDL bitset round-trips through the
 * TypeObject of §7.3.4.6: positions run cumulatively from zero in
 * declaration order, anonymous padding included.
 */
void zdw_write_bitset(zdw_writer *w, uint32_t anchor_bytes, uint64_t bits);

/* The Cyclone union overlay: the low `octets` of `pattern` as one
 * unsigned integer of that width in the stream's byte order. The
 * `zdw_bit_pattern_*` functions produce the pattern — the u64 whose
 * little-endian bytes are a union arm's little-endian storage octets,
 * zero-extended; the integer form masks rather than casts, so a
 * negative value keeps its two's-complement octets and nothing more. */
void zdw_write_union_overlay(zdw_writer *w, uint64_t pattern, uint32_t octets);
uint64_t zdw_bit_pattern_int(uint64_t value, uint32_t octets);
uint64_t zdw_bit_pattern_bool(bool value);
uint64_t zdw_bit_pattern_f32(float value);
uint64_t zdw_bit_pattern_f64(double value);

/* One bitfield's contribution, range-checked.
 *
 * A value wider than its field would overwrite the field above it.
 * Masking it would put a different number on the wire than the caller
 * asked for, so this refuses instead and returns zero with the writer
 * already failed.
 */
uint64_t zdw_bitfield(zdw_writer *w, uint64_t value, uint32_t width,
                      uint32_t position);

/* How many digit nibbles a `fixed<P,S>` puts on the wire.
 *
 * [CDR] §9.3.2.8 gives the packing and says nothing about the count:
 * omniORB 4.3.3 and this project write the declared `P`, JacORB 3.9 as
 * many digits as the value has, and ACE/TAO 2.5.24 the digits of the
 * string the value was parsed from. The counterpart of
 * `zerodds_ast_layout::FixedDigitCount`, implemented here from the
 * clause rather than called across.
 */
typedef enum zdw_fixed_digits {
    /* `P` digits, whatever the value is. The default. */
    ZDW_FIXED_DECLARED = 0,
    /* As many digits as the value has, and never fewer than one:
     * `BigDecimal("0.00").precision()` is 1 in Java, so a zero is the
     * single nibble `0` and the sign. */
    ZDW_FIXED_VALUE_PRECISION,
    /* The integer digits of the decimal as written, leading zeros
     * included, plus the S fraction digits, made odd by one leading zero: what ACE/TAO 2.5.24's
     * `ACE_CDR::Fixed::from_string` writes through its own CDR operator; OpenDDS writes the declared count. `0.12345`
     * under `fixed<5,5>` is `0012345c`, wider than `P`; a count past 31
     * is refused. Read like ZDW_FIXED_VALUE_PRECISION. */
    ZDW_FIXED_VALUE_STRING_DIGITS
} zdw_fixed_digits;

/* The lowest nibble value that is a sign and not a digit. [CDR]
 * §9.3.2.8: a digit is `0` to `9`. It is what makes the value-precision
 * form self-delimiting on the read side. */
#define ZDW_FIXED_FIRST_SIGN_NIBBLE 0x0au

/* `fixed<P,S>` from a decimal string, as packed BCD.
 *
 * DDS-XTypes 1.3 does not carry the type at all — no kind in §7.2.2, no
 * row in §7.3.1, no rule in §7.4.3.5.3. The form written is CORBA's,
 * [CDR] §9.3.2.8, which is where an IDL `fixed` came from and the only
 * wire definition it has: digit nibbles most-significant first, then a
 * sign nibble (`0xc` positive or zero, `0xd` negative), with a leading
 * zero nibble when the nibble count is odd. How many digits go out is
 * `zdw_fixed_digits`.
 */
void zdw_write_fixed(zdw_writer *w, const char *decimal, uint8_t digits,
                     uint8_t scale, zdw_fixed_digits count);

/* -- the KeyHash --------------------------------------------------------- */

/* The holder as a KeyHash: zero-padded when it is sixteen bytes or
 * fewer, MD5 of it when it is longer. DDSI-RTPS 2.5 §9.6.3.8. */
void zdw_zero_padded(const zdw_writer *w, uint8_t out[16]);
void zdw_md5_of(const zdw_writer *w, uint8_t out[16]);
void zdw_md5(const uint8_t *input, size_t length, uint8_t out[16]);

/* -- the reader ----------------------------------------------------------
 *
 * The mirror of the writer, and the same sticky-failure discipline: a
 * reader that returned a zero for a truncated stream would hand the
 * caller a value nobody sent.
 */
typedef struct zdw_reader {
    const uint8_t *bytes;
    size_t length;
    size_t at;
    /* Where alignment is counted from. Zero for a sample; a PL_CDR1
     * parameter value moves it, the same way the writer's does. */
    size_t origin;
    size_t max_align;
    zdw_endianness endianness;
    zdw_status status;
    /* Where the first failure was. A status with no offset sends its
     * reader back through the whole payload by hand. */
    size_t error_at;
    /* For ZDW_WRONG_FRAMING: the setting whose other value the stream
     * fits. NULL when none does, which makes it malformed after all. */
    const char *fits;
    zdw_pl_cdr1_framing pl_cdr1;
    /* Storage for the values a read has to materialise — a string's
     * bytes, a sequence's elements. The caller's, for the same reason
     * the writer's buffer is: an embedded target decides where memory
     * comes from, and a runtime that called `malloc` would decide for
     * it. A read that needs storage and has none fails with
     * ZDW_SHORT_BUFFER rather than returning a shorter value. */
    uint8_t *heap;
    size_t heap_capacity;
    size_t heap_used;
} zdw_reader;

void zdw_reader_init(zdw_reader *r, const uint8_t *bytes, size_t length,
                     zdw_endianness endianness, size_t max_align);
/* The same reader expecting another implementation's PL_CDR1 framing. */
void zdw_reader_set_pl_cdr1(zdw_reader *r, zdw_pl_cdr1_framing framing);
/* Where the variable-length values go. */
void zdw_reader_set_heap(zdw_reader *r, uint8_t *buffer, size_t capacity);
zdw_status zdw_reader_status(const zdw_reader *r);
/* Where the first failure was. Meaningless while the status is ZDW_OK. */
size_t zdw_reader_error_offset(const zdw_reader *r);
/* The setting the bytes fit, for ZDW_WRONG_FRAMING. NULL otherwise. */
const char *zdw_reader_fits(const zdw_reader *r);
size_t zdw_reader_position(const zdw_reader *r);
size_t zdw_reader_remaining(const zdw_reader *r);
void zdw_reader_align(zdw_reader *r, size_t align);

/* The read-side pair of the writer's packed-element origin. See
 * `zdw_begin_packed_element` for the axis and the recorded rows. */
size_t zdw_reader_begin_packed_element(zdw_reader *r);
void zdw_reader_finish_packed_element(zdw_reader *r, size_t saved);

/* Move the cursor forward to `target`, which is never backwards: a
 * framing word pointing behind the cursor means the reader has already
 * read past the end of what it framed, and seeking back would paper
 * over it. */
void zdw_reader_skip_to(zdw_reader *r, size_t target);

/* A block of the caller's storage, aligned for any scalar.
 *
 * Sixteen bytes of alignment for every block rather than the element's
 * own: C99 has no `alignof`, and a generated caller that passed
 * `sizeof(T)` would be stating a size where an alignment belongs.
 * Over-aligning is always correct and costs at most fifteen bytes.
 *
 * NULL with the reader failed when the heap is exhausted or absent.
 */
void *zdw_reader_alloc(zdw_reader *r, size_t bytes);

/* Everything a two-pass read has to put back.
 *
 * One caller is left: a `map` under `rule-15-omits`, where a delimited
 * map states no entry count and the entries have to be read once to
 * count them and again into storage sized from that count. The heap
 * moves during the first pass — an entry may hold a string — so the
 * mark carries `heap_used` and not only the cursor.
 *
 * The other caller was the `@optional` guess, on the reading that
 * nothing marks an absent optional member in a type that is not
 * `@mutable`. Rule (20) and §7.4.1.2 do mark it and all three
 * implementations write the marker, so that caller is gone. */
typedef struct zdw_reader_mark {
    size_t at;
    size_t origin;
    size_t heap_used;
    zdw_status status;
} zdw_reader_mark;

zdw_reader_mark zdw_reader_save(const zdw_reader *r);
void zdw_reader_restore(zdw_reader *r, zdw_reader_mark mark);

/* Refuse the stream from outside the runtime.
 *
 * The runtime owns the wire and the generated code owns the type, and
 * some refusals are the type's: an integer that names no enumerator, a
 * bitmask bit the declaration does not have, a sequence longer than its
 * bound, a member that did not arrive. This runtime cannot see any of
 * them, and a generated decoder with no way to say so would hand its
 * caller a value the type does not admit.
 *
 * The first refusal sticks, the same way an internal one does. */
void zdw_reader_refuse(zdw_reader *r, zdw_status status);

/* Take back a ZDW_DISCARDED, and only that one.
 *
 * A member whose own `@try_construct` is USE_DEFAULT but which also
 * carries an explicit `@default` cannot be answered inside the type's
 * decoder: the decoder knows Table 9's value for the type and not the
 * literal the member's declaration gave it, and §7.2.2.4.4.4.9 puts the
 * literal first — "An explicit definition associates a default value
 * with the member at the time where the aggregated type is defined."
 *
 * So the call site asks for DISCARD, and where it gets one it clears it
 * and assigns the literal itself. The reader's position is already
 * right: every narrowing reads the whole value off the wire before it
 * decides, so a discarded member has consumed exactly its own bytes.
 *
 * Every other status stays: a truncated stream is not a narrowing and
 * must not be cleared by anything. Returns whether there was one. */
bool zdw_reader_clear_discard(zdw_reader *r);

bool zdw_read_bool(zdw_reader *r);
uint8_t zdw_read_u8(zdw_reader *r);
int8_t zdw_read_i8(zdw_reader *r);
uint32_t zdw_read_char(zdw_reader *r);
/* `wchar`, at `width`. A four-byte character holding more than one
 * UTF-16 code unit is refused rather than narrowed. */
uint32_t zdw_read_wchar(zdw_reader *r, zdw_wchar_width width,
                        zdw_wchar_framing framing,
                        zdw_wstring_unit_order order);
uint16_t zdw_read_u16(zdw_reader *r);
/* A 16-bit value in little-endian order whatever the stream's order is.
 * The counterpart of `zdw_write_u16_little`. */
uint16_t zdw_read_u16_little(zdw_reader *r);
/* A 16-bit value in big-endian order whatever the stream's order is.
 * The counterpart of `zdw_write_u16_big`. */
uint16_t zdw_read_u16_big(zdw_reader *r);
/* The same pair at 32 bits. */
uint32_t zdw_read_u32_little(zdw_reader *r);
uint32_t zdw_read_u32_big(zdw_reader *r);
int16_t zdw_read_i16(zdw_reader *r);
uint32_t zdw_read_u32(zdw_reader *r);
int32_t zdw_read_i32(zdw_reader *r);
uint64_t zdw_read_u64(zdw_reader *r);
int64_t zdw_read_i64(zdw_reader *r);
float zdw_read_f32(zdw_reader *r);
double zdw_read_f64(zdw_reader *r);
zdw_f128 zdw_read_f128(zdw_reader *r);

/* A string into `buffer`, NUL-terminated. Returns the number of bytes
 * the text takes, terminator excluded. The buffer is the caller's, for
 * the same reason the writer's is. */
size_t zdw_read_string(zdw_reader *r, char *buffer, size_t capacity);

/* The same string, into the reader's heap.
 *
 * What a generated decoder needs: an IDL `string` maps to a
 * `const char *` and the length is the stream's, so the destination
 * cannot be a fixed field. NULL with the reader failed on any refusal,
 * never a shortened string. */
const char *zdw_read_string_alloc(zdw_reader *r);

/* A `wstring`, in whichever length convention the policy names, as
 * UTF-8 in the reader's heap.
 *
 * The inverse of `zdw_write_wstring_utf8`, and the conversion is here
 * for the same reason: §7.4.1.1.2's byte count depends on the surrogate
 * rule, so both directions of it belong to this runtime.
 *
 * Nothing here looks at the length to decide which convention it is.
 * The length is the field a peer controls and the conventions are not
 * separable anyway — a character-count `"ab"` followed by a `long` is
 * byte-identical to a specification `"a"` followed by the same `long`.
 * The configured form is read, and a length that form cannot have
 * produced fails with ZDW_WRONG_FRAMING and `zdw_reader_fits` naming
 * the one it does fit. */
const char *zdw_read_wstring_utf8(zdw_reader *r, zdw_wstring_policy policy,
                                  zdw_wstring_unit_order order);

/* The inverse of `zdw_write_wstring_utf8_octets`: the declared length is
 * the UTF-8 octet count, the terminator `policy` asks for is the last of
 * those octets, and a count the stream cannot hold, a missing or an
 * interior NUL, or an ill-formed UTF-8 sequence is refused. See
 * `WstringCodeUnits::Utf8`. */
const char *zdw_read_wstring_utf8_octets(zdw_reader *r,
                                         zdw_wstring_policy policy);

/* -- try_construct ------------------------------------------------------
 *
 * DDS-XTypes 1.3 §7.2.2.7 decides what a receiver does with a value its
 * own bound cannot hold. The whole value is always read off the wire
 * first — stopping at the bound would leave the rest of it in the
 * stream and put every following member at the wrong offset — and the
 * three helpers below are what the generated decoders apply afterwards.
 */

/* How many octets `text` is, which is what a `string<N>` bound counts:
 * `string<5>` is `char[6]` in the C mapping of every measured DDS
 * implementation, so a five-scalar value of ten octets is one no peer
 * can hold. Named beside its wide counterpart rather than written as
 * `strlen` at each use, so the two units stay visibly different. */
size_t zdw_utf8_octets(const char *text);

/* How many UTF-16 code units the UTF-8 `text` is, which is what a
 * `wstring<N>` bound counts. A surrogate pair is two. */
size_t zdw_utf16_units(const char *text);

/* `text` cut to at most `octets` bytes, in the reader's heap, never
 * inside a character.
 *
 * Table 11: "The order of the characters in the string ... is
 * preserved", so this is a prefix. A character that would straddle the
 * bound is left out whole: a peer holding `char[octets + 1]` cannot
 * store half of one either, and half a UTF-8 sequence is not text.
 *
 * Octets and not scalar values, because that is what the bound means to
 * every peer that has to hold the value — `string<5>` is `char[6]` in
 * the C mapping of all four measured DDS implementations. */
const char *zdw_trim_utf8(zdw_reader *r, const char *text, size_t octets);

/* `text` cut to at most `units` UTF-16 code units, in the reader's
 * heap, never inside a character or between the halves of a surrogate
 * pair. The counterpart of `zdw_trim_utf8` for a `wstring<N>`. */
const char *zdw_trim_utf16(zdw_reader *r, const char *text, size_t units);

/* `fixed<P,S>` from packed BCD, as the canonical decimal in the
 * reader's heap: one leading zero at most before the point, exactly `S`
 * digits after it, and a `-` only on a non-zero negative. [CDR]
 * §9.3.2.8 gives zero the positive sign nibble, so there is one zero
 * and one spelling of it. */
const char *zdw_read_fixed(zdw_reader *r, uint8_t digits, uint8_t scale,
                           zdw_fixed_digits count);

/* The length a DHEADER announces. */
uint32_t zdw_read_dheader(zdw_reader *r);

typedef struct zdw_emheader_read {
    uint32_t member_id;
    uint8_t length_code;
    bool must_understand;
    /* The NEXTINT, for the codes that carry one. Codes 5 to 7 report it
     * without consuming it: §7.4.3.4.2 says the word "shall be reused
     * also as part of the serialized member", so it is the member's own
     * leading length and a reader that ate it starts the member one
     * word late. */
    uint32_t next_int;
    bool has_next_int;
    /* How many bytes the member occupies after the header word,
     * NEXTINT included where there is one. What a reader skipping an
     * unknown member advances by. */
    size_t extent;
} zdw_emheader_read;

zdw_emheader_read zdw_read_emheader(zdw_reader *r);

/* A PL_CDR1 parameter header, in whichever form it was written.
 *
 * §7.4.1.2 splits the sixteen-bit field: the member id is the low
 * fourteen bits, bit 14 is must-understand and bit 15 is
 * implementation-specific. Comparing the whole word against `0x3f01`
 * and `0x3f02` recognises neither Cyclone's `PID_EXTENDED` nor the
 * sentinel Cyclone and RTI both write — `vendor_bytes.md` §2 records
 * `7f01` and `7f02` — and a reader that missed them takes the extended
 * header's id and length for eight bytes of value and runs on past the
 * end of the list.
 */
typedef struct zdw_pid_read {
    uint32_t pid;
    /* Which length convention `zdw_finish_pid_read` checks against. */
    zdw_parameter_role role;
    uint32_t length;
    bool end;
    bool must_understand;
    /* FLAG_IMPL_EXTENSION. §7.4.1.2.2: the parameter is the writing
     * implementation's own, and a user-defined member "will never set
     * the FLAG_IMPL_EXTENSION bit". Masking it off with the rest made a
     * vendor-private parameter indistinguishable from the member with
     * the same low fourteen bits, and its payload was read into that
     * member with no error. */
    bool impl_extension;
    /* The short-form id is inside §7.4.1.2.2's reserved range
     * 0x3F01-0x3FFF, which no member id occupies. PID_EXTENDED and
     * PID_LIST_END never reach this flag; what does is PID_IGNORE and
     * the range above it. */
    bool reserved;
    /* PID_IGNORE. §7.4.1.2.2: "All consumers of this Data
     * Representation shall ignore parameters with this ID" — with no
     * exception for the must-understand bit. */
    bool ignore;
    /* The stream origin this parameter's block displaced.
     * `zdw_finish_pid_read` puts it back. */
    size_t saved_origin;
} zdw_pid_read;

/* Whether this parameter can be one of the reading type's members.
 * False for a vendor-private parameter and for a reserved id; both are
 * legal for a peer to send and neither is a member, so both take the
 * unknown-parameter path. §7.4.1.2.2, §7.2.2.4.4.4.6. */
bool zdw_pid_is_user_member(zdw_pid_read header);

zdw_pid_read zdw_read_pid(zdw_reader *r);

/* The same header in a named role, which is what the declared-length
 * check is made against. `zdw_read_pid` is `ZDW_PARAM_MEMBER`. */
zdw_pid_read zdw_read_pid_as(zdw_reader *r, zdw_parameter_role role);

/* End a parameter's value and step to the next header.
 *
 * The padding to four follows the value and is outside the declared
 * length, so the next parameter is found by alignment and not by adding
 * the length — which is what lets one reader take RTI's rounded-up
 * length and Fast DDS's exact one. The declared length is checked
 * against the configured framing here, and a length belonging to the
 * other convention is ZDW_WRONG_FRAMING rather than ZDW_OUT_OF_RANGE.
 */
void zdw_finish_pid_read(zdw_reader *r, zdw_pid_read header);

/* An enumeration read back into a signed value, sign-extended from its
 * bound — the inverse of `zdw_write_enum`. */
int64_t zdw_read_enum(zdw_reader *r, uint16_t bit_bound);
uint64_t zdw_read_bitmask(zdw_reader *r, uint16_t bit_bound);
uint64_t zdw_read_bitset(zdw_reader *r, uint32_t anchor_bytes);

/* -- binary128 -----------------------------------------------------------
 *
 * `src/float128.c`. Conversions only; no arithmetic, because the wire
 * path needs none. Nothing here allocates and nothing here needs libm
 * except the two `long double` conversions, which use `frexpl`/`ldexpl`
 * rather than reading the object's bytes.
 */

/* Constants, as bit patterns: `zdw_f128` is a struct, so these are
 * functions rather than macros a caller could half-initialise. */
zdw_f128 zdw_f128_zero(void);
zdw_f128 zdw_f128_one(void);
zdw_f128 zdw_f128_infinity(void);
zdw_f128 zdw_f128_quiet_nan(void);

/* From the two halves. What a generated encoder's value literal uses:
 * the pattern is exact, and a decimal literal in generated C would be
 * parsed by whatever `strtold` the target has. */
zdw_f128 zdw_f128_from_parts(uint64_t high, uint64_t low);

/* The sixteen bytes, in the order named. The two orders are the whole
 * sixteen bytes reversed — CORBA Figure 9.2 — and not two eight-byte
 * halves swapped in place. */
zdw_f128 zdw_f128_from_bytes(const uint8_t bytes[16], zdw_endianness order);
void zdw_f128_to_bytes(zdw_f128 value, zdw_endianness order, uint8_t out[16]);

/* Classification. */
bool zdw_f128_is_nan(zdw_f128 value);
bool zdw_f128_is_infinite(zdw_f128 value);
bool zdw_f128_is_finite(zdw_f128 value);
bool zdw_f128_is_zero(zdw_f128 value);
bool zdw_f128_is_negative(zdw_f128 value);

/* Bit-pattern equality, which is what a key holder compares. Not IEEE
 * equality: this says -0.0 and +0.0 are different, because on the wire
 * they are. */
bool zdw_f128_same_bits(zdw_f128 a, zdw_f128 b);

/* Widening, which is always exact: binary128 covers every `double`. */
zdw_f128 zdw_f128_from_double(double value);
zdw_f128 zdw_f128_from_float(float value);

/* Narrowing. `*out` is always written with the correctly rounded result;
 * the return value says whether that rounding lost anything. A caller
 * that ignores it is choosing to narrow, which is different from not
 * being told. */
bool zdw_f128_to_double(zdw_f128 value, double *out);

/* The platform's `long double`, converted rather than copied.
 *
 * Exact in the widening direction on every platform whose `long double`
 * has 113 or fewer significand bits, which is all of them. The narrowing
 * direction reports, like `zdw_f128_to_double`: on x86-64 a value with
 * more than 64 significand bits does not fit, and on macOS arm64 one
 * with more than 53 does not. */
zdw_f128 zdw_f128_from_long_double(long double value);
bool zdw_f128_to_long_double(zdw_f128 value, long double *out);

/* -- binary128 as decimal ------------------------------------------------
 *
 * `src/float128_decimal.c`, a separate translation unit: it wants a few
 * kilobytes of stack for the long division and an embedded target that
 * only moves samples should not have to link it.
 *
 * The *exact* expansion — every digit the value has, which for a binary
 * value is always finitely many. That is the canonical form the interop
 * corpus compares columns in, because it is the one form four languages
 * can produce by printing, with no parser and no shortest-form search to
 * disagree about. `crates/wire-probe/src/decode.rs` says the same.
 */

/* Enough for any binary128. The longest expansion is the *largest*
 * subnormal, `(2^113 - 1) * 2^-16494`, whose exact decimal has 11 563
 * digits; with a sign, a point, `e-4931` and a NUL that is 11 572. The
 * bound is stated as the arithmetic rather than as a round number so the
 * next reader can check it. */
#define ZDW_F128_DECIMAL_MAX 11600

/* Writes the exact decimal into `out` and returns its length, excluding
 * the NUL. Writes nothing and returns 0 when `capacity` is too small, so
 * a short buffer is an empty answer and never a truncated number. An
 * infinity is "inf" or "-inf" and a NaN is "nan". */
size_t zdw_f128_to_decimal(zdw_f128 value, char *out, size_t capacity);

#ifdef __cplusplus
}
#endif

#endif /* ZERODDS_WIRE_H */

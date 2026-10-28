// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The C++ wire runtime: a fourth reading of DDS-XTypes 1.3 §7.4, beside
// `crates/wire`, `runtime/c` and `runtime/python/zerodds_wire`. Not a
// binding to any of them — nothing here calls into the C runtime and this
// header never includes `zerodds_wire.h`. A difference between this column
// and another is a difference in how two people read the clause, which is
// the only reason to have four.
//
// C++17, standard library only.
//
// ## The three things this header exists to keep straight
//
// - **Alignment is counted from the stream's origin and capped at
//   MAXALIGN.** §7.4.1: `ALIGN(N)` aligns to `MIN(N, XCDR.maxalign)`, and
//   MAXALIGN is 8 under XCDR1 and 4 under XCDR2. The offset is measured
//   from `origin`, not from zero and not from the start of the value. A
//   `long long` after a `char` sits at 8 under XCDR1 and at 4 under XCDR2,
//   and a runtime that padded to 8 in both puts four bytes on the wire
//   that no XCDR2 peer expects.
// - **A length is bytes and includes what a caller would not count.** A
//   `string`'s four-byte length counts its NUL terminator and counts
//   *octets* of UTF-8. A `wstring` under `WstringLengthPolicy::Specification`
//   counts octets too, carries no BOM and carries no terminator.
// - **A value its field cannot hold is refused, never truncated.** Every
//   integer write takes the widest natural type and range-checks it, so a
//   caller that hands `70000` to an `unsigned short` gets a `WireError` and
//   not `4464` on the wire.
//
// Errors are exceptions. Every one carries the same fields the Python
// column's carry: `at`, and `needed`/`available`, `claimed`/`available` or
// `expected` as the failure has them.
//
// Spec references are to OMG DDS-XTypes 1.3 unless another document is
// named. `[CDR]` is CORBA 3.3 part 2 §9.3.

#ifndef ZERODDS_WIRE_HPP
#define ZERODDS_WIRE_HPP

#include <array>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <vector>

namespace zerodds::wire {

// -- byte order -------------------------------------------------------------

/// Which end of a multi-byte value goes out first.
enum class Endianness { Little, Big };

/// Whether the low byte goes out first.
constexpr bool is_little(Endianness order) noexcept {
    return order == Endianness::Little;
}

/// The command line's spelling, or nothing for anything else.
///
/// Nothing and not a default: a typo in a wire-affecting flag has to be
/// loud, because the bytes it produces look plausible.
std::optional<Endianness> parse_endianness(std::string_view name);

// -- encodings --------------------------------------------------------------

/// Which encoding a plan and a stream are in.
///
/// XCDR1 and XCDR2 differ in their framing: XCDR1 has no delimiter header,
/// and `@mutable` uses PID headers there and EMHEADERs here.
enum class Encoding { PlainCdr1, PlCdr1, PlainCdr2, DelimitedCdr, PlCdr2 };

/// Whether this is an XCDR version 1 encoding.
constexpr bool is_v1(Encoding encoding) noexcept {
    return encoding == Encoding::PlainCdr1 || encoding == Encoding::PlCdr1;
}

/// Whether this is an XCDR version 2 encoding.
constexpr bool is_v2(Encoding encoding) noexcept { return !is_v1(encoding); }

/// The largest alignment any primitive takes under this encoding.
///
/// §7.4.1: `MAXALIGN(VERSION2) = 4`, `MAXALIGN(VERSION1) = 8`, and
/// `ALIGN(N)` aligns to `MIN(N, XCDR.maxalign)`. §7.6.8 repeats it for the
/// key holder: "the maximum alignment in PLAIN_CDR2 (i.e., 4)".
constexpr std::size_t max_alignment(Encoding encoding) noexcept {
    return is_v1(encoding) ? std::size_t{8} : std::size_t{4};
}

/// The command line's spelling, including the two shorthands `xcdr1` and
/// `xcdr2`, which name the plain form. An unknown name is nothing.
std::optional<Encoding> parse_encoding(std::string_view name);

/// How many zero bytes bring `length` up to the next multiple of four.
///
/// §7.4.1.1 pads the serialized data to a four-byte boundary; §7.6.3.1.2
/// states the count in the encapsulation options.
constexpr unsigned encapsulation_padding(std::size_t length) noexcept {
    return static_cast<unsigned>((4u - (length & 3u)) & 3u);
}

/// The four-byte encapsulation header.
///
/// The identifier itself is always big-endian on the wire, whatever the
/// payload behind it is — it is what tells a reader which order to switch
/// to. The two option bytes carry the payload's trailing padding count in
/// the low two bits of the second one (§7.6.3.1.2).
std::array<std::uint8_t, 4> encapsulation_header(Encoding encoding,
                                                 Endianness endianness,
                                                 unsigned padding = 0);

// -- errors -----------------------------------------------------------------

/// Base of every wire failure.
///
/// Every subclass names what was expected and where: a reader that returned
/// a zero for a truncated stream would hand the caller a value that was
/// never sent, and a writer that clamped an over-wide integer would put a
/// different number on the wire than the caller asked for.
class WireError : public std::runtime_error {
  public:
    WireError(std::size_t at, const std::string &message)
        : std::runtime_error(message), at_(at) {}

    /// The offset the failure was noticed at.
    std::size_t at() const noexcept { return at_; }

  private:
    std::size_t at_;
};

/// The stream ended before the value did.
class Truncated : public WireError {
  public:
    Truncated(std::size_t at, std::size_t needed, std::size_t available);
    std::size_t needed() const noexcept { return needed_; }
    std::size_t available() const noexcept { return available_; }

  private:
    std::size_t needed_;
    std::size_t available_;
};

/// A length field claims more than the stream holds.
///
/// Separate from `Truncated`: a length that cannot be true is a malformed
/// stream, not a short one, and allocating for it is how a reader turns a
/// bad message into an out-of-memory.
class ImpossibleLength : public WireError {
  public:
    ImpossibleLength(std::size_t at, std::size_t claimed, std::size_t available);
    std::size_t claimed() const noexcept { return claimed_; }
    std::size_t available() const noexcept { return available_; }

  private:
    std::size_t claimed_;
    std::size_t available_;
};

/// A string held bytes that are not UTF-8.
class NotUtf8 : public WireError {
  public:
    explicit NotUtf8(std::size_t at);
};

/// A value outside what its type admits.
///
/// Raised by the writer as well as the reader. The C++ column needs it on
/// the write side for the same reason the Python one does: the widest
/// natural type is wider than the field, so the check has to happen at the
/// point of the write rather than in the type system.
class OutOfRange : public WireError {
  public:
    OutOfRange(std::size_t at, const std::string &expected);
    /// What the value should have been, as prose.
    const std::string &expected() const noexcept { return expected_; }

  private:
    std::string expected_;
};

// -- what a reader reports to generated code --------------------------------

/// How a read ended, for the refusals a generated decoder makes itself.
///
/// The exceptions above are this runtime's own: a stream that ran out, a
/// length no buffer that long can carry, bytes that are not UTF-8. This
/// enumeration is the *other* channel, and it exists because some refusals
/// belong to the type rather than to the wire, and this runtime cannot see
/// one of them:
///
/// - an integer that names no enumerator;
/// - a bitmask bit the declaration does not have;
/// - a sequence or a string longer than its bound;
/// - a member the stream did not carry;
/// - §7.2.2.7's DISCARD, which is not a failure at all — the receiver drops
///   the object and carries on.
///
/// A generated decoder says so with `reader_refuse`, and the first refusal
/// sticks. `runtime/c`'s `zdw_status` is the same set under C's spelling,
/// and the two are kept in step deliberately: the columns are compared
/// against each other, and a status one of them cannot express is a
/// divergence that would show up as "the C++ probe said nothing".
///
/// ## What this does *not* do
///
/// It does not turn the exceptions into codes. A truncated stream still
/// throws, and a generated decoder does not catch it — the sample is gone
/// either way, and the probe's `main` is where the two channels meet. The
/// visible difference from the C column is that C keeps reading after its
/// own runtime failure, with every later read returning zero, while this
/// one unwinds; both end at "the sample could not be read", and neither
/// hands the caller a value the stream did not carry.
enum class Status {
    /// Nothing has been refused.
    Ok,
    /// A value needed storage that was not there. C's `ZDW_SHORT_BUFFER`;
    /// unreachable here, where the containers own their storage, and named
    /// so the two enumerations line up.
    ShortBuffer,
    /// A value outside what its field can hold.
    Range,
    /// A parameter no clause allows — a `@bit_bound` of zero.
    BadArgument,
    /// Bytes that are not well-formed UTF-8.
    NotUtf8,
    /// A `fixed` whose digits are not a decimal.
    BadDecimal,
    /// The stream ended where a value was expected.
    Truncated,
    /// A length no stream that long can carry.
    ImpossibleLength,
    /// A value outside what the *type* admits: an unnamed enumerator, an
    /// undeclared bitmask bit, a member that arrived twice.
    OutOfRange,
    /// Well-formed bytes belonging to another implementation's framing.
    WrongFraming,
    /// §7.2.2.7: the sample is discarded. Not a failure — see
    /// `reader_clear_discard`.
    Discarded,
};

/// The status's name, for a diagnostic. Never null.
const char *status_name(Status status) noexcept;

/// What a position does with a value the type cannot hold. §7.2.2.7.
///
/// Attached to the position and not to the type: the clause hangs the
/// behaviour on the member, the element, the map key or the discriminator,
/// and one `typedef string<5>` may be used at two positions that want
/// different answers. So it is a parameter every generated decoder takes.
enum class TryConstruct {
    /// The sample is discarded.
    Discard,
    /// The member takes Table 9's value.
    UseDefault,
    /// The value is trimmed to what fits.
    Trim,
};

/// Everything a two-pass read has to put back.
///
/// One caller: a `map` under `rule-15-omits`, where a delimited map states
/// no entry count and the entries have to be read once to count them and
/// again into storage sized from that count.
///
/// C's `zdw_reader_mark` carries `heap_used` as well, because its reader
/// allocates out of a buffer the caller set and the first pass moves it.
/// Here the entries go into a `std::vector` that is discarded, so there is
/// nothing to rewind but the cursor, the origin and the status.
struct ReaderMark {
    std::size_t at = 0;
    std::size_t origin = 0;
    Status status = Status::Ok;
};

/// How many octets of UTF-8 `text` takes, which is what a narrow string's
/// bound counts.
std::size_t utf8_octets(const std::string &text) noexcept;

/// How many UTF-16 code units `text` takes, which is what a `wstring`'s
/// bound counts — §7.4.1.1.2, so a scalar above the Basic Multilingual
/// Plane counts two.
std::size_t utf16_units(const std::string &text) noexcept;

// -- binary128 --------------------------------------------------------------

/// An IEEE 754-2008 binary128 value.
///
/// [CDR] §9.3.1.3 with Figure 9.2 gives CDR `long double` one sign bit, a
/// fifteen-bit exponent biased by 16383 and a 112-bit stored fraction;
/// Table 31 names the result — quadruple precision, sixteen bytes, aligned
/// to eight.
///
/// Held as two 64-bit halves in *value* order, high first, so the type
/// means the same thing on a big-endian and a little-endian host and the
/// writer is the only place a byte order is chosen.
///
/// ## Why this is not a `long double`
///
/// Because the C++ type is not the wire type, and on the platforms this
/// project runs it is not even one type: `LDBL_MANT_DIG` is 64 on Linux
/// x86-64 (x87 extended, padded to sixteen with six bytes never written),
/// 113 on Linux aarch64, and 53 on macOS arm64. Two DDS vendors memcpy the
/// object onto the wire, so the same IDL and the same value give three
/// different payloads. Nothing here ever reads the bytes of a
/// `long double`.
///
/// No arithmetic: nothing in the wire path needs it, and a half-correct
/// `operator+` is worse than none.
struct Float128 {
    /// Sign, exponent, and the top 48 fraction bits.
    std::uint64_t high = 0;
    /// The low 64 fraction bits.
    std::uint64_t low = 0;

    static constexpr Float128 from_parts(std::uint64_t high_half,
                                         std::uint64_t low_half) noexcept {
        return Float128{high_half, low_half};
    }

    static Float128 zero(bool negative = false) noexcept;
    static Float128 one() noexcept;
    static Float128 infinity(bool negative = false) noexcept;
    static Float128 quiet_nan() noexcept;

    /// The sixteen bytes in the order named.
    ///
    /// The two orders are the whole sixteen bytes reversed — [CDR]
    /// Figure 9.2 — and **not** two eight-byte halves swapped in place.
    static Float128 from_bytes(const std::array<std::uint8_t, 16> &bytes,
                               Endianness order) noexcept;
    std::array<std::uint8_t, 16> to_bytes(Endianness order) const noexcept;

    /// Widening, which is always exact: binary128 covers every `double`,
    /// subnormals included, with 60 significand bits to spare.
    static Float128 from_double(double value) noexcept;
    static Float128 from_float(float value) noexcept;

    /// To a `double`, rounded to nearest with ties to even.
    ///
    /// Overflow gives an infinity and underflow a signed zero, which is
    /// what IEEE 754-2008 clause 7.4 requires of the default rounding.
    double to_double() const noexcept;

    /// The same narrowing, saying whether it lost anything.
    ///
    /// `*out` is always the correctly rounded result; the return value
    /// says whether that rounding was exact. A caller that ignores it is
    /// choosing to narrow, which is different from not being told.
    bool to_double_checked(double *out) const noexcept;

    /// From an exact decimal, correctly rounded to nearest, ties to even.
    ///
    /// Never through a `double`: a two-step parse rounds twice, and double
    /// rounding gets the last bit wrong for exactly the values this type
    /// exists to carry. `inf`, `-inf`, `infinity` and `nan` are accepted.
    ///
    /// Throws `OutOfRange` for text that is not a decimal.
    static Float128 from_decimal(std::string_view text);

    /// Every digit the value has, in the corpus's canonical form.
    ///
    /// The *exact* expansion and not the shortest one. Every finite binary
    /// value has finitely many decimal digits, because `m * 2^-k` is
    /// `m * 5^k / 10^k`, so this needs no rounding and no search — which is
    /// why the corpus canonicalises to the exact form: four columns that
    /// each implemented a shortest-form search would disagree about which
    /// of two equally short spellings to emit long before they disagreed
    /// about a value, and the suite would read that as a wire difference.
    std::string to_exact_decimal() const;

    bool is_nan() const noexcept;
    bool is_infinite() const noexcept;
    bool is_finite() const noexcept;
    bool is_zero() const noexcept;
    bool is_negative() const noexcept;
    bool is_subnormal() const noexcept;

    /// Bit-pattern equality, which is what a key holder compares. Not IEEE
    /// equality: this says -0.0 and +0.0 are different, because on the wire
    /// they are, and it says a NaN equals itself.
    bool same_bits(const Float128 &other) const noexcept {
        return high == other.high && low == other.low;
    }
};

/// `octet`: DDS::Byte, XTypes 1.3 §7.5.1.1.2, Table 43.
///
/// "Types DDS::Boolean, DDS::Char8, and DDS::Byte may all map to the same underlying C++ type
/// [...] All other mappings for basic types shall be distinguishable for the purposes of
/// overloading." `uint8` is `std::uint8_t`, which is `unsigned char`, so an `octet` that were
/// `std::uint8_t` too could not be told from it by an overload. This is the one-octet class
/// that can: it converts to and from `std::uint8_t` implicitly, so a member of it takes and
/// gives the values the wire runtime reads and writes, and no codec is written differently
/// for it. The octets of a `std::vector<Octet>` or a `std::array<Octet, N>` are contiguous,
/// which is what `write_octets` and `read_octets` take.
class Octet {
public:
    constexpr Octet() noexcept : value_(0) {}
    constexpr Octet(std::uint8_t value) noexcept : value_(value) {}  // NOLINT: implicit by design
    constexpr operator std::uint8_t() const noexcept { return value_; }

private:
    std::uint8_t value_;
};

static_assert(sizeof(Octet) == 1, "an Octet is one octet");
static_assert(std::is_trivially_copyable<Octet>::value, "an Octet is copied as its octet");

inline bool operator==(const Float128 &a, const Float128 &b) noexcept {
    return a.same_bits(b);
}
inline bool operator!=(const Float128 &a, const Float128 &b) noexcept {
    return !a.same_bits(b);
}

// -- MD5 --------------------------------------------------------------------

/// RFC 1321. DDSI-RTPS 2.5 §9.6.3.8 names MD5 for the KeyHash of a holder
/// longer than sixteen bytes; it is not a security claim — the KeyHash
/// identifies an instance and is not a signature.
std::array<std::uint8_t, 16> md5(const std::uint8_t *data, std::size_t length);
std::array<std::uint8_t, 16> md5(std::string_view text);

// -- the PL_CDR1 dialect ----------------------------------------------------

/// Which of §7.4.1.2's two roles a parameter header stands in.
///
/// The header is the same header. The role decides what the length field
/// counts, and whether a member that took no value writes one at all: a
/// `@mutable` member that is absent leaves no trace in the parameter list,
/// an `@optional` member of a non-mutable type writes its header with a
/// declared length of zero.
enum class ParameterRole {
    /// A member of a `@mutable` type. Rules (24) and (25).
    Member,
    /// An `@optional` member of a type that is not `@mutable`. Rule (19).
    Optional,
};

/// Where the value being written or read sits.
///
/// It is an argument and not a setting, because one stream carries several
/// positions at once: a decision made when the writer was built could not
/// answer for all of them.
enum class WirePosition {
    /// A member's own parameter id.
    Member,
    /// An element of an array or a sequence. It has no parameter id.
    Element,
    /// The `PID_LIST_END` that closes a PL_CDR1 parameter list.
    Sentinel,
    /// The `PID_EXTENDED` marker that opens a parameter's extended header.
    ExtendedMarker,
};

/// What flags a PL_CDR1 parameter id carries.
///
/// §7.4.1.2 splits the sixteen-bit field: the low fourteen bits are the id,
/// bit 14 is `PID_MUST_UNDERSTAND` and bit 15 is `PID_IMPL_EXTENSION`. The
/// clause fixes neither flag on `PID_LIST_END`, and the four measured
/// implementations do not agree — `struct-mutable` and
/// `evolution-mutable-mustunderstand` under XCDR1:
///
///     Cyclone   member as declared    sentinel 7f02
///     RTI       member as declared    sentinel 7f02
///     Fast DDS  member never          sentinel 3f02
///     OpenDDS   member c0xx, always   sentinel 3f02
///
/// The sentinel's answer cannot be read off the member's, which is what
/// makes `WirePosition::Sentinel` a position on this axis rather than a
/// property of the list. The extended marker is the third position, and it
/// is here for the same reason: Cyclone and RTI agree across two different
/// framing forms while Fast DDS and OpenDDS share a form and disagree.
enum class PidFlagPolicy {
    /// A bare member id, `PID_MUST_UNDERSTAND` on the sentinel and on the
    /// extended marker. Cyclone DDS and RTI Connext 7.7.
    SentinelMustUnderstand,
    /// Bare everywhere. Fast DDS.
    Bare,
    /// Both flags on every member id whatever the member declared, a bare
    /// sentinel, and both flags on the extended marker. OpenDDS.
    ImplExtensionMembers,
};

/// The flag bits a parameter id carries at `position`.
///
/// A member's own `must_understand` is not on this axis; it is
/// `PlCdr1Framing::member_must_understand`, and the writer ORs the two.
std::uint16_t pid_flags_at(PidFlagPolicy policy, WirePosition position);

/// How a PL_CDR1 parameter list is framed.
///
/// The defaults are the clause's — `Dialect::Specification`, which is this
/// project's default dialect.
struct PlCdr1Framing {
    /// What flags a parameter id carries, asked per position.
    PidFlagPolicy pid_flags = PidFlagPolicy::SentinelMustUnderstand;
    /// The highest member id a short parameter header carries. §7.4.1.2.2
    /// stops at 0x3F00 inclusive and reserves the rest of the low fourteen
    /// bits; one implementation carries all fourteen, and a reader holding
    /// the clause's threshold as a constant reports `@id(16383)` as
    /// reserved and drops the member. Both sides: `write_pid` refuses an id
    /// above it rather than putting a reserved parameter on the wire.
    std::uint16_t highest_short_id = 0x3F00;
    /// Whether a `@mutable` member's declared length covers its padding.
    bool pad_declared_length = false;
    /// The same question for an `@optional` member of a type that is not
    /// `@mutable`. A separate axis because Cyclone answers the two
    /// differently: `04000000` for the two-byte `short c` of `ext_mutable`,
    /// `01000000` for the one-byte `@optional octet b` of
    /// `opt_final_present`. RTI pads both, Fast DDS pads neither.
    bool pad_declared_optional_length = false;
    /// Whether a member's own must-understand bit goes out. §7.4.1.2.2
    /// makes it unconditional and two of the three write it: Cyclone as
    /// `FLAG_2` of the extended `eMemberHeader`, RTI as bit 14 of the short
    /// id, Fast DDS not at all.
    bool member_must_understand = true;
    /// Whether the parameter that just ended writes its own trailing
    /// padding to the next four-byte boundary. Every header writer
    /// aligns itself, so the two readings only part at the end of the
    /// stream: RTI pads after a final `@optional` parameter, Fast DDS
    /// does not (`annotation-must-understand-unannotated`, 24 against
    /// 22 octets).
    bool pad_after_value = true;

    /// Whether a parameter in this role declares a padded length.
    bool pads_declared_length(ParameterRole role) const noexcept {
        return role == ParameterRole::Optional ? pad_declared_optional_length
                                               : pad_declared_length;
    }
};

// -- the wide-character axes ------------------------------------------------

/// How a `wstring`'s length is counted, and how wide its code units are —
/// C1 x C2 x C3, one enumerator per cell.
///
/// Three independent questions: what the length prefix counts (octets or
/// characters), whether a terminating NUL is written and counted, and how
/// wide one UTF-16 code unit is. All eight crossings are named, including
/// the three no measured implementation writes, because a cell that has no
/// name cannot be reached from a command line to be measured against —
/// which is exactly how the ROS form went unreachable while two
/// implementations were writing it.
enum class WstringLengthPolicy {
    /// Octets, no terminator, two-octet units. §7.4.1.1.2 and [CDR]
    /// §9.3.1.6's GIOP 1.2 `wstring` over a two-octet code set. Cyclone and
    /// OpenDDS under both versions, RTI under XCDR2, omniORB, JacORB. The
    /// default.
    Specification,
    /// Characters, no terminator, two-octet units. Fast DDS. A code point
    /// outside the BMP is refused rather than truncated: reproducing a
    /// vendor's length convention is a policy, reproducing its data loss is
    /// not.
    CharacterCount,
    /// Characters including a NUL, four-octet units. [CDR] §9.3.1.6's GIOP
    /// 1.1 `wstring` over a four-octet code set, and RTI Connext 7.7 under
    /// XCDR1. The four-octet characters are UTF-16 code units zero-extended,
    /// so a surrogate pair survives.
    CharacterCountTerminated,
    /// Octets, no terminator, four-octet units. [CDR] §9.3.1.6's GIOP 1.2
    /// `wstring` over a four-octet transmission code set — what TAO 2.5.24
    /// writes.
    SpecificationWideUnits,
    /// Characters, no terminator, four-octet units. Both
    /// `rosidl_typesupport_fastrtps` and `rmw_cyclonedds_cpp`.
    RosTypeSupport,
    /// The three cells no measured implementation writes. Named so that the
    /// product is complete and every cell is reachable.
    OctetsTerminated2,
    OctetsTerminated4,
    CharactersTerminated2,
};

/// C1 — whether the length prefix counts characters rather than the octets
/// they occupy.
bool wstring_counts_characters(WstringLengthPolicy policy);
/// C2 — whether a terminating NUL code unit is written and counted.
bool wstring_terminated(WstringLengthPolicy policy);
/// C3 — how wide one of a `wstring`'s code units is under this policy.
///
/// **Not** the width of a scalar `wchar`, which is `WcharWidth` and an
/// argument of its own: `rmw_cyclonedds_cpp` 1.3.4 writes two octets for a
/// `wchar` member and four for a `wstring`'s units.
std::size_t wstring_unit_width(WstringLengthPolicy policy);

/// The spelling `WstringLengthPolicy::as_str` uses in the Rust and Python
/// columns, including the coordinate form `counts/terminator/width` that
/// reaches every cell.
std::optional<WstringLengthPolicy> parse_wstring_policy(std::string_view name);

/// In what byte order a `wstring`'s code units go out.
///
/// A third axis, because [CDR] makes it one: omniORB and JacORB decode a
/// `wstring`'s units as UTF-16 big-endian whatever the stream says, and
/// omniORB writes a byte-order mark so that its own reader knows.
enum class WstringUnitOrder {
    /// The stream's own order. §7.4.1.1.2 through ESWAP.
    Stream,
    /// UTF-16 big-endian whatever the stream says, no mark. What omniORB
    /// and JacORB decode in the absence of one.
    BigEndianAlways,
    /// A byte-order mark, then the units, both little-endian. omniORB. The
    /// mark's octets are counted in the length and it is not a character of
    /// the string. On read the mark decides.
    ByteOrderMark,
    /// The units always little-endian, whatever the stream says, and no
    /// mark. This reproduces a peer misreading its own output — Cyclone
    /// swaps a `wstring`'s length word for a big-endian stream and does not
    /// swap its content. Never a default; without it nothing can exchange a
    /// `wstring` with Cyclone over big endian.
    HostUnswapped,
};

/// How a scalar `wchar` is framed.
///
/// A `wstring` is delimited as a whole and a scalar `wchar` is not, and
/// [CDR] §9.3.1.6 gives the scalar its own framing. All five DDS
/// implementations write Table 31's bare code unit; all three ORBs write an
/// octet count and then that many octets, aligned to one.
///
/// It is not a width. A `wchar` that carries its own count is one octet
/// longer and aligns to one, so every member behind it moves.
enum class WcharFraming {
    /// A bare code unit, aligned to its own width. Table 31.
    Fixed,
    /// One octet of length, then that many octets, aligned to one. [CDR]
    /// §9.3.1.6's GIOP 1.2 `wchar`.
    GiopLengthPrefixed,
};

/// How wide a scalar `wchar` is.
///
/// Two octets under Table 31 and for seven of the nine measured
/// implementations; four under RTI Connext's XCDR1, under TAO, and under
/// `rosidl_typesupport_fastrtps_c`. The values are the widths, so a
/// `switch` on this and an arithmetic use of it cannot disagree.
enum class WcharWidth : unsigned { Two = 2, Four = 4 };

/// How many digit nibbles a `fixed<P,S>` puts on the wire.
///
/// [CDR] §9.3.2.8 gives the packing and says nothing about the count.
enum class FixedDigitCount {
    /// `P` digits, whatever the value is. omniORB 4.3.3 and this project.
    /// The default.
    Declared,
    /// As many digits as the value has, never fewer than one. JacORB 3.9.
    /// `BigDecimal("0.00").precision()` is 1 in Java, which is where the
    /// "never fewer than one" comes from.
    ValuePrecision,
    /// The integer digits of the decimal as written, leading zeros
    /// included, plus the S fraction digits, made odd by one leading zero: ACE/TAO 2.5.24's
    /// `ACE_CDR::Fixed::from_string` through its own CDR operator (OpenDDS writes the declared count). `0.12345` under
    /// `fixed<5,5>` is `0012345c`, wider than `P`; a count past 31 is
    /// refused. Read like `ValuePrecision`.
    ValueStringDigits,
};

// -- @external -------------------------------------------------------------

/// Storage for an `@external` member, and a reference to it.
///
/// DDS-XTypes 1.3 Table 21 makes `@external` "stored externally to the
/// containing type"; `crates/lang-rules/src/adapter/cpp.rs` spells that
/// `std::shared_ptr<T>`, which is the one holder that is complete whatever
/// it points at — a member reaching its own enclosing class back has no size
/// held inline, and clang answers `field has incomplete type` for
/// `tests/interop-corpus/recursive-optional-external`.
///
/// A reader has nothing to write into until the pointer points at something,
/// so this is the counterpart of `std::optional::emplace`: it fills the slot
/// and hands back the place. Value-initialised, which is the same starting
/// value `emplace` gives, and the generated decoder overwrites it member by
/// member either way.
///
/// A function of the runtime and not `std::make_shared<T>()` written at the
/// call site: the generated statement has the member and not its type, and
/// `T` is deduced here.
template <typename T>
inline T &external_slot(std::shared_ptr<T> &slot) {
    slot = std::make_shared<T>();
    return *slot;
}

// -- optional and external members ------------------------------------------

/// An `@external` member that is not `@optional` had no value at the time of
/// a write.
///
/// DDS-XTypes 1.3 §7.5.1.2.3.2.1: "If the member is only external but not
/// optional, then it is not allowed for the member to be unset at the time of
/// a write." The writer refuses the sample, as it refuses any value it cannot
/// write, by throwing.
class UnsetExternal : public WireError {
  public:
    UnsetExternal(std::size_t at, const std::string &member)
        : WireError(at, "at " + std::to_string(at) + ": the external member `" +
                            member + "` is not optional and has no value") {}
};

/// `p` as the member's place, with an object made for it when it was empty.
///
/// An `@optional` member is a plain pointer (§7.5.1.2.4.2), NULL for an
/// omitted value, and a reader has nothing to write into until it points at
/// something. Value-initialised, and the generated decoder overwrites it
/// member by member. A function of the runtime because the generated
/// statement has the member and not its type; `T` is deduced here. It is a
/// template, so a type declared after the one holding the pointer is
/// complete where it is instantiated.
template <typename T>
inline T &pointer_slot(T *&p) {
    if (p == nullptr) {
        p = new T();
    }
    return *p;
}

/// Releases what `p` points at and leaves it NULL. §7.5.1.2.3.2: "The
/// destructor [...] shall delete the objects referenced by non-NULL external
/// member pointers", as it is in C.
template <typename T>
inline void destroy(T *&p) noexcept {
    delete p;
    p = nullptr;
}

/// An external member held in a `std::shared_ptr` lets go of its object.
template <typename T>
inline void destroy(std::shared_ptr<T> &p) noexcept {
    p.reset();
}

/// A deep copy: a new object that is a copy of the one `p` points at, or
/// NULL. §7.5.1.2.3.1: "The copy function of the enclosing object shall do
/// a deep copy of the external members."
template <typename T>
inline T *clone(const T *p) {
    return p == nullptr ? nullptr : new T(*p);
}

/// The deep copy of an external member held in a `std::shared_ptr`.
template <typename T>
inline std::shared_ptr<T> clone(const std::shared_ptr<T> &p) {
    return p ? std::make_shared<T>(*p) : nullptr;
}

/// The pointer, leaving NULL behind: a move of an owned pointer.
template <typename P>
inline P take(P &p) noexcept {
    P taken = std::move(p);
    p = nullptr;
    return taken;
}

// -- the named constants ----------------------------------------------------

/// The parameter id that introduces the extended form. §7.4.1.2.2.
inline constexpr std::uint16_t PID_EXTENDED = 0x3F01;
/// The parameter id that ends a parameter list. §7.4.1.2.4.
inline constexpr std::uint16_t PID_LIST_END = 0x3F02;
/// PID_IGNORE. §7.4.1.2.2: "All consumers of this Data Representation shall
/// ignore parameters with this ID" — including a consumer that would
/// otherwise discard the sample over the must-understand bit.
inline constexpr std::uint16_t PID_IGNORE = 0x3F03;
/// The highest member id a short parameter header can carry. §7.4.1.2.2
/// gives the range as 0 to 0x3F00 *inclusive*, and PID_EXTENDED is one
/// above it.
inline constexpr std::uint16_t PID_SHORT_MAX_ID = 0x3F00;
/// The longest value a short parameter header can state the length of.
/// §7.4.1.2.2 gives `ssize` sixteen bits; `Writer::finish_pid` rewrites the
/// header rather than truncating a longer length into two bytes.
inline constexpr std::size_t PID_SHORT_MAX_LEN = 0xFFFF;
/// FLAG_IMPL_EXTENSION, bit 15 of a short parameter id. §7.4.1.2.2.
inline constexpr std::uint16_t PID_IMPL_EXTENSION = 0x8000;
/// The must-understand bit of a parameter id. DDSI-RTPS 2.5 §9.4.2.11.2.
inline constexpr std::uint16_t PID_MUST_UNDERSTAND = 0x4000;
/// The low fourteen bits of a short parameter id, which are the id.
inline constexpr std::uint16_t PID_ID_MASK = 0x3FFF;

/// `FLAG_2` of the extended `eMemberHeader`, which is the **member's** own
/// must-understand. §7.4.1.2.1.
inline constexpr std::uint32_t E_FLAG_MUST_UNDERSTAND = 0x4000'0000u;
/// `FLAG_1` of the extended `eMemberHeader`, which is FLAG_IMPL_EXTENSION.
inline constexpr std::uint32_t E_FLAG_IMPL_EXTENSION = 0x8000'0000u;
/// The mask of the 28-bit member id beside those flags.
inline constexpr std::uint32_t E_MEMBER_ID_MASK = 0x0FFF'FFFFu;

/// A KeyHash is sixteen bytes. DDSI-RTPS 2.5 §9.6.3.8.
inline constexpr std::size_t KEY_HASH_LEN = 16;

/// The UTF-16 byte-order mark, and its byte-swapped form.
///
/// `U+FFFE` is not a character: a mark that reads as one says the units
/// behind it are the other way round from the order they were read in.
inline constexpr std::uint16_t BYTE_ORDER_MARK = 0xFEFF;
inline constexpr std::uint16_t SWAPPED_BYTE_ORDER_MARK = 0xFFFE;

// -- the framing handles ----------------------------------------------------

/// A DHEADER waiting for its length.
struct DHeader {
    std::size_t at = 0;
    /// Whether it was written at all, so generated code can hold one
    /// unconditionally and let the plan decide.
    bool present = false;
};

/// A DHEADER that is not written at all.
///
/// The free-function form of `Writer::no_dheader`. The shared codec walk
/// describes it as an operation on the runtime with no receiver, and a
/// static member of the *writer* was the wrong home for it: the reading
/// direction asks for such an operation too — `pid_is_user_member` — and
/// the writer has no business declaring that one.
inline DHeader no_dheader() noexcept { return DHeader{}; }

/// The IEEE 754 binary32 bits of `value`, zero-extended.
///
/// One half of `Writer::write_union_overlay`'s pattern argument — the
/// `u64` whose little-endian bytes are the value's little-endian
/// storage octets. The integer forms need no function: a masked cast
/// at the call site produces the same pattern.
inline std::uint64_t bit_pattern_f32(float value) noexcept {
    std::uint32_t bits = 0;
    std::memcpy(&bits, &value, sizeof bits);
    return bits;
}

/// The IEEE 754 binary64 bits of `value`. See `bit_pattern_f32`.
inline std::uint64_t bit_pattern_f64(double value) noexcept {
    std::uint64_t bits = 0;
    std::memcpy(&bits, &value, sizeof bits);
    return bits;
}

/// An EMHEADER waiting for its NEXTINT, when it has one.
struct EmHeader {
    /// Where the header word itself sits, so a short length code can be
    /// patched into it once the value's length is known.
    std::size_t word = 0;
    std::size_t next_int = 0;
    bool has_next_int = false;
    /// The code the header was written with, so that `finish_emheader` can
    /// hold the member's value to the length the code declares. Codes 0..3
    /// are lengths; anything else declares nothing of its own.
    unsigned length_code = 8;
};

/// A parameter header waiting for its length.
struct Pid {
    std::size_t at = 0;
    std::size_t saved_origin = 0;
    bool extended = false;
    /// Which length convention `finish_pid` patches by.
    ParameterRole role = ParameterRole::Member;
    /// The member id and its must-understand, kept so that a short header
    /// can be rewritten as an extended one when the value turns out not to
    /// fit sixteen bits. The bit is in a different word in the two forms.
    std::uint32_t pid = 0;
    bool must_understand = false;
};

/// An EMHEADER as read.
struct EmHeaderRead {
    std::uint32_t member_id = 0;
    unsigned length_code = 0;
    bool must_understand = false;
    /// The NEXTINT, for the codes that carry one. Codes 5 to 7 report it
    /// without consuming it: §7.4.3.4.2 says the word "shall be reused also
    /// as part of the serialized member", so it is the member's own leading
    /// length and a reader that ate it starts the member one word late.
    std::uint32_t next_int = 0;
    bool has_next_int = false;
    /// How many bytes the member occupies from where the header ended.
    ///
    /// The length code's whole meaning as one number, so a caller has one
    /// thing to add to the cursor rather than a `switch` of its own —
    /// which `skip_member` already is, and which a generated decoder
    /// needs too: it has to know where the member ends in order to check
    /// that the member's own decoder stopped there. C's
    /// `zdw_emheader_read` carries the same field.
    std::size_t extent = 0;
};

/// A parameter header as read.
struct PidRead {
    std::uint32_t pid = 0;
    std::uint32_t length = 0;
    bool end = false;
    /// Whether the writer set the member's must-understand flag.
    bool must_understand = false;
    /// Whether the writer set FLAG_IMPL_EXTENSION. §7.4.1.2.2: the
    /// parameter is the writing implementation's own, and a user-defined
    /// member "will never set the FLAG_IMPL_EXTENSION bit". Masking it off
    /// with the rest made a vendor-private parameter indistinguishable from
    /// the member with the same low fourteen bits.
    bool impl_extension = false;
    /// Whether the short-form id is inside §7.4.1.2.2's reserved range
    /// 0x3F01-0x3FFF, which no member id occupies.
    bool reserved = false;
    /// PID_IGNORE, which §7.4.1.2.2 says every consumer shall ignore — with
    /// no exception for the must-understand bit.
    bool ignore = false;
    /// Which of §7.4.1.2's two roles the caller read it in.
    ParameterRole role = ParameterRole::Member;

    /// Whether this parameter can be a member of the reading type.
    ///
    /// False for a vendor-private parameter and for a reserved id. Both are
    /// legal for a peer to send and neither is a member, so both take the
    /// unknown-parameter path — skipped, or the sample discarded when the
    /// writer set must-understand. §7.4.1.2.2, §7.2.2.4.4.4.6.
    bool is_user_member() const noexcept {
        return !impl_extension && !reserved && !ignore;
    }

    /// Where the origin was before this parameter's value began.
    ///
    /// A parameter is its own encapsulated block — §7.4.1.2 — so reading
    /// its value moves the origin alignment is counted from, and
    /// `finish_pid_read` puts it back. Carried in the header rather than
    /// left to the caller so that the save and the restore cannot be
    /// written a member apart and drift; C's `zdw_pid_read` carries it for
    /// the same reason.
    std::size_t saved_origin = 0;
};

/// Whether a parameter can be a member of the reading type.
///
/// The free-function form of `PidRead::is_user_member`, which is what a
/// generated decoder calls: the shared codec walk describes it as an
/// operation on the runtime with no receiver, and C exports
/// `zdw_pid_is_user_member` the same way.
bool pid_is_user_member(const PidRead &header) noexcept;

// -- text -------------------------------------------------------------------

namespace detail {

/// The Unicode scalar values of `text`, which must be well-formed UTF-8.
///
/// Written out rather than taken from a library: §7.4.1.1.2's byte count
/// depends on the surrogate rule, so a runtime that borrowed the decode
/// would not be reimplementing the clause.
///
/// A surrogate encoded in UTF-8 — CESU-8, which several toolchains emit —
/// is refused with the same `OutOfRange` the Python column raises for a
/// lone surrogate in a `str`: neither is a code point, and writing one
/// would put a sequence on the wire that is not UTF-16 at all.
std::vector<char32_t> utf8_code_points(std::string_view text);

/// The UTF-16 code units of those scalar values, without a BOM.
///
/// §7.4.1.1.2: "The serialized length of an object of type String<Char16>
/// shall be the number of bytes in the CDR buffer taken by the String<Char16>
/// characters. This is twice the number of characters in the string" — with
/// the clause's own rationale being that a code point outside the Basic
/// Multilingual Plane takes two units and a byte count still reaches the end
/// of the string. So `"𝄞"` is a length of four.
std::vector<char16_t> utf16_units(const std::vector<char32_t> &points);

/// Those scalar values back as UTF-8.
std::string utf8_of(const std::vector<char32_t> &points);

}  // namespace detail

/// The BCD nibbles of `text` under `fixed<digits, scale>`. [CDR] §9.3.2.8.
///
/// Digit nibbles most-significant first, then a sign nibble — `0xc` for
/// positive or zero, `0xd` for negative — with a leading zero nibble
/// prepended when the nibble count is odd so the nibbles pair up.
///
/// Neither half is padded in the direction that would lose a digit: a value
/// too long for its integer part or its fraction is one this type cannot
/// represent, and is refused.
std::vector<std::uint8_t> fixed_nibbles(std::string_view text, unsigned digits,
                                        unsigned scale,
                                        FixedDigitCount count = FixedDigitCount::Declared);

// -- the writer -------------------------------------------------------------

/// Accumulates the bytes of one sample.
///
/// Every method either writes what was asked for or throws. There is no
/// sticky-failure status and no half-written value: the buffer is a
/// `std::vector` and grows, so the only failures left are the ones about
/// the value rather than about the room for it.
class Writer {
  public:
    /// A writer in `endianness` whose alignment is capped at `max_align`.
    ///
    /// Four for XCDR2, eight for XCDR1. §7.4.1: `ALIGN(N)` aligns to
    /// `MIN(N, XCDR.maxalign)`. A writer that did not know this padded an
    /// eight-byte primitive to eight under XCDR2 and put four bytes on the
    /// wire that no peer expects.
    explicit Writer(Endianness endianness, std::size_t max_align = 8,
                    PlCdr1Framing framing = {}, bool short_length_codes = false);

    /// A big-endian writer, which is what a KeyHash holder is.
    static Writer big_endian() { return Writer(Endianness::Big); }
    static Writer little_endian() { return Writer(Endianness::Little); }

    Endianness endianness() const noexcept { return endianness_; }
    std::size_t max_align() const noexcept { return max_align_; }
    const PlCdr1Framing &pl_cdr1() const noexcept { return pl_cdr1_; }
    void set_pl_cdr1(PlCdr1Framing framing) noexcept { pl_cdr1_ = framing; }

    /// Whether an LC4 header whose member turns out to be 1, 2, 4 or 8
    /// bytes is rewritten with a short length code. Set it from
    /// `LengthCodePolicy::short_code_where_it_fits`; the layout plan already
    /// answers for every member whose type fixes the size, and this is the
    /// other half.
    bool short_length_codes() const noexcept { return short_length_codes_; }
    void set_short_length_codes(bool shorten) noexcept {
        short_length_codes_ = shorten;
    }

    /// How many bytes have been written.
    std::size_t position() const noexcept { return out_.size(); }

    /// The bytes written so far.
    const std::vector<std::uint8_t> &bytes() const noexcept { return out_; }
    /// The bytes, as a copy the caller owns.
    std::vector<std::uint8_t> finish() const { return out_; }

    /// Pad to `align`, counted from the stream's origin and capped at
    /// MAXALIGN.
    void align_to(std::size_t align);

    /// Move the alignment origin to the current position.
    ///
    /// `ElementPadding::Packed` is HDDS's measured departure from §7.4.1,
    /// which counts every element's alignment from the encapsulation: each
    /// collection element and each aggregate value counts its own alignment
    /// from its own first octet instead. `array-1d-struct-final` is
    /// `01000000 0200 03000000 0400 77`, the second `Cell`'s `long` at
    /// offset six where the clause puts it at eight, and `typedef-of-struct`
    /// is the same inside two `@final` members.
    ///
    /// Returns the origin displaced, for `finish_packed_element`.
    std::size_t begin_packed_element();
    /// Restore the origin `begin_packed_element` displaced.
    void finish_packed_element(std::size_t saved);

    // -- primitives ---------------------------------------------------------

    void write_bool(bool value);
    /// `octet` and `uint8`. The argument is the widest natural type and the
    /// value is range-checked, so a caller that hands this `-1` gets a
    /// refusal and not `0xff`.
    void write_u8(std::uint64_t value);
    void write_i8(std::int64_t value);
    /// `count` octets from `data`, as one run: what the elements of a
    /// `sequence<octet>` or an `octet[N]` are on the wire, with no
    /// alignment and no byte order to apply (XTypes 1.3 §7.4.3.4.1).
    /// Generated code calls it in place of a `write_u8` per element.
    void write_octets(const std::uint8_t *data, std::size_t count);
    /// The same for the elements of a `sequence<octet>` or an `octet[N]`: an `Octet` is one
    /// octet, so a run of them is a run of octets.
    template <typename T, typename = typename std::enable_if<std::is_same<T, Octet>::value>::type>
    void write_octets(const T *data, std::size_t count) {
        write_octets(reinterpret_cast<const std::uint8_t *>(data), count);
    }
    /// `char`, one byte. A code point above 255 is refused rather than
    /// truncated: §7.2.2.2.1.2.2 says a `CHAR_8_TYPE` "shall be representable
    /// using 8 bits", and `0xe4` is a different character from `U+00E4` read
    /// as one byte only by accident.
    void write_char(char32_t value);
    /// `wchar`, at `width` octets and in the shape `framing` does.
    ///
    /// The BMP restriction of §7.2.2.2.1.2.5 is a property of the type and
    /// not of the width: the four-byte form zero-extends a code unit, it
    /// does not carry a code point.
    void write_wchar(char32_t value, WcharWidth width = WcharWidth::Two,
                     WcharFraming framing = WcharFraming::Fixed,
                     WstringUnitOrder order = WstringUnitOrder::Stream);

    void write_i16(std::int64_t value);
    void write_u16(std::uint64_t value);
    void write_i32(std::int64_t value);
    void write_u32(std::uint64_t value);
    void write_i64(std::int64_t value);
    void write_u64(std::uint64_t value);
    /// `float`, rounded to single precision once, here.
    void write_f32(double value);
    void write_f64(double value);
    /// `long double`: sixteen bytes of IEEE 754 binary128.
    ///
    /// [CDR] §9.3.1.3 with Figure 9.2 gives the layout and shows the two
    /// byte orders as the whole sixteen bytes reversed, not two eight-byte
    /// halves swapped in place. Table 31 gives the alignment: eight, capped
    /// to four by MAXALIGN under XCDR2, which `align_to` already does.
    void write_f128(const Float128 &value);

    /// The Cyclone union overlay: the low `octets` of `pattern`, written
    /// as one unsigned integer of that width in the stream's byte order.
    ///
    /// A discriminator that carries a label the `union-labels` dialect
    /// axis says selects nothing makes the peer fall through to
    /// `default:` and serialise its one C union storage *as that arm* —
    /// the selected member's octets, reinterpreted. `pattern` is the
    /// `u64` whose little-endian bytes are the selected member's
    /// little-endian storage octets, zero-extended (`bit_pattern_f32`
    /// and `bit_pattern_f64` below produce it for the two float forms;
    /// an integer form is a masked cast at the call site), and `octets`
    /// is the width of the arm the fall-through selected.
    void write_union_overlay(std::uint64_t pattern, unsigned octets);

    /// A 16-bit value little-endian whatever the stream's order is. Two
    /// callers: the code units of the `HostUnswapped` order, and the
    /// `ByteOrderMark` one. Aligned to two like any other 16-bit value.
    void write_u16_little(std::uint64_t value);
    /// The other side of the same axis: what omniORB and JacORB decode a
    /// `wstring`'s code units as, whatever the stream says.
    void write_u16_big(std::uint64_t value);
    /// The same pair at 32 bits, for a four-octet transmission code set
    /// crossed with a fixed unit order.
    void write_u32_little(std::uint64_t value);
    void write_u32_big(std::uint64_t value);

    // -- strings ------------------------------------------------------------

    /// A string: a four-byte length **including the terminator**, the bytes,
    /// then a NUL. §7.4.1.1.
    ///
    /// The length counts the terminator, which is the one place XCDR differs
    /// from every other length-prefixed encoding, and it counts *octets* of
    /// UTF-8, not characters.
    void write_string(std::string_view value);

    /// A bounded string. The same bytes as an unbounded one: the bound is a
    /// promise about the source, not a difference on the wire — so this is
    /// the only place it can be enforced.
    ///
    /// Throws `OutOfRange` for a value longer than `bound`, refused rather
    /// than cut at a character boundary: §7.2.2.7's TRIM is a *read*-side
    /// behaviour that belongs to the receiver's own bound, and a writer that
    /// cut the value would put a sample on the wire nobody wrote.
    void write_string_bounded(std::string_view value, std::size_t bound);

    /// A bounded `wstring`, as `write_string_bounded` is to `write_string`.
    ///
    /// Throws `OutOfRange` for a value holding more than `bound` UTF-16 code
    /// units — not characters and not octets, whatever `policy` puts on the
    /// wire.
    void write_wstring_bounded(std::string_view utf8, std::size_t bound,
                       WstringLengthPolicy policy = WstringLengthPolicy::Specification,
                       WstringUnitOrder order = WstringUnitOrder::Stream);

    /// The count check every bounded `sequence` makes before a byte of it is
    /// written. Table 17 makes `O2.length <= T1.length` the condition for
    /// constructing a `sequence<T, N>`; throws `OutOfRange` for more than
    /// `bound` elements.
    void check_sequence_bound(std::size_t actual, std::size_t bound);

    /// The count check every bounded `map` makes before a byte of it is
    /// written. Its own message and not `check_sequence_bound`'s: the unit
    /// is an entry and not an element, per §7.4.1.1.4's map-as-a-sequence-of-
    /// pairs equivalence. Throws `OutOfRange` for more than `bound` entries.
    void check_map_bound(std::size_t actual, std::size_t bound);

    /// A `wstring`, in the form `policy` and `order` name.
    ///
    /// The value arrives as **UTF-8** and is converted to UTF-16 code units
    /// here, which is what `zdw_write_wstring_utf8` takes and what generated
    /// code has to hand: an IDL `wstring` has no one C++ representation
    /// (`std::wstring` is 16 bits on Windows and 32 elsewhere), and the
    /// surrogate rule §7.4.1.1.2's byte count depends on belongs to this
    /// runtime rather than to its caller.
    ///
    /// `Specification` is a four-byte byte count and then UTF-16 code units.
    /// No BOM and no terminator — §7.4.1.1.2 forbids both outright: "The
    /// UTF-16 representation ... shall not include a Byte Order Mark (BOM).
    /// The representation shall also not include any terminating NUL
    /// character(s)."
    void write_wstring_utf8(std::string_view utf8,
                       WstringLengthPolicy policy = WstringLengthPolicy::Specification,
                       WstringUnitOrder order = WstringUnitOrder::Stream);

    /// A `wstring` whose characters go out as their own UTF-8 octets.
    ///
    /// `WstringCodeUnits::Utf8` is the measured departure of one product's
    /// generated serialiser: XTypes 1.3 Table 31 gives a `wstring`'s code
    /// unit as `Char16` and §7.4.1.1.2 makes the length "twice the number of
    /// characters", so the clause has no UTF-8 form at all. The declared
    /// length is the UTF-8 octet count and only C2 — the terminator — still
    /// reaches the wire. `wstring-member` is `04 00 00 00 61 62 63 00` for
    /// `"abc"`: three octets, a NUL, and the four declares both.
    ///
    /// The value is checked as UTF-8, with the same refusal
    /// `write_wstring_utf8` makes; the order is inert and is not a parameter.
    void write_wstring_utf8_octets(
        std::string_view utf8,
        WstringLengthPolicy policy = WstringLengthPolicy::Specification);

    /// A bounded UTF-8-octet `wstring`, as `write_wstring_bounded` is to
    /// `write_wstring_utf8`: the bound still counts UTF-16 code units, which
    /// is what a `wstring<N>` bound means, while the wire carries the UTF-8
    /// octets.
    void write_wstring_utf8_octets_bounded(
        std::string_view utf8, std::size_t bound,
        WstringLengthPolicy policy = WstringLengthPolicy::Specification);

    /// `fixed<digits, scale>` as packed BCD. [CDR] §9.3.2.8.
    ///
    /// XTypes 1.3 does not carry the type at all — §7.2.2 has no
    /// fixed-point kind and §7.4.3.5.3 has no rule whose left-hand side
    /// matches one — so the form is CORBA's, which is where an IDL `fixed`
    /// came from and the only wire definition it has.
    ///
    /// The value arrives as a decimal string because a JSON number is a
    /// double and `fixed<31, 2>` is exactly the case a double cannot carry.
    void write_fixed(std::string_view value, unsigned digits, unsigned scale,
                     FixedDigitCount count = FixedDigitCount::Declared);

    // -- the bit-bounded kinds ----------------------------------------------

    /// An enumerator, as the integer its bit bound calls for.
    ///
    /// §7.4.3.5.3 rule (5) is `XCDR << {O : ENUM_TYPE} =
    /// XCDR << {O.value : O.holder_type}`, and the ENUM_TYPE row says
    /// "O.holder_type is either Int8, Int16 or Int32 depending on the value
    /// of the @bit_bound annotation". A **signed** holder: `@value(-1)` is
    /// the two's complement of one, so at the default bound it goes out as
    /// `ffffffff`. What fits is everything expressible in the bound's bits
    /// in either signedness — `-2^(n-1) ..= 2^n - 1`. §7.3.1.2.1.5 caps the
    /// bound itself at 1 to 32.
    void write_enum(unsigned bit_bound, std::int64_t value);

    /// A bitmask, as the unsigned integer its bit bound calls for.
    ///
    /// Rule (6) writes `O.value` as `O.holder_type`, and Table 33 of
    /// §7.4.1.1.3.2 gives the holder: `UInt8` to 8, `UInt16` to 16,
    /// `UInt32` to 32, `UInt64` to 64. The same clause fixes the direction —
    /// "Bit indexes are counted from zero starting at the least-significant
    /// bit of the full byte size of the bitmask" — so bit `n` is `1 << n`
    /// whatever the holder's width.
    void write_bitmask(unsigned bit_bound, std::uint64_t value);

    /// A bitset, as the anchor integer its bitfields pack into.
    ///
    /// XTypes 1.3 has no serialisation rule for a bitset — §7.4.3.5.3 runs
    /// from rule (1) to rule (30) and no left-hand side matches one. How
    /// wide the anchor is comes from the layout planner and arrives here as
    /// a byte count.
    void write_bitset(unsigned anchor_bytes, std::uint64_t value);

    /// One bitfield's contribution to an anchor, range-checked.
    ///
    /// A value wider than its field would overwrite the field above it, and
    /// masking it would put a different number on the wire than the caller
    /// asked for.
    std::uint64_t bitfield(std::uint64_t value, unsigned width,
                           unsigned position) const;

    // -- framing ------------------------------------------------------------

    /// A DHEADER: a four-byte length the members that follow occupy.
    ///
    /// Written as a placeholder and patched when the members are done,
    /// because the length is not known until then.
    DHeader write_dheader();
    /// A DHEADER that is not written at all.
    static DHeader no_dheader() { return DHeader{}; }
    void finish_dheader(DHeader header);

    /// An XCDR2 EMHEADER: the member id, its length code and the
    /// must-understand flag in one 32-bit word. §7.4.3.4.2.
    ///
    /// The length code is the layout plan's decision and arrives here as a
    /// number; this runtime does not choose one.
    ///
    /// LC0 through LC3 carry the length in the code itself. LC4 is followed
    /// by a NEXTINT holding the member's length. LC5 through LC7 also have a
    /// NEXTINT, but §7.4.3.4.2 says it "shall be reused also as part of the
    /// serialized member ... because the serialization of certain members
    /// also starts with an integer length, which would take exactly the same
    /// value as NEXTINT" — so **no placeholder is reserved** for them: the
    /// member's own leading word lands in that position and *is* the
    /// NEXTINT. Reserving one would write the length twice.
    ///
    /// A member id above `E_MEMBER_ID_MASK` throws `OutOfRange`:
    /// §7.2.2.4.4.4.4 runs the ids from 0 to 0x0FFFFFFF and the word holds
    /// exactly those bits, so masking a larger one would write a well-formed
    /// header naming a member the caller did not.
    EmHeader write_emheader(std::uint32_t member_id, unsigned length_code,
                            bool must_understand);
    /// Patch an EMHEADER's NEXTINT with the length of what followed.
    ///
    /// Under `short_length_codes` an LC4 header whose member turned out to
    /// be 1, 2, 4 or 8 bytes long instead loses its NEXTINT and states the
    /// length in the code, which is what Fast DDS does.
    void finish_emheader(EmHeader header);

    /// A PL_CDR1 parameter header, short form: `[id:u16][length:u16]`.
    ///
    /// `must_understand` is bit 14 of the id. §7.4.1.2.2: the flag "shall be
    /// set if and only if the must_understand property of the member is set
    /// to true".
    ///
    /// An id above `PlCdr1Framing::highest_short_id` throws `OutOfRange`:
    /// §7.4.1.2.2 reserves the range above it for the protocol's own
    /// parameters, and PID_EXTENDED is 0x3F01 itself.
    Pid write_pid(std::uint16_t pid, ParameterRole role = ParameterRole::Member,
                  bool must_understand = false);
    /// A PL_CDR1 parameter header, extended form:
    /// `[PID_EXTENDED][8][id:u32][length:u32]`, required when the id reaches
    /// the reserved range or the value is longer than `0xFFFF`, and used
    /// throughout under Cyclone DDS's framing. §7.4.1.2.
    ///
    /// A member id above `E_MEMBER_ID_MASK` throws `OutOfRange`, for the
    /// reason `write_emheader` does: the `eMemberHeader` of §7.4.1.2.1 has
    /// the same 28 bits.
    Pid write_pid_extended(std::uint32_t member_id,
                           ParameterRole role = ParameterRole::Member,
                           bool must_understand = false);
    /// Patch a parameter header with its value's length.
    ///
    /// The declared length covers the value's trailing padding or stops at
    /// the value, depending on the framing. The padding itself starts the
    /// next parameter at a four-byte offset measured from the enclosing
    /// stream, so the origin goes back before it is written. §7.4.1.2.
    void finish_pid(Pid header);

    /// The `PID_LIST_END` sentinel a PL_CDR1 parameter list ends with.
    ///
    /// A reader that stops at the last member it recognises instead of at
    /// the sentinel misreads whatever follows.
    void write_sentinel();

    // -- the KeyHash --------------------------------------------------------

    /// The holder as a KeyHash, zero-padded to sixteen bytes, for a holder
    /// of sixteen bytes or fewer. DDSI-RTPS 2.5 §9.6.3.8.
    std::array<std::uint8_t, KEY_HASH_LEN> zero_padded() const;
    /// The MD5 of the holder, for one longer than sixteen bytes.
    std::array<std::uint8_t, KEY_HASH_LEN> md5() const;

  private:
    void check(std::int64_t value, std::int64_t low, std::int64_t high,
               const char *expected) const;
    void check_unsigned(std::uint64_t value, std::uint64_t high,
                        const char *expected) const;
    void write_int(std::uint64_t unsigned_value, std::size_t width);
    void write_raw(std::uint64_t unsigned_value, std::size_t width);
    void patch(std::size_t at, std::uint64_t value, std::size_t width);
    std::size_t begin_parameter_value();
    void shorten_emheader(EmHeader header, std::size_t length);
    void promote_pid(Pid header, std::size_t length);
    void write_wstring_unit(std::uint16_t unit, std::size_t width,
                            WstringUnitOrder order);

    std::vector<std::uint8_t> out_;
    Endianness endianness_;
    /// Where alignment is counted from. Zero for a sample; a PL_CDR1
    /// parameter value moves it, because a parameter is its own encapsulated
    /// block. §7.4.1.2.
    std::size_t origin_ = 0;
    std::size_t max_align_;
    PlCdr1Framing pl_cdr1_;
    bool short_length_codes_;
};

// -- the reader -------------------------------------------------------------

/// Reads one sample's bytes.
///
/// The mirror of the writer, clause for clause. Every read that cannot be
/// satisfied throws a `WireError` naming what was expected and where: a
/// reader that returned a zero for a truncated stream would hand the caller
/// a value that was never sent.
///
/// The reader borrows its bytes and does not own them.
class Reader {
  public:
    /// A reader over `[bytes, bytes + length)`, capped at `max_align`.
    ///
    /// Four for XCDR2, eight for XCDR1 — the writer's counterpart. A reader
    /// that skipped padding the writer never wrote reads the wrong bytes
    /// from the second member on.
    ///
    /// Two fields of `framing` matter on this side. `pid_flags`, because
    /// under `ImplExtensionMembers` every member id carries
    /// FLAG_IMPL_EXTENSION and the flag no longer tells a vendor-private
    /// parameter from a user member; and `highest_short_id`, because a
    /// reader holding §7.4.1.2.2's threshold as a constant reports
    /// `@id(16383)` as reserved and drops the member.
    Reader(const std::uint8_t *bytes, std::size_t length, Endianness endianness,
           std::size_t max_align = 8, PlCdr1Framing framing = {});
    Reader(const std::vector<std::uint8_t> &bytes, Endianness endianness,
           std::size_t max_align = 8, PlCdr1Framing framing = {});

    Endianness endianness() const noexcept { return endianness_; }
    std::size_t max_align() const noexcept { return max_align_; }
    const PlCdr1Framing &pl_cdr1() const noexcept { return pl_cdr1_; }
    void set_pl_cdr1(PlCdr1Framing framing) noexcept { pl_cdr1_ = framing; }

    /// Where it is.
    std::size_t position() const noexcept { return at_; }
    /// How many bytes are left.
    std::size_t remaining() const noexcept {
        return at_ >= length_ ? std::size_t{0} : length_ - at_;
    }

    /// Skip padding to `align`, counted from the stream's origin.
    void align_to(std::size_t align);
    /// Step over `count` bytes, refusing to step past the end.
    void skip(std::size_t count);
    /// Move to an absolute offset inside the buffer.
    void seek(std::size_t at);

    // -- what a generated decoder says and asks ------------------------------
    //
    // The names are the shared codec walk's, and they are spelled here
    // rather than translated in the backend for the reason this runtime
    // spells `write_u32` and `finish_dheader` as it does: a column whose
    // generated source names one thing and whose runtime names another has
    // a mapping table between them, and a mapping table is where two
    // operations quietly become one.

    /// The first refusal, or `Status::Ok`.
    Status reader_status() const noexcept { return status_; }
    /// Where that refusal was. Meaningless while the status is `Ok`.
    std::size_t reader_error_offset() const noexcept { return error_at_; }

    /// Refuse the stream from outside the runtime.
    ///
    /// The runtime owns the wire and the generated code owns the type, and
    /// some refusals are the type's — see `Status`. The first refusal
    /// sticks; `Status::Ok` is not a refusal and does nothing.
    void reader_refuse(Status status) noexcept;

    /// Take back a `Status::Discarded`, and only that one.
    ///
    /// A member whose own `@try_construct` is USE_DEFAULT but which also
    /// carries an explicit `@default` cannot be answered inside the type's
    /// decoder: the decoder knows Table 9's value for the type and not the
    /// literal the member's declaration gave it, and §7.2.2.4.4.4.9 puts
    /// the literal first. So the call site asks for DISCARD, and where it
    /// gets one it clears it and assigns the literal itself. The position
    /// is already right — every narrowing reads the whole value off the
    /// wire before it decides.
    ///
    /// Every other status stays: a truncated stream is not a narrowing.
    /// Returns whether there was one.
    bool reader_clear_discard() noexcept;

    /// `position`, under the name the codec walk asks for.
    std::size_t reader_position() const noexcept { return position(); }
    /// `remaining`, under the name the codec walk asks for.
    std::size_t reader_remaining() const noexcept { return remaining(); }

    /// Move the alignment origin to the current position.
    ///
    /// The read-side pair of the writer's `begin_packed_element`; see it for
    /// the axis and the recorded rows. Returns the origin displaced.
    std::size_t reader_begin_packed_element();
    /// Restore the origin `reader_begin_packed_element` displaced.
    void reader_finish_packed_element(std::size_t saved);

    /// Everything a two-pass read has to put back. See `ReaderMark`.
    ReaderMark reader_save() const noexcept;
    void reader_restore(ReaderMark mark) noexcept;

    /// Move the cursor forward to `target`, which is never backwards.
    ///
    /// Absolute, and deliberately not `skip`, which is relative: a framing
    /// word pointing behind the cursor means the reader has already read
    /// past the end of what it framed, and seeking back would paper over
    /// it. C's `zdw_reader_skip_to` is the same function under the same
    /// rule; `seek` is the unconditional one and is not it.
    void reader_skip_to(std::size_t target);

    /// As many characters of `text` as fit in `octets` UTF-8 bytes.
    ///
    /// Whole characters: stopping mid-sequence would leave bytes that are
    /// not UTF-8. §7.3.1.2.1.12.3's TRIM, for a narrow string.
    ///
    /// Takes no state and could be a free function; it is a method because
    /// C's is `zdw_trim_utf8(r, text, octets)` — its trim allocates out of
    /// the reader's heap — and the walk hands the receiver either way.
    std::string trim_utf8(const std::string &text, std::size_t octets) const;
    /// The same, counting UTF-16 code units, for a `wstring`.
    std::string trim_utf16(const std::string &text, std::size_t units) const;

    // -- primitives ---------------------------------------------------------

    /// `boolean`. A byte that is neither 0 nor 1 is a stream that disagrees
    /// with its own type, and is refused rather than coerced.
    bool read_bool();
    std::uint8_t read_u8();
    std::int8_t read_i8();
    /// `count` octets into `out`, as one run; the read side of
    /// `Writer::write_octets`. Truncated when fewer are left, as `read_u8`.
    void read_octets(std::uint8_t *out, std::size_t count);
    /// The same into the elements of a `sequence<octet>` or an `octet[N]`.
    template <typename T, typename = typename std::enable_if<std::is_same<T, Octet>::value>::type>
    void read_octets(T *out, std::size_t count) {
        read_octets(reinterpret_cast<std::uint8_t *>(out), count);
    }
    /// `char`, one byte, as the code point it spells.
    char32_t read_char();
    /// `wchar`, at `width` octets and in `framing`'s shape.
    ///
    /// The four-byte form is RTI Connext's under XCDR1, where the high
    /// sixteen bits are padding around one UTF-16 code unit. A value that
    /// uses them is refused rather than narrowed.
    ///
    /// Under `GiopLengthPrefixed` the octet count is checked against the
    /// configured code-set width rather than believed: a stream whose first
    /// octet says 0xE4 — which is what a bare two-octet `wchar` looks like
    /// read this way — would otherwise consume 228 octets of whatever
    /// followed.
    char32_t read_wchar(WcharWidth width = WcharWidth::Two,
                        WcharFraming framing = WcharFraming::Fixed,
                        WstringUnitOrder order = WstringUnitOrder::Stream);

    std::int16_t read_i16();
    std::uint16_t read_u16();
    std::int32_t read_i32();
    std::uint32_t read_u32();
    std::int64_t read_i64();
    std::uint64_t read_u64();
    float read_f32();
    double read_f64();
    /// `long double`: all sixteen bytes, as IEEE 754 binary128.
    Float128 read_f128();

    std::uint16_t read_u16_little();
    std::uint16_t read_u16_big();
    std::uint32_t read_u32_little();
    std::uint32_t read_u32_big();

    // -- strings ------------------------------------------------------------

    /// A string: a length including the terminator, the bytes, the NUL.
    std::string read_string();
    /// `read_string`, under the name the codec walk asks for.
    ///
    /// C's is `zdw_read_string_alloc` because C has a second one that
    /// fills a caller's buffer, and the generated decoder wants the
    /// allocating half. Here there is only one and it owns its storage —
    /// the `_alloc` says nothing — but the walk names the operation and
    /// this runtime answers to the name, rather than the backend keeping a
    /// table of two spellings for one thing.
    std::string read_string_alloc() { return read_string(); }
    /// A `wstring`, in the form `policy` and `order` name, as UTF-8.
    ///
    /// Under `Specification` the count is bytes and not characters —
    /// §7.4.1.1.2 — so a count the code-unit width does not divide is a
    /// stream no encoder produced and is refused rather than rounded down:
    /// rounding would leave the remainder in the stream and put every later
    /// member at the wrong offset.
    ///
    /// The form is never inferred from the stream. The three are not
    /// separable: a Fast DDS `"ab"` followed by a `long` is byte-identical
    /// to a specification `"a"` followed by the same `long`, differing only
    /// in padding no clause fixes. Choosing between them would mean letting
    /// the length field — the one field a hostile peer controls — decide how
    /// much is read.
    std::string read_wstring(WstringLengthPolicy policy = WstringLengthPolicy::Specification,
                             WstringUnitOrder order = WstringUnitOrder::Stream);
    /// `read_wstring`, under the name the codec walk asks for — and the
    /// name this class's own writer half already uses.
    ///
    /// The asymmetry was real and is recorded: commit 75b447271 renamed
    /// the writer to `write_wstring_utf8` to match C, calling it "the
    /// third name to differ for one operation, after write_bitmask's
    /// argument order and Float128's parser", and did not rename the
    /// reader. So this column wrote `write_wstring_utf8` and read
    /// `read_wstring`, and a generator deriving the second from the first
    /// would have emitted a call no header declares.
    std::string read_wstring_utf8(
        WstringLengthPolicy policy = WstringLengthPolicy::Specification,
        WstringUnitOrder order = WstringUnitOrder::Stream) {
        return read_wstring(policy, order);
    }

    /// The inverse of `Writer::write_wstring_utf8_octets`.
    ///
    /// `WstringCodeUnits::Utf8`: the declared length is the UTF-8 octet
    /// count, and the terminator `policy` asks for is the last of those
    /// octets. A count the stream cannot hold, a missing or an interior NUL,
    /// or an ill-formed UTF-8 sequence is refused.
    std::string read_wstring_utf8_octets(
        WstringLengthPolicy policy = WstringLengthPolicy::Specification);
    /// `fixed<digits, scale>`, as the canonical decimal string the writer
    /// took: one leading zero at most before the point, exactly `scale`
    /// digits after it, and a `-` only on a non-zero negative.
    std::string read_fixed(unsigned digits, unsigned scale,
                           FixedDigitCount count = FixedDigitCount::Declared);

    // -- the bit-bounded kinds ----------------------------------------------

    /// An enumerator's holder, sign-extended from its bound.
    ///
    /// §7.4.3.5.3 rule (5) makes the holder signed, so a pattern with the
    /// top bit of the bound set reads back negative: `@bit_bound(8)` with
    /// the byte `c8` is −56.
    ///
    /// Nothing is lost by that. `@value(-1)` and `@value(255)` under a
    /// bound of eight are the same byte and nothing in the stream separates
    /// them; which one was meant is a question about the *type*, and the
    /// generated decoder answers it by comparing this against each declared
    /// enumerator sign-extended the same way. Sign extension is a bijection
    /// on the bound's width, so the comparison is the same comparison it
    /// would be on the raw pattern — it is which side does the extension
    /// that has to be agreed, and the agreement is with `zdw_read_enum`,
    /// which this now matches exactly.
    ///
    /// It did not, and that was a contradiction between two runtimes about
    /// one operation the shared codec walk names: the walk declares its
    /// local `int64_t` and compares against `INT64_C(-56)`, so a runtime
    /// answering `200` would have matched no enumerator and every
    /// `@bit_bound(8)` value above 127 would have come back as the
    /// enumeration's default.
    std::int64_t read_enum(unsigned bit_bound);
    std::uint64_t read_bitmask(unsigned bit_bound);
    std::uint64_t read_bitset(unsigned anchor_bytes);

    // -- framing ------------------------------------------------------------

    /// A sequence's element count, refusing one the stream cannot hold.
    ///
    /// A count is one element or more, so a stream with fewer bytes left
    /// than elements claimed cannot be honest about it. Allocating for it is
    /// how a reader turns a bad message into an out-of-memory.
    std::uint32_t read_sequence_length();
    /// A DHEADER: the length the members that follow occupy.
    std::uint32_t read_dheader();
    /// An EMHEADER, unpacked into its parts and the NEXTINT it carries.
    EmHeaderRead read_emheader();
    /// A PL_CDR1 parameter header, in whichever form it was written.
    ///
    /// `role` is the caller's, not the stream's: the bytes are the same
    /// bytes and only what the declared length means differs. §7.4.1.2.
    PidRead read_pid(ParameterRole role = ParameterRole::Member);
    /// `read_pid`, under the name the codec walk asks for when it names a
    /// role. C has two functions because C has no default arguments.
    PidRead read_pid_as(ParameterRole role) { return read_pid(role); }
    /// Step over a member this reader does not know. §7.4.3.4.2.
    void skip_member(const EmHeaderRead &header);

    /// Start reading a parameter's value, returning the origin it replaces.
    std::size_t begin_parameter_value();
    /// End a parameter's value and step over its trailing padding.
    void finish_parameter_value(std::size_t saved_origin);

    /// End a parameter, checking its declared length and putting the
    /// origin back.
    ///
    /// `finish_parameter_value` does only the second half. The first is
    /// the check C's `zdw_finish_pid_read` makes and this column did not:
    /// a declared length that is well formed but belongs to a different
    /// implementation's framing — RTI rounds a member's length up to the
    /// alignment where Fast DDS writes the exact one, and
    /// `vendor_bytes.md` §2 records both for the same `short`. Reported as
    /// `Status::WrongFraming` rather than thrown, because the caller can
    /// name the setting the bytes do fit.
    void finish_pid_read(const PidRead &header);

  private:
    const std::uint8_t *take(std::size_t count);
    std::uint64_t read_bits(std::size_t width);
    std::vector<std::uint16_t> wstring_units(std::size_t at, std::size_t count,
                                             std::size_t width,
                                             WstringUnitOrder order);

    const std::uint8_t *bytes_;
    std::size_t length_;
    std::size_t at_ = 0;
    Endianness endianness_;
    std::size_t max_align_;
    std::size_t origin_ = 0;
    PlCdr1Framing pl_cdr1_;
    /// The refusal channel. See `Status`.
    Status status_ = Status::Ok;
    std::size_t error_at_ = 0;
};

}  // namespace zerodds::wire

// The map class of XTypes 1.3 §7.5.1.3.3, which a generated header names beside the types of the
// wire. It includes `dds/types.hpp`, which includes this file: the guard above is what ends that.
#include "zerodds/map.hpp"

#endif  // ZERODDS_WIRE_HPP

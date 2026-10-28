// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// The policy values a generated codec names.
//
// The Go counterpart of the policy surface of
// runtime/python/zerodds_wire — the same clauses, implemented again
// rather than bound. Every wstring policy and unit order is spelled
// exactly as WstringLengthPolicy::as_str and WstringUnitOrder::as_str in
// crates/ast-layout/src/key.rs spell them, because the shared codec walk
// emits one constant into every column and a column that renamed a value
// would be refused by name at the first sample.

package zdwire

// Endianness is the byte order a stream is written or read in.
type Endianness int

const (
	// LittleEndian is the little-endian byte order.
	LittleEndian Endianness = iota
	// BigEndian is the big-endian byte order.
	BigEndian
)

// IsLittle answers whether this is the little-endian order.
func (e Endianness) IsLittle() bool { return e == LittleEndian }

// TryConstruct is what a receiving position does with a value it cannot
// hold. DDS-XTypes 1.3 §7.2.2.7, Table 11. Passed to every generated
// decoder, because the behaviour belongs to the position — this member,
// this element, this map key, this discriminator — and not to the type.
type TryConstruct int

const (
	// TryConstructDiscard propagates the failure; the sample is
	// discarded. The default where no annotation was written.
	TryConstructDiscard TryConstruct = iota
	// TryConstructUseDefault gives the member Table 9's value.
	TryConstructUseDefault
	// TryConstructTrim cuts a string, wstring, sequence or map to the
	// bound, order preserved.
	TryConstructTrim
)

// Status is what a reader answers about the type, without panicking.
//
// Two kinds of failure, and they are not one. A malformed stream
// panics with a WireError: a length past the end of the buffer is not a
// state a caller can carry on from. A refusal about the type sets this
// and the generated decoder walks on to the end of the sample, which is
// what lets one test at the top of the caller decide what to do with it.
type Status int

const (
	// StatusOk: nothing has been refused.
	StatusOk Status = iota
	// StatusTruncated: the stream ended inside a value.
	StatusTruncated
	// StatusImpossibleLength: a length field states more than the
	// stream holds.
	StatusImpossibleLength
	// StatusOutOfRange: a value the reading type cannot hold.
	StatusOutOfRange
	// StatusDiscarded: §7.2.2.7's DISCARD. An absence, not an error.
	StatusDiscarded
)

// String answers the spelling the sibling runtimes use in their logs.
func (s Status) String() string {
	switch s {
	case StatusOk:
		return "ok"
	case StatusTruncated:
		return "truncated"
	case StatusImpossibleLength:
		return "impossible-length"
	case StatusOutOfRange:
		return "out-of-range"
	case StatusDiscarded:
		return "discarded"
	}
	return "unknown"
}

// ParameterRole is which of §7.4.1.2's two roles a parameter header
// stands in. The header is the same header; the role decides what the
// declared length counts.
type ParameterRole int

const (
	// ParameterMember is a member of a @mutable type. Rules (24), (25).
	ParameterMember ParameterRole = iota
	// ParameterOptional is an @optional member of a type that is not
	// @mutable. Rule (19).
	ParameterOptional
)

// The parameter ids and flags of XTypes 1.3 §7.4.1.2.2 and
// DDSI-RTPS 2.5 §9.4.2.11.2.
const (
	// PidExtended introduces the extended form.
	PidExtended = 0x3F01
	// PidListEnd ends a parameter list.
	PidListEnd = 0x3F02
	// PidShortMaxID is the highest member id a short header carries;
	// §7.4.1.2.2 gives the range as 0 to 0x3F00 inclusive.
	PidShortMaxID = 0x3F00
	// PidImplExtension is FLAG_IMPL_EXTENSION, bit 15 of a short id.
	PidImplExtension = 0x8000
	// PidIgnore is the id every consumer shall ignore.
	PidIgnore = 0x3F03
	// PidShortMaxLen is the longest value a short header can state.
	PidShortMaxLen = 0xFFFF
	// PidMustUnderstand is the must-understand bit of a short id.
	PidMustUnderstand = 0x4000

	// The extended eMemberHeader's flags and id mask. §7.4.1.2.1.
	eFlagMustUnderstand = 0x4000_0000
	eFlagImplExtension  = 0x8000_0000
	eMemberIDMask       = 0x0FFF_FFFF

	// pidIDMask is the low fourteen bits of a short parameter id.
	pidIDMask = 0x3FFF
)

// pidFlagsAt answers what flags a PL_CDR1 parameter id carries at
// `position` — "member", "sentinel" or "extended-marker". The policies
// and the measurement are on zerodds_ast_layout::PidFlagPolicy; an
// unknown policy panics rather than falling back, because a typo in a
// wire-affecting setting must be loud.
func pidFlagsAt(policy string, position string) int {
	switch policy {
	case "sentinel-must-understand":
		if position == "sentinel" || position == "extended-marker" {
			return PidMustUnderstand
		}
		return 0
	case "bare":
		return 0
	case "impl-extension-members":
		if position == "member" || position == "extended-marker" {
			return PidImplExtension | PidMustUnderstand
		}
		return 0
	}
	panic(OutOfRange{At: 0, Expected: "a PID-flag policy, not `" + policy + "`"})
}

// PlCdr1Framing is how a PL_CDR1 parameter list is framed — the three
// ways the measured implementations differ inside an XCDR1 sample. The
// zero value is deliberately not a valid default; a harness constructs
// one with every field written out.
type PlCdr1Framing struct {
	// PidFlags is what flags a parameter id carries, asked per
	// position. zerodds_ast_layout::PidFlagPolicy::as_str names them.
	PidFlags string
	// HighestShortID is the highest member id a short parameter header
	// carries. Both sides: WritePid refuses an id above it rather than
	// putting a reserved parameter on the wire.
	HighestShortID int
	// PadDeclaredLength says whether a @mutable member's declared
	// length covers its padding.
	PadDeclaredLength bool
	// PadDeclaredOptionalLength is the same question for an @optional
	// member of a type that is not @mutable.
	PadDeclaredOptionalLength bool
	// MemberMustUnderstand says whether a member's own must-understand
	// bit goes out at all.
	MemberMustUnderstand bool
	// PadAfterValue says whether the parameter that just ended writes
	// its own trailing padding to the next four-byte boundary. Every
	// header writer aligns itself, so the two readings only part at the
	// end of the stream: RTI pads after a final @optional parameter,
	// Fast DDS does not.
	PadAfterValue bool
}

// padsDeclaredLength answers whether a parameter in `role` declares a
// padded length.
func (f PlCdr1Framing) padsDeclaredLength(role ParameterRole) bool {
	if role == ParameterOptional {
		return f.PadDeclaredOptionalLength
	}
	return f.PadDeclaredLength
}

// SpecificationFraming is the clause's own framing —
// Dialect::Specification, this project's default dialect.
func SpecificationFraming() PlCdr1Framing {
	return PlCdr1Framing{
		PidFlags:                  "sentinel-must-understand",
		HighestShortID:            PidShortMaxID,
		PadDeclaredLength:         false,
		PadDeclaredOptionalLength: false,
		MemberMustUnderstand:      true,
		PadAfterValue:             true,
	}
}

// wstringCell is one cell of the C1 x C2 x C3 product: whether the
// length counts characters, whether a terminator is written, and the
// unit width in octets.
type wstringCell struct {
	countsCharacters bool
	terminated       bool
	width            int
}

// wstringCellOf resolves a policy name — the five named forms and the
// three coordinate spellings — to its cell. The second answer is false
// for a name that is no policy.
func wstringCellOf(policy string) (wstringCell, bool) {
	switch policy {
	case "specification", "octets/unterminated/2":
		return wstringCell{false, false, 2}, true
	case "character-count", "characters/unterminated/2":
		return wstringCell{true, false, 2}, true
	case "character-count-terminated", "characters/terminated/4":
		return wstringCell{true, true, 4}, true
	case "specification-wide-units", "octets/unterminated/4":
		return wstringCell{false, false, 4}, true
	case "ros-type-support", "characters/unterminated/4":
		return wstringCell{true, false, 4}, true
	case "octets/terminated/2":
		return wstringCell{false, true, 2}, true
	case "octets/terminated/4":
		return wstringCell{false, true, 4}, true
	case "characters/terminated/2":
		return wstringCell{true, true, 2}, true
	}
	return wstringCell{}, false
}

// wstringOrderKnown answers whether `order` names a unit order.
func wstringOrderKnown(order string) bool {
	switch order {
	case "stream", "big-endian-always", "byte-order-mark", "host-unswapped":
		return true
	}
	return false
}

// The UTF-16 byte-order mark, and its byte-swapped form.
const (
	byteOrderMark        = 0xFEFF
	swappedByteOrderMark = 0xFFFE
)

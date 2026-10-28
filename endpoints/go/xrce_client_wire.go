// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

package zerodds

import "fmt"

// Wire codec for the DDS-XRCE 1.0 client-session frames XrceClient's request
// ids pair with. Byte-for-byte the layout `crates/xrce` encodes
// (MessageHeader §8.3.2, SubmessageHeader §8.3.4, ObjectId §7.2.1,
// BaseObjectRequest §7.7.8, ObjectVariant §7.7, ReadSpecification §8.3.5.9) —
// NOT the general-purpose XCDR2 `Writer`/`Reader` in wire.go, which is a
// different, alignment-based layout the XRCE headers do not follow.

// ObjectKind is the 4-bit XRCE object kind (spec §7.2 Table 4).
type ObjectKind byte

// OBJK_* constants, spec §7.2 Table 4.
const (
	ObjkInvalid     ObjectKind = 0x00
	ObjkParticipant ObjectKind = 0x01
	ObjkTopic       ObjectKind = 0x02
	ObjkPublisher   ObjectKind = 0x03
	ObjkSubscriber  ObjectKind = 0x04
	ObjkDataWriter  ObjectKind = 0x05
	ObjkDataReader  ObjectKind = 0x06
	ObjkType        ObjectKind = 0x0A
	ObjkQosProfile  ObjectKind = 0x0B
	ObjkApplication ObjectKind = 0x0C
	ObjkAgent       ObjectKind = 0x0D
	ObjkClient      ObjectKind = 0x0E
	ObjkOther       ObjectKind = 0x0F
)

// ObjectID is the 16-bit XRCE object id: kind in the lower 4 bits, an
// application/agent-assigned raw id in the upper 12 (spec §7.2.1).
type ObjectID uint16

// ObjectIDInvalid is OBJECTID_INVALID, {0x00, 0x00} (DDS-XRCE 1.0 §7.6).
const ObjectIDInvalid ObjectID = 0x0000

// NewObjectID packs a 12-bit rawID and a 4-bit kind into an ObjectID.
func NewObjectID(rawID uint16, kind ObjectKind) (ObjectID, error) {
	if rawID > 0x0FFF {
		return 0, fmt.Errorf("zerodds: xrce object id: raw id %#x exceeds 12 bits", rawID)
	}
	return ObjectID((rawID << 4) | uint16(kind)), nil
}

// Kind is the 4-bit object kind from the lower bits.
func (id ObjectID) Kind() ObjectKind { return ObjectKind(id & 0x0F) }

// RawID12 is the 12-bit raw id from the upper bits.
func (id ObjectID) RawID12() uint16 { return (uint16(id) >> 4) & 0x0FFF }

// Bytes is the wire encoding: 2 octets, big-endian (crates/xrce
// `ObjectId::to_bytes`).
func (id ObjectID) Bytes() [2]byte { return [2]byte{byte(id >> 8), byte(id)} }

// ObjectIDFromBytes decodes a big-endian 2-octet ObjectID.
func ObjectIDFromBytes(b []byte) (ObjectID, error) {
	if len(b) < 2 {
		return 0, fmt.Errorf("zerodds: xrce object id: need 2 bytes, got %d", len(b))
	}
	return ObjectID(uint16(b[0])<<8 | uint16(b[1])), nil
}

// objectVariantKind is the ObjectVariant wire discriminator (spec §7.7.2).
type objectVariantKind byte

const (
	variantByReference objectVariantKind = 0x01
	variantByXMLString objectVariantKind = 0x02
	variantInBinary    objectVariantKind = 0x03
)

// ObjectVariant is the object-specific data a CREATE carries (spec §7.7).
// Build one with ByReference, ByXMLString or InBinary.
type ObjectVariant struct {
	kind objectVariantKind
	str  string
	bin  []byte
}

// ByReference builds a REPRESENTATION_BY_REFERENCE variant: a name pointing
// to a template registered in the agent.
func ByReference(name string) ObjectVariant {
	return ObjectVariant{kind: variantByReference, str: name}
}

// ByXMLString builds a REPRESENTATION_AS_XML_STRING variant: inline XML
// describing the object.
func ByXMLString(xml string) ObjectVariant {
	return ObjectVariant{kind: variantByXMLString, str: xml}
}

// InBinary builds a REPRESENTATION_IN_BINARY variant: inline XCDR2-encoded
// bytes of the strong-typed object representation.
func InBinary(b []byte) ObjectVariant {
	return ObjectVariant{kind: variantInBinary, bin: b}
}

// encodeInner writes the discriminator (1) + padLen padding + u32 length
// (little endian, matching the E-flag every builder below sets) + payload,
// with a trailing NUL for the two string variants (XCDR2 `string<>`,
// crates/xrce/src/object_repr.rs). padLen is 3 for the never-used-on-the-
// wire single-level form (this structure alone starts 4-aligned) and 2 for
// EncodeForKind (a 1-byte ObjectKind precedes it there).
func (v ObjectVariant) encodeInner(padLen int) []byte {
	var payload []byte
	nul := 0
	switch v.kind {
	case variantByReference, variantByXMLString:
		payload = []byte(v.str)
		nul = 1
	case variantInBinary:
		payload = v.bin
	}
	n := len(payload) + nul
	out := make([]byte, 0, 1+padLen+4+n)
	out = append(out, byte(v.kind))
	out = append(out, make([]byte, padLen)...)
	out = append(out, byte(n), byte(n>>8), byte(n>>16), byte(n>>24))
	out = append(out, payload...)
	if nul == 1 {
		out = append(out, 0)
	}
	return out
}

// ObjectVariantExtra is the trailing field several ObjectKinds' own
// `..._Representation` struct appends after the inner representation (spec
// §7.7.3) — see EncodeForKind's doc. At most one of DomainID/Parent is ever
// set for a given, valid ObjectKind.
type ObjectVariantExtra struct {
	// DomainID is OBJK_PARTICIPANT_Representation.domain_id.
	DomainID *int16
	// Parent is OBJK_TOPIC_Representation.participant_id /
	// OBJK_PUBLISHER_Representation.participant_id /
	// OBJK_SUBSCRIBER_Representation.participant_id /
	// DATAWRITER_Representation.publisher_id /
	// DATAREADER_Representation.subscriber_id — "the id of the container
	// this one is created under", whatever Annex A calls the field for
	// this particular kind.
	Parent *ObjectID
}

func extraHasDomainID(kind ObjectKind) bool { return kind == ObjkParticipant }

func extraHasParent(kind ObjectKind) bool {
	switch kind {
	case ObjkTopic, ObjkPublisher, ObjkSubscriber, ObjkDataWriter, ObjkDataReader:
		return true
	default:
		return false
	}
}

// EncodeForKind encodes the real wire ObjectVariant (spec §7.7.3): a
// **two-level** discriminated union — the outer ObjectKind byte (`kind`,
// e.g. ObjkDataWriter) selects which `..._Representation` struct follows,
// then this inner format-discriminated payload (encodeInner), then that
// struct's own trailing field: DomainID (`short`, 2-byte aligned) for
// ObjkParticipant, Parent (ObjectID, unaligned) for ObjkTopic/ObjkPublisher/
// ObjkSubscriber/ObjkDataWriter/ObjkDataReader, or nothing for every other
// kind — byte-oracled in crates/xrce/src/object_repr.rs against real
// eProsima Micro-XRCE-DDS Client 3.0.1 CREATE traffic (ObjkTopic's own
// participant_id: commit 31c62df2a63). This — not the bare encodeInner —
// is the only form a real agent accepts.
func (v ObjectVariant) EncodeForKind(kind ObjectKind, extra ObjectVariantExtra) ([]byte, error) {
	hasDomainID := extraHasDomainID(kind)
	hasParent := extraHasParent(kind)
	if hasDomainID && extra.DomainID == nil {
		return nil, fmt.Errorf("zerodds: xrce object variant: OBJK_PARTICIPANT representation requires DomainID")
	}
	if !hasDomainID && extra.DomainID != nil {
		return nil, fmt.Errorf("zerodds: xrce object variant: only OBJK_PARTICIPANT representation carries DomainID")
	}
	if hasParent && extra.Parent == nil {
		return nil, fmt.Errorf("zerodds: xrce object variant: this ObjectKind's representation requires a Parent id")
	}
	if !hasParent && extra.Parent != nil {
		return nil, fmt.Errorf("zerodds: xrce object variant: this ObjectKind's representation carries no Parent id")
	}

	out := append([]byte{byte(kind)}, v.encodeInner(2)...)
	if extra.DomainID != nil {
		if len(out)%2 != 0 {
			out = append(out, 0) // `short` aligns to 2 bytes; ObjectID (below) does not
		}
		d := uint16(*extra.DomainID)
		out = append(out, byte(d), byte(d>>8))
	}
	if extra.Parent != nil {
		b := extra.Parent.Bytes()
		out = append(out, b[0], b[1])
	}
	return out, nil
}

// BaseObjectRequest is `request_id (2 octets) + object_id (2 octets)`
// (spec §7.7.8), the header every CREATE/DELETE/WRITE_DATA/READ_DATA body
// starts with.
type BaseObjectRequest struct {
	RequestID [2]byte
	ObjectID  ObjectID
}

// requestIDBytes packs a request id allocated by XrceClient into the wire
// RequestId field, most significant octet first.
//
// DDS-XRCE 1.0 gives RequestId as octet[2] and never as an integer, so the
// wire has no byte order — but this client hands out a uint16 counter, and
// the mapping from one to the other went unstated until three ports had
// already chosen one way and three the other. `crates/xrce-client`'s
// `request_id_octets` settles it as big-endian, which is what the fixtures
// under endpoints/testdata/xrce-client hold: [0x00, 0x01] for the first
// request.
func requestIDBytes(reqID uint16) [2]byte {
	return [2]byte{byte(reqID >> 8), byte(reqID)}
}

func (b BaseObjectRequest) encode() [4]byte {
	oid := b.ObjectID.Bytes()
	return [4]byte{b.RequestID[0], b.RequestID[1], oid[0], oid[1]}
}

// DataFormat is the 3-bit sample-delivery format (spec §8.3.5.8/.9). Only
// FORMAT_DATA is structured here — the format WRITE_DATA/READ_DATA use.
type DataFormat byte

// DataFormatData is FORMAT_DATA (0b000) — a bare sample, no SampleInfo.
const DataFormatData DataFormat = 0

// DataDeliveryControl paces a READ_DATA reply (spec §8.3.5.9): four
// unsigned shorts, 8 octets little-endian.
type DataDeliveryControl struct {
	MaxSamples        uint16
	MaxElapsedTime    uint16
	MaxBytesPerSecond uint16
	MinPacePeriod     uint16
}

func (d DataDeliveryControl) encode() []byte {
	return []byte{
		byte(d.MaxSamples), byte(d.MaxSamples >> 8),
		byte(d.MaxElapsedTime), byte(d.MaxElapsedTime >> 8),
		byte(d.MaxBytesPerSecond), byte(d.MaxBytesPerSecond >> 8),
		byte(d.MinPacePeriod), byte(d.MinPacePeriod >> 8),
	}
}

// ReadSpecification (spec §8.3.5.9) says how the client wants READ_DATA's
// samples delivered. The two `@optional` members use the conservative
// presence-flag encoding `crates/xrce/src/submessages/read_data.rs` chose
// (1 = present / 0 = absent, ahead of the payload) rather than the full
// XCDR2 EMHEADER form, which is Phase 2 there too.
type ReadSpecification struct {
	PreferredStreamID       byte
	DataFormat              DataFormat
	ContentFilterExpression *string
	DeliveryControl         *DataDeliveryControl
}

func (r ReadSpecification) encode() []byte {
	out := []byte{r.PreferredStreamID, byte(r.DataFormat)}
	if r.ContentFilterExpression != nil {
		s := []byte(*r.ContentFilterExpression)
		n := len(s)
		out = append(out, 1, byte(n), byte(n>>8), byte(n>>16), byte(n>>24))
		out = append(out, s...)
	} else {
		out = append(out, 0)
	}
	if r.DeliveryControl != nil {
		out = append(out, 1)
		out = append(out, r.DeliveryControl.encode()...)
	} else {
		out = append(out, 0)
	}
	return out
}

// --- MessageHeader (spec §8.3.2) -------------------------------------

// Reserved SessionId values (spec §8.3.2.1).
const (
	SessionIDNoneWithClientKey    byte = 0x00
	SessionIDNoneWithoutClientKey byte = 0x80
)

// StreamId values (spec §8.3.2.2).
const (
	StreamIDNone       byte = 0x00
	StreamIDBestEffort byte = 0x01
	StreamIDBuiltinRel byte = 0x80
)

// encodeMessageHeader writes session_id(1) + stream_id(1) + sequence_nr(2,
// LE) + an optional 4-octet ClientKey — present exactly when session<=127
// (spec §8.3.2.1). Returns an error if `key`'s presence disagrees with that
// rule, mirroring `MessageHeader::with_client_key` /
// `MessageHeader::without_client_key` refusing the same mismatch.
func encodeMessageHeader(session, stream byte, seq uint16, key *ClientKey) ([]byte, error) {
	carriesKey := session <= 127
	if carriesKey != (key != nil) {
		return nil, fmt.Errorf(
			"zerodds: xrce message header: session_id %#x requires client_key=%v, got %v",
			session, carriesKey, key != nil)
	}
	out := []byte{session, stream, byte(seq), byte(seq >> 8)}
	if key != nil {
		out = append(out, key[:]...)
	}
	return out, nil
}

// Submessage ids this file frames (spec §8.3.5, values 0..15).
const (
	smIDCreateClient byte = 0x00
	smIDCreate       byte = 0x01
	smIDDelete       byte = 0x03
	smIDWriteData    byte = 0x07
	smIDReadData     byte = 0x08
)

// flagELE is the E-flag: the submessage body is little-endian (spec
// §8.3.4). Every builder below sets it — this SDK always writes LE bodies.
const flagELE byte = 0x01

// buildMessage assembles one full XRCE Message: MessageHeader + one
// SubmessageHeader (id, flags, little-endian u16 length) + body. No padding
// is needed — a single submessage is always the last one in its message
// (spec §8.3.3 pads only *between* submessages).
func buildMessage(session, stream byte, seq uint16, key *ClientKey, smID, flags byte, body []byte) ([]byte, error) {
	header, err := encodeMessageHeader(session, stream, seq, key)
	if err != nil {
		return nil, err
	}
	if len(body) > 0xFFFF {
		return nil, fmt.Errorf("zerodds: xrce submessage body: %d octets exceeds the 16-bit length field", len(body))
	}
	n := uint16(len(body))
	out := make([]byte, 0, len(header)+4+len(body))
	out = append(out, header...)
	out = append(out, smID, flags, byte(n), byte(n>>8))
	out = append(out, body...)
	return out, nil
}

// DefaultMTU is eProsima's own hard datagram ceiling (crates/xrce/src/
// handshake.rs's module doc has the byte-oracle measurement backing it),
// used as this SDK's own CREATE_CLIENT requested_mtu.
const DefaultMTU uint16 = 508

// xrceCookie is XrceCookie (Annex A) — spells 'X' 'R' 'C' 'E'.
var xrceCookie = [4]byte{'X', 'R', 'C', 'E'}

// EncodeClientRepresentation encodes CLIENT_Representation (Annex A,
// §7.8.2.1): xrce_cookie(4) + xrce_version(2) + xrce_vendor_id(2) +
// client_key(4) + session_id(1) + a `0` properties-absent octet +
// requestedMTU (2, little endian) — eProsima's own non-normative MTU
// extension, always sent by this SDK. No PropertySeq support here — this
// client sends none.
func EncodeClientRepresentation(version [2]byte, vendorID [2]byte, key ClientKey, sessionID byte, requestedMTU uint16) []byte {
	return EncodeClientRepresentationWith(version, vendorID, key, sessionID, requestedMTU, RepresentationTimestampsAbsent, 0, 0, UnannotatedAppendable)
}

// RepresentationTimestamps selects whether CLIENT_Representation carries
// the `Time_t client_timestamp` that the prose of DDS-XRCE 1.0 section
// 7.8.2.1 lists after xrce_vendor_id. The IDL of Annex A and the bytes of
// Annex B.1 do not have it. The agent has to be set the same way
// (`representation_timestamps:` in its configuration).
type RepresentationTimestamps int

const (
	// RepresentationTimestampsAbsent writes the Annex A / B.1 form (default).
	RepresentationTimestampsAbsent RepresentationTimestamps = iota
	// RepresentationTimestampsPresent writes the prose form of section
	// 7.8.2.1: Time_t (i32 seconds, u32 nanoseconds, little endian) after
	// xrce_vendor_id. Time_t is APPENDABLE with UnannotatedAppendable (a
	// DHEADER = 8 in front, 12 octets), 8 octets with UnannotatedFinal.
	RepresentationTimestampsPresent
)

// EncodeClientRepresentationWith is EncodeClientRepresentation with the
// RepresentationTimestamps setting. With RepresentationTimestampsPresent the
// Time_t is (seconds, nanoseconds); `unann` selects its DHEADER
// (UnannotatedAppendable) or none (UnannotatedFinal). With
// RepresentationTimestampsAbsent seconds, nanoseconds and unann are ignored.
func EncodeClientRepresentationWith(version [2]byte, vendorID [2]byte, key ClientKey, sessionID byte, requestedMTU uint16, ts RepresentationTimestamps, seconds int32, nanoseconds uint32, unann UnannotatedTypes) []byte {
	out := make([]byte, 0, 28)
	out = append(out, xrceCookie[:]...)
	out = append(out, version[:]...)
	out = append(out, vendorID[:]...)
	if ts == RepresentationTimestampsPresent {
		if unann != UnannotatedFinal {
			out = append(out, 8, 0, 0, 0) // DHEADER of Time_t
		}
		s, n := uint32(seconds), nanoseconds
		out = append(out, byte(s), byte(s>>8), byte(s>>16), byte(s>>24))
		out = append(out, byte(n), byte(n>>8), byte(n>>16), byte(n>>24))
	}
	out = append(out, key[:]...)
	out = append(out, sessionID, 0) // session_id, then properties-absent
	out = append(out, byte(requestedMTU), byte(requestedMTU>>8))
	return out
}

// BuildCreateClientMessage frames CREATE_CLIENT (§7.8.2/§8.3.5.1): the
// client -> agent handshake `Connect()` starts. `representation` is the
// XCDR2-encoded `CLIENT_Representation` (build it with
// EncodeClientRepresentation) — this function only frames it, without
// interpreting it. `session` is the session_id of the header: for a CREATE_CLIENT
// SESSION_ID_NONE_WITH_CLIENT_KEY (0x00, with key) when the representation asks
// for a session_id of 0..127, or SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80,
// key nil) when it asks for 128..255 (DDS-XRCE 1.0 §8.3.2.1), not the session
// the representation asks for.
func BuildCreateClientMessage(session, stream byte, seq uint16, key *ClientKey, representation []byte) ([]byte, error) {
	return buildMessage(session, stream, seq, key, smIDCreateClient, flagELE, representation)
}

// BuildCreateMessage frames CREATE (§7.8.3/§8.3.5.2) for the request id
// XrceClient.CreateObject returned. `kind` is the outer ObjectKind byte the
// wire ObjectVariant (§7.7.3) always carries alongside `variant`; `extra`
// is that kind's own trailing field — see ObjectVariant.EncodeForKind.
// `reuse`/`replace` set CreationMode bits 1/2 of the submessage flags.
func BuildCreateMessage(session, stream byte, seq uint16, key *ClientKey, requestID uint16, objectID ObjectID, kind ObjectKind, variant ObjectVariant, extra ObjectVariantExtra, reuse, replace bool) ([]byte, error) {
	base := BaseObjectRequest{RequestID: requestIDBytes(requestID), ObjectID: objectID}.encode()
	repr, err := variant.EncodeForKind(kind, extra)
	if err != nil {
		return nil, err
	}
	body := make([]byte, 0, 4+len(repr))
	body = append(body, base[:]...)
	body = append(body, repr...)
	flags := flagELE
	if reuse {
		flags |= 0x02
	}
	if replace {
		flags |= 0x04
	}
	return buildMessage(session, stream, seq, key, smIDCreate, flags, body)
}

// BuildDeleteMessage frames DELETE (§7.8.3/§8.3.5.4) for the request id
// XrceClient.DeleteObject returned.
func BuildDeleteMessage(session, stream byte, seq uint16, key *ClientKey, requestID uint16, objectID ObjectID) ([]byte, error) {
	base := BaseObjectRequest{RequestID: requestIDBytes(requestID), ObjectID: objectID}.encode()
	return buildMessage(session, stream, seq, key, smIDDelete, flagELE, base[:])
}

// BuildWriteDataMessage frames WRITE_DATA (§7.8.4/§8.3.5.8, FORMAT_DATA)
// for the request id XrceClient.RequestWrite returned: BaseObjectRequest
// (4 octets) then `sample` verbatim — no length of its own (DDS-XRCE 1.0
// Annex B.3.1; crates/xrce/src/submessages/write_data.rs).
func BuildWriteDataMessage(session, stream byte, seq uint16, key *ClientKey, requestID uint16, writer ObjectID, sample []byte) ([]byte, error) {
	base := BaseObjectRequest{RequestID: requestIDBytes(requestID), ObjectID: writer}.encode()
	body := make([]byte, 0, 4+len(sample))
	body = append(body, base[:]...)
	body = append(body, sample...)
	return buildMessage(session, stream, seq, key, smIDWriteData, flagELE, body)
}

// BuildReadDataMessage frames READ_DATA (§7.8.5/§8.3.5.9) for the request id
// XrceClient.RequestRead returned.
func BuildReadDataMessage(session, stream byte, seq uint16, key *ClientKey, requestID uint16, reader ObjectID, spec ReadSpecification) ([]byte, error) {
	base := BaseObjectRequest{RequestID: requestIDBytes(requestID), ObjectID: reader}.encode()
	body := make([]byte, 0, 4+8)
	body = append(body, base[:]...)
	body = append(body, spec.encode()...)
	return buildMessage(session, stream, seq, key, smIDReadData, flagELE, body)
}

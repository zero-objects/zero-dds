// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

package zerodds

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

// --- state-machine tests, mirroring crates/xrce-client/src/lib.rs's own
// unit tests one for one -------------------------------------------------

func testKey() ClientKey { return ClientKey{0xAB, 0xAB, 0xAB, 0xAB} }

func TestNewClientStartsDisconnected(t *testing.T) {
	c := NewXrceClient(testKey())
	if c.State() != Disconnected {
		t.Fatalf("state = %v, want Disconnected", c.State())
	}
	if c.Key() != testKey() {
		t.Fatalf("key mismatch")
	}
}

func TestConnectTransitionsToConnecting(t *testing.T) {
	c := NewXrceClient(testKey())
	if err := c.Connect(); err != nil {
		t.Fatalf("connect: %v", err)
	}
	if c.State() != Connecting {
		t.Fatalf("state = %v, want Connecting", c.State())
	}
}

func TestMarkConnectedTransitionsToConnected(t *testing.T) {
	c := NewXrceClient(testKey())
	must(t, c.Connect())
	must(t, c.MarkConnected())
	if c.State() != Connected {
		t.Fatalf("state = %v, want Connected", c.State())
	}
}

func TestDoubleConnectRejected(t *testing.T) {
	c := NewXrceClient(testKey())
	must(t, c.Connect())
	if err := c.Connect(); err != ErrInvalidState {
		t.Fatalf("second connect: got %v, want ErrInvalidState", err)
	}
}

func TestCreateWithoutConnectRejected(t *testing.T) {
	c := NewXrceClient(testKey())
	oid, _ := NewObjectID(0x010, ObjkTopic)
	if _, err := c.CreateObject(oid, ByReference("topic-ref")); err != ErrInvalidState {
		t.Fatalf("create_object: got %v, want ErrInvalidState", err)
	}
}

func TestWriteWithoutConnectRejected(t *testing.T) {
	c := NewXrceClient(testKey())
	oid, _ := NewObjectID(0x010, ObjkTopic)
	if _, err := c.RequestWrite(oid, []byte("x")); err != ErrInvalidState {
		t.Fatalf("request_write: got %v, want ErrInvalidState", err)
	}
}

func TestReadWithoutConnectRejected(t *testing.T) {
	c := NewXrceClient(testKey())
	oid, _ := NewObjectID(0x010, ObjkTopic)
	if _, err := c.RequestRead(oid); err != ErrInvalidState {
		t.Fatalf("request_read: got %v, want ErrInvalidState", err)
	}
}

func TestFullLifecycleCreatesUniqueRequestIDs(t *testing.T) {
	c := NewXrceClient(testKey())
	must(t, c.Connect())
	must(t, c.MarkConnected())
	oid, _ := NewObjectID(0x010, ObjkTopic)
	r1, err := c.CreateObject(oid, ByReference("a"))
	must(t, err)
	r2, err := c.DeleteObject(oid)
	must(t, err)
	r3, err := c.RequestWrite(oid, []byte("x"))
	must(t, err)
	r4, err := c.RequestRead(oid)
	must(t, err)
	if r1 != 1 || r2 != 2 || r3 != 3 || r4 != 4 {
		t.Fatalf("request ids = %d,%d,%d,%d, want 1,2,3,4", r1, r2, r3, r4)
	}
}

func TestDisconnectClearsState(t *testing.T) {
	c := NewXrceClient(testKey())
	must(t, c.Connect())
	must(t, c.MarkConnected())
	c.Disconnect()
	if c.State() != Disconnected {
		t.Fatalf("state = %v, want Disconnected", c.State())
	}
}

func must(t *testing.T, err error) {
	t.Helper()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
}

// --- wire-octet tests -----------------------------------------------------
//
// Fixtures live under endpoints/testdata/xrce-client/*.bin, produced by
// endpoints/golden-gen actually running zerodds_xrce_client::XrceClient
// (crates/xrce-client) over zerodds_xrce (crates/xrce) — the same two
// crates this file ports. All 20 language ports (plus
// endpoints/xrce-agent-demo) check the same five files; a hardcoded hex
// string here would be a second copy of the truth that can drift from it
// unnoticed. ClientKey = {0x01,0x02,0x03,0x04} throughout, session_id =
// 0x01 (<=127, so every header after CREATE_CLIENT carries the ClientKey;
// the CREATE_CLIENT header itself has session 0x00, see below).
//
//	create_client: CREATE_CLIENT, a real CLIENT_Representation
//	  (version={1,0}, vendor_id={0,0}, this client_key, session_id=0x01,
//	  no properties, requested_mtu=508); header = (session 0x00, stream
//	  STREAMID_NONE, seq 0, key). The header session is
//	  SESSION_ID_NONE_WITH_CLIENT_KEY (DDS-XRCE 1.0 §8.3.2.1), not the
//	  session_id 0x01 the payload asks for.
//	create: CREATE, request_id={0x00,0x01}, object_id=pack(raw=0x001,
//	  kind=DATAWRITER), ObjectVariant::ByReference("SensorWriter") under
//	  parent=pack(raw=0x001, kind=PUBLISHER) — the real two-level
//	  ObjectVariant (§7.7.3), reuse=false, replace=false; header =
//	  (session 0x01, stream STREAMID_BUILTIN_RELIABLE, seq 0, key).
//	delete: DELETE, request_id={0x00,0x02}, object_id=pack(raw=0x001,
//	  kind=DATAWRITER) — deletes the writer CREATEd above; header =
//	  (session 0x01, stream STREAMID_BUILTIN_RELIABLE, seq 1, key).
//	read_data: READ_DATA, request_id={0x00,0x03}, object_id=pack(raw=0x002,
//	  kind=DATAREADER), preferred_stream_id=0x01, data_format=Data, no
//	  filter, no delivery control; header = (session 0x01, stream
//	  STREAMID_BUILTIN_RELIABLE, seq 2, key).
//	write_data: WRITE_DATA, request_id={0x00,0x04}, object_id=pack(raw=
//	  0x001, kind=DATAWRITER) — the same writer — sample=[0x19,0x00];
//	  header = (session 0x01, stream STREAMID_BUILTIN_RELIABLE, seq 3, key).
//
// These four post-handshake fixtures are read independently, each as one
// wire example of its own submessage kind — not as one
// application-consistent object lifecycle: write_data targets the writer
// delete already removed, and nothing about either message's bytes
// depends on the other one having run first.
func xrceFixture(t *testing.T, name string) []byte {
	t.Helper()
	b, err := os.ReadFile(filepath.Join("..", "testdata", "xrce-client", name+".bin"))
	if err != nil {
		t.Fatalf("read fixture %s: %v", name, err)
	}
	return b
}

func fixtureKey() *ClientKey { k := ClientKey{0x01, 0x02, 0x03, 0x04}; return &k }

func TestBuildCreateClientMessageMatchesFixture(t *testing.T) {
	want := xrceFixture(t, "xrce_client_create_client")
	rep := EncodeClientRepresentation([2]byte{0x01, 0x00}, [2]byte{0x00, 0x00}, *fixtureKey(), 0x01, DefaultMTU)
	// §8.3.2.1: the CREATE_CLIENT of a session_id below 128 goes in a header with
	// SESSION_ID_NONE_WITH_CLIENT_KEY (0x00); the payload asks for session 1.
	got, err := BuildCreateClientMessage(0x00, StreamIDNone, 0, fixtureKey(), rep)
	must(t, err)
	if !bytes.Equal(got, want) {
		t.Fatalf("CREATE_CLIENT mismatch:\n got  %x\n want %x", got, want)
	}
}

// TestSessionOperationsMatchFixtures drives one XrceClient through
// CREATE/DELETE/READ_DATA/WRITE_DATA, so its request-id counter is proven
// too, not just the encoder. All four are reliable, so they share
// StreamIDBuiltinRel and a real, strictly increasing sequence number
// (0..3) — not the best-effort stream with hand-picked numbers a previous
// fixture set used.
// DDS-XRCE 1.0 section 7.8.2.1 prose vs Annex A / B.1: the three forms of
// CLIENT_Representation.
func TestEncodeClientRepresentationTimestamps(t *testing.T) {
	ver, ven := [2]byte{0x01, 0x00}, [2]byte{0x00, 0x00}
	absent := []byte{0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x01, 0x00, 0xfc, 0x01}
	appendable := []byte{0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x00, 0x00,
		0x08, 0x00, 0x00, 0x00, 0x8c, 0xaa, 0x88, 0x5a, 0x00, 0x65, 0xcd, 0x1d,
		0x01, 0x02, 0x03, 0x04, 0x01, 0x00, 0xfc, 0x01}
	final := []byte{0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x00, 0x00,
		0x8c, 0xaa, 0x88, 0x5a, 0x00, 0x65, 0xcd, 0x1d,
		0x01, 0x02, 0x03, 0x04, 0x01, 0x00, 0xfc, 0x01}
	if got := EncodeClientRepresentation(ver, ven, *fixtureKey(), 0x01, DefaultMTU); !bytes.Equal(got, absent) {
		t.Fatalf("default: got %x want %x", got, absent)
	}
	if got := EncodeClientRepresentationWith(ver, ven, *fixtureKey(), 0x01, 508, RepresentationTimestampsAbsent, 1518905996, 500000000, UnannotatedAppendable); !bytes.Equal(got, absent) {
		t.Fatalf("absent: got %x want %x", got, absent)
	}
	if got := EncodeClientRepresentationWith(ver, ven, *fixtureKey(), 0x01, 508, RepresentationTimestampsPresent, 1518905996, 500000000, UnannotatedAppendable); !bytes.Equal(got, appendable) {
		t.Fatalf("present appendable: got %x want %x", got, appendable)
	}
	if got := EncodeClientRepresentationWith(ver, ven, *fixtureKey(), 0x01, 508, RepresentationTimestampsPresent, 1518905996, 500000000, UnannotatedFinal); !bytes.Equal(got, final) {
		t.Fatalf("present final: got %x want %x", got, final)
	}
	c := NewXrceClient(*fixtureKey())
	if c.RepresentationTimestamps() != RepresentationTimestampsAbsent {
		t.Fatalf("default setting is not absent")
	}
	if got := c.ClientRepresentation(ver, ven, 0x01, DefaultMTU); !bytes.Equal(got, absent) {
		t.Fatalf("client default: got %x want %x", got, absent)
	}
	c.SetRepresentationTimestamps(RepresentationTimestampsPresent, UnannotatedAppendable)
	if got := c.ClientRepresentation(ver, ven, 0x01, DefaultMTU); len(got) != 28 {
		t.Fatalf("client present: %d octets, want 28", len(got))
	}
}

func TestBuildCreateClientMessageHeaderSessionAbove127(t *testing.T) {
	// §8.3.2.1: a requested session_id of 128..255 goes in a header with
	// SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80), which carries no client key.
	rep := EncodeClientRepresentation([2]byte{0x01, 0x00}, [2]byte{0x00, 0x00}, *fixtureKey(), 0x81, DefaultMTU)
	got, err := BuildCreateClientMessage(SessionIDNoneWithoutClientKey, StreamIDNone, 0, nil, rep)
	must(t, err)
	if got[0] != 0x80 || len(got) != 4+4+len(rep) || got[4+4+12] != 0x81 {
		t.Fatalf("CREATE_CLIENT for session 0x81: header session/layout wrong: %x", got)
	}
}

func TestSessionOperationsMatchFixtures(t *testing.T) {
	c := NewXrceClient(*fixtureKey())
	must(t, c.Connect())
	must(t, c.MarkConnected())

	writer, err := NewObjectID(0x001, ObjkDataWriter)
	must(t, err)
	publisher, err := NewObjectID(0x001, ObjkPublisher)
	must(t, err)
	reader, err := NewObjectID(0x002, ObjkDataReader)
	must(t, err)
	variant := ByReference("SensorWriter")

	req, err := c.CreateObject(writer, variant)
	must(t, err)
	if req != 1 {
		t.Fatalf("create request id = %d, want 1", req)
	}
	got, err := BuildCreateMessage(0x01, StreamIDBuiltinRel, 0, fixtureKey(), req, writer, ObjkDataWriter, variant, ObjectVariantExtra{Parent: &publisher}, false, false)
	must(t, err)
	if want := xrceFixture(t, "xrce_client_create"); !bytes.Equal(got, want) {
		t.Fatalf("CREATE mismatch:\n got  %x\n want %x", got, want)
	}

	req, err = c.DeleteObject(writer)
	must(t, err)
	if req != 2 {
		t.Fatalf("delete request id = %d, want 2", req)
	}
	got, err = BuildDeleteMessage(0x01, StreamIDBuiltinRel, 1, fixtureKey(), req, writer)
	must(t, err)
	if want := xrceFixture(t, "xrce_client_delete"); !bytes.Equal(got, want) {
		t.Fatalf("DELETE mismatch:\n got  %x\n want %x", got, want)
	}

	req, err = c.RequestRead(reader)
	must(t, err)
	if req != 3 {
		t.Fatalf("read request id = %d, want 3", req)
	}
	spec := ReadSpecification{PreferredStreamID: 0x01, DataFormat: DataFormatData}
	got, err = BuildReadDataMessage(0x01, StreamIDBuiltinRel, 2, fixtureKey(), req, reader, spec)
	must(t, err)
	if want := xrceFixture(t, "xrce_client_read_data"); !bytes.Equal(got, want) {
		t.Fatalf("READ_DATA mismatch:\n got  %x\n want %x", got, want)
	}

	req, err = c.RequestWrite(writer, []byte{0x19, 0x00})
	must(t, err)
	if req != 4 {
		t.Fatalf("write request id = %d, want 4", req)
	}
	got, err = BuildWriteDataMessage(0x01, StreamIDBuiltinRel, 3, fixtureKey(), req, writer, []byte{0x19, 0x00})
	must(t, err)
	if want := xrceFixture(t, "xrce_client_write_data"); !bytes.Equal(got, want) {
		t.Fatalf("WRITE_DATA mismatch:\n got  %x\n want %x", got, want)
	}

	// A second CREATE, appended to the same session: OBJK_TOPIC's own
	// representation carries its participant's ObjectId (commit
	// 31c62df2a63, "xrce: OBJK_TOPIC_Representation carries its
	// participant's ObjectId"), the same trailing field OBJK_PUBLISHER's
	// representation carries above — xrce_client_create_topic.bin is a
	// sixth fixture generated the same way (endpoints/golden-gen), not a
	// hand-picked byte string.
	topic, err := NewObjectID(0x010, ObjkTopic)
	must(t, err)
	participant, err := NewObjectID(0x001, ObjkParticipant)
	must(t, err)
	topicVariant := ByReference("HelloWorldTopic")

	req, err = c.CreateObject(topic, topicVariant)
	must(t, err)
	if req != 5 {
		t.Fatalf("create topic request id = %d, want 5", req)
	}
	got, err = BuildCreateMessage(0x01, StreamIDBuiltinRel, 4, fixtureKey(), req, topic, ObjkTopic, topicVariant, ObjectVariantExtra{Parent: &participant}, false, false)
	must(t, err)
	if want := xrceFixture(t, "xrce_client_create_topic"); !bytes.Equal(got, want) {
		t.Fatalf("CREATE(topic) mismatch:\n got  %x\n want %x", got, want)
	}
}

// TestTopicRepresentationMatchesRustByteOracle: crates/xrce/src/object_repr.rs's
// own topic_representation_matches_the_captured_create_bytes test decodes
// these exact 96 bytes (the real eProsima Micro-XRCE-DDS 3.0.1 capture,
// tools/wire-capture/xrce-sessions/client-to-agent.hex) as
// OBJK_TOPIC/ByXMLString/participant_id=ObjectID(1, PARTICIPANT). This
// port's EncodeForKind — unlike xrce_client_create_topic.bin's ByReference
// case above — can also express ByXMLString, so it is checked against
// that same oracle directly.
func TestTopicRepresentationMatchesRustByteOracle(t *testing.T) {
	xml := "<dds><topic><name>HelloWorldTopic</name><dataType>HelloWorld</dataType></topic></dds>"
	want := append([]byte{0x02, 0x02, 0x00, 0x00, 0x56, 0x00, 0x00, 0x00}, []byte(xml)...)
	want = append(want, 0x00, 0x00, 0x11)
	participant, err := NewObjectID(0x001, ObjkParticipant)
	must(t, err)
	got, err := ByXMLString(xml).EncodeForKind(ObjkTopic, ObjectVariantExtra{Parent: &participant})
	must(t, err)
	if !bytes.Equal(got, want) {
		t.Fatalf("OBJK_TOPIC ByXMLString mismatch:\n got  %x\n want %x", got, want)
	}
}

// TestTopicCreateWithoutParentRejected: OBJK_TOPIC now carries a parent id
// (extraHasParent), so a CREATE without one is refused the same way
// OBJK_PUBLISHER's already is — EncodeForKind's own
// extraHasParent/Parent-nil guard, not a topic-specific check.
func TestTopicCreateWithoutParentRejected(t *testing.T) {
	if _, err := ByReference("HelloWorldTopic").EncodeForKind(ObjkTopic, ObjectVariantExtra{}); err == nil {
		t.Fatalf("OBJK_TOPIC CREATE without a parent: want error")
	}
}

// TestObjectIDPacking cross-checks the ObjectId bit layout independent of
// the fixtures above (crates/xrce/src/object_id.rs `new_packs_kind_into_lower_4_bits`).
func TestObjectIDPacking(t *testing.T) {
	id, err := NewObjectID(0x123, ObjkDataWriter)
	must(t, err)
	if id != ObjectID((0x123<<4)|0x05) {
		t.Fatalf("raw = %#x", id)
	}
	if id.Kind() != ObjkDataWriter || id.RawID12() != 0x123 {
		t.Fatalf("kind/raw mismatch: kind=%#x raw=%#x", id.Kind(), id.RawID12())
	}
	b := id.Bytes()
	back, err := ObjectIDFromBytes(b[:])
	must(t, err)
	if back != id {
		t.Fatalf("roundtrip mismatch: %#x != %#x", back, id)
	}
}

// TestMessageHeaderRejectsKeyMismatch cross-checks the constructor guard
// crates/xrce's `MessageHeader::with_client_key` /
// `without_client_key` enforce.
func TestMessageHeaderRejectsKeyMismatch(t *testing.T) {
	if _, err := encodeMessageHeader(10, StreamIDNone, 0, nil); err == nil {
		t.Fatalf("session_id<=127 without a key: want error")
	}
	if _, err := encodeMessageHeader(200, StreamIDNone, 0, fixtureKey()); err == nil {
		t.Fatalf("session_id>=128 with a key: want error")
	}
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

package zerodds

import (
	"errors"
	"time"
)

// XrceClient is a native Go port of the DDS-XRCE 1.0 client state machine in
// `crates/xrce-client/src/lib.rs` (340 lines), combined with the wire codec
// of `crates/xrce`. `crates/xrce-client` itself never touches the wire — its
// module doc says so directly: "the caller delegates submessage encoding;
// we only track state". This file is that combination: XrceClient tracks
// state and allocates request ids exactly as the Rust crate does, and the
// Build*Message functions below produce the same octets `crates/xrce`
// would for the object CREATE/DELETE/WRITE_DATA/READ_DATA session — the
// rung this SDK was missing between the transport-less endpoints/go XCDR
// core and the full crates/dcps stack that only the C API reaches.
//
// This sits beside the existing `Transport`, `Client`/`AsyncReader` (sync.go,
// async.go) and `ReliableSender`/`ReliableReceiver` (reliable.go) — none of
// that is touched. Those speak the pre-existing *ZeroDDS Endpoint Profile*
// WRITE_DATA framing (sample body only, no BaseObjectRequest — see the
// comment on `XrceWriteFrame` in endpoint.go); XrceClient speaks the
// spec-conformant DDS-XRCE 1.0 object-lifecycle frames a real XRCE agent
// expects for CREATE_CLIENT/CREATE/DELETE/READ_DATA, plus the conformant
// WRITE_DATA that carries the target ObjectId (`crates/xrce/src/submessages/
// write_data.rs`, DDS-XRCE 1.0 Annex B.3.1). It plugs into the same
// `Transport` interface — Deliver/Receive — and replaces neither.
//
// Spec mapping (mirrors the module doc of crates/xrce-client/src/lib.rs):
//
//	NewXrceClient  -> §7.8.2 CREATE_CLIENT
//	Connect        -> §8.4.5 (handshake)
//	CreateObject   -> §7.8.3 CREATE
//	DeleteObject   -> §7.8.3 DELETE
//	RequestWrite   -> §7.8.4 WRITE_DATA
//	RequestRead    -> §7.8.5 READ_DATA
//	Disconnect     -> §8.4.5

// ClientState is the XRCE client lifecycle status (crates/xrce-client
// `ClientState`).
type ClientState int

const (
	// Disconnected: the client knows its ClientKey but is not yet connected.
	Disconnected ClientState = iota
	// Connecting: CREATE_CLIENT sent, STATUS_AGENT not yet received.
	Connecting
	// Connected: operations are possible.
	Connected
)

// String names the state, for logging.
func (s ClientState) String() string {
	switch s {
	case Disconnected:
		return "Disconnected"
	case Connecting:
		return "Connecting"
	case Connected:
		return "Connected"
	default:
		return "Unknown"
	}
}

// ErrInvalidState mirrors `ClientError::InvalidState`: an operation was
// attempted in the wrong lifecycle state (e.g. CreateObject before Connect).
var ErrInvalidState = errors.New("zerodds: xrce client: invalid state for this operation")

// ClientKey is the 4-byte XRCE ClientKey (spec §7.7).
type ClientKey [4]byte

// ClientKeyInvalid is CLIENTKEY_INVALID (spec §7.8.2.2) — all-zero.
var ClientKeyInvalid = ClientKey{}

// XrceClient is the synchronous XRCE client state machine (port of
// crates/xrce-client's `XrceClient<T>`). It does not hold a Transport and
// does not send anything itself, exactly like the Rust crate — pair its
// request ids with the Build*Message functions and deliver the result
// yourself. That split is deliberate: it is what lets this file be tested
// against the same fixtures a Rust caller combining crates/xrce-client with
// crates/xrce would produce.
type XrceClient struct {
	key           ClientKey
	state         ClientState
	nextRequestID uint16
	repTimestamps RepresentationTimestamps
	repUnann      UnannotatedTypes
}

// NewXrceClient returns a client in the Disconnected state with the given
// ClientKey (spec §7.8.2 CREATE_CLIENT).
func NewXrceClient(key ClientKey) *XrceClient {
	return &XrceClient{key: key, state: Disconnected, nextRequestID: 1}
}

// State is the current lifecycle status.
func (c *XrceClient) State() ClientState { return c.state }

// SetRepresentationTimestamps sets whether ClientRepresentation writes the
// Time_t of DDS-XRCE 1.0 section 7.8.2.1 (default
// RepresentationTimestampsAbsent). unann selects the Time_t form
// (UnannotatedAppendable: DHEADER = 8, default; UnannotatedFinal: none).
func (c *XrceClient) SetRepresentationTimestamps(ts RepresentationTimestamps, unann UnannotatedTypes) {
	c.repTimestamps, c.repUnann = ts, unann
}

// RepresentationTimestamps is the setting of SetRepresentationTimestamps.
func (c *XrceClient) RepresentationTimestamps() RepresentationTimestamps { return c.repTimestamps }

// ClientRepresentation builds the CLIENT_Representation of this client for
// BuildCreateClientMessage with its RepresentationTimestamps setting. The
// Time_t is the wall clock at the time of the call.
func (c *XrceClient) ClientRepresentation(version, vendorID [2]byte, sessionID byte, requestedMTU uint16) []byte {
	now := time.Now()
	return EncodeClientRepresentationWith(version, vendorID, c.key, sessionID, requestedMTU,
		c.repTimestamps, int32(now.Unix()), uint32(now.Nanosecond()), c.repUnann)
}

// Key is the ClientKey this client was constructed with.
func (c *XrceClient) Key() ClientKey { return c.key }

// Connect initiates the handshake (Disconnected -> Connecting). The caller
// still has to build and deliver the CREATE_CLIENT message itself, e.g. via
// BuildCreateClientMessage.
func (c *XrceClient) Connect() error {
	if c.state != Disconnected {
		return ErrInvalidState
	}
	c.state = Connecting
	return nil
}

// MarkConnected completes the handshake once the caller has received
// STATUS_AGENT (Connecting -> Connected).
func (c *XrceClient) MarkConnected() error {
	if c.state != Connecting {
		return ErrInvalidState
	}
	c.state = Connected
	return nil
}

// Disconnect closes the session — back to Disconnected. Like the Rust
// client, this sends nothing: DELETE-ing the OBJECTID_CLIENT singleton
// first, if desired, is the caller's job.
func (c *XrceClient) Disconnect() { c.state = Disconnected }

func (c *XrceClient) requireConnected() error {
	if c.state != Connected {
		return ErrInvalidState
	}
	return nil
}

// allocRequestID hands out the next request id, wrapping at 0xFFFF back to 1
// (0 is never a valid request id) — identical to
// `next_request_id.wrapping_add(1).max(1)` in crates/xrce-client.
func (c *XrceClient) allocRequestID() uint16 {
	req := c.nextRequestID
	next := req + 1 // uint16 wraps to 0 at 0xFFFF+1
	if next == 0 {
		next = 1
	}
	c.nextRequestID = next
	return req
}

// CreateObject submits a CREATE (§7.8.3) and returns its request id.
func (c *XrceClient) CreateObject(ObjectID, ObjectVariant) (uint16, error) {
	if err := c.requireConnected(); err != nil {
		return 0, err
	}
	return c.allocRequestID(), nil
}

// DeleteObject submits a DELETE (§7.8.3) and returns its request id.
func (c *XrceClient) DeleteObject(ObjectID) (uint16, error) {
	if err := c.requireConnected(); err != nil {
		return 0, err
	}
	return c.allocRequestID(), nil
}

// RequestWrite submits a WRITE_DATA (§7.8.4) and returns its request id.
func (c *XrceClient) RequestWrite(ObjectID, []byte) (uint16, error) {
	if err := c.requireConnected(); err != nil {
		return 0, err
	}
	return c.allocRequestID(), nil
}

// RequestRead submits a READ_DATA (§7.8.5, pull-based) and returns its
// request id.
func (c *XrceClient) RequestRead(ObjectID) (uint16, error) {
	if err := c.requireConnected(); err != nil {
		return 0, err
	}
	return c.allocRequestID(), nil
}

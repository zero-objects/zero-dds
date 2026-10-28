// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-XRCE 1.0 client with the full object lifecycle, ported from
// `crates/xrce-client/src/lib.rs`. It does not touch `Transport`, `Client`, or
// `AsyncReader`; it is additive.
//
// This file used to justify itself by the frame: `zerodds.zig`'s `Client` wrote
// a non-conformant one and this was "the missing conformant rung". That reason
// is gone — `xrceWriteFrame` writes the §7.7.8 body under §8.3.5.8 flags now,
// and both clients put the same octets on the wire. What is left is the
// protocol above the frame, which is the mapping below: `zerodds.zig`'s client
// writes samples to a peer that already knows the endpoint, this one negotiates
// the session and the objects with an agent that does not.
//
// Spec mapping (OMG DDS-XRCE 1.0):
//   connect         -> CREATE_CLIENT, §7.8.2 / handshake §8.4.5
//   markConnected   -> handshake completion, §8.4.5 (pure bookkeeping, no
//                      wire I/O -- the caller has already received
//                      STATUS_AGENT out of band)
//   createObject    -> CREATE, §7.8.3
//   deleteObject    -> DELETE, §7.8.3
//   requestWrite    -> WRITE_DATA, §7.8.4
//   requestRead     -> READ_DATA, §7.8.5
//   disconnect      -> §8.4.5 (resets only lifecycle state)
//
// Two layers:
//   Layer 1 -- pure wire encoders: MessageHeader (§8.3.2), SubmessageHeader
//              (§8.3.4), ObjectId (§7.2.1), BaseObjectRequest (§7.7.8),
//              ObjectVariant::ByReference (§7.7.2), and the five full
//              submessages built from them. No state, no I/O -- these are
//              exactly the octets an independent agent parses.
//   Layer 2 -- XrceClient, a synchronous state machine mirroring
//              `crates/xrce-client` 1:1 (state transitions, InvalidState
//              refusals, monotonic request-id allocation skipping zero,
//              `disconnect` resetting only lifecycle state), but -- unlike
//              the bare Rust reference, whose ops are bookkeeping-only and
//              leave "the caller delegates submessage encoding" to someone
//              else -- this port actually encodes and hands each submessage
//              to the SDK's existing `Transport`, since closing that gap is
//              the point of this file. Zig's `Transport.deliver` returns a
//              `bool` success flag (Nim's and Lua's do not), so
//              `ClientError.Transport` is reachable here for real: every
//              send-path method returns `ClientError!u16` (or `!void` for
//              `connect`) and propagates `error.Transport` on a `false`.

const std = @import("std");
const zerodds = @import("zerodds.zig");
const reliable = @import("reliable.zig");

pub const Transport = zerodds.Transport;

// --- Layer 1: wire encoders ---

// MessageHeader session/stream reserved values, §8.3.2.
pub const SESSION_NONE_WITH_KEY: u8 = 0x00;
pub const SESSION_NONE_WITHOUT_KEY: u8 = 0x80;
pub const STREAM_NONE: u8 = 0x00;
pub const STREAM_BUILTIN_BEST_EFFORT: u8 = 0x01;
pub const STREAM_BUILTIN_RELIABLE: u8 = 0x80;

const FLAG_E_LITTLE_ENDIAN: u8 = 0x01;

const SUBMSG_CREATE_CLIENT: u8 = 0;
const SUBMSG_CREATE: u8 = 1;
const SUBMSG_DELETE: u8 = 3;
const SUBMSG_WRITE_DATA: u8 = 7;
const SUBMSG_READ_DATA: u8 = 8;

/// Object kinds used by this client, §7.2.1 (the low 4 bits of the wire word).
pub const ObjectKind = enum(u4) {
    participant = 1,
    topic = 2,
    publisher = 3,
    subscriber = 4,
    data_writer = 5,
    data_reader = 6,
    type_object = 0x0A,
    qos_profile = 0x0B,
    application = 0x0C,
    agent = 0x0D,
    client = 0x0E,
    domain = 0x0F,
};

/// Packs a 12-bit raw id and a 4-bit kind into the 16-bit ObjectId wire word,
/// §7.2.1: `word = (raw_id << 4) | kind`.
pub fn packObjectId(raw: u12, kind: ObjectKind) u16 {
    return (@as(u16, raw) << 4) | @as(u16, @intFromEnum(kind));
}

/// ObjectId is the one field in this spec written big-endian -- everything
/// else on the wire here is little-endian.
pub fn objectIdBytes(word: u16) [2]u8 {
    return .{ @truncate(word >> 8), @truncate(word) };
}

/// MessageHeader, §8.3.2. `session_id <= 0x7F` carries the ClientKey (8-byte
/// header); `session_id >= 0x80` omits it (4-byte header). Returns the
/// number of bytes written.
pub fn encodeMessageHeader(out: []u8, session_id: u8, stream_id: u8, sequence_nr: u16, client_key: [4]u8) usize {
    out[0] = session_id;
    out[1] = stream_id;
    out[2] = @truncate(sequence_nr);
    out[3] = @truncate(sequence_nr >> 8);
    if (session_id <= 0x7F) {
        out[4] = client_key[0];
        out[5] = client_key[1];
        out[6] = client_key[2];
        out[7] = client_key[3];
        return 8;
    }
    return 4;
}

/// SubmessageHeader, §8.3.4. `submessage_length` is always little-endian
/// regardless of the E-flag, which governs only the body. Always 4 bytes.
pub fn encodeSubmessageHeader(out: []u8, submessage_id: u8, flags: u8, submessage_length: u16) usize {
    out[0] = submessage_id;
    out[1] = flags;
    out[2] = @truncate(submessage_length);
    out[3] = @truncate(submessage_length >> 8);
    return 4;
}

/// BaseObjectRequest, §7.7.8: `request_id` (2 raw, caller-supplied bytes)
/// followed by the ObjectId's big-endian wire word. Always 4 bytes.
pub fn encodeBaseObjectRequest(out: []u8, request_id: [2]u8, object_id_word: u16) usize {
    out[0] = request_id[0];
    out[1] = request_id[1];
    const oid = objectIdBytes(object_id_word);
    out[2] = oid[0];
    out[3] = oid[1];
    return 4;
}

/// The real wire ObjectVariant, §7.7.3: a **two-level** discriminated union
/// -- the outer ObjectKind byte (`object_id_word`'s own low 4 bits -- the
/// outer discriminator and the object being created always agree, by
/// construction), then the inner REPRESENTATION_BY_REFERENCE payload
/// (discriminator 0x01, 2 padding bytes, a little-endian `strlen(name) + 1`
/// length -- the +1 is the NUL terminator XCDR2 strings carry -- the name
/// bytes, then the NUL terminator), then that kind's own trailing field:
/// `parent_id_word` (an ObjectId, unaligned) for a Topic/Publisher/
/// Subscriber/DataWriter/DataReader, `null` for every other kind --
/// byte-oracled in `crates/xrce/src/object_repr.rs` against real eProsima
/// Micro-XRCE-DDS Client 3.0.1 CREATE traffic (Topic's own
/// participant_id: commit 31c62df2a63). This -- not the single-level
/// form a previous version of this port wrote -- is the only form a real
/// agent accepts. Returns the bytes written.
pub fn encodeObjectVariantForKind(out: []u8, object_id_word: u16, name: []const u8, parent_id_word: ?u16) usize {
    out[0] = @truncate(object_id_word & 0x0F); // outer ObjectKind byte (low nibble)
    out[1] = 0x01; // inner disc = REPRESENTATION_BY_REFERENCE
    out[2] = 0x00;
    out[3] = 0x00;
    const len: u32 = @intCast(name.len + 1);
    out[4] = @truncate(len);
    out[5] = @truncate(len >> 8);
    out[6] = @truncate(len >> 16);
    out[7] = @truncate(len >> 24);
    std.mem.copyForwards(u8, out[8 .. 8 + name.len], name);
    out[8 + name.len] = 0x00;
    var n = 8 + name.len + 1;
    if (parent_id_word) |parent| {
        const oid = objectIdBytes(parent);
        out[n] = oid[0];
        out[n + 1] = oid[1];
        n += 2;
    }
    return n;
}

/// `CLIENT_Representation` (Annex A, §7.8.2.1): `xrce_cookie(4) ++
/// xrce_version(2) ++ xrce_vendor_id(2) ++ client_key(4) ++ session_id(1) ++
/// a 0 properties-absent octet ++ requested_mtu(2, LE)` -- eProsima's own
/// non-normative MTU extension (`crates/xrce/src/handshake.rs`'s module doc
/// has the byte-oracle measurement backing `DEFAULT_MTU`), always sent by
/// this client. No PropertySeq support here -- this client sends none.
pub const DEFAULT_MTU: u16 = 508;

/// Form of the `CLIENT_Representation` (DDS-XRCE 1.0 §7.8.2.1). The prose
/// lists `Time_t client_timestamp` after `xrce_vendor_id`; the IDL of Annex A
/// and the bytes of Annex B.1 do not have it. The bytes do not tell the forms
/// apart, so it is a setting; the agent has to be set the same way
/// (`representation_timestamps:` in its configuration). `.absent` (default):
/// the Annex A form. `.present`: the prose form.
pub const RepresentationTimestamps = enum { absent, present };

/// Options of `encodeClientRepresentationWith`. `seconds` / `nanoseconds` are
/// the `Time_t` of the client (read only for `.present`). `Time_t` has no
/// `@extensibility`, so it is APPENDABLE (DDS-XTypes 1.3 7.3.1.2.1.8):
/// `unannotated_types = .appendable` puts a DHEADER (u32 = 8) in front of it,
/// `.final` leaves it out.
pub const RepresentationOptions = struct {
    timestamps: RepresentationTimestamps = .absent,
    seconds: i32 = 0,
    nanoseconds: u32 = 0,
    unannotated_types: reliable.UnannotatedTypes = .appendable,
};

fn putU32Le(out: []u8, v: u32) void {
    out[0] = @truncate(v);
    out[1] = @truncate(v >> 8);
    out[2] = @truncate(v >> 16);
    out[3] = @truncate(v >> 24);
}

/// `encodeClientRepresentation` with the representation timestamps option.
/// `.absent`: 16 octets. `.present` + `.appendable`: 28 octets (`out` needs
/// 28), `.present` + `.final`: 24 octets. Little-endian.
pub fn encodeClientRepresentationWith(out: []u8, version: [2]u8, vendor_id: [2]u8, client_key: [4]u8, session_id: u8, requested_mtu: u16, opts: RepresentationOptions) usize {
    out[0] = 'X';
    out[1] = 'R';
    out[2] = 'C';
    out[3] = 'E';
    out[4] = version[0];
    out[5] = version[1];
    out[6] = vendor_id[0];
    out[7] = vendor_id[1];
    var n: usize = 8;
    if (opts.timestamps == .present) {
        if (opts.unannotated_types == .appendable) {
            putU32Le(out[n..], 8);
            n += 4;
        }
        putU32Le(out[n..], @bitCast(opts.seconds));
        putU32Le(out[n + 4 ..], opts.nanoseconds);
        n += 8;
    }
    @memcpy(out[n .. n + 4], &client_key);
    out[n + 4] = session_id;
    out[n + 5] = 0x00; // properties absent
    out[n + 6] = @truncate(requested_mtu);
    out[n + 7] = @truncate(requested_mtu >> 8);
    return n + 8;
}

/// The Annex A form (`.absent`, 16 octets); see `encodeClientRepresentationWith`.
pub fn encodeClientRepresentation(out: []u8, version: [2]u8, vendor_id: [2]u8, client_key: [4]u8, session_id: u8, requested_mtu: u16) usize {
    return encodeClientRepresentationWith(out, version, vendor_id, client_key, session_id, requested_mtu, .{});
}

/// CREATE_CLIENT, §7.8.2. `session_id` is the session id this client asks
/// for (it travels in the CLIENT_Representation, not in the header). The
/// handshake precedes any session, so the header carries
/// SESSION_ID_NONE_WITH_CLIENT_KEY (0x00) plus the ClientKey when
/// `session_id` is 0..127, and SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80)
/// without ClientKey when it is 128..255 (DDS-XRCE 1.0 §8.3.2.1).
/// `stream_id = STREAM_NONE`, `sequence_nr = 0` -- the handshake never
/// consumes a sequence number. `representation` is the XCDR2-encoded
/// `CLIENT_Representation` -- build it with `encodeClientRepresentation` --
/// copied through verbatim here.
pub fn encodeCreateClientMessage(out: []u8, session_id: u8, client_key: [4]u8, representation: []const u8) usize {
    const header_session: u8 = if (session_id <= 0x7F) 0x00 else 0x80;
    var n = encodeMessageHeader(out, header_session, STREAM_NONE, 0, client_key);
    const body_len: u16 = @intCast(representation.len);
    n += encodeSubmessageHeader(out[n..], SUBMSG_CREATE_CLIENT, FLAG_E_LITTLE_ENDIAN, body_len);
    std.mem.copyForwards(u8, out[n .. n + representation.len], representation);
    n += representation.len;
    return n;
}

/// CREATE, §7.8.3. `flags = E | (reuse ? REUSE : 0) | (replace ? REPLACE : 0)`.
/// Body = BaseObjectRequest ++ the real, two-level ObjectVariant
/// (`encodeObjectVariantForKind`). `parent_id_word` is that kind's own
/// trailing field (e.g. the publisher a DataWriter is created under),
/// `null` for a kind that carries none.
pub fn encodeCreateMessage(
    out: []u8,
    session_id: u8,
    stream_id: u8,
    seq: u16,
    client_key: [4]u8,
    request_id: [2]u8,
    object_id_word: u16,
    name: []const u8,
    parent_id_word: ?u16,
    reuse: bool,
    replace: bool,
) usize {
    var n = encodeMessageHeader(out, session_id, stream_id, seq, client_key);
    const variant_len: u16 = @intCast(1 + 1 + 2 + 4 + name.len + 1 + (if (parent_id_word != null) @as(usize, 2) else 0));
    const body_len: u16 = 4 + variant_len;
    var flags: u8 = FLAG_E_LITTLE_ENDIAN;
    if (reuse) flags |= 0x02;
    if (replace) flags |= 0x04;
    n += encodeSubmessageHeader(out[n..], SUBMSG_CREATE, flags, body_len);
    n += encodeBaseObjectRequest(out[n..], request_id, object_id_word);
    n += encodeObjectVariantForKind(out[n..], object_id_word, name, parent_id_word);
    return n;
}

/// DELETE, §7.8.3. Body = BaseObjectRequest only -- no ObjectVariant.
pub fn encodeDeleteMessage(
    out: []u8,
    session_id: u8,
    stream_id: u8,
    seq: u16,
    client_key: [4]u8,
    request_id: [2]u8,
    object_id_word: u16,
) usize {
    var n = encodeMessageHeader(out, session_id, stream_id, seq, client_key);
    n += encodeSubmessageHeader(out[n..], SUBMSG_DELETE, FLAG_E_LITTLE_ENDIAN, 4);
    n += encodeBaseObjectRequest(out[n..], request_id, object_id_word);
    return n;
}

/// WRITE_DATA, §7.8.4. `flags` is just the E-flag (DataFormat::Data = 0b000,
/// shifted into bit 1, contributes nothing). Body = BaseObjectRequest ++
/// `payload` verbatim, with no length prefix of its own -- the submessage's
/// own `submessage_length` is what bounds it.
pub fn encodeWriteDataMessage(
    out: []u8,
    session_id: u8,
    stream_id: u8,
    seq: u16,
    client_key: [4]u8,
    request_id: [2]u8,
    object_id_word: u16,
    payload: []const u8,
) usize {
    var n = encodeMessageHeader(out, session_id, stream_id, seq, client_key);
    const body_len: u16 = @intCast(4 + payload.len);
    n += encodeSubmessageHeader(out[n..], SUBMSG_WRITE_DATA, FLAG_E_LITTLE_ENDIAN, body_len);
    n += encodeBaseObjectRequest(out[n..], request_id, object_id_word);
    std.mem.copyForwards(u8, out[n .. n + payload.len], payload);
    n += payload.len;
    return n;
}

/// READ_DATA, §7.8.5. Body = BaseObjectRequest ++ ReadSpecification. This
/// client never sets a content filter or delivery control, so
/// ReadSpecification is always the 4 trailing bytes
/// `preferred_stream_id, data_format=0x00, 0x00, 0x00`.
pub fn encodeReadDataMessage(
    out: []u8,
    session_id: u8,
    stream_id: u8,
    seq: u16,
    client_key: [4]u8,
    request_id: [2]u8,
    object_id_word: u16,
    preferred_stream_id: u8,
) usize {
    var n = encodeMessageHeader(out, session_id, stream_id, seq, client_key);
    n += encodeSubmessageHeader(out[n..], SUBMSG_READ_DATA, FLAG_E_LITTLE_ENDIAN, 8);
    n += encodeBaseObjectRequest(out[n..], request_id, object_id_word);
    out[n] = preferred_stream_id;
    out[n + 1] = 0x00; // data_format: Data
    out[n + 2] = 0x00; // content_filter_present: absent
    out[n + 3] = 0x00; // delivery_control_present: absent
    n += 4;
    return n;
}

// --- Layer 2: the state machine ---

pub const ClientState = enum { disconnected, connecting, connected };

pub const ClientError = error{
    /// Operation in the wrong lifecycle status (e.g. send-without-connect).
    InvalidState,
    /// `Transport.deliver` returned `false`. Unreachable in the Nim/Lua
    /// ports, whose `deliver` has no failure channel -- Zig's does, so this
    /// is real here.
    Transport,
    /// Reserved for parity with `crates/xrce-client::ClientError`; unused
    /// even there.
    QueueFull,
};

/// Default session/stream, matching the committed fixtures. Every
/// post-handshake operation is reliable, so the default stream is
/// STREAM_BUILTIN_RELIABLE, not the best-effort stream.
pub const SESSION_ID_DEFAULT: u8 = 0x01;
pub const STREAM_ID_DEFAULT: u8 = STREAM_BUILTIN_RELIABLE;

/// Big-endian 2-byte encoding of a `u16` request id, matching the fixtures
/// (id 1 -> `[0x00, 0x01]`, id 2 -> `[0x00, 0x02]`, ...).
fn requestIdBytes(id: u16) [2]u8 {
    return .{ @truncate(id >> 8), @truncate(id) };
}

/// Synchronous DDS-XRCE client. Holds a fixed, no-heap send buffer (matching
/// `Client.txbuf`'s style in `zerodds.zig`) rather than an allocator-backed
/// queue, since the Rust reference's `Vec<Submessage> out_queue` has no
/// no_std-free equivalent this SDK wants.
pub const XrceClient = struct {
    client_key: [4]u8,
    // Named `state_` (not `state`) because Zig does not allow a field and a
    // method to share a name, unlike Rust's separate field/method
    // namespaces; `state()` below is the accessor.
    state_: ClientState = .disconnected,
    transport: *const Transport,
    next_request_id: u16 = 1,
    session_id: u8 = SESSION_ID_DEFAULT,
    stream_id: u8 = STREAM_ID_DEFAULT,
    // The CREATE_CLIENT handshake always sends sequence_nr = 0 on
    // stream_id = NONE and does not consume this counter; the first
    // reliable-stream operation gets sequence_nr 0.
    next_seq: u16 = 0,
    /// Form of the `CLIENT_Representation` (DDS-XRCE 1.0 §7.8.2.1 prose vs
    /// Annex A / B.1); default `.absent`.
    representation_timestamps: RepresentationTimestamps = .absent,
    txbuf: [1024]u8 = undefined,

    pub fn init(client_key: [4]u8, transport: *const Transport) XrceClient {
        return .{ .client_key = client_key, .transport = transport };
    }

    /// `init` with the representation timestamps option.
    pub fn initWith(client_key: [4]u8, transport: *const Transport, representation_timestamps: RepresentationTimestamps) XrceClient {
        return .{ .client_key = client_key, .transport = transport, .representation_timestamps = representation_timestamps };
    }

    /// Builds this client's `CLIENT_Representation` in its
    /// `representation_timestamps` form (`out` needs 28 octets); with
    /// `.present` the timestamp is the wall clock at the time of the call.
    pub fn clientRepresentation(self: *const XrceClient, out: []u8, version: [2]u8, vendor_id: [2]u8, requested_mtu: u16) usize {
        var ts: std.posix.timespec = undefined;
        _ = std.posix.system.clock_gettime(.REALTIME, &ts);
        return encodeClientRepresentationWith(out, version, vendor_id, self.client_key, self.session_id, requested_mtu, .{
            .timestamps = self.representation_timestamps,
            .seconds = @truncate(ts.sec),
            .nanoseconds = @intCast(ts.nsec),
        });
    }

    /// `connect` with the representation built by `clientRepresentation`.
    pub fn connectDefault(self: *XrceClient, version: [2]u8, vendor_id: [2]u8, requested_mtu: u16) ClientError!void {
        var representation: [28]u8 = undefined;
        const n = self.clientRepresentation(&representation, version, vendor_id, requested_mtu);
        return self.connect(representation[0..n]);
    }

    /// Current lifecycle status.
    pub fn state(self: *const XrceClient) ClientState {
        return self.state_;
    }

    /// The ClientKey.
    pub fn clientKey(self: *const XrceClient) [4]u8 {
        return self.client_key;
    }

    /// Allocates the next request id: monotonic, wrapping, and skipping 0
    /// (mirrors Rust's `next_request_id.wrapping_add(1).max(1)`).
    fn allocRequestId(self: *XrceClient) u16 {
        const req = self.next_request_id;
        const wrapped = self.next_request_id +% 1;
        self.next_request_id = if (wrapped == 0) 1 else wrapped;
        return req;
    }

    /// Initiates the handshake: encodes and delivers CREATE_CLIENT, then
    /// moves to `.connecting`. Refuses unless `.disconnected`.
    pub fn connect(self: *XrceClient, representation: []const u8) ClientError!void {
        if (self.state_ != .disconnected) return error.InvalidState;
        const n = encodeCreateClientMessage(&self.txbuf, self.session_id, self.client_key, representation);
        const ok = self.transport.deliver(self.transport.ctx, self.txbuf[0..n]);
        if (!ok) return error.Transport;
        self.state_ = .connecting;
    }

    /// Marks the handshake complete (the caller has received STATUS_AGENT
    /// out of band). Pure bookkeeping -- no wire I/O. Refuses unless
    /// `.connecting`.
    pub fn markConnected(self: *XrceClient) ClientError!void {
        if (self.state_ != .connecting) return error.InvalidState;
        self.state_ = .connected;
    }

    /// Submits a CREATE operation for a ByReference object. Returns the
    /// request id. Refuses unless `.connected`.
    pub fn createObject(self: *XrceClient, object_id_word: u16, name: []const u8, parent_id_word: ?u16, reuse: bool, replace: bool) ClientError!u16 {
        if (self.state_ != .connected) return error.InvalidState;
        const req = self.allocRequestId();
        const n = encodeCreateMessage(
            &self.txbuf,
            self.session_id,
            self.stream_id,
            self.next_seq,
            self.client_key,
            requestIdBytes(req),
            object_id_word,
            name,
            parent_id_word,
            reuse,
            replace,
        );
        const ok = self.transport.deliver(self.transport.ctx, self.txbuf[0..n]);
        self.next_seq +%= 1;
        if (!ok) return error.Transport;
        return req;
    }

    /// Submits a DELETE operation. Returns the request id. Refuses unless
    /// `.connected`.
    pub fn deleteObject(self: *XrceClient, object_id_word: u16) ClientError!u16 {
        if (self.state_ != .connected) return error.InvalidState;
        const req = self.allocRequestId();
        const n = encodeDeleteMessage(
            &self.txbuf,
            self.session_id,
            self.stream_id,
            self.next_seq,
            self.client_key,
            requestIdBytes(req),
            object_id_word,
        );
        const ok = self.transport.deliver(self.transport.ctx, self.txbuf[0..n]);
        self.next_seq +%= 1;
        if (!ok) return error.Transport;
        return req;
    }

    /// Submits a WRITE_DATA operation. Returns the request id. Refuses
    /// unless `.connected`.
    pub fn requestWrite(self: *XrceClient, writer_object_id_word: u16, payload: []const u8) ClientError!u16 {
        if (self.state_ != .connected) return error.InvalidState;
        const req = self.allocRequestId();
        const n = encodeWriteDataMessage(
            &self.txbuf,
            self.session_id,
            self.stream_id,
            self.next_seq,
            self.client_key,
            requestIdBytes(req),
            writer_object_id_word,
            payload,
        );
        const ok = self.transport.deliver(self.transport.ctx, self.txbuf[0..n]);
        self.next_seq +%= 1;
        if (!ok) return error.Transport;
        return req;
    }

    /// Submits a READ_DATA (pull) operation. Returns the request id.
    /// Refuses unless `.connected`.
    pub fn requestRead(self: *XrceClient, reader_object_id_word: u16) ClientError!u16 {
        if (self.state_ != .connected) return error.InvalidState;
        const req = self.allocRequestId();
        const n = encodeReadDataMessage(
            &self.txbuf,
            self.session_id,
            self.stream_id,
            self.next_seq,
            self.client_key,
            requestIdBytes(req),
            reader_object_id_word,
            self.stream_id,
        );
        const ok = self.transport.deliver(self.transport.ctx, self.txbuf[0..n]);
        self.next_seq +%= 1;
        if (!ok) return error.Transport;
        return req;
    }

    /// Closes the session -- back to `.disconnected`. That is all: unlike a
    /// "helpful" reimplementation, this does NOT reset `next_request_id`,
    /// `session_id`, or `next_seq` -- matching `crates/xrce-client`'s
    /// `disconnect()`, which resets only `state`.
    pub fn disconnect(self: *XrceClient) void {
        self.state_ = .disconnected;
    }
};

// ============================ tests ============================

const testing = std.testing;

/// Read a golden fixture from the working directory.
///
/// Zig 0.16 moved the filesystem behind `std.Io`: `std.fs.cwd` is gone,
/// `std.Io.Dir.cwd` replaces it, and `readFileAlloc` now takes the `Io`
/// first, the allocator after the path, and an `Io.Limit` instead of a
/// plain byte count. `std.testing.io` is the `Io.Threaded` handle the
/// test runner sets up.
fn readFixture(name: []const u8) ![]u8 {
    var path_buf: [256]u8 = undefined;
    const path = try std.fmt.bufPrint(&path_buf, "../testdata/xrce-client/{s}", .{name});
    return std.Io.Dir.cwd().readFileAlloc(testing.io, path, testing.allocator, .limited(4096));
}

const test_key = [4]u8{ 0x01, 0x02, 0x03, 0x04 };

test "fixture: CREATE_CLIENT byte identity" {
    const golden = try readFixture("xrce_client_create_client.bin");
    defer testing.allocator.free(golden);
    var representation: [16]u8 = undefined;
    _ = encodeClientRepresentation(&representation, .{ 0x01, 0x00 }, .{ 0x00, 0x00 }, test_key, 0x01, DEFAULT_MTU);
    var out: [64]u8 = undefined;
    const n = encodeCreateClientMessage(&out, 0x01, test_key, &representation);
    try testing.expectEqualSlices(u8, golden, out[0..n]);
    try testing.expectEqual(@as(u8, 0x00), out[0]);

    // Requested session >= 128: header session 0x80, no client key.
    _ = encodeClientRepresentation(&representation, .{ 0x01, 0x00 }, .{ 0x00, 0x00 }, test_key, 0x81, DEFAULT_MTU);
    const n2 = encodeCreateClientMessage(&out, 0x81, test_key, &representation);
    try testing.expectEqual(@as(usize, 24), n2);
    try testing.expectEqual(@as(u8, 0x80), out[0]);
}

test "CLIENT_Representation: representation timestamps" {
    const want_absent = [_]u8{ 0x58, 0x52, 0x43, 0x45, 1, 0, 0, 0, 1, 2, 3, 4, 1, 0, 0xFC, 1 };
    const want_app = [_]u8{ 0x58, 0x52, 0x43, 0x45, 1, 0, 0, 0, 8, 0, 0, 0, 0x8C, 0xAA, 0x88, 0x5A, 0x00, 0x65, 0xCD, 0x1D, 1, 2, 3, 4, 1, 0, 0xFC, 1 };
    const want_final = [_]u8{ 0x58, 0x52, 0x43, 0x45, 1, 0, 0, 0, 0x8C, 0xAA, 0x88, 0x5A, 0x00, 0x65, 0xCD, 0x1D, 1, 2, 3, 4, 1, 0, 0xFC, 1 };
    var out: [28]u8 = undefined;
    // Default stays absent.
    var n = encodeClientRepresentation(&out, .{ 1, 0 }, .{ 0, 0 }, test_key, 0x01, DEFAULT_MTU);
    try testing.expectEqualSlices(u8, &want_absent, out[0..n]);
    n = encodeClientRepresentationWith(&out, .{ 1, 0 }, .{ 0, 0 }, test_key, 0x01, 508, .{});
    try testing.expectEqualSlices(u8, &want_absent, out[0..n]);
    n = encodeClientRepresentationWith(&out, .{ 1, 0 }, .{ 0, 0 }, test_key, 0x01, 508, .{
        .timestamps = .present,
        .seconds = 1518905996,
        .nanoseconds = 500000000,
    });
    try testing.expectEqualSlices(u8, &want_app, out[0..n]);
    n = encodeClientRepresentationWith(&out, .{ 1, 0 }, .{ 0, 0 }, test_key, 0x01, 508, .{
        .timestamps = .present,
        .seconds = 1518905996,
        .nanoseconds = 500000000,
        .unannotated_types = .final,
    });
    try testing.expectEqualSlices(u8, &want_final, out[0..n]);
}

// session_id = 0x01 throughout (<=0x7F, so every header carries the
// ClientKey). CREATE_CLIENT travels on STREAM_NONE, seq 0, with header
// session id 0x00 (SESSION_ID_NONE_WITH_CLIENT_KEY, DDS-XRCE 1.0 §8.3.2.1;
// the payload carries session_id 0x01). The four
// post-handshake operations are all reliable, so they share
// STREAM_BUILTIN_RELIABLE (0x80) and a real, strictly increasing sequence
// number (0..3) -- not the best-effort stream with hand-picked numbers a
// previous fixture set used.
test "fixture: CREATE byte identity" {
    const golden = try readFixture("xrce_client_create.bin");
    defer testing.allocator.free(golden);
    var out: [64]u8 = undefined;
    const object_id = packObjectId(0x001, .data_writer);
    const publisher_id = packObjectId(0x001, .publisher);
    const n = encodeCreateMessage(&out, 0x01, STREAM_BUILTIN_RELIABLE, 0, test_key, .{ 0x00, 0x01 }, object_id, "SensorWriter", publisher_id, false, false);
    try testing.expectEqualSlices(u8, golden, out[0..n]);
}

test "fixture: DELETE byte identity" {
    const golden = try readFixture("xrce_client_delete.bin");
    defer testing.allocator.free(golden);
    var out: [64]u8 = undefined;
    const object_id = packObjectId(0x001, .data_writer);
    const n = encodeDeleteMessage(&out, 0x01, STREAM_BUILTIN_RELIABLE, 1, test_key, .{ 0x00, 0x02 }, object_id);
    try testing.expectEqualSlices(u8, golden, out[0..n]);
}

test "fixture: READ_DATA byte identity" {
    const golden = try readFixture("xrce_client_read_data.bin");
    defer testing.allocator.free(golden);
    var out: [64]u8 = undefined;
    const object_id = packObjectId(0x002, .data_reader);
    const n = encodeReadDataMessage(&out, 0x01, STREAM_BUILTIN_RELIABLE, 2, test_key, .{ 0x00, 0x03 }, object_id, 0x01);
    try testing.expectEqualSlices(u8, golden, out[0..n]);
}

test "fixture: WRITE_DATA byte identity" {
    const golden = try readFixture("xrce_client_write_data.bin");
    defer testing.allocator.free(golden);
    var out: [64]u8 = undefined;
    const object_id = packObjectId(0x001, .data_writer);
    const n = encodeWriteDataMessage(&out, 0x01, STREAM_BUILTIN_RELIABLE, 3, test_key, .{ 0x00, 0x04 }, object_id, &[_]u8{ 0x19, 0x00 });
    try testing.expectEqualSlices(u8, golden, out[0..n]);
}

// A second CREATE, appended to the same session: request_id=[0x00,0x05],
// object_id=pack(raw=0x010,kind=topic), the real two-level
// ObjectVariant::ByReference("HelloWorldTopic") under
// parent=pack(raw=0x001,kind=participant), sequence_nr=4. OBJK_TOPIC's own
// representation carries its participant's ObjectId (commit 31c62df2a63,
// "xrce: OBJK_TOPIC_Representation carries its participant's ObjectId"),
// the same trailing field OBJK_PUBLISHER's representation carries above --
// this fixture is generated the same way (endpoints/golden-gen), not a
// hand-picked byte string.
test "fixture: CREATE(topic) byte identity" {
    const golden = try readFixture("xrce_client_create_topic.bin");
    defer testing.allocator.free(golden);
    var out: [64]u8 = undefined;
    const topic_id = packObjectId(0x010, .topic);
    const participant_id = packObjectId(0x001, .participant);
    const n = encodeCreateMessage(&out, 0x01, STREAM_BUILTIN_RELIABLE, 4, test_key, .{ 0x00, 0x05 }, topic_id, "HelloWorldTopic", participant_id, false, false);
    try testing.expectEqualSlices(u8, golden, out[0..n]);
}

// Small in-memory FIFO transport for the state-machine tests, local to this
// file (not exported from `zerodds.zig`) -- same ring-buffer pattern as the
// private `Fifo` in `zerodds.zig`'s own test section.
const Fifo = struct {
    frames: [16][256]u8 = undefined,
    lens: [16]usize = undefined,
    head: usize = 0,
    tail: usize = 0,
    count: usize = 0,

    fn deliver(ctx: *anyopaque, frame: []const u8) bool {
        const f: *Fifo = @ptrCast(@alignCast(ctx));
        if (f.count == 16) return false;
        std.mem.copyForwards(u8, f.frames[f.tail][0..frame.len], frame);
        f.lens[f.tail] = frame.len;
        f.tail = (f.tail + 1) % 16;
        f.count += 1;
        return true;
    }
    fn receive(ctx: *anyopaque, buf: []u8) ?usize {
        const f: *Fifo = @ptrCast(@alignCast(ctx));
        if (f.count == 0) return null;
        const l = f.lens[f.head];
        std.mem.copyForwards(u8, buf[0..l], f.frames[f.head][0..l]);
        f.head = (f.head + 1) % 16;
        f.count -= 1;
        return l;
    }
};

fn testKey() [4]u8 {
    return .{ 0xAB, 0xAB, 0xAB, 0xAB };
}

test "new client starts disconnected" {
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    const c = XrceClient.init(testKey(), &t);
    try testing.expectEqual(ClientState.disconnected, c.state());
    try testing.expectEqualSlices(u8, &testKey(), &c.clientKey());
}

test "connect transitions to connecting" {
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var c = XrceClient.init(testKey(), &t);
    try c.connect(&[_]u8{ 0x58, 0x52, 0x43, 0x45 });
    try testing.expectEqual(ClientState.connecting, c.state());
}

test "mark connected transitions to connected" {
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var c = XrceClient.init(testKey(), &t);
    try c.connect(&[_]u8{});
    try c.markConnected();
    try testing.expectEqual(ClientState.connected, c.state());
}

test "double connect rejected" {
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var c = XrceClient.init(testKey(), &t);
    try c.connect(&[_]u8{});
    try testing.expectError(error.InvalidState, c.connect(&[_]u8{}));
}

test "create without connect rejected" {
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var c = XrceClient.init(testKey(), &t);
    const oid = packObjectId(0x010, .topic);
    try testing.expectError(error.InvalidState, c.createObject(oid, "topic-ref", null, false, false));
}

test "write without connect rejected" {
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var c = XrceClient.init(testKey(), &t);
    const oid = packObjectId(0x010, .data_writer);
    try testing.expectError(error.InvalidState, c.requestWrite(oid, "x"));
}

test "read without connect rejected" {
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var c = XrceClient.init(testKey(), &t);
    const oid = packObjectId(0x010, .data_reader);
    try testing.expectError(error.InvalidState, c.requestRead(oid));
}

test "full lifecycle creates unique request ids" {
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var c = XrceClient.init(testKey(), &t);
    try c.connect(&[_]u8{});
    try c.markConnected();
    const oid = packObjectId(0x010, .data_writer);
    const r1 = try c.createObject(oid, "a", null, false, false);
    const r2 = try c.deleteObject(oid);
    const r3 = try c.requestWrite(oid, "x");
    const r4 = try c.requestRead(oid);
    // Request ids are strictly monotonic.
    try testing.expectEqual(@as(u16, 1), r1);
    try testing.expectEqual(@as(u16, 2), r2);
    try testing.expectEqual(@as(u16, 3), r3);
    try testing.expectEqual(@as(u16, 4), r4);
}

test "disconnect clears state" {
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var c = XrceClient.init(testKey(), &t);
    try c.connect(&[_]u8{});
    try c.markConnected();
    c.disconnect();
    try testing.expectEqual(ClientState.disconnected, c.state());
}

test "transport failure surfaces as ClientError.Transport" {
    // A full 16-slot Fifo makes `deliver` return false -- the one place
    // Zig's Transport has a failure channel the Nim/Lua ports lack.
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var i: usize = 0;
    while (i < 16) : (i += 1) try testing.expect(Fifo.deliver(&fifo, &[_]u8{0x00}));
    var c = XrceClient.init(testKey(), &t);
    try testing.expectError(error.Transport, c.connect(&[_]u8{}));
    // connect() checked state before attempting delivery and only flips to
    // .connecting on success, so a failed attempt leaves it .disconnected.
    try testing.expectEqual(ClientState.disconnected, c.state());
}

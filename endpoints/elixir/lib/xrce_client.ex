# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# DDS-XRCE 1.0 client, ported from `crates/xrce-client`. Three modules:
#
#   ZeroDDS.XrceObjectKind — the 4-bit OBJK_* codes (Spec §7.2 Table 4).
#   ZeroDDS.XrceObjectId   — the packed 16-bit ObjectId word (Spec §7.2.1).
#   ZeroDDS.XrceWire       — pure wire encoders (no state, no I/O) for
#                            MessageHeader, SubmessageHeader,
#                            BaseObjectRequest, ObjectVariant::ByReference,
#                            and the five submessage bodies this client
#                            speaks (CREATE_CLIENT/CREATE/DELETE/
#                            WRITE_DATA/READ_DATA).
#   ZeroDDS.XrceClient     — a synchronous state machine mirroring
#                            `crates/xrce-client::XrceClient` 1:1 in its
#                            lifecycle rules (Disconnected -> Connecting ->
#                            Connected, monotonic request ids, `disconnect`
#                            resetting only `state`).
#
# This reuses the SDK's existing `%{deliver: fun/1, receive: fun/0}` transport
# shape from `zerodds.ex` as-is. It is additive: `ZeroDDS.Client`/
# `ZeroDDS.AsyncReader` speak an older, in-house, non-conformant frame on
# purpose; this module is the conformant DDS-XRCE 1.0 sibling that can talk
# to a real agent.
#
# One difference from the Rust reference is worth flagging up front: in
# `crates/xrce-client`, `create_object`/`delete_object`/`request_write`/
# `request_read` take unused parameters and never touch the wire — they are
# pure request-id bookkeeping, with encoding left to a caller that does not
# exist yet. This port closes that gap: every op here actually encodes its
# submessage with `ZeroDDS.XrceWire` and calls `transport.deliver`, because
# "reach a real XRCE agent over the network" is the entire point of the port.
# The state-machine *rules* (refusals, transitions, id allocation, what
# `disconnect` resets) are unchanged.
#
# Idiom: `ZeroDDS.XrceClient` is immutable data (a struct), like every other
# value in Elixir — an operation on it returns a *new* client alongside its
# result, and the caller rebinds it (`{:ok, c} = XrceClient.connect(c, ...)`),
# the same way `ZeroDDS.Client.write/2` in `zerodds.ex` already does. Refusals
# are `{:error, :invalid_state}` tuples, not exceptions: every operation
# pattern-matches on `%__MODULE__{state: ...}` to decide whether it applies at
# all, so the state machine's own transition rules are expressed as function
# clauses rather than as `if` chains.

defmodule ZeroDDS.XrceObjectKind do
  @moduledoc "The 4-bit OBJK_* codes (Spec §7.2 Table 4). Values are the wire codes themselves, not sequential — 0x07..0x09 have no spec meaning."

  def participant, do: 1
  def topic, do: 2
  def publisher, do: 3
  def subscriber, do: 4
  def datawriter, do: 5
  def datareader, do: 6
  def type, do: 10
  def qosprofile, do: 11
  def application, do: 12
  def agent, do: 13
  def client, do: 14
  def domain, do: 15
end

defmodule ZeroDDS.XrceObjectId do
  @moduledoc """
  `ObjectId` (Spec §7.2.1): a 16-bit word, `kind` in the low 4 bits and
  `raw_id` in the high 12 — represented here as the plain integer word
  itself, since Elixir has no fixed-width type to wrap it in.
  """

  import Bitwise

  @doc "`word = (raw_id <<< 4) ||| kind`."
  def pack(raw_id, kind)
      when is_integer(raw_id) and raw_id in 0..0x0FFF and is_integer(kind) and kind in 0..0x0F do
    (raw_id <<< 4) ||| kind
  end

  @doc "Constructs directly from an already-packed 16-bit word (e.g. a value read off the wire, or a well-known singleton like OBJECTID_AGENT)."
  def from_raw_word(word) when is_integer(word) and word in 0..0xFFFF, do: word

  @doc "Wire encoding: big-endian — the one field in this spec that is *not* little-endian (everything else in a DDS-XRCE message is LE)."
  def to_bytes(word), do: <<word::16>>
end

defmodule ZeroDDS.XrceWire do
  @moduledoc """
  Pure wire encoders (no state, no I/O) for the five submessage bodies
  `ZeroDDS.XrceClient` speaks. Reused by that module; also usable directly by
  anything that wants to build a DDS-XRCE 1.0 message without the state
  machine (e.g. tests pinning a specific request_id/sequence_nr).
  """

  import Bitwise
  alias ZeroDDS.XrceObjectId

  @flag_e_little_endian 0x01
  @sm_create_client 0x00
  @sm_create 0x01
  @sm_delete 0x03
  @sm_write_data 0x07
  @sm_read_data 0x08
  # `session_id <= 0x7F` carries a client_key (8-byte header); `>= 0x80` does
  # not (4-byte header). Reserved: SESSION_NONE_WITH_KEY = 0x00,
  # SESSION_NONE_WITHOUT_KEY = 0x80.
  @session_carries_key_max 0x7F

  @doc "MessageHeader (Spec §8.3.2): session_id, stream_id, sequence_nr (LE), and client_key — present only when session_id <= 0x7F."
  def encode_message_header(session_id, stream_id, sequence_nr, client_key)
      when byte_size(client_key) == 4 do
    hdr = <<session_id::8, stream_id::8, sequence_nr::little-16>>
    if session_id <= @session_carries_key_max, do: hdr <> client_key, else: hdr
  end

  @doc "SubmessageHeader (Spec §8.3.4): id, flags, and the body's byte length — always little-endian regardless of the body's own byte order."
  def encode_submessage_header(id, flags, body_len) do
    <<id::8, flags::8, body_len::little-16>>
  end

  @doc "BaseObjectRequest (Spec §7.7.8): `request_id` passes through as the two raw bytes the caller supplies, followed by the (already-encoded, big-endian) object_id."
  def encode_base_object_request(request_id_bytes, object_id_bytes)
      when byte_size(request_id_bytes) == 2 and byte_size(object_id_bytes) == 2 do
    request_id_bytes <> object_id_bytes
  end

  @doc "ObjectVariant::ByReference's *inner* payload only (Spec §7.7.2): discriminator 0x01, three padding bytes to a 4-byte boundary (correct only when this structure itself starts 4-aligned), then an XCDR2 string — a 4-byte LE length that includes the NUL terminator, the name bytes, and the NUL terminator itself. Not the wire form a real agent accepts on its own — see `encode_object_variant_for_kind/3`, whose 1-byte outer ObjectKind shifts this padding to two bytes."
  def encode_by_reference(name) do
    n = byte_size(name) + 1
    <<0x01, 0, 0, 0>> <> <<n::little-32>> <> name <> <<0>>
  end

  @doc "The real wire ObjectVariant (Spec §7.7.3): a **two-level** discriminated union — the outer ObjectKind byte (`object_id`'s own low 4 bits — the outer discriminator and the object being created always agree, by construction), then the inner REPRESENTATION_BY_REFERENCE payload (disc 0x01, two padding bytes — the outer kind byte already took the first of the four — then the same XCDR2 string as `encode_by_reference/1`), then that kind's own trailing field: `parent_object_id` (an ObjectId, unaligned) for OBJK_TOPIC/OBJK_PUBLISHER/OBJK_SUBSCRIBER/OBJK_DATAWRITER/OBJK_DATAREADER, `nil` for every other kind — byte-oracled in `crates/xrce/src/object_repr.rs` against real eProsima Micro-XRCE-DDS Client 3.0.1 CREATE traffic (OBJK_TOPIC's own participant_id: commit 31c62df2a63). This — not `encode_by_reference/1` alone — is the only form a real agent accepts."
  def encode_object_variant_for_kind(object_id, name, parent_object_id \\ nil) do
    kind_byte = <<(object_id &&& 0x0F)::8>>
    n = byte_size(name) + 1
    inner = <<0x01, 0, 0>> <> <<n::little-32>> <> name <> <<0>>
    parent = if parent_object_id, do: XrceObjectId.to_bytes(parent_object_id), else: <<>>
    kind_byte <> inner <> parent
  end

  # header ++ submessage_header ++ body — every operation this client sends
  # is exactly one submessage per message, so the inter-submessage alignment
  # padding rule (Spec §8.3.3) never applies here.
  defp encode_message(session_id, stream_id, sequence_nr, client_key, submessage_id, flags, body) do
    encode_message_header(session_id, stream_id, sequence_nr, client_key) <>
      encode_submessage_header(submessage_id, flags, byte_size(body)) <> body
  end

  # eProsima's own hard datagram ceiling (`crates/xrce/src/handshake.rs`'s
  # module doc has the byte-oracle measurement backing it), used as this
  # port's own CREATE_CLIENT requested_mtu.
  @default_mtu 508

  @doc "The MTU this client requests in its `CLIENT_Representation` (508)."
  def default_mtu, do: @default_mtu

  @doc """
  `CLIENT_Representation` (Annex A, Spec §7.8.2.1): xrce_cookie(4) ++ xrce_version(2) ++ xrce_vendor_id(2) ++ client_key(4) ++ session_id(1) ++ a 0 properties-absent octet ++ requested_mtu(2, LE) — eProsima's own non-normative MTU extension, always sent by this client. No PropertySeq support here — this client sends none.

  Option `representation_timestamps:` (`:absent` default | `:present`). DDS-XRCE 1.0 contradicts itself: the prose of 7.8.2.1 lists `Time_t client_timestamp` after `xrce_vendor_id`, the IDL of Annex A and the bytes of Annex B.1 do not have it. `:absent` writes the Annex A form (16 octets with the defaults). `:present` writes the Time_t after `xrce_vendor_id`: seconds i32 and nanoseconds u32, little-endian, taken from option `timestamp:` (`{seconds, nanoseconds}`, default: wall clock at the time of the call). `Time_t` carries no `@extensibility`, so it is APPENDABLE and gets a DHEADER (u32 = 8, 28 octets in all); option `unannotated_types: :final` (same values as in `ZeroDDS.Reliable`) leaves the DHEADER out (24 octets). The agent has to be set the same way (`representation_timestamps:` in its configuration).
  """
  def encode_client_representation(version, vendor_id, client_key, session_id, requested_mtu \\ @default_mtu, opts \\ []) do
    head = "XRCE" <> version <> vendor_id

    time =
      case Keyword.get(opts, :representation_timestamps, :absent) do
        :absent ->
          <<>>

        :present ->
          {sec, nsec} = Keyword.get_lazy(opts, :timestamp, &wall_clock_time/0)
          body = <<sec::little-signed-32, nsec::little-unsigned-32>>

          case Keyword.get(opts, :unannotated_types, :appendable) do
            :appendable -> <<8::little-32>> <> body
            :final -> body
            _ -> raise ArgumentError, "unannotated_types must be :appendable or :final"
          end

        _ ->
          raise ArgumentError, "representation_timestamps must be :absent or :present"
      end

    head <> time <> client_key <> <<session_id::8, 0>> <> <<requested_mtu::little-16>>
  end

  defp wall_clock_time do
    ns = System.os_time(:nanosecond)
    {div(ns, 1_000_000_000), rem(ns, 1_000_000_000)}
  end

  @doc "Spec §7.8.2 (CREATE_CLIENT, `XrceClient.connect/2`). `session_id` is the id this client asks for (it belongs in the `CLIENT_Representation`). The header carries SESSION_ID_NONE_WITH_CLIENT_KEY (0x00, with the key) when it is 0..127 and SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80, no key) when it is 128..255 (DDS-XRCE 1.0 §8.3.2.1), stream=NONE(0x00), sequence_nr=0 — the handshake precedes any session. `representation` is the XCDR2-encoded `CLIENT_Representation` — build it with `encode_client_representation/5` — passed through verbatim here."
  def encode_create_client_message(session_id, client_key, representation) do
    header_session_id = if session_id < 128, do: 0x00, else: 0x80
    encode_message(header_session_id, 0x00, 0, client_key, @sm_create_client, @flag_e_little_endian, representation)
  end

  @doc "Spec §7.8.3 (CREATE, `XrceClient.create_object/6`): BaseObjectRequest ++ the real, two-level ObjectVariant (`encode_object_variant_for_kind/3`). Flags bit 1 = reuse, bit 2 = replace."
  def encode_create_message(
        session_id,
        stream_id,
        sequence_nr,
        client_key,
        request_id_bytes,
        object_id,
        name,
        parent_object_id \\ nil,
        reuse \\ false,
        replace \\ false
      ) do
    flags = @flag_e_little_endian
    flags = if reuse, do: flags ||| 0x02, else: flags
    flags = if replace, do: flags ||| 0x04, else: flags

    body =
      encode_base_object_request(request_id_bytes, XrceObjectId.to_bytes(object_id)) <>
        encode_object_variant_for_kind(object_id, name, parent_object_id)

    encode_message(session_id, stream_id, sequence_nr, client_key, @sm_create, flags, body)
  end

  @doc "Spec §7.8.3 (DELETE, `XrceClient.delete_object/2`): BaseObjectRequest only — no ObjectVariant."
  def encode_delete_message(session_id, stream_id, sequence_nr, client_key, request_id_bytes, object_id) do
    body = encode_base_object_request(request_id_bytes, XrceObjectId.to_bytes(object_id))
    encode_message(session_id, stream_id, sequence_nr, client_key, @sm_delete, @flag_e_little_endian, body)
  end

  @doc "Spec §7.8.4 (WRITE_DATA, `XrceClient.request_write/3`): BaseObjectRequest ++ serialized_data verbatim, with no length prefix of its own — the sample is however many bytes are left in the submessage body, and `submessage_length` is what bounds it (DDS-XRCE Annex B.3.1: 4 octets of BaseObjectRequest + 2 octets of sample is submessageLength=6, not 10 — a `sequence<octet>` length would add four more)."
  def encode_write_data_message(
        session_id,
        stream_id,
        sequence_nr,
        client_key,
        request_id_bytes,
        object_id,
        serialized_data
      ) do
    body = encode_base_object_request(request_id_bytes, XrceObjectId.to_bytes(object_id)) <> serialized_data
    encode_message(session_id, stream_id, sequence_nr, client_key, @sm_write_data, @flag_e_little_endian, body)
  end

  @doc "Spec §7.8.5 (READ_DATA, `XrceClient.request_read/2`): BaseObjectRequest ++ ReadSpecification. This client never sets a content filter or delivery control, so ReadSpecification is always: preferred_stream_id, data_format = Data (0x00), no filter, no delivery control."
  def encode_read_data_message(
        session_id,
        stream_id,
        sequence_nr,
        client_key,
        request_id_bytes,
        object_id,
        preferred_stream_id
      ) do
    read_spec = <<preferred_stream_id::8, 0, 0, 0>>
    body = encode_base_object_request(request_id_bytes, XrceObjectId.to_bytes(object_id)) <> read_spec
    encode_message(session_id, stream_id, sequence_nr, client_key, @sm_read_data, @flag_e_little_endian, body)
  end
end

defmodule ZeroDDS.XrceClient do
  @moduledoc """
  DDS-XRCE 1.0 client: a synchronous state machine mirroring
  `crates/xrce-client::XrceClient` (Spec §7.2 lifecycle: Disconnected ->
  Connecting -> Connected). Immutable, like every Elixir value — each
  operation takes the current client and returns `{:ok, new_client}` (or
  `{:ok, request_id, new_client}` for the four request-bearing ops), or
  `{:error, :invalid_state}` when the lifecycle rule refuses the call. The
  refusal path is a function clause, not an `if`: every op has one clause
  guarded on `%__MODULE__{state: ...}` for the state it requires, and a
  catch-all clause beneath it that always answers `{:error, :invalid_state}`.
  """

  alias ZeroDDS.XrceWire

  defstruct client_key: nil,
            state: :disconnected,
            transport: nil,
            next_request_id: 1,
            # Defaults match the fixtures: session_id=0x01 (carries
            # client_key), stream_id=0x80 (builtin-reliable — every
            # post-handshake operation is reliable). next_seq starts at 0 —
            # the CREATE_CLIENT handshake always sends sequence_nr=0 on
            # stream_id=NONE and does not consume this counter; the first
            # reliable-stream operation gets sequence_nr 0.
            session_id: 0x01,
            stream_id: 0x80,
            next_seq: 0,
            representation_timestamps: :absent

  @doc "Constructor. `state = :disconnected`; `next_request_id` starts at 1. `transport` is the SDK's existing `%{deliver: fun/1, receive: fun/0}` shape (e.g. `ZeroDDS.MemTransport.new/0`) — no new transport type is introduced here. Option `representation_timestamps:` (`:absent` default | `:present`) selects the form of the `CLIENT_Representation` that `client_representation/4` builds (DDS-XRCE 1.0 7.8.2.1 prose vs. Annex A / B.1)."
  def new(client_key, transport, opts \\ []) when byte_size(client_key) == 4 do
    form = Keyword.get(opts, :representation_timestamps, :absent)

    if form not in [:absent, :present],
      do: raise(ArgumentError, "representation_timestamps must be :absent or :present")

    %__MODULE__{client_key: client_key, transport: transport, representation_timestamps: form}
  end

  @doc "Sets the `representation_timestamps` form (`:absent` | `:present`) of this client."
  def set_representation_timestamps(%__MODULE__{} = c, form) when form in [:absent, :present],
    do: %{c | representation_timestamps: form}

  @doc "Builds this client's `CLIENT_Representation` for `connect/2` from its own session id, key and `representation_timestamps` form. With `:present` the timestamp is the wall clock at the time of the call."
  def client_representation(%__MODULE__{} = c, version, vendor_id, requested_mtu \\ XrceWire.default_mtu()) do
    XrceWire.encode_client_representation(version, vendor_id, c.client_key, c.session_id, requested_mtu,
      representation_timestamps: c.representation_timestamps
    )
  end

  defp alloc_request_id(c) do
    # The Rust reference's `next_request_id.wrapping_add(1).max(1)`: wrap a
    # u16 and skip 0 — request_id 0 is never handed out once the counter has
    # started.
    req = c.next_request_id
    next = rem(c.next_request_id + 1, 0x10000)
    next = if next == 0, do: 1, else: next
    {req, %{c | next_request_id: next}}
  end

  # Big-endian encoding of the auto-allocated request id — matching
  # `XrceObjectId.to_bytes/1` (also big-endian) so both halves of a
  # BaseObjectRequest use the same byte order (id 1 -> `<<0x00, 0x01>>`,
  # matching the same choice the other ports made).
  defp request_id_bytes(id), do: <<id::16>>

  @doc "Initiates the handshake (Spec §8.4.5): encodes and delivers CREATE_CLIENT, then transitions to `:connecting`."
  def connect(%__MODULE__{state: :disconnected} = c, representation) do
    frame = XrceWire.encode_create_client_message(c.session_id, c.client_key, representation)
    c.transport.deliver.(frame)
    {:ok, %{c | state: :connecting}}
  end

  def connect(%__MODULE__{}, _representation), do: {:error, :invalid_state}

  @doc "Marks the handshake complete — the caller has externally received STATUS_AGENT. Pure bookkeeping, no wire I/O: matches `crates/xrce-client::mark_connected` exactly."
  def mark_connected(%__MODULE__{state: :connecting} = c), do: {:ok, %{c | state: :connected}}
  def mark_connected(%__MODULE__{}), do: {:error, :invalid_state}

  @doc "Submits a CREATE operation (Spec §7.8.3) for an `ObjectVariant::ByReference(name)`, under an optional parent object (that kind's own trailing field, Spec §7.7.3). Returns `{:ok, request_id, new_client}`."
  def create_object(c, object_id, name, parent_object_id \\ nil, reuse \\ false, replace \\ false)

  def create_object(%__MODULE__{state: :connected} = c, object_id, name, parent_object_id, reuse, replace) do
    {req, c1} = alloc_request_id(c)

    frame =
      XrceWire.encode_create_message(
        c.session_id,
        c.stream_id,
        c.next_seq,
        c.client_key,
        request_id_bytes(req),
        object_id,
        name,
        parent_object_id,
        reuse,
        replace
      )

    c.transport.deliver.(frame)
    {:ok, req, %{c1 | next_seq: c.next_seq + 1}}
  end

  def create_object(%__MODULE__{}, _object_id, _name, _parent_object_id, _reuse, _replace),
    do: {:error, :invalid_state}

  @doc "Submits a DELETE operation (Spec §7.8.3). Returns `{:ok, request_id, new_client}`."
  def delete_object(%__MODULE__{state: :connected} = c, object_id) do
    {req, c1} = alloc_request_id(c)
    frame = XrceWire.encode_delete_message(c.session_id, c.stream_id, c.next_seq, c.client_key, request_id_bytes(req), object_id)
    c.transport.deliver.(frame)
    {:ok, req, %{c1 | next_seq: c.next_seq + 1}}
  end

  def delete_object(%__MODULE__{}, _object_id), do: {:error, :invalid_state}

  @doc "Submits a WRITE_DATA operation (Spec §7.8.4). Returns `{:ok, request_id, new_client}`."
  def request_write(%__MODULE__{state: :connected} = c, writer_object_id, payload) do
    {req, c1} = alloc_request_id(c)

    frame =
      XrceWire.encode_write_data_message(
        c.session_id,
        c.stream_id,
        c.next_seq,
        c.client_key,
        request_id_bytes(req),
        writer_object_id,
        payload
      )

    c.transport.deliver.(frame)
    {:ok, req, %{c1 | next_seq: c.next_seq + 1}}
  end

  def request_write(%__MODULE__{}, _writer_object_id, _payload), do: {:error, :invalid_state}

  @doc "Submits a READ_DATA operation (Spec §7.8.5, pull-based). `preferred_stream_id` is the client's own `stream_id`, `data_format` is always Data. Returns `{:ok, request_id, new_client}`."
  def request_read(%__MODULE__{state: :connected} = c, reader_object_id) do
    {req, c1} = alloc_request_id(c)

    frame =
      XrceWire.encode_read_data_message(
        c.session_id,
        c.stream_id,
        c.next_seq,
        c.client_key,
        request_id_bytes(req),
        reader_object_id,
        c.stream_id
      )

    c.transport.deliver.(frame)
    {:ok, req, %{c1 | next_seq: c.next_seq + 1}}
  end

  def request_read(%__MODULE__{}, _reader_object_id), do: {:error, :invalid_state}

  @doc "Closes the session — goes back to `:disconnected`. That is all: matches `crates/xrce-client::disconnect` exactly, which resets only `state`. `next_request_id`, `session_id`, `stream_id`, and `next_seq` are deliberately left alone — a \"helpful\" reset of more than that would be wrong."
  def disconnect(c), do: %{c | state: :disconnected}
end

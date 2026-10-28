(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   Tests for the DDS-XRCE 1.0 client in `xrce_client.ml`:

     1. Byte-identity of the five Layer 1 message encoders against the Rust-
        generated fixtures in `../testdata/xrce-client/*.bin`.
     2. The state machine's lifecycle rules, translated from
        `crates/xrce-client/src/lib.rs`'s own `#[cfg(test)] mod tests`.

   Built alongside zerodds.ml and xrce_client.ml (threads.posix), the same
   recipe `test.ml` uses. *)

let fixture name =
  let ic = open_in_bin (Filename.concat "../testdata/xrce-client" (name ^ ".bin")) in
  let n = in_channel_length ic in
  let b = Bytes.create n in
  really_input ic b 0 n;
  close_in ic;
  b

let key1234 = Bytes.of_string "\x01\x02\x03\x04"

(* session_id = 0x01 throughout (<=0x7F, so every header except CREATE_CLIENT's carries the
   ClientKey). CREATE_CLIENT travels on stream=NONE, seq 0. The four
   post-handshake operations are all reliable, so they share stream=0x80
   (builtin-reliable) and a real, strictly increasing sequence number
   (0..3) — not the best-effort stream with hand-picked numbers a
   previous fixture set used. *)
let fixture_tests () =
  (* 1. CREATE_CLIENT: a real CLIENT_Representation (version={1,0},
     vendor_id={0,0}, this client_key, session_id=0x01, no properties,
     requested_mtu=508). *)
  let repr =
    Xrce_client.encode_client_representation ~version:(Bytes.of_string "\x01\x00")
      ~vendor_id:(Bytes.of_string "\x00\x00") ~client_key:key1234 ~session_id:0x01 ()
  in
  let got = Xrce_client.encode_create_client_message 0x01 key1234 repr in
  if got <> fixture "xrce_client_create_client" then
    failwith "xrce_client_create_client.bin: not byte-identical";
  (* representation timestamps (7.8.2.1 prose vs Annex A / B.1) *)
  (let v = Bytes.of_string "\x01\x00" and vd = Bytes.of_string "\x00\x00" in
   let ts = (1518905996, 500000000) in
   let enc ?representation_timestamps ?unannotated_types () =
     Bytes.to_string
       (Xrce_client.encode_client_representation ~version:v ~vendor_id:vd ~client_key:key1234
          ~session_id:0x01 ?representation_timestamps ~timestamp:ts ?unannotated_types ())
   in
   let want_absent = "XRCE\x01\x00\x00\x00\x01\x02\x03\x04\x01\x00\xfc\x01" in
   let want_app =
     "XRCE\x01\x00\x00\x00\x08\x00\x00\x00\x8c\xaa\x88\x5a\x00\x65\xcd\x1d\x01\x02\x03\x04\x01\x00\xfc\x01"
   in
   let want_final =
     "XRCE\x01\x00\x00\x00\x8c\xaa\x88\x5a\x00\x65\xcd\x1d\x01\x02\x03\x04\x01\x00\xfc\x01"
   in
   if Bytes.to_string repr <> want_absent then failwith "default representation must stay absent";
   if enc () <> want_absent then failwith "representation absent";
   if enc ~representation_timestamps:Xrce_client.Present () <> want_app then
     failwith "representation present appendable";
   if enc ~representation_timestamps:Xrce_client.Present ~unannotated_types:Reliable.Final ()
      <> want_final
   then failwith "representation present final";
   let cl = Xrce_client.create ~representation_timestamps:Xrce_client.Present key1234 (Zerodds.MemTransport.create ()) in
   if Bytes.length (Xrce_client.client_representation cl ~version:v ~vendor_id:vd ()) <> 28 then
     failwith "client representation present: 28 octets");
  (* DDS-XRCE 1.0 Sec 8.3.2.1: requested session id >= 128 -> header 0x80,
     no client key. *)
  let repr_hi =
    Xrce_client.encode_client_representation ~version:(Bytes.of_string "\x01\x00")
      ~vendor_id:(Bytes.of_string "\x00\x00") ~client_key:key1234 ~session_id:0x81 ()
  in
  let got_hi = Xrce_client.encode_create_client_message 0x81 key1234 repr_hi in
  if Bytes.get_uint8 got_hi 0 <> 0x80 then
    failwith "CREATE_CLIENT with session 0x81: header session id must be 0x80";
  if Bytes.length got_hi <> Bytes.length got - 4 then
    failwith "CREATE_CLIENT with session 0x81: header must carry no client key";

  (* 2. CREATE: request_id=[0x00,0x01], object_id=pack(raw=0x001,kind=Datawriter),
     the real two-level ObjectVariant::ByReference("SensorWriter") under
     parent=pack(raw=0x001,kind=Publisher), reuse=false, replace=false. *)
  let oid = Xrce_client.pack_object_id 0x001 Xrce_client.Datawriter in
  let publisher_oid = Xrce_client.pack_object_id 0x001 Xrce_client.Publisher in
  let got =
    Xrce_client.encode_create_message 0x01 0x80 0 key1234 (0x00, 0x01) oid "SensorWriter"
      ~parent_object_id:publisher_oid ()
  in
  if got <> fixture "xrce_client_create" then failwith "xrce_client_create.bin: not byte-identical";

  (* 3. DELETE: request_id=[0x00,0x02], object_id=pack(raw=0x001,kind=Datawriter)
     — deletes the writer CREATEd above. *)
  let got = Xrce_client.encode_delete_message 0x01 0x80 1 key1234 (0x00, 0x02) oid in
  if got <> fixture "xrce_client_delete" then failwith "xrce_client_delete.bin: not byte-identical";

  (* 4. READ_DATA: request_id=[0x00,0x03], object_id=pack(raw=0x002,kind=Datareader),
     preferred_stream_id=0x01, data_format=Data, no filter, no delivery control. *)
  let read_oid = Xrce_client.pack_object_id 0x002 Xrce_client.Datareader in
  let got = Xrce_client.encode_read_data_message 0x01 0x80 2 key1234 (0x00, 0x03) read_oid 0x01 in
  if got <> fixture "xrce_client_read_data" then
    failwith "xrce_client_read_data.bin: not byte-identical";

  (* 5. WRITE_DATA: request_id=[0x00,0x04], object_id=pack(raw=0x001,kind=Datawriter)
     — the same writer — serialized_data=[0x19,0x00]. *)
  let got =
    Xrce_client.encode_write_data_message 0x01 0x80 3 key1234 (0x00, 0x04) oid
      (Bytes.of_string "\x19\x00")
  in
  if got <> fixture "xrce_client_write_data" then
    failwith "xrce_client_write_data.bin: not byte-identical";

  (* 6. A second CREATE, appended to the same session:
     request_id=[0x00,0x05], object_id=pack(raw=0x010,kind=Topic), the
     real two-level ObjectVariant::ByReference("HelloWorldTopic") under
     parent=pack(raw=0x001,kind=Participant), sequence_nr=4. OBJK_TOPIC's
     own representation carries its participant's ObjectId (commit
     31c62df2a63, "xrce: OBJK_TOPIC_Representation carries its
     participant's ObjectId"), the same trailing field OBJK_PUBLISHER's
     representation carries above — this fixture is generated the same
     way (endpoints/golden-gen), not a hand-picked byte string. *)
  let topic_oid = Xrce_client.pack_object_id 0x010 Xrce_client.Topic in
  let participant_oid = Xrce_client.pack_object_id 0x001 Xrce_client.Participant in
  let got =
    Xrce_client.encode_create_message 0x01 0x80 4 key1234 (0x00, 0x05) topic_oid
      "HelloWorldTopic" ~parent_object_id:participant_oid ()
  in
  if got <> fixture "xrce_client_create_topic" then
    failwith "xrce_client_create_topic.bin: not byte-identical";

  print_string "xrce-client fixtures: 6/6 byte-identical\n"

(* Mirrors `expect_err("...")` + `assert_eq!(err, ClientError::InvalidState)`
   in the Rust reference: the refusal is measured on the `result`'s `Error`
   arm, not by inspecting state afterwards. *)
let assert_invalid_state label = function
  | Error Xrce_client.Invalid_state -> ()
  | Ok _ -> failwith (label ^ ": expected Error Invalid_state, got Ok")

let state_machine_tests () =
  let test_key = Bytes.of_string "\xab\xab\xab\xab" in

  (* new_client_starts_disconnected *)
  let c = Xrce_client.create test_key (Zerodds.MemTransport.create ()) in
  if c.state <> Xrce_client.Disconnected then failwith "new client must start Disconnected";
  if c.client_key <> test_key then failwith "client_key must round-trip";

  (* connect_transitions_to_connecting *)
  let c = Xrce_client.create test_key (Zerodds.MemTransport.create ()) in
  (match Xrce_client.connect c (Bytes.of_string "\x00") with
   | Ok () -> ()
   | Error _ -> failwith "connect must succeed from Disconnected");
  if c.state <> Xrce_client.Connecting then failwith "connect must transition to Connecting";

  (* mark_connected_transitions_to_connected *)
  (match Xrce_client.mark_connected c with
   | Ok () -> ()
   | Error _ -> failwith "mark_connected must succeed from Connecting");
  if c.state <> Xrce_client.Connected then failwith "mark_connected must transition to Connected";

  (* double_connect_rejected *)
  assert_invalid_state "double_connect_rejected" (Xrce_client.connect c (Bytes.of_string "\x00"));

  (* create_without_connect_rejected *)
  let fresh = Xrce_client.create test_key (Zerodds.MemTransport.create ()) in
  let oid = Xrce_client.object_id_from_raw 0x0010 in
  assert_invalid_state "create_without_connect_rejected"
    (Xrce_client.create_object fresh oid "topic-ref" ());

  (* write_without_connect_rejected *)
  assert_invalid_state "write_without_connect_rejected"
    (Xrce_client.request_write fresh oid (Bytes.of_string "x"));

  (* read_without_connect_rejected *)
  assert_invalid_state "read_without_connect_rejected" (Xrce_client.request_read fresh oid);

  (* full_lifecycle_creates_unique_request_ids *)
  let r1 =
    match Xrce_client.create_object c oid "a" () with
    | Ok req -> req
    | Error _ -> failwith "create_object must succeed once connected"
  in
  let r2 =
    match Xrce_client.delete_object c oid with
    | Ok req -> req
    | Error _ -> failwith "delete_object must succeed once connected"
  in
  let r3 =
    match Xrce_client.request_write c oid (Bytes.of_string "x") with
    | Ok req -> req
    | Error _ -> failwith "request_write must succeed once connected"
  in
  let r4 =
    match Xrce_client.request_read c oid with
    | Ok req -> req
    | Error _ -> failwith "request_read must succeed once connected"
  in
  (* Request ids are strictly monotonic. *)
  if (r1, r2, r3, r4) <> (1, 2, 3, 4) then failwith "request ids must be strictly monotonic: 1,2,3,4";

  (* disconnect_clears_state *)
  Xrce_client.disconnect c;
  if c.state <> Xrce_client.Disconnected then failwith "disconnect must return to Disconnected";

  print_string "xrce-client state machine: 9/9 OK\n"

let () =
  fixture_tests ();
  state_machine_tests ();
  print_string "ALL OK\n"

# ROS-2-Wire Ground-Truth (CycloneDDS = rmw_cyclonedds)

Captured on codepit via `run_capture.sh` (CycloneDDS 11.0.1, talker+listener
on `rt/chatter`). This is **byte-identical to a real `ros2 topic pub
/chatter std_msgs/String`**, because `rmw_cyclonedds` uses exactly these
CycloneDDS primitives. The pcap is reproducible (`crates/ros2-rmw/interop/run_capture.sh`).

## SEDP — endpoint discovery (tshark decode)
```
DATA(w) -> rt/chatter        # writer announcement
DATA(r) -> rt/chatter        # reader announcement
  PID_TOPIC_NAME (0x0005):   rt/chatter
  PID_TYPE_NAME  (0x0007):   std_msgs::msg::dds_::String_
  PID_RELIABILITY (0x001a):  RELIABLE_RELIABILITY_QOS
  PID_HISTORY    (0x0040):   KEEP_LAST_HISTORY_QOS
```

## DATA(m) — sample payload (CDR)
A `std_msgs/String{ data: "Hello ZeroDDS from ROS wire 0" }`:
```
1e000000 48656c6c6f205a65726f4444532066726f6d20524f5320776972652030 000000
└len=30┘ └────────────── "Hello ZeroDDS from ROS wire 0" ───────────┘└nul+pad┘
```
String CDR: `u32 length (incl. NUL, little-endian)` + bytes + NUL + align padding.

## Confirmed
The ZeroDDS `ros2-rmw` convention is correct:
- `topic_mangling`: `/chatter` → `rt/chatter` ✓
- `type_mapping`: `std_msgs/msg/String` → `std_msgs::msg::dds_::String_` ✓
- RMW default QoS: RELIABLE + KEEP_LAST + VOLATILE ✓

## RTPS submessage distribution (capture)
SPDP `DATA(p)`, SEDP `DATA(w)/DATA(r)` on `rt/chatter`, `DATA(m)` (samples),
`HEARTBEAT` (reliability) — a complete reliable pub/sub flow.

## Live interop (ZeroDDS ↔ CycloneDDS) — SOLVED, bidirectionally green

Status: **✅ both directions 20/20 samples** (codepit, CycloneDDS 11.0.1).
Reproducible: `run_interop.sh` (Cyclone talker → ZeroDDS sub) +
the `ros2_chatter_publisher` example (ZeroDDS pub → Cyclone listener).

```
Cyclone talker -> ZeroDDS sub:   == ZeroDDS received 20 samples ==
ZeroDDS pub    -> Cyclone listener: == listener got 20 samples ==
```

### Root cause (verified via CycloneDDS finest trace + source reading)

**entityKind mismatch (keyed vs. no-key), NOT DATA_REPRESENTATION or
type_information.** Both of the latter were red herrings:
- DATA_REPRESENTATION matches: reader `2(0,2)=[XCDR1,XCDR2]` contains the writer's
  first ID `0=XCDR1` → `data_representation_match_p` = true.
- type_information is NOT a blocker: Cyclone's `ddsi_qos_match_mask_p`
  (`ddsi_qosmatch.c:223`) falls back to a **type-NAME** comparison on a missing type ID
  (XTypes §7.6.3.4.2), as long as `force_type_validation=false` (= our case,
  TCE `1:11000`). The names are identical → match OK.

The actual gate sits in `topickind_qos_match_p_lock`
(`ddsi_endpoint_match.c:175`), **before** the QoS comparison:
```c
if (ddsi_is_keyed_endpoint_entityid(rd) != ddsi_is_keyed_endpoint_entityid(wr)) {
  *reason = DDS_INVALID_QOS_POLICY_ID;   // "different topics" → silently ignored
  return false;                          // NO mismatch log!
}
```
RTPS §9.3.1.2 entityKind: `0x02`=WriterWithKey, `0x03`=WriterNoKey,
`0x04`=ReaderNoKey, `0x07`=ReaderWithKey.
- Cyclone's keyless `std_msgs/String` writer: `…:203` = `0x03` = **NoKey** (correct).
- ZeroDDS reader (before the fix): `…:107` = `0x07` = **WithKey** → keyed≠no-key →
  silent reject. That's why it was misread for months as a type_information problem
  (no log, because the ignore reason is `DDS_INVALID_QOS_POLICY_ID`).

### Fix (committed)
`create_datawriter`/`create_datareader` (publisher.rs / subscriber.rs) called
`register_user_writer`/`_reader`, which **hardcoded** `is_keyed=true` instead of
consulting `DdsType::HAS_KEY`. Now `register_user_writer_kind`/
`_reader_kind` with `T::HAS_KEY`. A keyless type ⇒ NoKey endpoint ⇒ matches
Cyclone's keyless endpoints. Regression: `runtime::tests::
user_endpoint_entity_kind_follows_keyedness` +
`publisher::tests::live_datawriter_entity_kind_is_nokey_for_keyless_type`.

### Diagnostic method (reproducible)
1. `discovery,traffic` trace: the reader SEDP arrives, `match_proxy_reader_with_
   writers scanning all wrs of topic rt/chatter` → no connection, no log.
2. `Verbosity finest` trace: confirms that `generic_do_match_connect` runs,
   but `topickind_qos_match_p_lock` aborts before the QoS comparison.
3. Source reading of `ddsi_qosmatch.c` + `ddsi_endpoint_match.c` localized the
   keyed/no-key gate. The entityKind of the GUIDs (`203`=0x03 vs `107`=0x07) confirmed
   the mismatch. ZeroDDS↔ZeroDDS was green because both sides consistently used
   WithKey.

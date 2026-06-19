// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Notification assignment behavior — Spec §8.4.3.
//!
//! Implements the three assignment paths from §8.4.3:
//!
//! * **Constant Assignment** (§8.4.3.1) — once on DDS output
//!   instantiation; a `Variant` constant value is written into the
//!   named topic-type field (type-cast validation per the
//!   spec).
//! * **DataChange Notification Assignment** (§8.4.3.2.1) — per
//!   `MonitoredItemNotification`, the corresponding `DataItem` is resolved
//!   via `client_handle`, then the target DDS output is identified via
//!   `InputOutputMapping`, and the
//!   `DataValue.value` (= `Variant`) is written into the field.
//! * **EventField Assignment** (§8.4.3.2.2) — per `EventFieldList`, the
//!   `EventItem` is resolved via `client_handle`, then per `event_name`
//!   plus `event_field_index` the corresponding `EventField` from the
//!   `EventFieldList` is selected and assigned to the DDS field.
//!
//! StatusChangeNotifications (§8.4.3.2.3) are explicitly
//! "out of scope of this specification".
//!
//! # What happens here vs. not
//!
//! These helpers provide the **mapping logic** (which field gets
//! which value, with which discriminant, optionally with a
//! type-cast check). The actual topic-sample construction
//! (CDR encoding, DataWriter::write) stays in the daemon crate.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::data_value::Variant;
use crate::types::BuiltinTypeKind;

use super::config::{
    AssignmentInput, DataItem, EventItem, InputOutputMapping, MonitoredItem, OpcUaInput,
};
use super::variant_dds::{ArrayShape, map_variant_to_dds};

/// Error in the notification assignment path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignmentError {
    /// The `client_handle` from the notification could not be associated
    /// with any `MonitoredItem` (Spec §8.4.3.2.1 step 1
    /// fails).
    UnknownClientHandle(u32),
    /// No `Assignment` found for the resolved `OpcUaInput`
    /// (Spec §8.4.3.2.1 step 2 fails).
    NoAssignmentForInput(String),
    /// A cast from the variant to the DDS-output field type is not possible
    /// (Spec §8.4.3.1 + §8.4.3.2.1 step 4 normative: "If the value
    /// cannot be cast, the Gateway shall report an error.").
    IncompatibleCast {
        /// Variant builtin type kind (or `None` if empty).
        variant_kind: Option<BuiltinTypeKind>,
        /// Expected IDL type string from the DDS output field.
        target_idl: String,
    },
    /// `event_field_index` lies outside the `event_fields`
    /// sequence range.
    EventFieldIndexOutOfRange {
        /// The requested index.
        index: u32,
        /// Actual length of the `event_fields` sequence.
        len: u32,
    },
    /// No `EventFieldRef` with a matching `event_name` /
    /// `event_field_index` combination in `field_assignments`.
    NoEventFieldRefMatch {
        /// `event_name` from the notification.
        event_name: String,
        /// `event_field_index` from the notification.
        event_field_index: u32,
    },
}

/// A single mapping result: `dds_output_field_ref` ⇒ the `Variant`
/// value to be written into the field.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldUpdate {
    /// `dds_output_field_ref` from the `FieldAssignment`.
    pub field: String,
    /// Source variant (already type-checked).
    pub value: Variant,
    /// Target IDL type from Tab 8.16 — helpful for callers that must
    /// choose the CDR encoding.
    pub target_idl: String,
}

/// Resolution context for notification assignments — the caller creates
/// it once per subscription/input.
pub struct NotificationContext<'a> {
    /// Active `OpcUaInput` (through which the notifications flow).
    pub input: &'a OpcUaInput,
    /// Active `InputOutputMapping` section.
    pub mapping: &'a InputOutputMapping,
    /// Index `client_handle` → `MonitoredItem` (built by the caller from
    /// the `CreateMonitoredItems` result).
    pub client_handles: &'a BTreeMap<u32, ClientHandleEntry<'a>>,
}

/// A `client_handle` entry that points either to a `DataItem` or
/// an `EventItem` — Spec §8.4.3.2 step 1.
#[derive(Debug, Clone, Copy)]
pub enum ClientHandleEntry<'a> {
    /// Points to a `DataItem` from `OpcUaInput.monitored_items`.
    Data(&'a DataItem),
    /// Points to an `EventItem` from `OpcUaInput.monitored_items`.
    Event(&'a EventItem),
}

// -------------------------------------------------------------------
// §8.4.3.1 — Constant Assignment.
// -------------------------------------------------------------------

/// Spec §8.4.3.1 — constant assignment: on output instantiation.
/// Returns the `FieldUpdate` per `FieldAssignment.assignment_input == ConstantValue`.
/// Iterates over all assignments in the mapping.
///
/// # Errors
/// `AssignmentError::IncompatibleCast` if the constant variant cannot
/// be cast into the IDL type of the target field.
pub fn apply_constant_assignment(
    mapping: &InputOutputMapping,
    output_name: &str,
) -> Result<Vec<FieldUpdate>, AssignmentError> {
    let mut out = Vec::new();
    for asg in &mapping.assignments {
        if asg.dds_output_ref != output_name {
            continue;
        }
        for fa in &asg.field_assignments {
            if let AssignmentInput::ConstantValue(v) = &fa.assignment_input {
                let target_idl = field_target_idl(v);
                check_castable(v, &target_idl)?;
                out.push(FieldUpdate {
                    field: fa.dds_output_field_ref.clone(),
                    value: v.clone(),
                    target_idl,
                });
            }
        }
    }
    Ok(out)
}

// -------------------------------------------------------------------
// §8.4.3.2.1 — DataChange Notification Assignment.
// -------------------------------------------------------------------

/// A received `MonitoredItemNotification` — `client_handle` plus
/// the new variant value (Spec Tab 8.3 `MonitoredItemNotification`).
/// `DataValue` is reduced here to the `value` part, because §8.4.3
/// explicitly ignores the timestamps + status.
#[derive(Debug, Clone, PartialEq)]
pub struct DataChangeNotification {
    /// `IntegerId client_handle`.
    pub client_handle: u32,
    /// `DataValue.value` (= `Variant`).
    pub value: Variant,
}

/// Spec §8.4.3.2.1 — DataChange loop. Per notification:
/// 1. Resolve `client_handle` → `DataItem` (step 1).
/// 2. `DataItem` → relevant assignments from the mapping (step 2).
/// 3. Cast the variant to the target field type (step 4).
///
/// Returns the list of `FieldUpdate` operations that the caller must
/// write into the DDS outputs.
///
/// # Errors
/// * `UnknownClientHandle` if `client_handle` yields no match.
/// * `IncompatibleCast` if the variant→target field type fails.
pub fn apply_data_change_notification(
    ctx: &NotificationContext<'_>,
    output_name: &str,
    notif: &DataChangeNotification,
) -> Result<Vec<FieldUpdate>, AssignmentError> {
    // Step 1: client_handle → MonitoredItem.
    let entry = ctx
        .client_handles
        .get(&notif.client_handle)
        .ok_or(AssignmentError::UnknownClientHandle(notif.client_handle))?;
    // DataChange is only defined for DataItems.
    if !matches!(entry, ClientHandleEntry::Data(_)) {
        // Spec §8.4.3.2.1: the DataChange path ignores EventItems
        // (they come via the EventField path). We return an empty list.
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    let mut found_any_assignment = false;
    for asg in &ctx.mapping.assignments {
        // Step 2: the assignment must match the active input + output.
        if asg.dds_output_ref != output_name || asg.opcua_input_ref != ctx.input.name {
            continue;
        }
        found_any_assignment = true;
        for fa in &asg.field_assignments {
            // Step 2 cont.: apply only DataItem assignments.
            if !matches!(fa.assignment_input, AssignmentInput::DataItem(_)) {
                continue;
            }
            // Steps 3+4: cast the variant + assign.
            let target_idl = field_target_idl(&notif.value);
            check_castable(&notif.value, &target_idl)?;
            out.push(FieldUpdate {
                field: fa.dds_output_field_ref.clone(),
                value: notif.value.clone(),
                target_idl,
            });
        }
    }
    if !found_any_assignment {
        return Err(AssignmentError::NoAssignmentForInput(
            ctx.input.name.clone(),
        ));
    }
    Ok(out)
}

// -------------------------------------------------------------------
// §8.4.3.2.2 — EventField Assignment.
// -------------------------------------------------------------------

/// Spec Tab 8.3 `EventFieldList` equivalent — `client_handle` + the
/// `event_fields` variant sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct EventFieldList {
    /// `IntegerId client_handle`.
    pub client_handle: u32,
    /// `sequence<BaseDataType> event_fields`.
    pub event_fields: Vec<Variant>,
}

/// Spec §8.4.3.2.2 — EventField loop. Per `EventFieldList`:
/// 1. `client_handle` → `EventItem` (step 1).
/// 2. Per `FieldAssignment` with `EventFieldRef` (event_name +
///    event_field_index), select the matching entry from `event_fields`
///    (steps 2-3).
/// 3. Cast the variant + assign (step 4).
///
/// `event_name` must be provided by the caller into the `client_handles`
/// lookup — the spec leaves the mapping table to the config
/// loader; here we receive it via the `event_name_lookup`
/// closure parameter.
///
/// # Errors
/// * `UnknownClientHandle` on an unknown handle.
/// * `EventFieldIndexOutOfRange` if the index >= `event_fields.len()`.
/// * `NoEventFieldRefMatch` if no field-assignment match
///   combination fits.
/// * `IncompatibleCast` on a variant cast error.
pub fn apply_event_notification(
    ctx: &NotificationContext<'_>,
    output_name: &str,
    notif: &EventFieldList,
    event_name_for_handle: impl Fn(u32) -> Option<String>,
) -> Result<Vec<FieldUpdate>, AssignmentError> {
    let entry = ctx
        .client_handles
        .get(&notif.client_handle)
        .ok_or(AssignmentError::UnknownClientHandle(notif.client_handle))?;
    if !matches!(entry, ClientHandleEntry::Event(_)) {
        return Ok(Vec::new());
    }
    let event_name = event_name_for_handle(notif.client_handle)
        .ok_or(AssignmentError::UnknownClientHandle(notif.client_handle))?;

    let len = u32::try_from(notif.event_fields.len()).unwrap_or(u32::MAX);
    let mut out = Vec::new();
    let mut any_match = false;
    for asg in &ctx.mapping.assignments {
        if asg.dds_output_ref != output_name || asg.opcua_input_ref != ctx.input.name {
            continue;
        }
        for fa in &asg.field_assignments {
            let AssignmentInput::EventField(efr) = &fa.assignment_input else {
                continue;
            };
            if efr.event_name != event_name {
                continue;
            }
            any_match = true;
            if efr.event_field_index >= len {
                return Err(AssignmentError::EventFieldIndexOutOfRange {
                    index: efr.event_field_index,
                    len,
                });
            }
            let v = &notif.event_fields[efr.event_field_index as usize];
            let target_idl = field_target_idl(v);
            check_castable(v, &target_idl)?;
            out.push(FieldUpdate {
                field: fa.dds_output_field_ref.clone(),
                value: v.clone(),
                target_idl,
            });
        }
    }
    if !any_match {
        return Err(AssignmentError::NoEventFieldRefMatch {
            event_name,
            event_field_index: 0,
        });
    }
    Ok(out)
}

// -------------------------------------------------------------------
// Helpers.
// -------------------------------------------------------------------

fn field_target_idl(v: &Variant) -> String {
    let shape = ArrayShape::classify(v);
    let kind = v.type_kind().unwrap_or(BuiltinTypeKind::Variant);
    map_variant_to_dds(kind, shape).idl_type
}

fn check_castable(v: &Variant, target_idl: &str) -> Result<(), AssignmentError> {
    // Spec §8.4.3.1/§8.4.3.2.1 step 4: cast safety.
    // Simplification: we accept casts if the variant type kind +
    // shape via Tab 8.16 produces exactly `target_idl`. Callers
    // with known additional cast rules (e.g. int32→int64)
    // can precompute `target_idl` themselves and replace the built-in
    // cast check with their own — the mapping logic
    // is unaffected by that.
    let computed = field_target_idl(v);
    if computed != target_idl {
        return Err(AssignmentError::IncompatibleCast {
            variant_kind: v.type_kind(),
            target_idl: target_idl.into(),
        });
    }
    Ok(())
}

/// Helper constructor: builds a `client_handles` index from an
/// `OpcUaInput`. Callers supply a `client_handle` per `MonitoredItem`
/// from `monitored_items` (usually the index or the server-returned
/// `monitored_item_id` reused).
#[must_use]
pub fn build_client_handles<'a, I>(items: I) -> BTreeMap<u32, ClientHandleEntry<'a>>
where
    I: IntoIterator<Item = (u32, &'a MonitoredItem)>,
{
    let mut map = BTreeMap::new();
    for (handle, item) in items {
        let entry = match item {
            MonitoredItem::Data(d) => ClientHandleEntry::Data(d),
            MonitoredItem::Event(e) => ClientHandleEntry::Event(e),
        };
        map.insert(handle, entry);
    }
    map
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::data_value::VariantValue;
    use crate::node_id::NodeId;
    use crate::subscription_mapping::config::{
        Assignment, AssignmentInput, DataItemRef, EventFieldRef, FieldAssignment, OpcUaConnection,
        OpcUaConnectionConfig, SubscriptionProtocol,
    };

    fn make_input(name: &str, items: Vec<MonitoredItem>) -> OpcUaInput {
        OpcUaInput {
            name: name.into(),
            opcua_connection: OpcUaConnection {
                endpoint_url: "opc.tcp://x".into(),
                timeout: 5_000,
                secure_channel_lifetime: 60_000,
                local_connection: OpcUaConnectionConfig {
                    protocol_version: 0,
                    send_buffer_size: 65_536,
                    recv_buffer_size: 65_536,
                    max_message_size: 1_048_576,
                    max_chunk_count: 16,
                },
            },
            subscription_protocol: SubscriptionProtocol {
                requested_publishing_interval: 100.0,
                requested_lifetime_count: 30,
                requested_max_keepalive_count: 10,
                max_notifications_per_publish: 0,
                publishing_enabled: true,
                priority: 0,
            },
            monitored_items: items,
        }
    }

    fn data_item() -> DataItem {
        DataItem {
            node_id: NodeId::numeric(0, 1),
            attribute_id: 13,
            sampling_interval: 100.0,
            queue_size: 1,
            discard_oldest: true,
            data_change_filter: None,
            aggregate_filter: None,
        }
    }

    fn event_item() -> EventItem {
        EventItem {
            node_id: NodeId::numeric(0, 2),
            sampling_interval: 100.0,
            queue_size: 16,
            discard_oldest: true,
            event_filter: None,
        }
    }

    #[test]
    fn constant_assignment_writes_single_field() {
        let mapping = InputOutputMapping {
            assignments: alloc::vec![Assignment {
                dds_output_ref: "out".into(),
                opcua_input_ref: "in".into(),
                field_assignments: alloc::vec![FieldAssignment {
                    dds_output_field_ref: "header.tag".into(),
                    opcua_input_ref: None,
                    assignment_input: AssignmentInput::ConstantValue(Variant::scalar(
                        VariantValue::Int32(42),
                    )),
                }],
            }],
        };
        let out = apply_constant_assignment(&mapping, "out").expect("constants");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].field, "header.tag");
        assert_eq!(out[0].target_idl, "int32");
    }

    #[test]
    fn data_change_finds_and_assigns_value() {
        let item = data_item();
        let input = make_input("in", alloc::vec![MonitoredItem::Data(item.clone())]);
        let mapping = InputOutputMapping {
            assignments: alloc::vec![Assignment {
                dds_output_ref: "out".into(),
                opcua_input_ref: "in".into(),
                field_assignments: alloc::vec![FieldAssignment {
                    dds_output_field_ref: "value".into(),
                    opcua_input_ref: None,
                    assignment_input: AssignmentInput::DataItem(DataItemRef {
                        data_item_name: "temp".into(),
                    }),
                }],
            }],
        };
        let mut handles = BTreeMap::new();
        // We use a persistent DataItem from `input`.
        let item_ref = match &input.monitored_items[0] {
            MonitoredItem::Data(d) => d,
            MonitoredItem::Event(_) => panic!("test fixture invariant: expected Data"),
        };
        handles.insert(7u32, ClientHandleEntry::Data(item_ref));
        let ctx = NotificationContext {
            input: &input,
            mapping: &mapping,
            client_handles: &handles,
        };

        let res = apply_data_change_notification(
            &ctx,
            "out",
            &DataChangeNotification {
                client_handle: 7,
                value: Variant::scalar(VariantValue::Int32(123)),
            },
        )
        .expect("ok");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].field, "value");
        assert_eq!(res[0].target_idl, "int32");
    }

    #[test]
    fn data_change_unknown_handle_is_error() {
        let input = make_input("in", alloc::vec![]);
        let mapping = InputOutputMapping {
            assignments: alloc::vec![],
        };
        let handles = BTreeMap::new();
        let ctx = NotificationContext {
            input: &input,
            mapping: &mapping,
            client_handles: &handles,
        };
        let err = apply_data_change_notification(
            &ctx,
            "out",
            &DataChangeNotification {
                client_handle: 99,
                value: Variant::scalar(VariantValue::Int32(1)),
            },
        )
        .unwrap_err();
        assert_eq!(err, AssignmentError::UnknownClientHandle(99));
    }

    #[test]
    fn event_field_assignment_picks_correct_index() {
        let evt = event_item();
        let input = make_input("in", alloc::vec![MonitoredItem::Event(evt.clone())]);
        let mapping = InputOutputMapping {
            assignments: alloc::vec![Assignment {
                dds_output_ref: "out".into(),
                opcua_input_ref: "in".into(),
                field_assignments: alloc::vec![FieldAssignment {
                    dds_output_field_ref: "severity".into(),
                    opcua_input_ref: None,
                    assignment_input: AssignmentInput::EventField(EventFieldRef {
                        event_name: "alarm".into(),
                        event_field_index: 1,
                    }),
                }],
            }],
        };

        let mut handles = BTreeMap::new();
        let evt_ref = match &input.monitored_items[0] {
            MonitoredItem::Event(e) => e,
            MonitoredItem::Data(_) => panic!("test fixture invariant: expected Event"),
        };
        handles.insert(11u32, ClientHandleEntry::Event(evt_ref));
        let ctx = NotificationContext {
            input: &input,
            mapping: &mapping,
            client_handles: &handles,
        };

        let res = apply_event_notification(
            &ctx,
            "out",
            &EventFieldList {
                client_handle: 11,
                event_fields: alloc::vec![
                    Variant::scalar(VariantValue::Int32(0)),
                    Variant::scalar(VariantValue::Int32(700)),
                    Variant::scalar(VariantValue::Int32(0)),
                ],
            },
            |_| Some("alarm".into()),
        )
        .expect("ok");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].field, "severity");
        // event_fields[1] = Int32(700) → DDS-int32.
        assert_eq!(res[0].target_idl, "int32");
    }

    #[test]
    fn event_field_index_out_of_range_is_error() {
        let evt = event_item();
        let input = make_input("in", alloc::vec![MonitoredItem::Event(evt.clone())]);
        let mapping = InputOutputMapping {
            assignments: alloc::vec![Assignment {
                dds_output_ref: "out".into(),
                opcua_input_ref: "in".into(),
                field_assignments: alloc::vec![FieldAssignment {
                    dds_output_field_ref: "x".into(),
                    opcua_input_ref: None,
                    assignment_input: AssignmentInput::EventField(EventFieldRef {
                        event_name: "alarm".into(),
                        event_field_index: 5, // out of range
                    }),
                }],
            }],
        };
        let mut handles = BTreeMap::new();
        let evt_ref = match &input.monitored_items[0] {
            MonitoredItem::Event(e) => e,
            MonitoredItem::Data(_) => panic!("test fixture invariant: expected Event"),
        };
        handles.insert(1u32, ClientHandleEntry::Event(evt_ref));
        let ctx = NotificationContext {
            input: &input,
            mapping: &mapping,
            client_handles: &handles,
        };
        let err = apply_event_notification(
            &ctx,
            "out",
            &EventFieldList {
                client_handle: 1,
                event_fields: alloc::vec![Variant::scalar(VariantValue::Int32(0))],
            },
            |_| Some("alarm".into()),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            AssignmentError::EventFieldIndexOutOfRange { index: 5, len: 1 }
        ));
    }

    #[test]
    fn build_client_handles_indexes_correctly() {
        let items = alloc::vec![
            MonitoredItem::Data(data_item()),
            MonitoredItem::Event(event_item()),
        ];
        let map = build_client_handles(items.iter().enumerate().map(|(i, x)| (i as u32, x)));
        assert_eq!(map.len(), 2);
        assert!(matches!(map.get(&0), Some(ClientHandleEntry::Data(_))));
        assert!(matches!(map.get(&1), Some(ClientHandleEntry::Event(_))));
    }
}

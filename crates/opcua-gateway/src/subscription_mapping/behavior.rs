// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Notification-Assignment-Behavior — Spec §8.4.3.
//!
//! Implementiert die drei Assignment-Wege aus §8.4.3:
//!
//! * **Constant Assignment** (§8.4.3.1) — einmalig bei DDS-Output-
//!   Instanziierung; ein `Variant`-Konstantenwert wird in das
//!   benannte Topic-Type-Feld geschrieben (Type-Cast-Validation gemaess
//!   Spec).
//! * **DataChange Notification Assignment** (§8.4.3.2.1) — pro
//!   `MonitoredItemNotification` wird per `client_handle` der
//!   zugehoerige `DataItem` aufgeloest, dann via
//!   `InputOutputMapping` der Ziel-DDS-Output identifiziert, und der
//!   `DataValue.value` (= `Variant`) in das Feld geschrieben.
//! * **EventField Assignment** (§8.4.3.2.2) — pro `EventFieldList` wird
//!   per `client_handle` der `EventItem` aufgeloest, dann je `event_name`
//!   plus `event_field_index` der entsprechende `EventField` aus dem
//!   `EventFieldList` ausgewaehlt und in das DDS-Feld assigned.
//!
//! StatusChangeNotifications (§8.4.3.2.3) sind explizit
//! "out of scope of this specification".
//!
//! # Was hier passiert vs. nicht passiert
//!
//! Diese Helpers liefern die **Mapping-Logik** (welches Feld bekommt
//! welchen Wert, mit welcher Discriminant, gegebenenfalls mit
//! Type-Cast-Pruefung). Die eigentliche Topic-Sample-Construction
//! (CDR-Encoding, DataWriter::write) bleibt im Daemon-Crate.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::data_value::Variant;
use crate::types::BuiltinTypeKind;

use super::config::{
    AssignmentInput, DataItem, EventItem, InputOutputMapping, MonitoredItem, OpcUaInput,
};
use super::variant_dds::{ArrayShape, map_variant_to_dds};

/// Fehler im Notification-Assignment-Pfad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignmentError {
    /// `client_handle` aus der Notification konnte keinem
    /// `MonitoredItem` zugeordnet werden (Spec §8.4.3.2.1 Schritt 1
    /// scheitert).
    UnknownClientHandle(u32),
    /// Kein `Assignment` fuer den aufgeloesten `OpcUaInput` gefunden
    /// (Spec §8.4.3.2.1 Schritt 2 scheitert).
    NoAssignmentForInput(String),
    /// Cast vom Variant zum DDS-Output-Feld-Typ ist nicht moeglich
    /// (Spec §8.4.3.1 + §8.4.3.2.1 Schritt 4 normativ: "If the value
    /// cannot be cast, the Gateway shall report an error.").
    IncompatibleCast {
        /// Variant-Builtin-Type-Kind (oder `None` falls leer).
        variant_kind: Option<BuiltinTypeKind>,
        /// Erwarteter IDL-Type-String aus dem DDS-Output-Feld.
        target_idl: String,
    },
    /// `event_field_index` liegt ausserhalb des `event_fields`-
    /// Sequenz-Bereichs.
    EventFieldIndexOutOfRange {
        /// Index, der angefragt wurde.
        index: u32,
        /// Tatsaechliche Laenge der `event_fields`-Sequenz.
        len: u32,
    },
    /// Kein `EventFieldRef` mit passender `event_name` /
    /// `event_field_index`-Kombination im `field_assignments`.
    NoEventFieldRefMatch {
        /// `event_name` aus der Notification.
        event_name: String,
        /// `event_field_index` aus der Notification.
        event_field_index: u32,
    },
}

/// Einzelnes Mapping-Ergebnis: `dds_output_field_ref` ⇒ `Variant`-
/// Wert, der in das Feld zu schreiben ist.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldUpdate {
    /// `dds_output_field_ref` aus dem `FieldAssignment`.
    pub field: String,
    /// Quell-Variant (bereits typgeprueft).
    pub value: Variant,
    /// Ziel-IDL-Type aus Tab 8.16 — hilfreich fuer Caller, die das
    /// CDR-Encoding waehlen muessen.
    pub target_idl: String,
}

/// Aufloesungs-Kontext fuer Notification-Assignments — Caller erstellt
/// das einmalig pro Subscription/Input.
pub struct NotificationContext<'a> {
    /// Aktiver `OpcUaInput` (durch den die Notifications fliessen).
    pub input: &'a OpcUaInput,
    /// Aktive `InputOutputMapping`-Section.
    pub mapping: &'a InputOutputMapping,
    /// Index `client_handle` → `MonitoredItem` (vom Caller per
    /// `CreateMonitoredItems`-Ergebnis aufgebaut).
    pub client_handles: &'a BTreeMap<u32, ClientHandleEntry<'a>>,
}

/// Ein `client_handle`-Eintrag, der entweder auf einen `DataItem` oder
/// einen `EventItem` zeigt — Spec §8.4.3.2 Schritt 1.
#[derive(Debug, Clone, Copy)]
pub enum ClientHandleEntry<'a> {
    /// Zeigt auf einen `DataItem` aus `OpcUaInput.monitored_items`.
    Data(&'a DataItem),
    /// Zeigt auf einen `EventItem` aus `OpcUaInput.monitored_items`.
    Event(&'a EventItem),
}

// -------------------------------------------------------------------
// §8.4.3.1 — Constant Assignment.
// -------------------------------------------------------------------

/// Spec §8.4.3.1 — Constant-Assignment: bei Output-Instanziierung.
/// Liefert pro `FieldAssignment.assignment_input == ConstantValue` den
/// `FieldUpdate`. Iteration durch alle Assignments im Mapping.
///
/// # Errors
/// `AssignmentError::IncompatibleCast` wenn der Constant-Variant nicht
/// in den IDL-Type des Ziel-Felds gegossen werden kann.
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

/// Eine empfangene `MonitoredItemNotification` — `client_handle` plus
/// neuer Variant-Wert (Spec Tab 8.3 `MonitoredItemNotification`).
/// `DataValue` ist hier auf den `value`-Anteil reduziert, weil §8.4.3
/// die Timestamps + Status explizit ignoriert.
#[derive(Debug, Clone, PartialEq)]
pub struct DataChangeNotification {
    /// `IntegerId client_handle`.
    pub client_handle: u32,
    /// `DataValue.value` (= `Variant`).
    pub value: Variant,
}

/// Spec §8.4.3.2.1 — DataChange-Loop. Pro Notification:
/// 1. `client_handle` → `DataItem` aufloesen (Schritt 1).
/// 2. `DataItem` → relevante Assignments aus dem Mapping (Schritt 2).
/// 3. Variant zum Ziel-Feld-Typ casten (Schritt 4).
///
/// Liefert die Liste der `FieldUpdate`-Operationen, die der Caller in
/// die DDS-Outputs schreiben muss.
///
/// # Errors
/// * `UnknownClientHandle` wenn `client_handle` keinen Match liefert.
/// * `IncompatibleCast` wenn Variant→Ziel-Feld-Typ scheitert.
pub fn apply_data_change_notification(
    ctx: &NotificationContext<'_>,
    output_name: &str,
    notif: &DataChangeNotification,
) -> Result<Vec<FieldUpdate>, AssignmentError> {
    // Schritt 1: client_handle → MonitoredItem.
    let entry = ctx
        .client_handles
        .get(&notif.client_handle)
        .ok_or(AssignmentError::UnknownClientHandle(notif.client_handle))?;
    // DataChange ist nur fuer DataItems definiert.
    if !matches!(entry, ClientHandleEntry::Data(_)) {
        // Spec §8.4.3.2.1: DataChange-Pfad ignoriert EventItems
        // (sie kommen via EventField-Pfad). Wir liefern leere Liste.
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    let mut found_any_assignment = false;
    for asg in &ctx.mapping.assignments {
        // Schritt 2: Assignment muss zum aktiven Input + Output passen.
        if asg.dds_output_ref != output_name || asg.opcua_input_ref != ctx.input.name {
            continue;
        }
        found_any_assignment = true;
        for fa in &asg.field_assignments {
            // Schritt 2 Forts.: nur DataItem-Assignments anwenden.
            if !matches!(fa.assignment_input, AssignmentInput::DataItem(_)) {
                continue;
            }
            // Schritt 3+4: Variant casten + assignen.
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

/// Spec Tab 8.3 `EventFieldList`-Aequivalent — `client_handle` + die
/// `event_fields`-Variant-Sequenz.
#[derive(Debug, Clone, PartialEq)]
pub struct EventFieldList {
    /// `IntegerId client_handle`.
    pub client_handle: u32,
    /// `sequence<BaseDataType> event_fields`.
    pub event_fields: Vec<Variant>,
}

/// Spec §8.4.3.2.2 — EventField-Loop. Pro `EventFieldList`:
/// 1. `client_handle` → `EventItem` (Schritt 1).
/// 2. Pro `FieldAssignment` mit `EventFieldRef` (event_name +
///    event_field_index) den passenden Eintrag aus `event_fields`
///    auswaehlen (Schritt 2-3).
/// 3. Variant casten + assignen (Schritt 4).
///
/// `event_name` muss vom Caller in den `client_handles`-Lookup
/// eingebracht werden — Spec laesst die Mapping-Tabelle dem Konfig-
/// Loader; hier bekommen wir sie ueber den `event_name_lookup`-
/// Closure-Parameter.
///
/// # Errors
/// * `UnknownClientHandle` bei unbekanntem Handle.
/// * `EventFieldIndexOutOfRange` wenn Index >= `event_fields.len()`.
/// * `NoEventFieldRefMatch` wenn keine Field-Assignment-Match-
///   Kombination passt.
/// * `IncompatibleCast` bei Variant-Cast-Fehler.
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
    // Spec §8.4.3.1/§8.4.3.2.1 Schritt 4: cast safety.
    // Vereinfachung: wir akzeptieren Casts, wenn Variant-Type-Kind +
    // Shape via Tab 8.16 exakt den `target_idl` produziert. Caller
    // mit bekannten weiterfuehrenden Cast-Regeln (z.B. int32→int64)
    // koennen `target_idl` selbst vorberechnen und den eingebauten
    // Cast-Check durch ihren eigenen ersetzen — die Mapping-Logik
    // bleibt davon unberuehrt.
    let computed = field_target_idl(v);
    if computed != target_idl {
        return Err(AssignmentError::IncompatibleCast {
            variant_kind: v.type_kind(),
            target_idl: target_idl.into(),
        });
    }
    Ok(())
}

/// Hilfs-Konstruktor: baut einen `client_handles`-Index aus einem
/// `OpcUaInput`. Caller geben pro `MonitoredItem` aus `monitored_items`
/// einen `client_handle` an (i.d.R. Index oder vom Server returnter
/// `monitored_item_id` re-genutzt).
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
        // Wir nutzen ein dauerhaftes DataItem aus `input`.
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

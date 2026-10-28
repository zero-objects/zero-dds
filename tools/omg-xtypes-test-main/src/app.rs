// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The two runs the suite starts: one publisher and one subscriber, on
//! the same topic and — in 430 of `test_suite.py`'s 670 entity
//! expectations — on two types that are not assignable to each other.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use zerodds_dcps::DomainParticipant;
use zerodds_dcps::psm_constants::status as status_bits;
use zerodds_dcps::qos::{DataReaderQos, DataWriterQos, PublisherQos, SubscriberQos, TopicQos};
use zerodds_types::qos::TypeConsistencyKind;

use crate::carrier::Sample;
use crate::cli::{Options, TriState};
use crate::data;
use crate::listeners::{
    Flags, XtypesReaderListener, XtypesTopicListener, XtypesWriterListener, line,
};

/// How often the run drives its listeners and, on the publish side,
/// writes a sample. The harness's own step timeout is ten seconds.
const TICK: Duration = Duration::from_millis(100);

/// The `DataRepresentationQosPolicy` value for `-x` (XTypes 1.3
/// §7.6.3.1.1: `XCDR_DATA_REPRESENTATION = 0`,
/// `XCDR2_DATA_REPRESENTATION = 2`).
///
/// Pinned rather than left at the default: the sample octets were built
/// under the encoding `-x` names, and an endpoint that offered or
/// requested the other one would stamp an encapsulation header the body
/// does not match.
fn data_representation(options: &Options) -> Vec<i16> {
    if options.data_representation == 1 {
        vec![0]
    } else {
        vec![2]
    }
}

/// Builds the reader's `TypeConsistencyEnforcementQosPolicy` from the
/// suite's six tri-state switches (XTypes 1.3 §7.6.3.4.1).
///
/// Two baselines, not one, and that is the whole point of the `d`
/// value. `test_main.cxx`'s `TestOptions()` constructor — the reference
/// client RTI Connext 7.7 and CoreDX 6.14 are both built from — starts
/// every run at
///
/// ```text
/// type_consistency.force_type_validation  = DDS_BOOLEAN_FALSE;
/// type_consistency.ignore_member_names    = DDS_BOOLEAN_TRUE;
/// type_consistency.ignore_sequence_bounds = DDS_BOOLEAN_TRUE;
/// type_consistency.ignore_string_bounds   = DDS_BOOLEAN_TRUE;
/// type_consistency.prevent_type_widening  = DDS_BOOLEAN_FALSE;
/// type_consistency.kind                   = ALLOW_TYPE_COERCION;
/// ```
///
/// and `d` asks for `TypeConsistency_get_default()`, the *vendor's*
/// policy default, which §7.6.3.4.1 gives as
/// `ignore_member_names = FALSE`. The two differ in exactly that one
/// field, and the suite tests the difference: `struct_different_names_ok`
/// passes no flag and expects the pair to communicate, while
/// `tc_ignore_member_names_3` passes `d` on the same shape of type pair
/// and expects `INCONSISTENT_TOPIC`.
///
/// Reading "no flag" as "the DDS default" collapsed the two and cost
/// nine cases. `DataReaderQos::default()` is the DDS default and stays
/// what `d` selects.
fn type_consistency(options: &Options, qos: &mut DataReaderQos) {
    let tce = &mut qos.type_consistency;
    // The suite's baseline, before any flag is read.
    tce.kind = TypeConsistencyKind::AllowTypeCoercion;
    tce.ignore_sequence_bounds = true;
    tce.ignore_string_bounds = true;
    tce.ignore_member_names = true;
    tce.prevent_type_widening = false;
    tce.force_type_validation = false;
    let spec_default = zerodds_types::qos::TypeConsistencyEnforcement::default();
    match options.allow_type_coercion {
        TriState::True => tce.kind = TypeConsistencyKind::AllowTypeCoercion,
        TriState::False => tce.kind = TypeConsistencyKind::DisallowTypeCoercion,
        // Both baselines name `ALLOW_TYPE_COERCION` for `kind`, so `d`
        // and absent put back what is already there.
        TriState::Default => tce.kind = spec_default.kind,
        TriState::Absent => {}
    }
    apply(
        options.ignore_seq_bounds,
        &mut tce.ignore_sequence_bounds,
        spec_default.ignore_sequence_bounds,
    );
    apply(
        options.ignore_str_bounds,
        &mut tce.ignore_string_bounds,
        spec_default.ignore_string_bounds,
    );
    apply(
        options.ignore_member_names,
        &mut tce.ignore_member_names,
        spec_default.ignore_member_names,
    );
    apply(
        options.prevent_type_widening,
        &mut tce.prevent_type_widening,
        spec_default.prevent_type_widening,
    );
    apply(
        options.force_type_validation,
        &mut tce.force_type_validation,
        spec_default.force_type_validation,
    );
}

/// One switch: `t`/`f` set the field, `d` puts the QoS policy default of
/// §7.6.3.4.1 there, and an absent flag leaves the suite's baseline
/// alone. `d` and absent are different answers and the suite tests
/// both — see [`TriState`].
fn apply(switch: TriState, field: &mut bool, spec_default: bool) {
    match switch {
        TriState::True => *field = true,
        TriState::False => *field = false,
        TriState::Default => *field = spec_default,
        TriState::Absent => {}
    }
}

/// Registers the type's own `TypeObject` and every object it reaches, so
/// a peer resolving this endpoint's `TypeIdentifier` can complete the
/// assignability walk of XTypes 1.3 §7.2.4 without a `getTypes`
/// round-trip. Endpoint creation registers only the top-level object.
fn register_types(participant: &DomainParticipant) {
    let cx = crate::carrier::context();
    if cx.disable_type_info {
        return;
    }
    for obj in std::iter::once(&cx.model.type_object).chain(cx.model.dependencies.iter()) {
        let _ = participant.register_type_object(obj.clone());
    }
}

/// The publish side.
///
/// Prints, in the order the harness's publisher state machine reads
/// them: `Create topic:` (step 2), `Create writer for topic` (step 3),
/// then whichever of `on_publication_matched()`,
/// `on_offered_incompatible_qos` or `on_inconsistent_topic` the run
/// reaches (step 4).
pub fn publish(
    participant: &DomainParticipant,
    options: &Options,
    stop: &Arc<AtomicBool>,
) -> Result<(), String> {
    let topic_name = options.topic_name.clone().unwrap_or_default();
    register_types(participant);
    let flags = Arc::new(Flags::default());

    let topic = participant
        .create_topic::<Sample>(&topic_name, TopicQos::default())
        .map_err(|e| format!("create topic failed: {e}"))?;
    topic.set_listener(
        Some(Arc::new(XtypesTopicListener::new(
            topic_name.clone(),
            Arc::clone(&flags),
        ))),
        status_bits::ANY,
    );
    line(&format!("Create topic: {topic_name}"));

    let publisher = participant.create_publisher(PublisherQos::default());
    let mut qos = DataWriterQos {
        data_representation: Some(data_representation(options)),
        ..DataWriterQos::default()
    };
    if !options.reliable {
        qos.reliability.kind = zerodds_dcps::qos::ReliabilityKind::BestEffort;
    }
    // Printed before the writer exists, as `test_main.c` and
    // `test_main.cxx` do: the matching pass of `create_datawriter` can
    // already raise INCONSISTENT_TOPIC against a publication discovered
    // earlier, and the harness reads step 4 only after this line.
    line(&format!("Create writer for topic: {topic_name}"));
    let writer = publisher
        .create_datawriter::<Sample>(&topic, qos)
        .map_err(|e| format!("create writer failed: {e}"))?;
    writer.set_listener(
        Some(Arc::new(XtypesWriterListener::new(Arc::clone(&flags)))),
        status_bits::ANY,
    );

    // A publisher with no `--data-file` writes an unpopulated sample —
    // `test_main.cxx` allows that under `#if 0`, and the suite always
    // passes a data file, so an absent one is reported rather than
    // guessed at.
    let value = data::load(options.data_folder.as_deref(), options.data_file.as_deref())?;
    // The suite's JSON sample for a union names the branch and not the
    // discriminator; the codec's value contract needs `_d`. See
    // `data::complete`.
    let cx = crate::carrier::context();
    let value = data::complete(&cx.model.ast, cx.model.type_, &value)?;
    let sample = Sample::from_value(&value).map_err(|e| format!("cannot encode sample: {e}"))?;

    let mut count: u64 = 0;
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(TICK);
        writer.drive_listeners();
        // Reading the status is what dispatches the listener on a
        // delta (DDS 1.4 §2.2.4.1). The count itself is the runtime's,
        // recorded where XTypes 1.3 §7.6.3.4.2 is decided.
        let _ = topic.inconsistent_topic_status();
        if flags.inconsistent.load(Ordering::SeqCst) {
            // The two types are not consistent, so nothing will match
            // and nothing is written. The harness has its line.
            continue;
        }
        if let Err(e) = writer.write(&sample) {
            eprintln!("write failed: {e}");
            return Err(format!("write failed: {e}"));
        }
        count += 1;
        if options.print_writer_samples {
            // The harness's `-w` branch scans for `<n> <n> [<n>]`
            // (`interoperability_report.py`, the `re.search` in the
            // publisher's step 5). No test in `test_suite.py` passes
            // `-w`; the line is written in that shape so that one which
            // did would be read.
            line(&format!("{count} {count} [{count}]"));
        }
    }
    Ok(())
}

/// The subscribe side.
///
/// Prints `Create topic:` (step 2), `Create reader for topic:` (step 3),
/// then per sample `sample_received()` (step 4 index 3), a `[<n>]` line
/// (index 0) and the verdict `data_is_correct` waits for.
pub fn subscribe(
    participant: &DomainParticipant,
    options: &Options,
    stop: &Arc<AtomicBool>,
) -> Result<(), String> {
    let topic_name = options.topic_name.clone().unwrap_or_default();
    register_types(participant);
    let flags = Arc::new(Flags::default());

    let topic = participant
        .create_topic::<Sample>(&topic_name, TopicQos::default())
        .map_err(|e| format!("create topic failed: {e}"))?;
    topic.set_listener(
        Some(Arc::new(XtypesTopicListener::new(
            topic_name.clone(),
            Arc::clone(&flags),
        ))),
        status_bits::ANY,
    );
    line(&format!("Create topic: {topic_name}"));

    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let mut qos = DataReaderQos {
        data_representation: Some(data_representation(options)),
        ..DataReaderQos::default()
    };
    if !options.reliable {
        qos.reliability.kind = zerodds_dcps::qos::ReliabilityKind::BestEffort;
    }
    type_consistency(options, &mut qos);
    // Printed before the reader exists, as `test_main.c` and
    // `test_main.cxx` do: the matching pass of `create_datareader` can
    // already raise INCONSISTENT_TOPIC against a publication discovered
    // earlier, and the harness looks for `on_inconsistent_topic` only
    // after this line (measured: a publication pushed before the reader
    // was created put the status line first, DATA_NOT_RECEIVED).
    line(&format!("Create reader for topic: {topic_name}"));
    let reader = subscriber
        .create_datareader::<Sample>(&topic, qos)
        .map_err(|e| format!("create reader failed: {e}"))?;
    reader.set_listener(
        Some(Arc::new(XtypesReaderListener::new(Arc::clone(&flags)))),
        status_bits::ANY,
    );

    // The expected sample. `data_is_correct` waits for the verdict line,
    // so a subscriber that cannot load the file says so instead of
    // silently reporting every sample as correct.
    let expected = data::load(options.data_folder.as_deref(), options.data_file.as_deref()).ok();

    let mut count: u64 = 0;
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(TICK);
        reader.drive_listeners();
        // Reading the status is what dispatches the listener on a
        // delta (DDS 1.4 §2.2.4.1). The count itself is the runtime's,
        // recorded where XTypes 1.3 §7.6.3.4.2 is decided.
        let _ = topic.inconsistent_topic_status();
        let Ok(samples) = reader.take_with_info() else {
            continue;
        };
        for s in &samples {
            if !s.info.valid_data {
                continue;
            }
            count += 1;
            line("sample_received()");
            line(&format!("[{count}]"));
            match (&expected, &s.data.value) {
                (Some(want), Some(got)) => match data::mismatch(want, got) {
                    None => line("Received sample is the same as loaded"),
                    Some(why) => line(&format!("Received sample is not the same as loaded: {why}")),
                },
                (_, None) => line("Received sample is not the same as loaded: it did not decode"),
                (None, _) => {
                    line("Received sample is not the same as loaded: no data file was loaded");
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_dcps::qos::DataReaderQos;

    fn tce_for(args: &[&str]) -> zerodds_types::qos::TypeConsistencyEnforcement {
        let mut options = Options::default();
        if !args.is_empty() {
            let argv: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
            options = crate::cli::parse(&argv).unwrap();
        }
        let mut qos = DataReaderQos::default();
        type_consistency(&options, &mut qos);
        qos.type_consistency
    }

    /// `test_main.cxx::TestOptions()` starts at `ignore_member_names =
    /// DDS_BOOLEAN_TRUE`; `--ignore-member-names d` asks for
    /// `TypeConsistency_get_default()`, which XTypes 1.3 §7.6.3.4.1
    /// gives as FALSE. An absent flag and `d` are different answers,
    /// and the suite tests both — `struct_different_names_ok` (no flag,
    /// must communicate) against `tc_ignore_member_names_3` (`d`, must
    /// not).
    #[test]
    fn an_absent_switch_is_the_suite_baseline_and_d_is_the_qos_default() {
        assert!(
            tce_for(&[]).ignore_member_names,
            "no flag is the suite's own baseline"
        );
        assert!(
            !tce_for(&["--ignore-member-names", "d"]).ignore_member_names,
            "`d` is the QoS policy default of §7.6.3.4.1"
        );
        assert!(tce_for(&["--ignore-member-names", "t"]).ignore_member_names);
        assert!(!tce_for(&["--ignore-member-names", "f"]).ignore_member_names);
    }

    /// The five other fields agree between the two baselines, so an
    /// absent switch and `d` answer alike there.
    #[test]
    fn the_other_five_switches_agree_between_the_two_baselines() {
        let absent = tce_for(&[]);
        assert!(absent.ignore_sequence_bounds);
        assert!(absent.ignore_string_bounds);
        assert!(!absent.prevent_type_widening);
        assert!(!absent.force_type_validation);
        assert_eq!(absent.kind, TypeConsistencyKind::AllowTypeCoercion);

        for flag in [
            "--ignore-seq-bounds",
            "--ignore-str-bounds",
            "--prevent-type-widening",
            "--force-type-validation",
        ] {
            assert_eq!(
                tce_for(&[flag, "d"]),
                absent,
                "{flag}: the two baselines agree on this field, so `d` changes nothing"
            );
            assert_ne!(
                tce_for(&[flag, "t"]),
                tce_for(&[flag, "f"]),
                "{flag}: t and f must differ"
            );
        }
    }
}

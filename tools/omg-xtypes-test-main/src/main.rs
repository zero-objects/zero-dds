// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `main()` of `src/cxx/test_main.cxx`, in the order it does things:
//! parse, validate, read the type, create the participant, run one role.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use zerodds_dcps::{DomainParticipantFactory, qos::DomainParticipantQos};
use zerodds_omg_xtypes_test_main::{app, carrier, cli, model};

/// The process exit codes. The harness reads the printed protocol and
/// not the status, but a person running one side by hand reads this.
const EXIT_OK: i32 = 0;
const EXIT_USAGE: i32 = 1;
const EXIT_ERROR: i32 = 2;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let argv: Vec<String> = std::env::args().collect();
    let prog = argv.first().cloned().unwrap_or_else(|| "test_main".into());
    let options = match cli::parse(&argv[1..]) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            eprint!("{}", cli::usage(&prog));
            return EXIT_USAGE;
        }
    };
    if let Err(e) = options.validate() {
        eprintln!("{e}");
        eprint!("{}", cli::usage(&prog));
        return EXIT_USAGE;
    }

    let type_name = options.type_name.clone().unwrap_or_default();
    let loaded = match model::load(
        options.type_folder.as_deref(),
        options.type_file.as_deref(),
        &type_name,
        options.key_on_union_discriminator,
        options.object_form(),
    ) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("ERROR: {e}");
            return EXIT_ERROR;
        }
    };
    eprintln!(
        "Info: type '{}' read from {} (@{:?}{})",
        loaded.type_name,
        loaded.source_path.display(),
        loaded.extensibility,
        if loaded.keyed { ", keyed" } else { "" }
    );

    // `--print-typeid` creates no entities: it prints the identity the
    // run would announce and stops. XTypes 1.3 §7.3.4.5 gives every type
    // both an equivalence hash under the MINIMAL form and one under the
    // COMPLETE form, and the suite's typeid tests compare them across
    // two products.
    if options.print_typeid {
        return print_typeid(&loaded, options.type_object_version, options.object_form());
    }

    // The Table 19 axis is the runtime's, not a QoS: both sides of a
    // pair decide under it, so it goes in through the runtime
    // configuration rather than the reader's announced policy.
    // On top of the standard configuration (files, environment), not the
    // compiled defaults: `RuntimeConfig::default()` reads nothing, and a
    // client that started from it would silently drop ZERODDS_OBSERVABILITY
    // and every file-borne setting the plain `create_participant` honours.
    let base = match &options.runtime_config {
        Some(path) => zerodds_dcps::runtime::ConfigSource::from_path(path)
            .and_then(|source| zerodds_dcps::runtime::RuntimeConfig::from_source(&source)),
        None => zerodds_dcps::runtime::RuntimeConfig::from_standard_paths(),
    };
    let mut runtime = match base {
        Ok(base) => zerodds_dcps::runtime::RuntimeConfig {
            key_member_bounds: options.key_member_bounds,
            // The suite's answer unless the flag or the environment names
            // one (README "Where the suite expects another answer than the
            // clause").
            strong_assignability: options.strong_assignability.unwrap_or(
                if std::env::var_os("ZERODDS_STRONG_ASSIGNABILITY").is_some() {
                    base.strong_assignability
                } else {
                    zerodds_types::assignability::StrongAssignability::NamesIgnored
                },
            ),
            ..base
        },
        Err(e) => {
            eprintln!("ERROR reading the runtime configuration: {e}");
            return EXIT_ERROR;
        }
    };
    suite_settings(&mut runtime, |name| std::env::var_os(name).is_some());

    // The leak is one `&'static str` per process. `DdsType::type_name`
    // hands out a `&'static str` because a compiled-in type's name is a
    // constant; this run's is not, and it lives as long as the run.
    let leaked: &'static str = Box::leak(loaded.type_name.clone().into_boxed_str());
    let mut request = carrier::request_for(
        &loaded.type_name,
        options.data_representation,
        loaded.extensibility,
        false,
        runtime.pl_cdr1_reading,
    );
    request.defaulted_discriminator_member = options.defaulted_discriminator_member;
    if !carrier::install(carrier::Context {
        model: loaded,
        request,
        type_name: leaked,
        disable_type_info: options.disable_type_info,
    }) {
        eprintln!("ERROR: the run's type was installed twice");
        return EXIT_ERROR;
    }

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = Arc::clone(&stop);
        let _ = ctrlc::set_handler(move || stop.store(true, Ordering::SeqCst));
    }

    let participant = match DomainParticipantFactory::instance().create_participant_with_config(
        i32::try_from(options.domain_id).unwrap_or(0),
        DomainParticipantQos::default(),
        runtime,
    ) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ERROR creating participant: {e}");
            return EXIT_ERROR;
        }
    };

    if let Err(e) = participant.set_minimal_type_object_camp(options.minimal_hash_camp) {
        eprintln!("ERROR selecting minimal-hash camp: {e}");
        return EXIT_ERROR;
    }
    if let Err(e) = participant.set_minimal_type_flags_kept(options.minimal_type_flags_kept) {
        eprintln!("ERROR selecting the minimal type flags: {e}");
        return EXIT_ERROR;
    }

    let result = if options.publish {
        app::publish(&participant, &options, &stop)
    } else {
        app::subscribe(&participant, &options, &stop)
    };
    match result {
        Ok(()) => EXIT_OK,
        Err(e) => {
            eprintln!("ERROR: {e}");
            EXIT_ERROR
        }
    }
}

/// The settings the OMG dds-xtypes suite needs of a ZeroDDS process that
/// meets the other vendors' `test_main`, each unless the environment names
/// it (`named`). The runtime's defaults stay the readings of the
/// specifications; this client sets them so that a run without
/// `run_configs.sh` (the official matrix starts the binary without an
/// environment) gets them as well, as `omg-shape-main` does for dds-rtps
/// (README "Where the suite expects another answer than the clause").
fn suite_settings(config: &mut zerodds_dcps::runtime::RuntimeConfig, named: impl Fn(&str) -> bool) {
    // OpenDDS and RTI Connext (as recorded) send the ACKNACK of a reader
    // that holds nothing with bitmapBase 0 and numBits 0, which DDSI-RTPS
    // 2.5 section 8.3.5.5 makes invalid; by default the rest of such a
    // datagram is dropped (section 8.3.4.1 rule 6), and with it the SEDP
    // data that follows in it.
    if !named("ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO") {
        config.accept_empty_acknack_base_zero = true;
    }
    // Cyclone DDS and RTI Connext write the length of a PL_CDR1 parameter
    // rounded up to four (DDSI-RTPS 2.5 section 9.4.2.11); XTypes 1.3
    // section 7.4.1.2.2 has the exact length, which the default reader
    // holds a parameter to (official workflow, Cyclone 11.0.1 -> ZeroDDS
    // `primitives_struct_mutable` under `-x 1`).
    if !named("ZERODDS_WIRE_PL_CDR1_READING") {
        config.pl_cdr1_reading = zerodds_ast_layout::PlCdr1Reading::Tolerant;
    }
    // Cyclone DDS 11.0.1's reader announces no TypeConsistencyEnforcement
    // (PID 0x0074) with its default policy; XTypes 1.3 section 7.6.3.4.1
    // makes such a reader DISALLOW_TYPE_COERCION, and the suite expects
    // the pair to match (ZeroDDS -> Cyclone `ext_appendable_struct_2`,
    // `_3`, `ext_mutable_struct_2` to `_5`, `union_different_order_2`).
    if !named("ZERODDS_TYPE_CONSISTENCY_ABSENT") {
        config.absent_type_consistency = zerodds_dcps::runtime::AbsentTypeConsistency::Allow;
    }
}

/// `--print-typeid`. The suite compares the printed hash between two
/// products for the same type file, so the two lines name which form
/// they carry.
fn print_typeid(
    loaded: &model::Model,
    version: u8,
    form: zerodds_types::from_ast::TypeObjectForm,
) -> i32 {
    if version == 1 {
        return print_type_id_v1(loaded);
    }
    // The minimal object in the form the runtime projects it to
    // (`DomainParticipant::set_minimal_type_object_camp`,
    // `set_minimal_type_flags_kept`), so the printed hash is the announced
    // one.
    let minimal = zerodds_types::from_ast::minimal_type_object_in(&loaded.ast, loaded.type_, form)
        .ok()
        .and_then(|m| {
            zerodds_types::hash::compute_minimal_hash_with(&m, form.minimal_member_trailing_octet)
                .ok()
        });
    let complete = match &loaded.type_object {
        zerodds_types::type_object::TypeObject::Complete(c) => {
            zerodds_types::hash::compute_complete_hash(c).ok()
        }
        _ => None,
    };
    match (minimal, complete) {
        (Some(m), Some(c)) => {
            println!("Minimal Type Object V2 - Equivalence Hash: {}", hex(&m.0));
            println!("Complete Type Object V2 - Equivalence Hash: {}", hex(&c.0));
            EXIT_OK
        }
        _ => {
            eprintln!("ERROR: cannot compute equivalence hash");
            EXIT_ERROR
        }
    }
}

/// `--print-typeid --type-object-version 1`: the `_TypeId` of DDS-XTypes
/// 1.1 (XTypes 1.3 Annex D keeps it as `TypeIdV1`), from the type's
/// TypeObject of 1.1 (`zerodds_types::type_object_v1`, §7.3.4.1.2 of 1.1),
/// printed as the suite reads it (`print_ids.py`: "Type ID: <number>").
fn print_type_id_v1(loaded: &model::Model) -> i32 {
    use zerodds_types::type_object_v1::TypeId;
    let mut registry = zerodds_types::resolve::TypeRegistry::new();
    if let Err(e) = zerodds_types::from_ast::register_all(&loaded.ast, &mut registry) {
        eprintln!("ERROR: cannot register the types: {e}");
        return EXIT_ERROR;
    }
    let object = match zerodds_types::type_object_v1::convert::from_complete(
        &loaded.type_identifier,
        &registry,
    ) {
        Ok(object) => object,
        Err(e) => {
            eprintln!("ERROR: no TypeObject of DDS-XTypes 1.1 for this type: {e}");
            return EXIT_ERROR;
        }
    };
    match object.the_type.first() {
        Some(TypeId::Constructed { id, .. }) => {
            println!("Type ID: {id}");
            EXIT_OK
        }
        Some(TypeId::Primitive(kind)) => {
            println!("Type ID: {kind}");
            EXIT_OK
        }
        None => {
            eprintln!("ERROR: the TypeObject of DDS-XTypes 1.1 names no type");
            EXIT_ERROR
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The suite's settings apply over the runtime's defaults, and a
    /// setting the environment names keeps the value the environment gave
    /// it.
    #[test]
    fn the_suite_settings_apply_unless_the_environment_names_them() {
        let mut config = zerodds_dcps::runtime::RuntimeConfig::default();
        suite_settings(&mut config, |_| false);
        assert!(config.accept_empty_acknack_base_zero);
        assert_eq!(
            config.pl_cdr1_reading,
            zerodds_ast_layout::PlCdr1Reading::Tolerant
        );
        assert_eq!(
            config.absent_type_consistency,
            zerodds_dcps::runtime::AbsentTypeConsistency::Allow
        );

        let defaults = zerodds_dcps::runtime::RuntimeConfig::default();
        let mut config = zerodds_dcps::runtime::RuntimeConfig::default();
        suite_settings(&mut config, |_| true);
        assert_eq!(
            config.accept_empty_acknack_base_zero,
            defaults.accept_empty_acknack_base_zero
        );
        assert_eq!(config.pl_cdr1_reading, defaults.pl_cdr1_reading);
        assert_eq!(
            config.absent_type_consistency,
            defaults.absent_type_consistency
        );
    }

    /// The defaults of the axes the suite answers otherwise than the
    /// clause are the suite's; the flags give the clause's reading.
    #[test]
    fn the_axis_defaults_are_the_suites() {
        let options = cli::Options::default();
        assert_eq!(
            options.key_member_bounds,
            zerodds_types::assignability::KeyMemberBounds::Ignored
        );
        assert_eq!(
            options.defaulted_discriminator_member,
            zerodds_ast_layout::DefaultedDiscriminatorMember::StreamValue
        );
    }
}

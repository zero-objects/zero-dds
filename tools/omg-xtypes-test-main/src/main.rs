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
        return print_typeid(
            &loaded,
            options.type_object_version,
            options.minimal_hash_camp,
        );
    }

    // The leak is one `&'static str` per process. `DdsType::type_name`
    // hands out a `&'static str` because a compiled-in type's name is a
    // constant; this run's is not, and it lives as long as the run.
    let leaked: &'static str = Box::leak(loaded.type_name.clone().into_boxed_str());
    let mut request = carrier::request_for(
        &loaded.type_name,
        options.data_representation,
        loaded.extensibility,
        false,
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
    let runtime = match base {
        Ok(base) => zerodds_dcps::runtime::RuntimeConfig {
            key_member_bounds: options.key_member_bounds,
            strong_assignability: options
                .strong_assignability
                .unwrap_or(base.strong_assignability),
            ..base
        },
        Err(e) => {
            eprintln!("ERROR reading the runtime configuration: {e}");
            return EXIT_ERROR;
        }
    };
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

/// `--print-typeid`. The suite compares the printed hash between two
/// products for the same type file, so the two lines name which form
/// they carry.
fn print_typeid(loaded: &model::Model, version: u8, rti_camp: bool) -> i32 {
    if version == 1 {
        // XTypes 1.3 §7.3.4.2: TypeObject V1 is the pre-1.2 form. This
        // client does not emit it, and says so rather than printing a V2
        // hash under a V1 label.
        eprintln!("ERROR: --type-object-version 1 is not implemented");
        return EXIT_ERROR;
    }
    let minimal =
        zerodds_types::from_ast::minimal_type_object_with(&loaded.ast, loaded.type_, rti_camp)
            .ok()
            .and_then(|m| zerodds_types::hash::compute_minimal_hash_with(&m, rti_camp).ok());
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

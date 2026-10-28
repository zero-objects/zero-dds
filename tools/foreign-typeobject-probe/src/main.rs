// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-foreign-typeobject-probe` — a foreign vendor's samples, read
//! through the `TypeObject` that vendor announced.
//!
//! Every reflective measurement in this tree used a `TypeObject` this tree
//! wrote. This one does not: it takes the `TypeInformation` a peer's writer
//! announced over SEDP, fetches the complete `TypeObject` from that peer's
//! TypeLookup service (DDS-XTypes 1.3 §7.6.3.3), builds a `DynamicType`
//! from it (`zerodds_types::dynamic::to_ast::dynamic_type`) and decodes the
//! peer's own samples with it. A type object this project writes and reads
//! consistently, and a vendor reads differently, shows up here.
//!
//! ```text
//! zerodds-foreign-typeobject-probe --domain 47 --topic ProbeTopic \
//!     --type-name probe::Sample --count 3 [--timeout-s 30] [--dialect rti]
//! ```
//!
//! Prints one line `TYPE <scoped name>` and then one JSON object per sample,
//! in the value form `zerodds-wire-codec` reads and writes.

// A command-line probe: its output is what it prints.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process::ExitCode;
use std::time::{Duration, Instant};

use zerodds_ast_layout::Dialect;
use zerodds_ast_layout::key::{Encoding, Endianness};
use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample};
use zerodds_rtps::wire_types::{EntityKind, GuidPrefix};
use zerodds_types::dynamic::descriptor::ExtensibilityKind;
use zerodds_types::dynamic::to_ast::dynamic_type;
use zerodds_types::{TypeIdentifier, TypeInformation};
use zerodds_wire_codec::{ProbeRequest, decode};

struct Args {
    domain: i32,
    topic: String,
    type_name: String,
    count: usize,
    timeout: Duration,
    /// The writer's wire dialect: where vendors frame the same type
    /// differently (PL_CDR1 parameter headers, `wstring`), the reader has
    /// to be told which form to expect.
    dialect: Dialect,
}

fn parse_args() -> Result<Args, String> {
    let mut domain = None;
    let mut topic = None;
    let mut type_name = None;
    let mut count = 1;
    let mut timeout = Duration::from_secs(30);
    let mut dialect = Dialect::Cyclone;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--domain" => domain = Some(value.parse().map_err(|_| "--domain: not a number")?),
            "--topic" => topic = Some(value),
            "--type-name" => type_name = Some(value),
            "--count" => count = value.parse().map_err(|_| "--count: not a number")?,
            "--timeout-s" => {
                timeout =
                    Duration::from_secs(value.parse().map_err(|_| "--timeout-s: not a number")?);
            }
            "--dialect" => {
                dialect =
                    Dialect::parse(&value).ok_or_else(|| format!("--dialect: unknown {value}"))?;
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(Args {
        domain: domain.ok_or("--domain is required")?,
        topic: topic.ok_or("--topic is required")?,
        type_name: type_name.ok_or("--type-name is required")?,
        count,
        timeout,
        dialect,
    })
}

/// What the first writer SEDP reports on a topic with a `TypeInformation`
/// announced.
struct Announced {
    /// The complete `TypeIdentifier` of its type.
    type_id: TypeIdentifier,
    /// The complete identifiers of the types it depends on, as far as the
    /// writer listed them (§7.6.3.2.2).
    dependents: Vec<TypeIdentifier>,
    /// The writer's participant.
    peer: GuidPrefix,
    /// Whether the writer is a WithKey writer: a reader matches only a
    /// writer of its own topic kind (DDSI-RTPS 2.5 §9.3.1.2).
    keyed: bool,
}

fn announced_type(rt: &DcpsRuntime, topic: &str) -> Option<Announced> {
    let sedp = rt.sedp.lock().ok()?;
    sedp.cache().publications().find_map(|publication| {
        let data = &publication.data;
        if data.topic_name != topic {
            return None;
        }
        let blob = data.type_information.as_ref()?;
        let information = TypeInformation::from_bytes_le(blob).ok()?;
        let dependents = information
            .complete
            .dependent_typeids
            .iter()
            .map(|with_size| with_size.type_id.clone())
            .collect();
        Some(Announced {
            type_id: information.complete.typeid_with_size.type_id,
            dependents,
            peer: data.participant_key.prefix,
            keyed: data.key.entity_id.entity_kind != EntityKind::UserWriterNoKey,
        })
    })
}

fn run(args: &Args) -> Result<(), String> {
    let prefix = GuidPrefix::from_bytes(*b"zdftoprobe00");
    let rt = DcpsRuntime::start(args.domain, prefix, RuntimeConfig::default())
        .map_err(|e| format!("runtime: {e}"))?;
    let deadline = Instant::now() + args.timeout;

    let Announced {
        type_id,
        dependents,
        peer,
        keyed,
    } = loop {
        if let Some(found) = announced_type(&rt, &args.topic) {
            break found;
        }
        if Instant::now() > deadline {
            return Err(format!(
                "no writer on {} announced a TypeInformation",
                args.topic
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let TypeIdentifier::EquivalenceHashComplete(hash) = &type_id else {
        return Err(format!(
            "the announced complete identifier is not a hash: {type_id:?}"
        ));
    };
    // getTypes answers for the identifiers asked; a vendor that does not add
    // the closure (Cyclone DDS 11) leaves the nested types to a request of
    // their own, so they are asked for together.
    let mut asked = vec![type_id.clone()];
    asked.extend(dependents);
    rt.send_type_lookup_request(peer, &asked)
        .map_err(|e| format!("type lookup request: {e}"))?
        .ok_or("the peer announced no TypeLookup locator")?;
    // A writer that lists no dependencies (RTI Connext 7.7 under the
    // dds-xtypes suite) leaves them to getTypeDependencies (§7.6.3.3.4);
    // the runtime asks getTypes for every identifier its reply names.
    rt.send_type_dependencies_request(peer, core::slice::from_ref(&type_id))
        .map_err(|e| format!("type dependencies request: {e}"))?;
    let ty = loop {
        let found = {
            let server = rt
                .type_lookup_server
                .lock()
                .map_err(|_| "type lookup server poisoned")?;
            server
                .registry
                .get_complete(hash)
                .cloned()
                .map(|object| dynamic_type(&object, &server.registry))
        };
        // The reply for the nested types may come after the root's; a model
        // that cannot resolve one yet is asked again until the deadline.
        match found {
            Some(Ok(ty)) => break ty,
            Some(Err(e)) if Instant::now() > deadline => {
                return Err(format!("DynamicType: {e:?}"));
            }
            _ => {}
        }
        if Instant::now() > deadline {
            return Err("no TypeLookup reply carried the announced complete TypeObject".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    // Registered once the writer's topic kind is known.
    let (_reader, rx) = rt
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: args.topic.clone(),
                type_name: args.type_name.clone(),
                reliable: true,
                ..Default::default()
            },
            keyed,
        )
        .map_err(|e| format!("reader: {e}"))?;
    let type_name = ty.qualified_name();
    println!("TYPE {type_name}");

    let mut read = 0;
    while read < args.count {
        let left = deadline.saturating_duration_since(Instant::now());
        let sample = rx
            .recv_timeout(left)
            .map_err(|_| format!("{read} of {} samples arrived", args.count))?;
        let UserSample::Alive {
            payload,
            representation,
            big_endian,
            ..
        } = sample
        else {
            continue;
        };
        let xcdr2 = representation == 1;
        let encoding = match (ty.extensibility(), xcdr2) {
            (Some(ExtensibilityKind::Mutable), false) => Encoding::PlCdr1,
            (Some(ExtensibilityKind::Mutable), true) => Encoding::PlCdr2,
            (Some(ExtensibilityKind::Appendable), true) => Encoding::DelimitedCdr,
            (_, true) => Encoding::PlainCdr2,
            (_, false) => Encoding::PlainCdr1,
        };
        let endianness = if big_endian {
            Endianness::Big
        } else {
            Endianness::Little
        };
        let request = ProbeRequest::in_dialect(&type_name, encoding, endianness, args.dialect);
        let value: serde_json::Value =
            decode(ty.ast(), &request, &payload).map_err(|e| format!("decode: {e:?}"))?;
        println!("{value}");
        read += 1;
    }
    rt.shutdown();
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(why) => {
            eprintln!("zerodds-foreign-typeobject-probe: {why}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("zerodds-foreign-typeobject-probe: {why}");
            ExitCode::FAILURE
        }
    }
}

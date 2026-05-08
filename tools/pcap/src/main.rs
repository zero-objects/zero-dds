// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-pcap` — offline pcap-Datei lesen und RTPS-Submessages
//! drucken.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::collections::BTreeMap;
use std::env;
use std::process::ExitCode;

use zerodds_pcap::{Command, FileArgs, PcapIter, find_rtps_offset, parse_args, parse_file_header};
use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("zerodds-pcap {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let cmd = match parse_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            print_help();
            return ExitCode::from(2);
        }
    };
    match cmd {
        Command::Parse(a) => run_parse(&a),
        Command::Stats(a) => run_stats(&a),
    }
}

fn run_parse(a: &FileArgs) -> ExitCode {
    let bytes = match std::fs::read(&a.file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", a.file);
            return ExitCode::from(3);
        }
    };
    let (hdr, be) = match parse_file_header(&bytes) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: pcap header: {e}");
            return ExitCode::from(3);
        }
    };
    println!(
        "pcap: linktype={} snaplen={} version={}.{}",
        hdr.linktype, hdr.snaplen, hdr.version_major, hdr.version_minor
    );

    let mut iter = PcapIter::new(&bytes, be);
    let mut frame_no: u64 = 0;
    let mut rtps_count: u64 = 0;
    loop {
        match iter.next_record() {
            Ok(Some((rh, payload))) => {
                frame_no += 1;
                let Some(off) = find_rtps_offset(payload) else {
                    continue;
                };
                rtps_count += 1;
                match decode_datagram(&payload[off..]) {
                    Ok(dg) => {
                        let ts = format!("{}.{:06}", rh.ts_sec, rh.ts_usec);
                        println!(
                            "[{frame_no:>5}] ts={ts} guid_prefix={} submessages={}",
                            hex12(&dg.header.guid_prefix.0),
                            dg.submessages.len()
                        );
                        for sm in &dg.submessages {
                            println!("    · {}", submessage_label(sm));
                        }
                    }
                    Err(e) => {
                        println!("[{frame_no:>5}] decode_error: {e:?}");
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("error: pcap iter: {e}");
                return ExitCode::from(3);
            }
        }
    }
    println!("done · frames={frame_no} rtps={rtps_count}");
    ExitCode::SUCCESS
}

fn run_stats(a: &FileArgs) -> ExitCode {
    let bytes = match std::fs::read(&a.file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", a.file);
            return ExitCode::from(3);
        }
    };
    let (_hdr, be) = match parse_file_header(&bytes) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: pcap header: {e}");
            return ExitCode::from(3);
        }
    };
    let mut counts: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut total_frames: u64 = 0;
    let mut rtps_frames: u64 = 0;

    let mut iter = PcapIter::new(&bytes, be);
    loop {
        match iter.next_record() {
            Ok(Some((_rh, payload))) => {
                total_frames += 1;
                let Some(off) = find_rtps_offset(payload) else {
                    continue;
                };
                rtps_frames += 1;
                match decode_datagram(&payload[off..]) {
                    Ok(dg) => {
                        for sm in &dg.submessages {
                            *counts.entry(submessage_kind(sm)).or_insert(0) += 1;
                        }
                    }
                    Err(_) => {
                        *counts.entry("decode-error").or_insert(0) += 1;
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("error: pcap iter: {e}");
                return ExitCode::from(3);
            }
        }
    }
    println!("=== pcap stats: {} ===", a.file);
    println!("frames total:   {total_frames}");
    println!("frames w/ RTPS: {rtps_frames}");
    println!();
    println!("submessage counts:");
    for (k, v) in &counts {
        println!("  {v:>10}  {k}");
    }
    ExitCode::SUCCESS
}

fn hex12(b: &[u8; 12]) -> String {
    let mut s = String::with_capacity(24);
    for byte in b {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

fn submessage_kind(sm: &ParsedSubmessage) -> &'static str {
    match sm {
        ParsedSubmessage::Data(_) => "DATA",
        ParsedSubmessage::DataFrag(_) => "DATA_FRAG",
        ParsedSubmessage::Heartbeat(_) => "HEARTBEAT",
        ParsedSubmessage::HeartbeatFrag(_) => "HEARTBEAT_FRAG",
        ParsedSubmessage::AckNack(_) => "ACKNACK",
        ParsedSubmessage::NackFrag(_) => "NACK_FRAG",
        ParsedSubmessage::Gap(_) => "GAP",
        ParsedSubmessage::HeaderExtension(_) => "HEADER_EXTENSION",
        ParsedSubmessage::InfoSource(_) => "INFO_SOURCE",
        ParsedSubmessage::InfoReply(_) => "INFO_REPLY",
        ParsedSubmessage::InfoTimestamp(_) => "INFO_TIMESTAMP",
        ParsedSubmessage::Unknown { .. } => "UNKNOWN",
    }
}

fn submessage_label(sm: &ParsedSubmessage) -> String {
    match sm {
        ParsedSubmessage::Unknown { id, flags } => {
            format!("UNKNOWN id=0x{id:02x} flags=0x{flags:02x}")
        }
        other => submessage_kind(other).to_string(),
    }
}

fn print_help() {
    let v = env!("CARGO_PKG_VERSION");
    println!(
        "zerodds-pcap {v}\n\
         Offline pcap parser — extract and decode RTPS submessages.\n\
\n\
         USAGE:\n  \
           zerodds-pcap <SUBCOMMAND> <FILE>\n\
\n\
         SUBCOMMANDS:\n  \
           parse <FILE>   Print every RTPS frame with submessage list\n  \
           stats <FILE>   Print aggregate counts per submessage kind\n\
\n\
         GLOBAL OPTIONS:\n  \
           -h, --help     Show this message\n  \
           -V, --version  Print version\n\
\n\
         EXIT CODES:\n  \
           0    success\n  \
           2    CLI parse error\n  \
           3    file / pcap / RTPS decode error\n"
    );
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-chaos` — Chaos-Engineering-CLI fuer DDS-Netzwerke.
//!
//! Crate `zerodds-chaos`. Safety classification: **COMFORT**.
//!
//! # Sub-Commands
//!
//! * `proxy` — In-Process UDP-Chaos-Proxy (kein root).
//! * `tc-apply` — Linux netem qdisc setzen (root).
//! * `tc-clear` — netem qdisc entfernen.
//! * `help` — Usage-Print.

#![warn(missing_docs)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::net::SocketAddr;
use std::process::ExitCode;
use std::time::Duration;

use zerodds_chaos::endpoint_flap::{self, FlapConfig};
use zerodds_chaos::partition::{self, PartitionConfig};
use zerodds_chaos::proxy::{ChaosConfig, UdpChaosProxy};
use zerodds_chaos::tc::{self, NetemConfig};

fn parse_duration_ms(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    let n: u64 = if let Some(rest) = s.strip_suffix("ms") {
        rest.parse().map_err(|e| format!("ms: {e}"))?
    } else if let Some(rest) = s.strip_suffix('s') {
        let v: u64 = rest.parse().map_err(|e| format!("s: {e}"))?;
        v * 1000
    } else {
        s.parse().map_err(|e| format!("ms (default): {e}"))?
    };
    Ok(Duration::from_millis(n))
}

struct ProxyArgs {
    bind: SocketAddr,
    forward: SocketAddr,
    cfg: ChaosConfig,
    duration: Option<Duration>,
}

fn parse_proxy(argv: &[String]) -> Result<ProxyArgs, String> {
    let mut bind: Option<SocketAddr> = None;
    let mut forward: Option<SocketAddr> = None;
    let mut cfg = ChaosConfig::default();
    let mut duration: Option<Duration> = None;
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        let val = || {
            argv.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("missing value for {a}"))
        };
        match a.as_str() {
            "--listen" => {
                bind = Some(val()?.parse().map_err(|e| format!("listen: {e}"))?);
                i += 2;
            }
            "--forward" => {
                forward = Some(val()?.parse().map_err(|e| format!("forward: {e}"))?);
                i += 2;
            }
            "--loss" => {
                cfg.loss = val()?.parse().map_err(|e| format!("loss: {e}"))?;
                i += 2;
            }
            "--burst" => {
                cfg.loss_burst = val()?.parse().map_err(|e| format!("burst: {e}"))?;
                i += 2;
            }
            "--duplicate" => {
                cfg.duplicate = val()?.parse().map_err(|e| format!("duplicate: {e}"))?;
                i += 2;
            }
            "--reorder" => {
                cfg.reorder = val()?.parse().map_err(|e| format!("reorder: {e}"))?;
                i += 2;
            }
            "--jitter" => {
                let s = val()?;
                let parts: Vec<&str> = s.split(':').collect();
                let (lo, hi) = match parts.as_slice() {
                    [single] => (parse_duration_ms(single)?, parse_duration_ms(single)?),
                    [a, b] => (parse_duration_ms(a)?, parse_duration_ms(b)?),
                    _ => return Err("--jitter needs N or LO:HI".into()),
                };
                cfg.jitter_min = lo;
                cfg.jitter_max = hi;
                i += 2;
            }
            "--seed" => {
                cfg.seed = val()?.parse().map_err(|e| format!("seed: {e}"))?;
                i += 2;
            }
            "--duration" => {
                duration = Some(parse_duration_ms(&val()?)?);
                i += 2;
            }
            other => return Err(format!("unknown proxy flag: {other}")),
        }
    }
    let bind = bind.ok_or("--listen ADDR:PORT required")?;
    let forward = forward.ok_or("--forward ADDR:PORT required")?;
    Ok(ProxyArgs {
        bind,
        forward,
        cfg,
        duration,
    })
}

fn parse_tc_apply(argv: &[String]) -> Result<(String, NetemConfig), String> {
    let mut iface: Option<String> = None;
    let mut cfg = NetemConfig::default();
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        let val = || {
            argv.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("missing value for {a}"))
        };
        match a.as_str() {
            "--interface" | "--dev" => {
                iface = Some(val()?);
                i += 2;
            }
            "--loss" => {
                cfg.loss = val()?.parse().map_err(|e| format!("loss: {e}"))?;
                i += 2;
            }
            "--delay" => {
                cfg.delay = parse_duration_ms(&val()?)?;
                i += 2;
            }
            "--jitter" => {
                cfg.jitter = parse_duration_ms(&val()?)?;
                i += 2;
            }
            "--duplicate" => {
                cfg.duplicate = val()?.parse().map_err(|e| format!("duplicate: {e}"))?;
                i += 2;
            }
            "--reorder" => {
                cfg.reorder = val()?.parse().map_err(|e| format!("reorder: {e}"))?;
                i += 2;
            }
            "--correlation" => {
                cfg.loss_correlation_pct =
                    val()?.parse().map_err(|e| format!("correlation: {e}"))?;
                i += 2;
            }
            other => return Err(format!("unknown tc-apply flag: {other}")),
        }
    }
    let iface = iface.ok_or("--interface NAME required")?;
    Ok((iface, cfg))
}

fn print_help() {
    println!(
        "zerodds-chaos — Chaos-Engineering CLI

USAGE:
  zerodds-chaos proxy --listen ADDR:PORT --forward ADDR:PORT \\
            [--loss FRAC] [--burst N] [--duplicate FRAC] \\
            [--reorder FRAC] [--jitter MS or LO:HI] \\
            [--seed N] [--duration N(ms|s)]

  zerodds-chaos tc-apply --interface eth0 \\
            [--loss FRAC] [--delay 10ms] [--jitter 5ms] \\
            [--duplicate FRAC] [--reorder FRAC] [--correlation PCT]

  zerodds-chaos tc-clear --interface eth0

  zerodds-chaos partition --group-a 10.0.0.1,10.0.0.2 \\
                      --group-b 10.0.0.10 [--duration 30s]
  zerodds-chaos partition-clear

  zerodds-chaos endpoint-flap --interface eth0 \\
                          [--interval 5s] [--total 60s] [--start-down]

EXAMPLES:
  # 10% Loss + Burst-2 + 5-15ms Jitter, 60s
  zerodds-chaos proxy --listen 0.0.0.0:7400 --forward 192.0.2.10:7400 \\
            --loss 0.1 --burst 2 --jitter 5ms:15ms --duration 60s

  # Linux Network-Chaos auf eth0
  sudo zerodds-chaos tc-apply --interface eth0 --loss 0.05 --delay 50ms --jitter 10ms
  sudo zerodds-chaos tc-clear --interface eth0
"
    );
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() < 2 || argv[1] == "help" || argv[1] == "--help" || argv[1] == "-h" {
        print_help();
        return ExitCode::SUCCESS;
    }
    let cmd = &argv[1];
    let rest: Vec<String> = argv.iter().skip(2).cloned().collect();
    match cmd.as_str() {
        "proxy" => {
            let p = match parse_proxy(&rest) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: {e}");
                    print_help();
                    return ExitCode::from(2);
                }
            };
            let mut proxy = match UdpChaosProxy::new(p.bind, p.forward, p.cfg.clone()) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("proxy bind error: {e}");
                    return ExitCode::from(1);
                }
            };
            println!(
                "proxy: {} -> {} loss={:.3} burst={} dup={:.3} reorder={:.3} jitter=[{:?},{:?}]",
                p.bind,
                p.forward,
                p.cfg.loss,
                p.cfg.loss_burst,
                p.cfg.duplicate,
                p.cfg.reorder,
                p.cfg.jitter_min,
                p.cfg.jitter_max
            );
            let dur = p.duration.unwrap_or(Duration::from_secs(86_400));
            if let Err(e) = proxy.run_for(dur) {
                eprintln!("proxy error: {e}");
                return ExitCode::from(1);
            }
            let s = &proxy.stats;
            println!(
                "stats: forwarded={} dropped={} duplicated={} reordered={} bytes_in={} bytes_out={}",
                s.forwarded, s.dropped, s.duplicated, s.reordered, s.bytes_in, s.bytes_out
            );
            ExitCode::SUCCESS
        }
        "tc-apply" => {
            let (iface, cfg) = match parse_tc_apply(&rest) {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("error: {e}");
                    print_help();
                    return ExitCode::from(2);
                }
            };
            match tc::apply(&iface, &cfg) {
                Ok(()) => {
                    println!("tc-apply: netem on {iface} ok");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("tc-apply: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "tc-clear" => {
            // Re-parse just for --interface.
            let mut iface: Option<String> = None;
            let mut i = 0;
            while i < rest.len() {
                if rest[i] == "--interface" || rest[i] == "--dev" {
                    iface = rest.get(i + 1).cloned();
                    i += 2;
                } else {
                    i += 1;
                }
            }
            let Some(iface) = iface else {
                eprintln!("tc-clear: --interface NAME required");
                return ExitCode::from(2);
            };
            match tc::clear(&iface) {
                Ok(()) => {
                    println!("tc-clear: ok on {iface}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("tc-clear: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "partition" => run_partition(&rest),
        "partition-clear" => match partition::clear() {
            Ok(()) => {
                println!("partition-clear: ok");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("partition-clear: {e}");
                ExitCode::from(1)
            }
        },
        "endpoint-flap" => run_flap(&rest),
        other => {
            eprintln!("unknown subcommand: {other}");
            print_help();
            ExitCode::from(2)
        }
    }
}

fn run_partition(argv: &[String]) -> ExitCode {
    let mut group_a: Vec<String> = Vec::new();
    let mut group_b: Vec<String> = Vec::new();
    let mut duration: Option<Duration> = None;
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--group-a" => {
                if let Some(v) = argv.get(i + 1) {
                    group_a = v.split(',').map(|s| s.trim().to_string()).collect();
                }
                i += 2;
            }
            "--group-b" => {
                if let Some(v) = argv.get(i + 1) {
                    group_b = v.split(',').map(|s| s.trim().to_string()).collect();
                }
                i += 2;
            }
            "--duration" => {
                if let Some(v) = argv.get(i + 1) {
                    if let Ok(d) = parse_duration_ms(v) {
                        duration = Some(d);
                    }
                }
                i += 2;
            }
            other => {
                eprintln!("unknown partition flag: {other}");
                return ExitCode::from(2);
            }
        }
    }
    if group_a.is_empty() || group_b.is_empty() {
        eprintln!("partition: --group-a IP,IP --group-b IP,IP required");
        return ExitCode::from(2);
    }
    let cfg = PartitionConfig { group_a, group_b };
    if let Err(e) = partition::apply(&cfg) {
        eprintln!("partition apply: {e}");
        return ExitCode::from(1);
    }
    println!(
        "partition: A({}) vs B({}) up",
        cfg.group_a.len(),
        cfg.group_b.len()
    );
    if let Some(d) = duration {
        std::thread::sleep(d);
        if let Err(e) = partition::clear() {
            eprintln!("partition clear: {e}");
            return ExitCode::from(1);
        }
        println!("partition: cleared after {} s", d.as_secs());
    } else {
        println!("partition: applied (no --duration → manuell mit partition-clear loeschen)");
    }
    ExitCode::SUCCESS
}

fn run_flap(argv: &[String]) -> ExitCode {
    let mut iface: Option<String> = None;
    let mut interval = Duration::from_secs(5);
    let mut total = Duration::from_secs(60);
    let mut start_down = false;
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--interface" | "--dev" => {
                iface = argv.get(i + 1).cloned();
                i += 2;
            }
            "--interval" => {
                if let Some(v) = argv.get(i + 1) {
                    if let Ok(d) = parse_duration_ms(v) {
                        interval = d;
                    }
                }
                i += 2;
            }
            "--total" => {
                if let Some(v) = argv.get(i + 1) {
                    if let Ok(d) = parse_duration_ms(v) {
                        total = d;
                    }
                }
                i += 2;
            }
            "--start-down" => {
                start_down = true;
                i += 1;
            }
            other => {
                eprintln!("unknown endpoint-flap flag: {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(interface) = iface else {
        eprintln!("endpoint-flap: --interface NAME required");
        return ExitCode::from(2);
    };
    let cfg = FlapConfig {
        interface,
        interval,
        total,
        start_down,
    };
    println!(
        "endpoint-flap: dev={} interval={:?} total={:?}",
        cfg.interface, cfg.interval, cfg.total
    );
    match endpoint_flap::run(&cfg) {
        Ok(s) => {
            println!(
                "endpoint-flap: stats up={} down={} total={:?}",
                s.up_count, s.down_count, s.total
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("endpoint-flap: {e}");
            ExitCode::from(1)
        }
    }
}

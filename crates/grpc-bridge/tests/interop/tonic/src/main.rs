// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A tonic client of zerodds-grpc-bridged for
//! `crates/grpc-bridge/tests/cross_vendor.rs` (zerodds-grpc-bridge-1.0 §10,
//! §12.3): the stubs tonic-prost-build generates from
//! `crates/grpc-bridge/proto/zerodds/chat/v1/bridge.proto`, over h2c.
//!
//! ```text
//! tonic-client ADDRESS publish HEX...    one Publish stream of these payloads
//! tonic-client ADDRESS publish-one HEX   one PublishOne
//! tonic-client ADDRESS subscribe N       a Subscribe stream until N samples
//! tonic-client ADDRESS catalog           one Catalog
//! tonic-client ADDRESS echo SECONDS HEX...
//! ```
//!
//! Each result is one line on stdout: `ack seq=S`, `sample type=T topic=P
//! flags=F payload=HEX`, `topic dds_name=D dds_type=T grpc_service=S
//! idl_url=U`; a call that ends with a status other than OK prints `status
//! code=C message=M` and exits 3.
//!
//! `echo` is the client of the Compose file of §12.3: it opens a Subscribe
//! stream, then publishes the payloads with PublishOne once a second until
//! the stream brings a sample that is not one of them (the answer of the
//! Cyclone DDS participant) or SECONDS pass; it prints that sample and
//! exits 0, or exits 1.

use std::process::ExitCode;
use std::time::Duration;

pub mod chat {
    tonic::include_proto!("zerodds.chat.v1");
}

use chat::chat_message_stream_client::ChatMessageStreamClient;
use chat::{CatalogReq, Sample, SubscribeReq};
use tonic::transport::Channel;

fn payload(text: &str) -> Vec<u8> {
    assert!(text.len() % 2 == 0, "not hex: {text}");
    (0..text.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&text[i..i + 2], 16).unwrap_or_else(|_| panic!("not hex: {text}"))
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sample_line(s: &Sample) -> String {
    format!(
        "sample type={} topic={} flags={} payload={}",
        s.type_name,
        s.topic_name,
        s.flags,
        hex(&s.cdr_payload)
    )
}

fn failed(status: &tonic::Status) -> ExitCode {
    println!(
        "status code={} message={}",
        status.code() as i32,
        status.message()
    );
    ExitCode::from(3)
}

async fn connect(address: &str) -> Channel {
    let endpoint = Channel::from_shared(format!("http://{address}"))
        .unwrap_or_else(|e| panic!("{address}: {e}"))
        .timeout(Duration::from_secs(20));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        match endpoint.connect().await {
            Ok(channel) => return channel,
            // The Compose file starts the client with the bridge; the
            // bridge may not listen yet.
            Err(e) if tokio::time::Instant::now() < deadline => {
                eprintln!("connect {address}: {e}");
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(e) => panic!("connect {address}: {e}"),
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: tonic-client ADDRESS publish|publish-one|subscribe|catalog|echo [ARGS]");
        return ExitCode::from(2);
    }
    let mut client = ChatMessageStreamClient::new(connect(&args[1]).await);
    let rest = &args[3..];
    match args[2].as_str() {
        "publish" => {
            let samples: Vec<Sample> = rest
                .iter()
                .map(|h| Sample {
                    cdr_payload: payload(h),
                    ..Default::default()
                })
                .collect();
            match client.publish(tokio_stream::iter(samples)).await {
                Ok(ack) => println!("ack seq={}", ack.into_inner().sequence_number),
                Err(status) => return failed(&status),
            }
        }
        "publish-one" => {
            let sample = Sample {
                cdr_payload: payload(&rest[0]),
                ..Default::default()
            };
            match client.publish_one(sample).await {
                Ok(ack) => println!("ack seq={}", ack.into_inner().sequence_number),
                Err(status) => return failed(&status),
            }
        }
        "subscribe" => {
            let n: usize = rest[0].parse().expect("a count");
            let mut stream = match client.subscribe(SubscribeReq::default()).await {
                Ok(response) => response.into_inner(),
                Err(status) => return failed(&status),
            };
            for _ in 0..n {
                match stream.message().await {
                    Ok(Some(s)) => println!("{}", sample_line(&s)),
                    Ok(None) => {
                        println!("status code=0 message=the stream ended");
                        return ExitCode::from(3);
                    }
                    Err(status) => return failed(&status),
                }
            }
        }
        "catalog" => match client.catalog(CatalogReq::default()).await {
            Ok(resp) => {
                for t in resp.into_inner().topics {
                    println!(
                        "topic dds_name={} dds_type={} grpc_service={} idl_url={}",
                        t.dds_name, t.dds_type, t.grpc_service, t.idl_url
                    );
                }
            }
            Err(status) => return failed(&status),
        },
        "echo" => return echo(client, rest).await,
        other => {
            eprintln!("unknown command {other}");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}

async fn echo(mut client: ChatMessageStreamClient<Channel>, rest: &[String]) -> ExitCode {
    let seconds: u64 = rest[0].parse().expect("seconds");
    let sent: Vec<Vec<u8>> = rest[1..].iter().map(|h| payload(h)).collect();
    let mut stream = match client.subscribe(SubscribeReq::default()).await {
        Ok(response) => response.into_inner(),
        Err(status) => return failed(&status),
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if tokio::time::Instant::now() >= deadline {
                    eprintln!("no answer within {seconds} s");
                    return ExitCode::FAILURE;
                }
                for p in &sent {
                    let sample = Sample { cdr_payload: p.clone(), ..Default::default() };
                    match client.publish_one(sample).await {
                        Ok(ack) => println!("ack seq={}", ack.into_inner().sequence_number),
                        Err(status) => return failed(&status),
                    }
                }
            }
            message = stream.message() => match message {
                Ok(Some(s)) if sent.contains(&s.cdr_payload) => {}
                Ok(Some(s)) => {
                    println!("{}", sample_line(&s));
                    return ExitCode::SUCCESS;
                }
                Ok(None) => {
                    println!("status code=0 message=the stream ended");
                    return ExitCode::from(3);
                }
                Err(status) => return failed(&status),
            },
        }
    }
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Spec `zerodds-grpc-bridge-1.0.md` §12.3: the tonic client, a Cyclone DDS
//! participant and the bridge in Docker Compose
//! (`tests/compose/docker-compose.yml`).
//!
//! The test builds the three images (the bridge from
//! `packaging/docker/grpc-bridged/Dockerfile`) and starts the file. It needs
//! a Docker daemon with Compose, which no GitLab runner of the project has
//! (they build images with kaniko): it is a manual test on the Linux test
//! host (`ci/manual-tests.tsv`, decision 11 of
//! `internal/release/rc8-test-suite.md`). The image builds take minutes, so
//! it runs under `cargo test`, which has no time limit per test:
//! ```bash
//! cargo test -p zerodds-grpc-bridge --test compose_e2e -- --ignored
//! ```

#![cfg(feature = "std")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::path::PathBuf;
use std::process::Command;

/// The Compose file and the project name of this run; `docker compose
/// down` on drop removes the containers, the network and the images the
/// run built.
struct Project {
    file: PathBuf,
    name: String,
    /// A second file that gives the three services the label of
    /// `ZERODDS_COMPOSE_LABEL` (`key=value`), for a host that tracks its
    /// containers by label.
    labels: Option<PathBuf>,
}

impl Project {
    fn compose(&self) -> Command {
        let mut c = Command::new("docker");
        // No colours and no terminal control sequences in the log lines.
        c.args(["compose", "--ansi", "never", "-f"]).arg(&self.file);
        if let Some(labels) = &self.labels {
            c.arg("-f").arg(labels);
        }
        c.args(["-p", &self.name]);
        c
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = self
            .compose()
            .args(["down", "--rmi", "local", "--volumes", "--timeout", "40"])
            .output();
    }
}

/// The `seq` and `text` of a `Chat::Message` in XCDR1 or XCDR2, little
/// endian (encapsulation CDR_LE or CDR2_LE; a final struct has no DHEADER).
fn chat_message_of(cdr: &[u8]) -> (i32, String) {
    assert!(
        cdr.len() >= 12 && matches!(cdr[..4], [0x00, 0x01, _, _] | [0x00, 0x07, _, _]),
        "not CDR_LE or CDR2_LE: {cdr:02x?}"
    );
    let body = &cdr[4..];
    let seq = i32::from_le_bytes(body[0..4].try_into().unwrap());
    let len = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
    assert_eq!(body[8 + len - 1], 0, "the string's NUL");
    (
        seq,
        String::from_utf8(body[8..8 + len - 1].to_vec()).unwrap(),
    )
}

/// `line` without the ECMA-48 control sequences `ESC [ ... final byte`.
fn without_control_sequences(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for f in chars.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&f) {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex"))
        .collect()
}

/// Spec §12.3: `docker compose up` of `tests/compose/docker-compose.yml`
/// builds the bridge's image, a Cyclone DDS 0.10 image and the tonic
/// client's image and runs them on one Compose network. The tonic client
/// (in the bridge's network namespace, h2c on 127.0.0.1) opens a
/// `Subscribe` stream and calls `PublishOne` with `Chat::Message { 7,
/// "hello from compose" }`; the bridge writes it into DDS, Cyclone's
/// DataReader takes it and Cyclone's DataWriter answers `{ 7, "echo: hello
/// from compose" }`, which the bridge's DataReader puts on the tonic stream.
/// The client exits 0 with that sample, and Compose with the client's code.
#[test]
#[ignore = "needs Docker with Compose; a manual test on the Linux test host (ci/manual-tests.tsv)"]
fn grpcbridge_12_3_the_compose_file_runs_bridge_cyclone_and_tonic_and_the_answer_comes_back() {
    let version = Command::new("docker")
        .args(["compose", "version"])
        .output()
        .expect("docker");
    assert!(
        version.status.success(),
        "`docker compose` is not available: {}",
        String::from_utf8_lossy(&version.stderr)
    );
    let name = format!("zd-grpc-compose-{}", std::process::id());
    let labels = std::env::var("ZERODDS_COMPOSE_LABEL").ok().map(|label| {
        let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-labels.yml"));
        let service = |s: &str| format!("  {s}:\n    labels: [\"{label}\"]\n");
        std::fs::write(
            &path,
            format!(
                "services:\n{}{}{}",
                service("bridge"),
                service("cyclone"),
                service("client")
            ),
        )
        .unwrap();
        path
    });
    let project = Project {
        file: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/compose/docker-compose.yml"),
        name,
        labels,
    };
    let up = project
        .compose()
        .args([
            "up",
            "--build",
            "--abort-on-container-exit",
            "--exit-code-from",
            "client",
        ])
        .output()
        .expect("docker compose up");
    let log = String::from_utf8_lossy(&up.stdout).into_owned();
    let tail: String = {
        let lines: Vec<&str> = log.lines().collect();
        lines[lines.len().saturating_sub(60)..].join("\n")
    };
    assert!(
        up.status.success(),
        "docker compose up: {:?}\n{tail}\n{}",
        up.status.code(),
        String::from_utf8_lossy(&up.stderr)
            .lines()
            .rev()
            .take(30)
            .collect::<Vec<_>>()
            .join("\n")
    );
    // Compose prefixes each line with its service: `client-1  | ...`;
    // Compose 2.26 writes an erase-line sequence before it also with
    // `--ansi never`.
    let of = |service: &str| -> Vec<String> {
        log.lines()
            .map(without_control_sequences)
            .filter_map(|l| {
                let (who, line) = l.split_once(" | ")?;
                who.trim()
                    .starts_with(&format!("{service}-"))
                    .then(|| line.to_string())
            })
            .collect()
    };
    let answer = of("client")
        .into_iter()
        .find_map(|l| {
            l.strip_prefix("sample type=Chat::Message topic=ChatMessage flags=0 payload=")
                .map(str::to_string)
        })
        .unwrap_or_else(|| panic!("no sample on the tonic client's stream:\n{tail}"));
    assert_eq!(
        chat_message_of(&unhex(&answer)),
        (7, "echo: hello from compose".to_string())
    );
    assert!(
        of("cyclone")
            .iter()
            .any(|l| l == "{\"seq\":7,\"text\":\"hello from compose\"}"),
        "Cyclone DDS did not take the client's sample:\n{tail}"
    );
}

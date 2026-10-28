//! Local vendor children for the existing live suites. No SSH or global cleanup.
#![allow(dead_code, clippy::expect_used, clippy::print_stderr)]

use std::fs::OpenOptions;
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastReliability {
    BestEffort,
    Reliable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastDurability {
    Volatile,
    TransientLocal,
}
#[derive(Debug, Clone, Copy)]
pub struct FastQos {
    pub reliability: FastReliability,
    pub durability: FastDurability,
}

pub fn require_or_skip(label: &str) -> bool {
    if std::env::var("INTEROP_REQUIRE").as_deref() == Ok("1")
        || std::env::var("INTEROP_HOST_AVAILABLE").as_deref() == Ok("1")
    {
        return true;
    }
    eprintln!("INTEROP-SKIP[{label}]: opt in with INTEROP_REQUIRE=1 and local vendor binaries");
    false
}

#[derive(Debug)]
pub struct VendorChild {
    child: Child,
    log: PathBuf,
}

impl VendorChild {
    fn spawn(label: &str, command: &mut Command) -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory =
            std::env::var_os("INTEROP_LOG_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
        std::fs::create_dir_all(&directory)?;
        let log = directory.join(format!(
            "{label}-{}-{}.log",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let file = OpenOptions::new().write(true).create_new(true).open(&log)?;
        let child = command
            .stdout(Stdio::from(file.try_clone()?))
            .stderr(Stdio::from(file))
            .spawn()?;
        eprintln!("vendor pid={} log={}", child.id(), log.display());
        Ok(Self { child, log })
    }
    pub fn ensure_running(&mut self) -> io::Result<()> {
        if let Some(status) = self.child.try_wait()? {
            return Err(io::Error::other(format!(
                "vendor exited {status}: {}",
                self.log.display()
            )));
        }
        Ok(())
    }
    pub fn output(&self) -> io::Result<String> {
        std::fs::read_to_string(&self.log)
    }
}
impl Drop for VendorChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_fastdds(mode: &str, topic: &str, domain: u32, qos: &FastQos) -> io::Result<VendorChild> {
    let binary = std::env::var_os("FASTDDS_SHAPES").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "FASTDDS_SHAPES must name the built local vendor client",
        )
    })?;
    let mut command = Command::new(binary);
    command.args([
        mode,
        topic,
        &domain.to_string(),
        match qos.reliability {
            FastReliability::BestEffort => "besteffort",
            FastReliability::Reliable => "reliable",
        },
        match qos.durability {
            FastDurability::Volatile => "volatile",
            FastDurability::TransientLocal => "transient_local",
        },
    ]);
    VendorChild::spawn(&format!("fastdds-{mode}-{domain}-{topic}"), &mut command)
}
pub fn start_fastdds_pub(topic: &str, domain: u32, qos: &FastQos) -> io::Result<VendorChild> {
    start_fastdds("pub", topic, domain, qos)
}
pub fn start_fastdds_sub(topic: &str, domain: u32, qos: &FastQos) -> io::Result<VendorChild> {
    start_fastdds("sub", topic, domain, qos)
}
/// A Cyclone reader through the `cyclonedds` Python binding: `script` is
/// run with `python3` (or `PYTHON`) and the given arguments; what it
/// prints goes to the child's log.
pub fn start_cyclone_python(label: &str, script: &str, args: &[String]) -> io::Result<VendorChild> {
    let directory =
        std::env::var_os("INTEROP_LOG_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{label}-{}.py", std::process::id()));
    std::fs::write(&path, script)?;
    let python = std::env::var_os("PYTHON").unwrap_or_else(|| "python3".into());
    let mut command = Command::new(python);
    command.arg("-u").arg(&path).args(args);
    VendorChild::spawn(label, &mut command)
}

pub fn start_cyclone_ddsperf_sub(domain: u32, duration_secs: u32) -> io::Result<VendorChild> {
    let binary = std::env::var_os("DDSPERF").unwrap_or_else(|| "ddsperf".into());
    let mut command = Command::new(binary);
    command.args([
        "-i",
        &domain.to_string(),
        "-D",
        &duration_secs.to_string(),
        "sub",
    ]);
    VendorChild::spawn(&format!("cyclone-sub-{domain}"), &mut command)
}

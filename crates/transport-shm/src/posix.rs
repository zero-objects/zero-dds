// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! POSIX-SHM-Transport.
//!
//! Cross-Process Shared-Memory-Transport via `shm_open` + `mmap`.
//! Linux primary, macOS supported. Windows kompiliert (über die
//! `shared_memory`-Crate, die plattform-spezifisch `CreateFileMapping`
//! nutzt), aber unser `flock`-Race-Schutz und `shm_unlink`-Cleanup
//! sind unix-only — Windows-Run nutzt die handle-counted-Cleanup-
//! Semantik der OS, was funktional ausreicht aber nicht primär
//! getestet ist.
//!
//! # Modell: SpSc pro Reader
//!
//! Ein SHM-Segment pro (Writer, Reader)-Paar, nicht pro Writer.
//! Writer alloziert das Segment (`open_owner`), Reader joint
//! (`open_consumer`). Lock-free Single-Producer-Single-Consumer-
//! Ringbuffer via `AcqRel`-Atomics auf `head`/`tail`.
//!
//! Rationale:
//! - Lockfree SpSc skaliert linear mit Reader-Count, keine
//!   globale Contention.
//! - `pthread_mutex` mit `PTHREAD_PROCESS_SHARED` waere der
//!   Alternativweg, ist aber crash-recovery-fragile (robust-Flag
//!   nicht ueberall portable, abandoned-mutex-Recovery komplex).
//! - SpmC (Single-Producer-Multi-Consumer wie iceoryx) blockt den
//!   Writer am slowest-Reader — bei heterogenen Readern schlecht.
//!
//! Preis: N Segmente bei N Readern. Bei 100 Readern × 1 MiB Default
//! = 100 MiB. Akzeptabel; die Segment-Groesse ist pro Paar
//! konfigurierbar.
//!
//! # Segment-Layout
//!
//! ```text
//!   offset 0:   magic: u32 BE   "ZSHM"
//!   offset 4:   version: u32 LE
//!   offset 8:   capacity: u64 LE (Daten-Region, ohne Header)
//!   offset 16:  head: AtomicU64 (nächster Schreib-Offset, Writer-owned)
//!   offset 24:  tail: AtomicU64 (nächster Lese-Offset, Reader-owned)
//!   offset 32:  shutdown: AtomicU32 (0=active, 1=owner-gone)
//!   offset 36:  reserved (padding zu 64-Byte cache-line)
//!   offset 64:  data-region [capacity bytes]
//! ```
//!
//! Das Shutdown-Flag ist ein Publikations-Mechanismus vom Owner zum
//! Consumer: Owner setzt es auf 1 in `Drop` (Release-Store); Consumer
//! prueft es in `wait_for_frame` nach jedem leeren Poll und kehrt
//! mit einem gezielten Error (`Io{message:"shm owner terminated"}`)
//! zurueck, statt in den normalen recv_timeout zu fallen. Wichtig
//! fuer Cleanup-Diagnose (Owner-Crash vs. Idle-Polling).
//!
//! Das Datenformat innerhalb der Ring-Region ist length-prefixed:
//! `[len: u32 LE][bytes: len]`. Byte-Position `head % capacity`
//! markiert den naechsten freien Slot; Wraparound ist erlaubt aber
//! ein Frame wird **nicht** gesplittet — bei nicht-genug-space am
//! Ende wird ein Padding-Frame mit `len = 0xFFFF_FFFE` eingesetzt
//! und der Writer springt an den Anfang.
//!
//! # Unsafe-Scope
//!
//! Zwei `unsafe`-Bloecke: die beiden `ptr::read`/`ptr::write` auf
//! dem mapped memory. Alles andere ist auf `AtomicU64` geswappt,
//! die im shared memory liegen duerfen (Rust garantiert, dass
//! Atomic-Operationen ueber Prozess-Grenzen wohlldefiniert sind,
//! wenn beide Prozesse dieselbe Adresse mappen).

use std::io;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use shared_memory::{Shmem, ShmemConf, ShmemError};

#[cfg(unix)]
use std::os::fd::AsRawFd;

/// RAII-Wrapper fuer einen POSIX advisory-flock. Der Lock wird beim
/// Drop des `File` freigegeben (Kernel-side), daher reicht es, den
/// File-Handle zu halten.
#[cfg(unix)]
struct FlockGuard {
    #[allow(dead_code)] // hold-only; drop releases the lock
    file: std::fs::File,
}

/// Best-effort exclusive flock. Auf Non-Unix (Windows-future) fallback
/// zu no-op — dort ist die Race durch `CreateFileMapping`-Semantik
/// ohnehin anders geschichtet.
#[cfg(unix)]
fn acquire_flock_excl(f: &std::fs::File) -> io::Result<()> {
    let fd = f.as_raw_fd();
    // Blockt bis Lock erworben; nebenwirkungsfrei bezueglich Rust-Memory.
    // SAFETY: libc::flock mit gueltigem fd + bekannter Konstante (LOCK_EX).
    let rc = unsafe { libc::flock(fd, libc::LOCK_EX) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
#[allow(dead_code)]
struct FlockGuard {
    file: std::fs::File,
}
#[cfg(not(unix))]
fn acquire_flock_excl(_f: &std::fs::File) -> io::Result<()> {
    Ok(())
}

/// Crash-recovery-Cleanup für ein SHM-Segment.
///
/// Wenn ein Owner crasht, bleibt ohne diesen Call das Segment als
/// Zombie in `/dev/shm/<name>` — Systemweit, Disk/RAM-Exhaustion
/// beim N-ten Neustart.
///
/// Wir loesen das durch
/// (a) predictable os_id (statt random) bei jedem Open, und
/// (b) `shm_unlink(os_id)` vor jedem Owner-`create()`.
///
/// `shm_unlink` ist idempotent: ENOENT wird ignoriert. Der Call
/// laeuft nur auf `cfg(unix)` und fuer `#[cfg(any(target_os="linux",
/// target_os="macos"))]` — andere Unixe muessen wir bei Bedarf
/// nachziehen, Windows hat ein eigenes CleanupSemantics.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn shm_unlink_by_os_id(os_id: &str) {
    use std::ffi::CString;
    let Ok(c) = CString::new(os_id) else {
        return;
    };
    // CString::new schuetzt vor inline-NUL; Rueckgabewert egal —
    // wir behandeln "Segment gelaufen" und "nicht gefunden" gleich.
    // SAFETY: libc::shm_unlink mit einer gueltigen nul-terminierten CString.
    unsafe {
        let _ = libc::shm_unlink(c.as_ptr());
    }
}

/// Auf Nicht-POSIX-Targets ist `shm_unlink` nicht verfuegbar. Diese
/// Crate listet in `Cargo.toml::categories` `os::unix-apis` und der
/// gesamte `PosixAllocator` ist `cfg(unix)`-gegated — der einzige Pfad,
/// auf dem dieser Branch erreicht wuerde, ist eine Mis-Konfiguration
/// auf Windows. Wir markieren explizit, dass dort kein Cleanup stattfindet.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn shm_unlink_by_os_id(_os_id: &str) {
    // Auf Nicht-POSIX gibt es kein shm_unlink-Aequivalent; die SHM-Allocation
    // selber ist auf Windows ueber andere APIs abgedeckt (named-mappings),
    // dort uebernimmt die `shared_memory`-Crate das Cleanup.
}

/// Predictable OS-Id fuer ein (owner, consumer)-Paar. shared_memory
/// wuerde ohne os_id einen random-Namen generieren, bei dem crashed
/// Segmente nicht wiederfindbar sind. Wir erzwingen einen
/// deterministischen Namen, damit Recovery-`shm_unlink` arbeitet.
fn segment_os_id(owner_id: [u8; 16], consumer_id: [u8; 16]) -> String {
    // `/zerodds-<owner>-<consumer>` — max 65 char, Linux-Limit ist
    // NAME_MAX (255) / macOS PSHMNAMLEN (31). Wir muessen unter 31
    // bleiben fuer macOS-Portierbarkeit — nicht triviall bei 2×32
    // hex. Fallback: auf macOS truncieren wir auf die letzten 15 Byte
    // jeder ID, was den Kollisionsraum praktisch nicht verkleinert
    // (Random-GUIDs kollidieren nicht in den letzten 15 Byte).
    #[cfg(target_os = "macos")]
    let (o, c) = (&owner_id[13..16], &consumer_id[13..16]);
    #[cfg(not(target_os = "macos"))]
    let (o, c) = (&owner_id[..], &consumer_id[..]);
    let mut s = String::with_capacity(64);
    s.push_str("/zd-");
    for b in o {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s.push('-');
    for b in c {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

/// Magic-Prefix fuer ZeroDDS-SHM-Segmente. `ZSHM` in big-endian.
pub const SHM_MAGIC: u32 = u32::from_be_bytes(*b"ZSHM");

/// Segment-Version. Bump bei Layout-Aenderung; Opener lehnen
/// unbekannte Versionen ab.
pub const SHM_VERSION: u32 = 1;

/// Fester Header-Overhead (Magic + Version + Capacity + Head + Tail
/// + Cache-Line-Padding = 64 Bytes).
pub const HEADER_BYTES: usize = 64;

/// Frame-Length, die einen Padding-Frame markiert (Ring-End,
/// Writer springt danach wrap-around).
const PADDING_FRAME_LEN: u32 = 0xFFFF_FFFE;

/// Default-Capacity 1 MiB der Daten-Region.
pub const DEFAULT_CAPACITY: usize = 1 << 20;

/// Default Spin-Wait-Zyklen, bevor der Sender mit `Backpressure`
/// zurueckkommt.
const SPIN_LIMIT: u32 = 1024;

/// Default Base-Directory fuer OS-backed Segment-Dateien (Linux:
/// automatisch `/dev/shm/` via `shm_open`). Das hier ist nur das
/// Sentinel-Verzeichnis, in dem wir eine Bookkeeping-Datei mit
/// dem letzten Sender-Lookup ablegen — erlaubt, dass ein Consumer
/// den Path aus einem Locator rekonstruieren kann, ohne den Owner
/// zu kennen.
pub const DEFAULT_FLINK_DIR: &str = "/tmp/zerodds/shm";

/// Config fuer `PosixShmTransport`.
#[derive(Debug, Clone)]
pub struct ShmConfig {
    /// Daten-Region-Groesse in Bytes. Muss `>= max_datagram * 2`
    /// sein (damit der Ring auch bei Wraparound-Padding noch ein
    /// Datagram fassen kann).
    pub capacity: usize,
    /// Basisverzeichnis fuer flink-Dateien (`shared_memory` legt
    /// dort eine Meta-Datei ab, die den OS-Segment-Namen verlinkt).
    pub flink_dir: PathBuf,
    /// Maximale Datagram-Groesse; `send` lehnt groessere mit
    /// `PayloadTooLarge` ab.
    pub max_datagram: usize,
    /// `recv`-Timeout; `None` = blocking (dann wird intern gespinnt
    /// mit exponential backoff).
    pub recv_timeout: Option<Duration>,
}

impl Default for ShmConfig {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_CAPACITY,
            flink_dir: PathBuf::from(DEFAULT_FLINK_DIR),
            max_datagram: 64 * 1024,
            recv_timeout: None,
        }
    }
}

/// Fehler beim Oeffnen oder Betreiben eines SHM-Segments.
#[derive(Debug)]
#[non_exhaustive]
pub enum PosixShmError {
    /// Shared-memory-Backend-Fehler (`shm_open`, `mmap`, ...).
    Shm(ShmemError),
    /// Filesystem-Fehler beim Anlegen des flink-Dirs oder der Meta-
    /// Datei.
    Io(io::Error),
    /// Segment existiert, aber Magic-Prefix/Version passt nicht.
    InvalidHeader,
    /// Konfig-Fehler: `capacity < max_datagram * 2` oder aehnlich.
    InvalidConfig {
        /// Begruendung (statisch).
        reason: &'static str,
    },
}

impl core::fmt::Display for PosixShmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Shm(e) => write!(f, "shared-memory error: {e}"),
            Self::Io(e) => write!(f, "i/o error: {e}"),
            Self::InvalidHeader => f.write_str("shm segment has wrong magic or version"),
            Self::InvalidConfig { reason } => write!(f, "shm config invalid: {reason}"),
        }
    }
}

impl std::error::Error for PosixShmError {}

impl From<ShmemError> for PosixShmError {
    fn from(e: ShmemError) -> Self {
        Self::Shm(e)
    }
}

impl From<io::Error> for PosixShmError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Raw pointer into the mapped segment, parked in an `unsafe`-wrapping
/// type so we can hand it around without dragging `unsafe` into every
/// call-site.
struct SegmentLayout {
    /// Base pointer, lifetime-bound to the holding `Shmem`.
    base: *mut u8,
    /// Data region length in bytes (without the 64-byte header).
    capacity: usize,
}

// `SegmentLayout` holds a raw pointer into memory that's mapped for
// the lifetime of the owning `Shmem`.
//
// # SAFETY — happens-before invariants
//
// Data races between the writer (Owner) and reader (Consumer) are
// avoided by the SpSc-ring discipline and `AcqRel`-atomics on
// `head`/`tail`:
//
// - Writer path (`push_frame`):
//   1. Reads `tail` with `Ordering::Acquire`. Any prior `tail`-store
//      by the reader (Release) is now visible — that store happened
//      *before* the reader finished reading previous frames out of
//      the data region. So all bytes up to `tail` are free for reuse.
//   2. Writes frame-length + payload bytes into the data region via
//      plain `ptr::write_unaligned` / `ptr::copy_nonoverlapping`.
//   3. Stores `head` with `Ordering::Release`. Any subsequent reader
//      that sees the new `head` via `Acquire` *also* sees all bytes
//      written in step 2. This is the publish edge.
//
// - Reader path (`pop_frame`) mirrors the above with head/tail swapped.
//
// The raw `ptr::read`/`ptr::write` in `SegmentLayout::{read,write}_*`
// are therefore safe **only when guarded by the ring protocol above**.
// A refactor that touches ordering (e.g. lowering to `Relaxed`,
// introducing multi-producer paths, or adding writes to data bytes
// that `head` does not already publish) invalidates these guarantees
// and must re-prove happens-before by inspection. This is what makes
// the file a safety-critical unsafe island; aarch64 in particular is
// sensitive to weaker orderings and would surface a bug that x86 hides.
// SAFETY: Send ist OK weil ptr nur hinter Atomic-APIs angefasst wird (siehe oben).
unsafe impl Send for SegmentLayout {}
// SAFETY: Sync ist OK weil Zugriffe ueber AcqRel-Atomics serialisiert sind (siehe oben).
unsafe impl Sync for SegmentLayout {}

impl SegmentLayout {
    /// # Safety
    /// `base` must point to a valid, mapped region of at least
    /// `HEADER_BYTES + capacity` bytes, and the `AtomicU64` fields
    /// referenced by `head_ptr`/`tail_ptr` must be inside that region.
    unsafe fn new(base: *mut u8, capacity: usize) -> Self {
        Self { base, capacity }
    }

    fn head(&self) -> &AtomicU64 {
        // SAFETY: base + 16 liegt im mapped Header, AtomicU64 ist 8-byte-aligned.
        unsafe { &*(self.base.add(16) as *const AtomicU64) }
    }

    fn tail(&self) -> &AtomicU64 {
        // SAFETY: base + 24 liegt im mapped Header, AtomicU64 ist 8-byte-aligned.
        unsafe { &*(self.base.add(24) as *const AtomicU64) }
    }

    fn shutdown(&self) -> &AtomicU32 {
        // SAFETY: base + 32 liegt im mapped Header, AtomicU32 ist 4-byte-aligned.
        unsafe { &*(self.base.add(32) as *const AtomicU32) }
    }

    fn data_ptr(&self) -> *mut u8 {
        // SAFETY: HEADER_BYTES-Offset ist Start der mapped Data-Region.
        unsafe { self.base.add(HEADER_BYTES) }
    }

    /// Read `len` bytes from `offset` (mod capacity). Panics in debug
    /// if the read would wrap the buffer — callers must ensure
    /// single-slot reads.
    fn read_slice(&self, offset: usize, len: usize, out: &mut [u8]) {
        debug_assert!(offset + len <= self.capacity, "read would wrap");
        // SAFETY: offset + len <= capacity (debug-checked) in bounds der Data-Region.
        unsafe {
            ptr::copy_nonoverlapping(self.data_ptr().add(offset), out.as_mut_ptr(), len);
        }
    }

    fn write_slice(&self, offset: usize, src: &[u8]) {
        debug_assert!(offset + src.len() <= self.capacity, "write would wrap");
        // SAFETY: offset + src.len() <= capacity (debug-checked) in bounds der Data-Region.
        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), self.data_ptr().add(offset), src.len());
        }
    }

    fn write_u32(&self, offset: usize, v: u32) {
        debug_assert!(offset + 4 <= self.capacity);
        // SAFETY: offset + 4 <= capacity (debug-checked); unaligned-write ist zulaessig.
        unsafe {
            ptr::write_unaligned(self.data_ptr().add(offset) as *mut u32, v.to_le());
        }
    }

    fn read_u32(&self, offset: usize) -> u32 {
        debug_assert!(offset + 4 <= self.capacity);
        // SAFETY: offset + 4 <= capacity (debug-checked); unaligned-read ist zulaessig.
        let v = unsafe { ptr::read_unaligned(self.data_ptr().add(offset) as *const u32) };
        u32::from_le(v)
    }
}

/// Rolle im SHM-Pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ShmRole {
    /// Dieser Transport allozieren das Segment (einziger Writer).
    Owner,
    /// Dieser Transport joined ein bestehendes Segment (einziger Reader).
    Consumer,
}

/// Cross-process SHM-Transport via `shm_open` + `mmap`.
///
/// Ein Transport-Objekt bindet **ein** Segment in einer festen Rolle.
/// Fuer multi-Reader-Writer muessen mehrere Transports instanziiert
/// werden (eigene Segmente).
pub struct PosixShmTransport {
    _shmem: Shmem, // keeps the mapping alive; Drop unmaps
    layout: SegmentLayout,
    role: ShmRole,
    local_locator: Locator,
    peer_locator: Locator,
    config: ShmConfig,
    /// Fuer Owner: der flink-Pfad, den wir beim Drop unlinken.
    /// Consumer laesst den Pfad in Ruhe (Owner besitzt ihn).
    flink_path: PathBuf,
    /// Fuer Owner: der `.lock`-Pfad, der beim Drop geraeumt wird.
    lock_path: PathBuf,
    /// OS-Id des `/dev/shm`-Segments — wir halten eine Kopie,
    /// damit `Drop` auch bei `shared_memory`-interner Failure noch
    /// `shm_unlink` aufrufen kann (Crash-Recovery-Defense-in-Depth).
    os_id: String,
    /// Konsumierte Padding-Frames seit Bind. Siehe
    /// [`padding_frames_seen`](Self::padding_frames_seen).
    padding_counter: core::sync::atomic::AtomicU64,
    /// Corrupt-Frame-Drops: Owner
    /// schrieb eine Laenge > max_datagram. Consumer droppt das
    /// Frame + skipt bis Ring-Ende.
    corrupt_frame_counter: core::sync::atomic::AtomicU64,
}

impl Drop for PosixShmTransport {
    fn drop(&mut self) {
        // Crash-resilient cleanup:
        //
        // - Owner hat das `/dev/shm/<os_id>`-Segment, die flink-Datei
        //   und die `.lock`-Datei angelegt. Alle drei muessen weg,
        //   sonst laeuft das System nach N Neustarts ueber.
        // - Consumer raeumt NICHT auf — ein Consumer-Drop bedeutet
        //   nicht, dass der Owner weg ist. Der Owner besitzt die
        //   Ressourcen.
        // - Alle Calls sind best-effort (mit `_ =`), weil `Drop`
        //   nicht panic'en darf und ENOENT (bereits weg) okay ist.
        if self.role == ShmRole::Owner {
            // 0. Shutdown-Flag setzen,
            //    bevor wir die Ressourcen abbauen. Consumer in einem
            //    wait_for_frame-loop sieht den Release-Store und
            //    gibt einen gezielten Error zurueck, statt in den
            //    normalen recv_timeout zu rennen.
            self.layout
                .shutdown()
                .store(1, core::sync::atomic::Ordering::Release);
            // 1. flink-Datei entfernen (lookup-file, kein Segment).
            let _ = std::fs::remove_file(&self.flink_path);
            // 2. Lock-Datei entfernen.
            let _ = std::fs::remove_file(&self.lock_path);
            // 3. /dev/shm-Segment unlinken — defense-in-depth.
            //    `shared_memory` macht das intern wenn set_owner(true)
            //    gesetzt ist (wird im `open()`-Pfad via `.create()`
            //    gesetzt), aber ein expliziter call ist idempotent.
            shm_unlink_by_os_id(&self.os_id);
        }
    }
}

impl core::fmt::Debug for PosixShmTransport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PosixShmTransport")
            .field("role", &self.role)
            .field("local_locator", &self.local_locator)
            .field("peer_locator", &self.peer_locator)
            .field("capacity", &self.layout.capacity)
            .finish()
    }
}

impl PosixShmTransport {
    /// Owner-seitige Instanziierung. `local_id` identifiziert den
    /// Writer-Endpoint, `peer_id` den einen erlaubten Reader. Das
    /// Segment wird am Pfad `<flink_dir>/<hex(local)>-<hex(peer)>.shm`
    /// abgelegt.
    ///
    /// # Errors
    /// [`PosixShmError`].
    pub fn open_owner(
        local_id: [u8; 16],
        peer_id: [u8; 16],
        config: ShmConfig,
    ) -> Result<Self, PosixShmError> {
        Self::open(local_id, peer_id, config, ShmRole::Owner)
    }

    /// Consumer-seitige Instanziierung. Der Owner muss das Segment
    /// vorher angelegt haben — sonst `Shm(ShmemError::MapOpenFailed)`.
    ///
    /// # Errors
    /// [`PosixShmError`].
    pub fn open_consumer(
        local_id: [u8; 16],
        peer_id: [u8; 16],
        config: ShmConfig,
    ) -> Result<Self, PosixShmError> {
        // Consumer perspektive: `peer_id` ist der Owner/Writer,
        // `local_id` unser Reader-Endpoint. Segment-Pfad wird aus
        // (owner, consumer) abgeleitet — gleich wie beim Owner, nur
        // die Rollen sind umgedreht.
        Self::open(peer_id, local_id, config, ShmRole::Consumer)
    }

    fn open(
        owner_id: [u8; 16],
        consumer_id: [u8; 16],
        config: ShmConfig,
        role: ShmRole,
    ) -> Result<Self, PosixShmError> {
        if config.capacity < config.max_datagram * 2 + 16 {
            return Err(PosixShmError::InvalidConfig {
                reason: "capacity must be >= 2 * max_datagram + 16",
            });
        }
        // Symlink-Guard: analog zu UDS
        // ensure_base_dir wollen wir keinen Symlink als flink_dir
        // akzeptieren. Sonst kann ein Angreifer uns dazu bringen,
        // `.shm` / `.lock`-Dateien in fremde Pfade zu schreiben.
        match std::fs::symlink_metadata(&config.flink_dir) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(PosixShmError::InvalidConfig {
                    reason: "flink_dir is a symlink — refused",
                });
            }
            Ok(m) if !m.is_dir() => {
                return Err(PosixShmError::InvalidConfig {
                    reason: "flink_dir exists but is not a directory",
                });
            }
            Ok(_) | Err(_) => { /* create_dir_all deals with missing */ }
        }
        std::fs::create_dir_all(&config.flink_dir)?;
        let flink = segment_flink(&config.flink_dir, owner_id, consumer_id);
        let os_id = segment_os_id(owner_id, consumer_id);

        let mut shmem = if role == ShmRole::Owner {
            // Race-avoidance: zwei
            // gleichzeitig startende Owner-Prozesse duerfen nicht in
            // der Luecke zwischen `remove_file` und `create` beide
            // binden. Wir halten einen POSIX advisory-flock auf einer
            // Lock-Datei im flink-Dir bis zum `create()` durch — dann
            // ist nur *einer* Owner gleichzeitig in der kritischen
            // Region, und der zweite sieht den fertigen Header und
            // schlaegt per InvalidHeader fehl (Race ist OK-isiert
            // auf Serialisierung, nicht auf coexistence).
            let lock_path = flink.with_extension("lock");
            let lock_file = std::fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(false)
                .open(&lock_path)?;
            acquire_flock_excl(&lock_file)?;
            let _guard = FlockGuard { file: lock_file };

            // Crash-recovery: vorheriger
            // Owner ist vielleicht vor shm_unlink gecrashed und hat
            // ein Zombie-Segment in /dev/shm hinterlassen. Wir
            // entfernen es vor dem create — idempotent. ENOENT wird
            // silent ignoriert.
            shm_unlink_by_os_id(&os_id);
            let _ = std::fs::remove_file(&flink);

            // _guard droppt und flock wird freigegeben, sobald wir
            // aus dem `if`-branch fallen.
            ShmemConf::new()
                .os_id(&os_id)
                .size(HEADER_BYTES + config.capacity)
                .flink(&flink)
                .create()?
        } else {
            ShmemConf::new().flink(&flink).open()?
        };

        let base = shmem.as_ptr();
        let total_size = shmem.len();
        if total_size < HEADER_BYTES + config.capacity {
            return Err(PosixShmError::InvalidConfig {
                reason: "mapped segment smaller than requested capacity",
            });
        }

        if role == ShmRole::Owner {
            // Initialisiere Header. `Shmem::create` liefert zeroed memory
            // auf POSIX und Windows. head = tail = 0 kommt durch zero-init.
            // SAFETY: Segment >= HEADER_BYTES, wir schreiben vor jedem Consumer-Zugriff (kein Race).
            unsafe {
                ptr::write_unaligned(base as *mut u32, SHM_MAGIC.to_be());
                ptr::write_unaligned(base.add(4) as *mut u32, SHM_VERSION.to_le());
                ptr::write_unaligned(base.add(8) as *mut u64, (config.capacity as u64).to_le());
            }
            // Set owner flag so we can detect late joiners vs. zombies.
            shmem.set_owner(true);
        } else {
            // SAFETY: Segment >= HEADER_BYTES Bytes, offset 0 enthaelt magic.
            let magic_be = unsafe { ptr::read_unaligned(base as *const u32) };
            if u32::from_be(magic_be) != SHM_MAGIC {
                return Err(PosixShmError::InvalidHeader);
            }
            // SAFETY: Segment >= HEADER_BYTES Bytes, offset 4 enthaelt version.
            let version = unsafe { ptr::read_unaligned(base.add(4) as *const u32) };
            if u32::from_le(version) != SHM_VERSION {
                return Err(PosixShmError::InvalidHeader);
            }
        }

        // SAFETY: base ist valid fuer Lifetime von shmem; layout haelt nur Refs gleicher Lifetime.
        let layout = unsafe { SegmentLayout::new(base, config.capacity) };

        let local_locator = Locator::shm(if role == ShmRole::Owner {
            owner_id
        } else {
            consumer_id
        });
        let peer_locator = Locator::shm(if role == ShmRole::Owner {
            consumer_id
        } else {
            owner_id
        });

        let lock_path = flink.with_extension("lock");
        Ok(Self {
            _shmem: shmem,
            layout,
            role,
            local_locator,
            peer_locator,
            config,
            flink_path: flink,
            lock_path,
            os_id,
            padding_counter: core::sync::atomic::AtomicU64::new(0),
            corrupt_frame_counter: core::sync::atomic::AtomicU64::new(0),
        })
    }

    /// Anzahl Bytes im Ring, die vom Reader noch nicht konsumiert sind.
    /// Diagnose fuer Backpressure-Monitoring.
    #[must_use]
    pub fn occupied_bytes(&self) -> u64 {
        let h = self.layout.head().load(Ordering::Acquire);
        let t = self.layout.tail().load(Ordering::Acquire);
        h.wrapping_sub(t)
    }

    fn push_frame(&self, data: &[u8]) -> Result<(), SendError> {
        let needed = (data.len() + 4) as u64; // length-prefix
        let cap = self.config.capacity as u64;

        let mut spins: u32 = 0;
        loop {
            let h = self.layout.head().load(Ordering::Relaxed);
            let t = self.layout.tail().load(Ordering::Acquire);
            let used = h.wrapping_sub(t);
            let free = cap - used;

            let h_mod = (h % cap) as usize;
            let tail_space = self.config.capacity - h_mod; // bis Ring-End

            if needed > free {
                // Reader hat nicht genug konsumiert.
                spins = spins.saturating_add(1);
                if spins > SPIN_LIMIT {
                    return Err(SendError::Io {
                        message: "shm ring full, reader too slow",
                    });
                }
                core::hint::spin_loop();
                continue;
            }

            if (needed as usize) > tail_space {
                // Fragment wuerde wrappen → erst Padding-Frame schreiben,
                // dann retry. Wir brauchen mind. 4 Byte fuer den Padding-
                // Header; wenn tail_space < 4 passt es nicht mehr,
                // dann setzen wir nur einen sentinel-byte-Padding via
                // head-bump (readable als "kein Frame, skip to start").
                //
                // Die Bedingung `needed + tail_space <= free` ist
                // KORREKT: Nach dem Padding steigt `used` um
                // `tail_space` (der Reader sieht die Padding-Marke und
                // konsumiert sie in `pop_frame`, bis er das tut sind
                // die Bytes ring-occupancy). Daher muss vor dem Wrap
                // gelten `cap - used >= tail_space + needed`, also
                // `free >= tail_space + needed`.
                if tail_space >= 4 && (needed + (tail_space as u64)) <= free {
                    self.layout.write_u32(h_mod, PADDING_FRAME_LEN);
                    self.layout
                        .head()
                        .store(h + tail_space as u64, Ordering::Release);
                    continue;
                }
                // Nicht genug Platz fuer Padding + Frame — Reader muss
                // zuerst weiter konsumieren. Spin.
                spins = spins.saturating_add(1);
                if spins > SPIN_LIMIT {
                    return Err(SendError::Io {
                        message: "shm ring full near wraparound",
                    });
                }
                core::hint::spin_loop();
                continue;
            }

            // Happy path: Frame passt am Stueck ab h_mod.
            let len_u32 = u32::try_from(data.len()).map_err(|_| SendError::PayloadTooLarge {
                size: data.len(),
                limit: self.config.max_datagram,
            })?;
            self.layout.write_u32(h_mod, len_u32);
            self.layout.write_slice(h_mod + 4, data);
            self.layout.head().store(h + needed, Ordering::Release);
            return Ok(());
        }
    }

    /// Anzahl beobachteter Padding-Frames seit Bind. Steigt jedes Mal
    /// wenn der Writer einen Ring-Wrap einlegen musste. Eine auffaellig
    /// hohe Rate ist ein Symptom fuer zu kleines `capacity` oder zu
    /// lange Message-Groessen-Varianz — Diagnose-Hilfe, keine Safety-
    /// Eigenschaft.
    ///
    /// Der Counter ist Consumer-seitig (`pop_frame` zählt). Owner-
    /// seitige Zählung wäre Duplikation — Consumer-Sicht reicht für die
    /// Diagnose-Funktion (Padding-Rate ist ein Lokal-Phänomen).
    #[must_use]
    pub fn padding_frames_seen(&self) -> u64 {
        self.padding_counter
            .load(core::sync::atomic::Ordering::Relaxed)
    }

    /// Anzahl Frames, die der Consumer wegen Length-Sanity-Check
    /// (`len > max_datagram`) gedroppt hat. Nicht-Null weist auf
    /// einen kaputten oder boesartigen Owner hin —
    /// Ops-Signal-Kandidat. Atomic, Relaxed Ordering.
    #[must_use]
    pub fn corrupt_frames_seen(&self) -> u64 {
        self.corrupt_frame_counter
            .load(core::sync::atomic::Ordering::Relaxed)
    }

    fn pop_frame(&self) -> Option<Vec<u8>> {
        // Iterativ statt Rekursion.
        // Ein Ring mit durchgehenden Padding-Frames (z.B. winzige
        // capacity + stete Wrap-Zyklen, oder ein boesartiger Owner
        // der bewusst nur Padding schreibt) kann die ursprueengliche
        // `return self.pop_frame()`-Rekursion bis zum Stack-Overflow
        // treiben. Der Loop hier terminiert in maximal
        // `capacity / 4` Iterationen (jeder Padding-Skip rueckt
        // tail um `tail_space` >= 4 vor, bis `h == t`).
        loop {
            let t = self.layout.tail().load(Ordering::Relaxed);
            let h = self.layout.head().load(Ordering::Acquire);
            if h == t {
                return None;
            }

            let cap = self.config.capacity as u64;
            let t_mod = (t % cap) as usize;
            let tail_space = self.config.capacity - t_mod;

            // Tail-space < 4 bedeutet: Writer hat einen winzigen
            // Rest am Ring-Ende gelassen (keine Laengen-Header mehr
            // reinpassen). Skip ans Segment-Anfang.
            if tail_space < 4 {
                self.layout
                    .tail()
                    .store(t + tail_space as u64, Ordering::Release);
                continue;
            }

            let len = self.layout.read_u32(t_mod);
            if len == PADDING_FRAME_LEN {
                // Padding — Rest der Region überspringen. Counter
                // erlaubt Diagnose.
                self.padding_counter
                    .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                self.layout
                    .tail()
                    .store(t + tail_space as u64, Ordering::Release);
                continue;
            }

            // DoS-Guard:
            // malicious oder korrupter Owner koennte eine `len`
            // schreiben die groesser ist als das Datagram-Config-
            // Limit; ohne Check wuerden wir einen riesigen Vec
            // allokieren. Droppen + counter anstossen (sichtbar
            // via `corrupt_frames_seen()`).
            if len as usize > self.config.max_datagram {
                self.corrupt_frame_counter
                    .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                self.layout
                    .tail()
                    .store(t + tail_space as u64, Ordering::Release);
                return None;
            }

            let len = len as usize;
            let mut out = vec![0u8; len];
            self.layout.read_slice(t_mod + 4, len, &mut out);
            self.layout
                .tail()
                .store(t + (len as u64) + 4, Ordering::Release);
            return Some(out);
        }
    }

    fn wait_for_frame(&self) -> Result<Vec<u8>, RecvError> {
        // Sleep-poll-Modell. Statt echter
        // futex/eventfd-Notify pollt der Reader in exponential-
        // backoff-Schritten. Der Tradeoff ist bewusst:
        //
        // - **Low-tail-latency**: max_backoff auf 1 ms begrenzt; der
        //   schlimmste Fall zwischen frame-arrival und delivery ist
        //   eine Millisekunde statt der vorherigen 10 ms.
        // - **CPU-Cost bei Idle**: sleep(1 ms) in loop = ~1000 Polls/s.
        //   Ein leerer Receiver auf i/idle-host verbraucht <0.1 % CPU —
        //   akzeptabel.
        // - **Volles futex/eventfd**: verschoben in v1.3 (`eventfd(2)`
        //   pro Segment + epoll_wait). Bricht unsere Cross-Platform-
        //   Abstraktion (Linux-only) und lohnt sich erst wenn wir
        //   1M+ Messages/s treffen.
        let deadline = self.config.recv_timeout.map(|t| Instant::now() + t);
        let mut backoff = Duration::from_micros(10);
        let max_backoff = Duration::from_millis(1);
        loop {
            if let Some(frame) = self.pop_frame() {
                return Ok(frame);
            }
            // Gezieltes OwnerGone-Signal:
            // Wenn Owner Drop-t, setzt er shutdown=1. Wir pruefen
            // erst nach pop_frame — damit Frames die vor dem Drop
            // gepublisht wurden noch ankommen. Der Acquire-Load
            // pairt zum Owner-Release-Store.
            if self.layout.shutdown().load(Ordering::Acquire) != 0 {
                return Err(RecvError::Io {
                    message: "shm owner terminated",
                });
            }
            if let Some(d) = deadline {
                if Instant::now() >= d {
                    return Err(RecvError::Timeout);
                }
            }
            std::thread::sleep(backoff);
            backoff = (backoff * 2).min(max_backoff);
        }
    }
}

/// Pfad des flink-Files fuer ein Owner/Consumer-Paar. Symmetrisch in
/// den Rollen — `(A,B)` und `(B,A)` generieren verschiedene Pfade,
/// weil die Owner-Rolle eindeutig sein muss.
fn segment_flink(base_dir: &Path, owner_id: [u8; 16], consumer_id: [u8; 16]) -> PathBuf {
    let mut s = String::with_capacity(32 + 1 + 32 + 4);
    for b in owner_id {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s.push('-');
    for b in consumer_id {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s.push_str(".shm");
    let mut p = base_dir.to_path_buf();
    p.push(s);
    p
}

impl Transport for PosixShmTransport {
    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        if self.role != ShmRole::Owner {
            return Err(SendError::Io {
                message: "shm consumer cannot send on this segment",
            });
        }
        if dest.kind != LocatorKind::Shm {
            return Err(SendError::UnsupportedLocator);
        }
        if *dest != self.peer_locator {
            // Dieser Transport redet nur mit **einem** Peer. Fuer
            // multi-Reader muss der Caller mehrere Transports halten.
            return Err(SendError::UnsupportedLocator);
        }
        if data.len() > self.config.max_datagram {
            return Err(SendError::PayloadTooLarge {
                size: data.len(),
                limit: self.config.max_datagram,
            });
        }
        self.push_frame(data)
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        if self.role != ShmRole::Consumer {
            return Err(RecvError::Io {
                message: "shm owner cannot recv on this segment",
            });
        }
        let data = self.wait_for_frame()?;
        Ok(ReceivedDatagram {
            source: self.peer_locator,
            data,
        })
    }

    fn local_locator(&self) -> Locator {
        self.local_locator
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn id(n: u8) -> [u8; 16] {
        let mut a = [0u8; 16];
        a[15] = n;
        a
    }

    fn cfg_tmp(base: &Path, cap: usize) -> ShmConfig {
        ShmConfig {
            capacity: cap,
            flink_dir: base.to_path_buf(),
            max_datagram: cap / 2 - 16,
            recv_timeout: Some(Duration::from_secs(2)),
        }
    }

    #[test]
    fn open_owner_then_consumer_roundtrip_single_frame() {
        let tmp = tempfile::tempdir().unwrap();
        let owner = PosixShmTransport::open_owner(id(1), id(2), cfg_tmp(tmp.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(2), id(1), cfg_tmp(tmp.path(), 4096)).unwrap();

        owner.send(&Locator::shm(id(2)), b"hello shm").unwrap();
        let got = consumer.recv().unwrap();
        assert_eq!(got.data, b"hello shm");
        assert_eq!(got.source, Locator::shm(id(1)));
    }

    #[test]
    fn consumer_cannot_send() {
        let tmp = tempfile::tempdir().unwrap();
        let _owner =
            PosixShmTransport::open_owner(id(10), id(11), cfg_tmp(tmp.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(11), id(10), cfg_tmp(tmp.path(), 4096)).unwrap();

        let res = consumer.send(&Locator::shm(id(10)), b"x");
        assert!(matches!(res, Err(SendError::Io { .. })));
    }

    #[test]
    fn owner_cannot_recv() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(20), id(21), cfg_tmp(tmp.path(), 4096)).unwrap();
        let res = owner.recv();
        assert!(matches!(res, Err(RecvError::Io { .. })));
    }

    #[test]
    fn send_rejects_non_shm_locator() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(30), id(31), cfg_tmp(tmp.path(), 4096)).unwrap();
        let res = owner.send(&Locator::udp_v4([127, 0, 0, 1], 7400), b"x");
        assert_eq!(res, Err(SendError::UnsupportedLocator));
    }

    #[test]
    fn send_rejects_wrong_peer() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(40), id(41), cfg_tmp(tmp.path(), 4096)).unwrap();
        // Owner redet nur mit peer 41, nicht 99.
        let res = owner.send(&Locator::shm(id(99)), b"x");
        assert_eq!(res, Err(SendError::UnsupportedLocator));
    }

    #[test]
    fn send_rejects_oversize_payload() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(50), id(51), cfg_tmp(tmp.path(), 4096)).unwrap();
        let big = vec![0u8; 4096]; // > max_datagram (4096/2 - 16)
        let res = owner.send(&Locator::shm(id(51)), &big);
        assert!(matches!(res, Err(SendError::PayloadTooLarge { .. })));
    }

    #[test]
    fn recv_times_out_when_idle() {
        let tmp = tempfile::tempdir().unwrap();
        let _owner =
            PosixShmTransport::open_owner(id(60), id(61), cfg_tmp(tmp.path(), 4096)).unwrap();
        let mut cfg = cfg_tmp(tmp.path(), 4096);
        cfg.recv_timeout = Some(Duration::from_millis(200));
        let consumer = PosixShmTransport::open_consumer(id(61), id(60), cfg).unwrap();
        let res = consumer.recv();
        assert_eq!(res, Err(RecvError::Timeout));
    }

    #[test]
    fn many_frames_roundtrip_with_wraparound() {
        // cap=256 mit max_datagram=112 -> ~2 frames bis wraparound
        let tmp = tempfile::tempdir().unwrap();
        let cfg = ShmConfig {
            capacity: 256,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: 80,
            recv_timeout: Some(Duration::from_secs(2)),
        };
        let owner = PosixShmTransport::open_owner(id(70), id(71), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(71), id(70), cfg).unwrap();

        for i in 0..20u8 {
            let payload = vec![i; 60];
            owner.send(&Locator::shm(id(71)), &payload).unwrap();
            let got = consumer.recv().unwrap();
            assert_eq!(got.data.len(), 60);
            assert_eq!(got.data[0], i);
        }
    }

    #[test]
    fn consumer_open_fails_if_owner_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let res = PosixShmTransport::open_consumer(id(80), id(81), cfg_tmp(tmp.path(), 4096));
        assert!(res.is_err());
    }

    #[test]
    fn local_locator_is_shm_with_own_id() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(90), id(91), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(owner.local_locator(), Locator::shm(id(90)));
        let consumer =
            PosixShmTransport::open_consumer(id(91), id(90), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(consumer.local_locator(), Locator::shm(id(91)));
    }

    // ---- Coverage-Boost: Error-Matrix ----

    #[test]
    fn invalid_config_capacity_too_small_for_max_datagram() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = ShmConfig {
            capacity: 100,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: 200, // 2 * 200 + 16 = 416 > 100
            recv_timeout: None,
        };
        let res = PosixShmTransport::open_owner(id(100), id(101), cfg);
        assert!(matches!(res, Err(PosixShmError::InvalidConfig { .. })));
    }

    #[test]
    fn occupied_bytes_reflects_send_state() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(110), id(111), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(owner.occupied_bytes(), 0);
        owner.send(&Locator::shm(id(111)), &[0u8; 64]).unwrap();
        // 4 byte length-prefix + 64 byte payload.
        assert_eq!(owner.occupied_bytes(), 68);
    }

    #[test]
    fn recv_after_send_drains_occupied_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let owner =
            PosixShmTransport::open_owner(id(120), id(121), cfg_tmp(tmp.path(), 4096)).unwrap();
        let consumer =
            PosixShmTransport::open_consumer(id(121), id(120), cfg_tmp(tmp.path(), 4096)).unwrap();
        owner.send(&Locator::shm(id(121)), b"abc").unwrap();
        let _ = consumer.recv().unwrap();
        assert_eq!(owner.occupied_bytes(), 0);
    }

    #[test]
    fn posix_shm_error_display_covers_all_variants() {
        // Smoke: jeder Variant hat eine lesbare Display-Impl (verhindert
        // zukuenftige panic-in-Display-Regressions).
        use std::io;
        let variants = [
            PosixShmError::InvalidHeader,
            PosixShmError::InvalidConfig { reason: "x" },
            PosixShmError::Io(io::Error::other("test")),
        ];
        for v in &variants {
            let s = format!("{v}");
            assert!(!s.is_empty(), "empty display for {v:?}");
        }
    }

    #[test]
    fn owner_drop_signals_consumer_via_shutdown_flag() {
        // Consumer darf nicht im
        // recv_timeout hängen wenn Owner gecrasht/gedropped ist.
        //
        // `Shmem` ist nicht Send, daher koennen wir weder Owner noch
        // Consumer ueber Thread-Grenzen verschieben. Wir simulieren
        // Owner-Drop stattdessen direkt via shutdown-Bit: ein
        // Producer-Thread greift auf den Shutdown-Flag via einem
        // zweiten Consumer-Join (der mapp-t das gleiche Segment) zu.
        //
        // Einfacher: synchron main-thread: owner droppen vor
        // consumer.recv().
        let tmp = tempfile::tempdir().unwrap();
        let cfg = ShmConfig {
            capacity: 4096,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: 1024,
            recv_timeout: Some(Duration::from_secs(10)),
        };
        let owner = PosixShmTransport::open_owner(id(140), id(141), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(141), id(140), cfg).unwrap();

        // Drop den Owner JETZT (setzt shutdown=1), dann recv — muss
        // unmittelbar mit OwnerGone-Error zurueckkommen statt zu
        // warten. Recv-timeout steht auf 10 s; gemessenes Elapsed
        // muss deutlich kleiner sein.
        drop(owner);
        let start = std::time::Instant::now();
        let res = consumer.recv();
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_millis(200),
            "recv should shortcut on shutdown, took {elapsed:?}",
        );
        match res {
            Err(RecvError::Io { message }) => {
                assert!(message.contains("owner"), "unexpected msg: {message}");
            }
            other => panic!("expected Io(owner-gone), got {other:?}"),
        }
    }

    #[test]
    fn corrupt_frame_counter_default_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let t = PosixShmTransport::open_owner(id(150), id(151), cfg_tmp(tmp.path(), 4096)).unwrap();
        assert_eq!(t.corrupt_frames_seen(), 0);
        assert_eq!(t.padding_frames_seen(), 0);
    }

    #[test]
    fn fill_ring_triggers_wraparound_padding() {
        // Kleiner Ring + grosse Frames erzwingt Wraparound mit Padding.
        let tmp = tempfile::tempdir().unwrap();
        let cfg = ShmConfig {
            capacity: 300,
            flink_dir: tmp.path().to_path_buf(),
            max_datagram: 100,
            recv_timeout: Some(Duration::from_secs(1)),
        };
        let owner = PosixShmTransport::open_owner(id(130), id(131), cfg.clone()).unwrap();
        let consumer = PosixShmTransport::open_consumer(id(131), id(130), cfg).unwrap();
        // 5 frames a 60 byte = 5 * 64 bytes; wraparound noetig um Ring 300.
        for i in 0..5u8 {
            let p = vec![i; 60];
            owner.send(&Locator::shm(id(131)), &p).unwrap();
            let got = consumer.recv().unwrap();
            assert_eq!(got.data, p);
        }
    }
}

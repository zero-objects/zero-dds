// SPDX-License-Identifier: Apache-2.0
//! `libiceoryx_binding_c` FFI + safe reader/writer for Cyclone's iceoryx PSMX.

use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::sync::Once;

use crate::DdsPsmxMetadata;

// --- opaque handles + storage (iceoryx C binding: storage holds the handle) ---
#[repr(C)]
struct IoxSub(*mut c_void);
#[repr(C)]
struct IoxPub(*mut c_void);
#[repr(C)]
struct IoxDisc(*mut c_void);
#[repr(C)]
struct IoxChunkHeader(*mut c_void);

// Storage backing each handle. The headers declare these as `uint64_t[1]`; we
// over-allocate to be robust against version drift (the binding writes only its
// own object size). 8-byte aligned via the u64 element type.
#[repr(C)]
struct Storage([u64; 16]);
impl Storage {
    fn new() -> Box<Self> {
        Box::new(Storage([0u64; 16]))
    }
}

#[repr(C)]
struct IoxSubOptions {
    queue_capacity: u64,
    history_request: u64,
    node_name: *const c_char,
    subscribe_on_create: bool,
    queue_full_policy: i32,
    require_publisher_history_support: bool,
    init_check: u64,
}

#[repr(C)]
struct IoxPubOptions {
    history_capacity: u64,
    node_name: *const c_char,
    offer_on_create: bool,
    subscriber_too_slow_policy: i32,
    init_check: u64,
}

const CHUNK_RECEIVE_SUCCESS: i32 = 3;
const ALLOCATION_SUCCESS: i32 = 8;
const MESSAGING_PATTERN_PUB_SUB: i32 = 0;
const CHUNK_DEFAULT_USER_PAYLOAD_ALIGNMENT: u32 = 8;

type FindServiceCb = extern "C" fn(IoxServiceDescriptionByVal);
// find_service_apply_callable passes the description BY VALUE.
#[repr(C)]
#[derive(Clone, Copy)]
struct IoxServiceDescriptionByVal {
    service: [c_char; 101],
    instance: [c_char; 101],
    event: [c_char; 101],
}

extern "C" {
    fn iox_runtime_init(name: *const c_char);

    fn iox_service_discovery_init(storage: *mut Storage) -> *mut IoxDisc;
    fn iox_service_discovery_find_service_apply_callable(
        self_: *mut IoxDisc,
        service: *const c_char,
        instance: *const c_char,
        event: *const c_char,
        callable: FindServiceCb,
        pattern: i32,
    );

    fn iox_sub_options_init(options: *mut IoxSubOptions);
    fn iox_sub_init(
        storage: *mut Storage,
        service: *const c_char,
        instance: *const c_char,
        event: *const c_char,
        options: *const IoxSubOptions,
    ) -> *mut IoxSub;
    fn iox_sub_subscribe(self_: *mut IoxSub);
    fn iox_sub_take_chunk(self_: *mut IoxSub, payload: *mut *const c_void) -> i32;
    fn iox_sub_release_chunk(self_: *mut IoxSub, payload: *const c_void);
    fn iox_sub_deinit(self_: *mut IoxSub);

    fn iox_pub_options_init(options: *mut IoxPubOptions);
    fn iox_pub_init(
        storage: *mut Storage,
        service: *const c_char,
        instance: *const c_char,
        event: *const c_char,
        options: *const IoxPubOptions,
    ) -> *mut IoxPub;
    fn iox_pub_offer(self_: *mut IoxPub);
    fn iox_pub_loan_aligned_chunk_with_user_header(
        self_: *mut IoxPub,
        payload: *mut *mut c_void,
        user_payload_size: u32,
        user_payload_alignment: u32,
        user_header_size: u32,
        user_header_alignment: u32,
    ) -> i32;
    fn iox_pub_publish_chunk(self_: *mut IoxPub, payload: *mut c_void);
    fn iox_pub_deinit(self_: *mut IoxPub);

    fn iox_chunk_header_from_user_payload_const(payload: *const c_void) -> *const IoxChunkHeader;
    fn iox_chunk_header_to_user_header_const(ch: *const IoxChunkHeader) -> *const c_void;
    fn iox_chunk_header_from_user_payload(payload: *mut c_void) -> *mut IoxChunkHeader;
    fn iox_chunk_header_to_user_header(ch: *mut IoxChunkHeader) -> *mut c_void;
}

static RUNTIME: Once = Once::new();

/// Registers this process with `iox-roudi` under `app_name` (idempotent).
pub fn init_runtime(app_name: &str) {
    RUNTIME.call_once(|| {
        let c = CString::new(app_name).expect("app name");
        // SAFETY: valid NUL-terminated string; iceoryx copies it.
        unsafe { iox_runtime_init(c.as_ptr()) };
    });
}

fn cstr_to_string(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

// Service-discovery callback shuttles matches through a thread-local sink (the
// C callback carries no context pointer in this binding variant).
thread_local! {
    static DISCOVERED: std::cell::RefCell<Vec<(String, String, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

extern "C" fn collect_cb(d: IoxServiceDescriptionByVal) {
    let triple = (
        cstr_to_string(&d.service),
        cstr_to_string(&d.instance),
        cstr_to_string(&d.event),
    );
    DISCOVERED.with(|v| v.borrow_mut().push(triple));
}

/// A ZeroDDS reader of Cyclone PSMX samples over iceoryx SHM.
pub struct CycloneIoxReader {
    sub: *mut IoxSub,
    _storage: Box<Storage>,
}

impl CycloneIoxReader {
    /// Subscribes to the iceoryx service `{service, instance, event}`.
    pub fn new(app: &str, service: &str, instance: &str, event: &str) -> Self {
        init_runtime(app);
        let (cs, ci, ce) = (
            CString::new(service).unwrap(),
            CString::new(instance).unwrap(),
            CString::new(event).unwrap(),
        );
        let mut storage = Storage::new();
        // SAFETY: valid storage + NUL-terminated strings; options default-init'd.
        let sub = unsafe {
            let mut opts: IoxSubOptions = std::mem::zeroed();
            iox_sub_options_init(&mut opts);
            opts.queue_capacity = 16;
            opts.history_request = 4;
            let s = iox_sub_init(
                storage.as_mut() as *mut Storage,
                cs.as_ptr(),
                ci.as_ptr(),
                ce.as_ptr(),
                &opts,
            );
            iox_sub_subscribe(s);
            s
        };
        Self {
            sub,
            _storage: storage,
        }
    }

    /// Discovers a registered Cyclone iceoryx service whose instance (DDS type
    /// name) equals `instance`, returning the live `(service, instance, event)`
    /// triple. Retries for ~`tries * 200 ms`.
    pub fn discover_by_instance(
        app: &str,
        instance: &str,
        tries: u32,
    ) -> Option<(String, String, String)> {
        init_runtime(app);
        let mut storage = Storage::new();
        // SAFETY: valid storage; the callback only appends to a thread-local.
        let disc = unsafe { iox_service_discovery_init(storage.as_mut() as *mut Storage) };
        for _ in 0..tries {
            DISCOVERED.with(|v| v.borrow_mut().clear());
            // SAFETY: valid handle + callback; null filters = wildcards.
            unsafe {
                iox_service_discovery_find_service_apply_callable(
                    disc,
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    collect_cb,
                    MESSAGING_PATTERN_PUB_SUB,
                );
            }
            let hit =
                DISCOVERED.with(|v| v.borrow().iter().find(|(_, i, _)| i == instance).cloned());
            if hit.is_some() {
                return hit;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        None
    }

    /// Takes the next sample, returning its PSMX metadata header and a copy of
    /// the raw payload. `None` when no sample is queued.
    pub fn take(&self) -> Option<(DdsPsmxMetadata, Vec<u8>)> {
        let mut payload: *const c_void = std::ptr::null();
        // SAFETY: valid subscriber; payload pointer set by the binding on SUCCESS.
        unsafe {
            if iox_sub_take_chunk(self.sub, &mut payload) != CHUNK_RECEIVE_SUCCESS {
                return None;
            }
            let ch = iox_chunk_header_from_user_payload_const(payload);
            let md = iox_chunk_header_to_user_header_const(ch) as *const DdsPsmxMetadata;
            let meta = *md;
            let bytes = std::slice::from_raw_parts(payload as *const u8, meta.sample_size as usize)
                .to_vec();
            iox_sub_release_chunk(self.sub, payload);
            Some((meta, bytes))
        }
    }
}

impl Drop for CycloneIoxReader {
    fn drop(&mut self) {
        // SAFETY: sub was created by iox_sub_init and not yet deinit'd.
        unsafe { iox_sub_deinit(self.sub) };
    }
}

// SAFETY: the held value is an iceoryx C-binding handle into the process-global
// RouDi-managed shared memory; it is not tied to the creating thread. Callers
// serialize access (the DCPS side-table wraps it in a Mutex), so moving it
// across threads is sound.
unsafe impl Send for CycloneIoxReader {}

/// A ZeroDDS writer that publishes Cyclone-PSMX-shaped samples over iceoryx SHM.
pub struct CycloneIoxWriter {
    pubp: *mut IoxPub,
    _storage: Box<Storage>,
}

impl CycloneIoxWriter {
    /// Offers the iceoryx service `{service, instance, event}` as a publisher.
    pub fn new(app: &str, service: &str, instance: &str, event: &str) -> Self {
        init_runtime(app);
        let (cs, ci, ce) = (
            CString::new(service).unwrap(),
            CString::new(instance).unwrap(),
            CString::new(event).unwrap(),
        );
        let mut storage = Storage::new();
        // SAFETY: valid storage + strings; options default-init'd.
        let pubp = unsafe {
            let mut opts: IoxPubOptions = std::mem::zeroed();
            iox_pub_options_init(&mut opts);
            opts.history_capacity = 4;
            let p = iox_pub_init(
                storage.as_mut() as *mut Storage,
                cs.as_ptr(),
                ci.as_ptr(),
                ce.as_ptr(),
                &opts,
            );
            iox_pub_offer(p);
            p
        };
        Self {
            pubp,
            _storage: storage,
        }
    }

    /// Loans a chunk sized for `payload`, writes the [`DdsPsmxMetadata`] user
    /// header + the raw payload, and publishes it. Returns `false` if the loan
    /// failed.
    pub fn write_raw(&self, meta: &DdsPsmxMetadata, payload: &[u8]) -> bool {
        let mut up: *mut c_void = std::ptr::null_mut();
        // SAFETY: valid publisher; sizes/alignments match the loan contract and
        // Cyclone's loan (user-header = dds_psmx_metadata_t).
        unsafe {
            let r = iox_pub_loan_aligned_chunk_with_user_header(
                self.pubp,
                &mut up,
                payload.len() as u32,
                CHUNK_DEFAULT_USER_PAYLOAD_ALIGNMENT,
                std::mem::size_of::<DdsPsmxMetadata>() as u32,
                std::mem::align_of::<DdsPsmxMetadata>() as u32,
            );
            if r != ALLOCATION_SUCCESS {
                return false;
            }
            let ch = iox_chunk_header_from_user_payload(up);
            let hdr = iox_chunk_header_to_user_header(ch) as *mut DdsPsmxMetadata;
            *hdr = *meta;
            std::ptr::copy_nonoverlapping(payload.as_ptr(), up as *mut u8, payload.len());
            iox_pub_publish_chunk(self.pubp, up);
        }
        true
    }
}

impl Drop for CycloneIoxWriter {
    fn drop(&mut self) {
        // SAFETY: pub created by iox_pub_init, not yet deinit'd.
        unsafe { iox_pub_deinit(self.pubp) };
    }
}

// SAFETY: see `CycloneIoxReader` — an iceoryx C-binding handle into process
// shared memory, used under external serialization (Mutex in the DCPS table).
unsafe impl Send for CycloneIoxWriter {}

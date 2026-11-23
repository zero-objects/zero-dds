// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! zerodds-shm-transport-1.0 §6.4 without an operating system: core 0 of a
//! QEMU `virt` machine (two Cortex-A53) is the owner of a ring in a region
//! of RAM, core 1, which core 0 starts over PSCI, its consumer. They use
//! `RegionShmTransport` as an integrator would: no `std`, a bump allocator,
//! `WFE`/`SEV` as the wait. The owner sends 10 000 frames of 4 to 203
//! bytes through an 8 KiB ring; the consumer checks each one, in order and
//! byte by byte. Then the consumer parks in one more `recv` without a
//! timeout, and core 0 drops the owner: the drop sets the shutdown flag and
//! sends the event, and the `recv` returns the targeted error (§5.3). Core
//! 0 prints the result to the PL011 UART and ends QEMU over semihosting with
//! exit code 0 when every frame arrived and the drop ended the park, else 1.
//!
//! Both cores run with the MMU on (an identity map: RAM normal and
//! cacheable, inner shareable; the devices below 1 GiB device memory), so
//! that the exclusive accesses of the atomics and unaligned copies are
//! those of normal memory, as on a board.

#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::arch::{asm, global_asm};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use core::time::Duration;

use zerodds_rtps::wire_types::Locator;
use zerodds_transport::{RecvError, SendError, Transport};
use zerodds_transport_shm::region::{
    Parked, RegionConfig, RegionShmError, RegionShmTransport, RingWait,
};
use zerodds_transport_shm::ring::{HEADER_BYTES, RingError};

const FRAMES: u32 = 10_000;
const CAPACITY: usize = 8192;
const LEN: usize = HEADER_BYTES + CAPACITY;
const OWNER: [u8; 16] = [0xA0; 16];
const CONSUMER: [u8; 16] = [0xB0; 16];
const CONFIG: RegionConfig = RegionConfig {
    max_datagram: 512,
    recv_timeout: None,
};

#[repr(C, align(64))]
struct Region([u8; LEN]);

/// The region both cores map: the ring's header and data.
static mut REGION: Region = Region([0; LEN]);

/// Set by core 1 when it is done; `RECEIVED` frames arrived whole, and
/// `MISMATCH` names the first that did not (`u32::MAX`: none).
static DONE: AtomicBool = AtomicBool::new(false);
static RECEIVED: AtomicU32 = AtomicU32::new(0);
static MISMATCH: AtomicU32 = AtomicU32::new(u32::MAX);
/// Parks of core 1's `recv`; `LAST_PARK` is the count its last `recv`
/// reaches when it parks (0: not there yet).
static PARKS: AtomicU32 = AtomicU32::new(0);
static LAST_PARK: AtomicU32 = AtomicU32::new(0);
/// Set by core 1 when its last `recv` returned the owner's drop.
static ENDED_BY_DROP: AtomicBool = AtomicBool::new(false);

global_asm!(
    r#"
    .section .text.boot, "ax"
    .global _start
_start:
    mrs     x1, mpidr_el1
    and     x1, x1, #0xff
    cbnz    x1, 9f
    ldr     x0, =__stack0_top
    mov     sp, x0
    // .bss to zero
    ldr     x0, =__bss_start
    ldr     x1, =__bss_end
1:  cmp     x0, x1
    b.hs    2f
    stp     xzr, xzr, [x0], #16
    b       1b
2:  // Level-1 table, 1 GiB blocks: 0..1 GiB device (UART), 1..2 GiB RAM.
    ldr     x0, =__page_table
    mov     x1, #0x401              // block, AF, AttrIndx 0
    str     x1, [x0]
    ldr     x1, =0x40000705         // 1 GiB, block, AF, inner shareable, AttrIndx 1
    str     x1, [x0, #8]
    dsb     sy
    bl      mmu_on
    bl      main0
9:  wfe
    b       9b

    .global _start1
_start1:
    ldr     x0, =__stack1_top
    mov     sp, x0
    bl      mmu_on
    bl      main1
8:  wfe
    b       8b

mmu_on:
    mov     x0, #(3 << 20)          // FP and SIMD at EL1 and EL0
    msr     cpacr_el1, x0
    mov     x0, #0xff00             // attr 0 device nGnRnE, attr 1 normal WB
    msr     mair_el1, x0
    ldr     x0, =0x200803519        // T0SZ 25, WB WA, inner shareable, 4K, no TTBR1, 40-bit PA
    msr     tcr_el1, x0
    ldr     x0, =__page_table
    msr     ttbr0_el1, x0
    isb
    tlbi    vmalle1
    dsb     nsh
    isb
    mrs     x0, sctlr_el1
    orr     x0, x0, #1              // M
    orr     x0, x0, #(1 << 2)       // C
    orr     x0, x0, #(1 << 12)      // I
    msr     sctlr_el1, x0
    isb
    ret
    .ltorg
"#
);

unsafe extern "C" {
    fn _start1();
}

/// `WFE` until the park is over (a frame, the owner's shutdown flag); the
/// other core's `SEV` ends each `WFE`. A timeout is measured on the generic
/// timer.
struct Events;

impl RingWait for Events {
    fn park(&self, on: &Parked<'_>, timeout: Option<Duration>) -> bool {
        PARKS.fetch_add(1, Ordering::AcqRel);
        let deadline = timeout.map(|t| now().saturating_add(ticks(t)));
        loop {
            if on.is_over() {
                return true;
            }
            match deadline {
                // SAFETY: waits for an event; no memory is touched.
                None => unsafe { asm!("wfe", options(nomem, nostack)) },
                Some(deadline) if now() >= deadline => return false,
                Some(_) => core::hint::spin_loop(),
            }
        }
    }

    fn wake(&self, _word: &AtomicU32) {
        // SAFETY: orders the store before the event, which every core sees.
        unsafe { asm!("dsb ish", "sev", options(nostack)) };
    }
}

fn now() -> u64 {
    let ticks: u64;
    // SAFETY: reads the virtual counter.
    unsafe { asm!("mrs {}, cntvct_el0", out(reg) ticks, options(nomem, nostack)) };
    ticks
}

fn frequency() -> u64 {
    let hz: u64;
    // SAFETY: reads the counter's frequency.
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) hz, options(nomem, nostack)) };
    hz.max(1)
}

/// Ticks of the generic timer in `t`.
fn ticks(t: Duration) -> u64 {
    u64::try_from(t.as_nanos())
        .unwrap_or(u64::MAX)
        .saturating_mul(frequency())
        / 1_000_000_000
}

/// Frame `i`: its number, then `(i * 37) % 200` bytes of a pattern.
fn frame(i: u32, buf: &mut [u8; 512]) -> &[u8] {
    let len = 4 + (i as usize * 37) % 200;
    buf[..4].copy_from_slice(&i.to_le_bytes());
    for (k, b) in buf[4..len].iter_mut().enumerate() {
        *b = (i as u8) ^ (k as u8);
    }
    &buf[..len]
}

fn region() -> *mut u8 {
    (&raw mut REGION).cast::<u8>()
}

/// Core 0: starts core 1, opens the owner side and sends every frame.
#[unsafe(no_mangle)]
extern "C" fn main0() -> ! {
    let entry = _start1 as *const () as usize as u64;
    if psci_cpu_on(1, entry) != 0 {
        print("zerodds-shm-region-two-cores: PSCI CPU_ON failed\n");
        exit(1);
    }
    // SAFETY: the region is this program's, 64-aligned, `LEN` bytes, and
    // used by this owner and core 1's consumer alone.
    let owner = match unsafe {
        RegionShmTransport::open_owner(region(), LEN, OWNER, CONSUMER, CONFIG, Events)
    } {
        Ok(owner) => owner,
        Err(_) => {
            print("zerodds-shm-region-two-cores: the owner did not open\n");
            exit(1);
        }
    };
    while !owner.peer_attached() {
        core::hint::spin_loop();
    }
    let mut buf = [0u8; 512];
    let mut full = 0u64;
    for i in 0..FRAMES {
        loop {
            match owner.send(&Locator::shm(CONSUMER), frame(i, &mut buf)) {
                Ok(()) => break,
                Err(SendError::Io { message }) if message.starts_with("shm ring full") => {
                    full += 1;
                }
                Err(_) => {
                    print("zerodds-shm-region-two-cores: a send failed\n");
                    exit(1);
                }
            }
        }
    }
    // Core 1 parks in its last `recv`, without a timeout; 10 ms later the
    // owner's drop has to end that park.
    while !DONE.load(Ordering::Acquire) {
        let last = LAST_PARK.load(Ordering::Acquire);
        if last != 0 && PARKS.load(Ordering::Acquire) >= last {
            break;
        }
        core::hint::spin_loop();
    }
    let parked = !DONE.load(Ordering::Acquire);
    let settle = now().saturating_add(ticks(Duration::from_millis(10)));
    while now() < settle {
        core::hint::spin_loop();
    }
    drop(owner);
    while !DONE.load(Ordering::Acquire) {
        // SAFETY: waits for core 1's event.
        unsafe { asm!("wfe", options(nomem, nostack)) };
    }
    let received = RECEIVED.load(Ordering::Acquire);
    let mismatch = MISMATCH.load(Ordering::Acquire);
    let ended = parked && ENDED_BY_DROP.load(Ordering::Acquire);
    print("zerodds-shm-region-two-cores: sent ");
    print_u64(u64::from(FRAMES));
    print(", received ");
    print_u64(u64::from(received));
    print(", sends on a full ring ");
    print_u64(full);
    if mismatch != u32::MAX {
        print(", first wrong frame ");
        print_u64(u64::from(mismatch));
    }
    print(if ended {
        ", the owner's drop ended the parked recv"
    } else {
        ", the owner's drop did not end a parked recv"
    });
    print("\n");
    exit(if received == FRAMES && mismatch == u32::MAX && ended {
        0
    } else {
        1
    });
}

/// Core 1: opens the consumer side once the owner made the ring, checks
/// every frame, then parks in one more `recv` until the owner's drop.
#[unsafe(no_mangle)]
extern "C" fn main1() -> ! {
    let consumer = loop {
        // SAFETY: as in `main0`.
        match unsafe {
            RegionShmTransport::open_consumer(region(), LEN, CONSUMER, OWNER, CONFIG, Events)
        } {
            Ok(consumer) => break consumer,
            Err(RegionShmError::Ring(RingError::NotReady)) => core::hint::spin_loop(),
            Err(_) => finish(0, 0),
        }
    };
    let mut buf = [0u8; 512];
    for i in 0..FRAMES {
        match consumer.recv() {
            Ok(datagram) if &datagram.data[..] == frame(i, &mut buf) => {
                RECEIVED.store(i + 1, Ordering::Release);
            }
            _ => finish(i, i),
        }
    }
    // Every frame is read: the next `recv` parks, and its park is this one.
    LAST_PARK.store(PARKS.load(Ordering::Acquire) + 1, Ordering::Release);
    if let Err(RecvError::Io {
        message: "shm owner terminated",
    }) = consumer.recv()
    {
        ENDED_BY_DROP.store(true, Ordering::Release);
    }
    finish(FRAMES, u32::MAX)
}

/// Core 1's end: the counts for core 0, then its event.
fn finish(received: u32, mismatch: u32) -> ! {
    RECEIVED.store(received, Ordering::Release);
    MISMATCH.store(mismatch, Ordering::Release);
    DONE.store(true, Ordering::Release);
    // SAFETY: orders the stores before the event.
    unsafe { asm!("dsb ish", "sev", options(nostack)) };
    loop {
        // SAFETY: parks the core.
        unsafe { asm!("wfe", options(nomem, nostack)) };
    }
}

/// PSCI `CPU_ON` over HVC, as QEMU's `virt` offers it to a kernel at EL1.
fn psci_cpu_on(target: u64, entry: u64) -> i64 {
    let mut x0: u64 = 0xC400_0003;
    // SAFETY: the PSCI call takes x0 to x3 and returns in x0.
    unsafe {
        asm!("hvc #0", inout("x0") x0, in("x1") target, in("x2") entry, in("x3") 0u64,
             options(nostack));
    }
    x0 as i64
}

const UART: *mut u32 = 0x0900_0000 as *mut u32;

fn print(s: &str) {
    for b in s.bytes() {
        // SAFETY: the data register of QEMU's PL011, mapped as device memory.
        unsafe { core::ptr::write_volatile(UART, u32::from(b)) };
    }
}

fn print_u64(mut n: u64) {
    let mut digits = [0u8; 20];
    let mut i = digits.len();
    loop {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    print(core::str::from_utf8(&digits[i..]).unwrap_or("?"));
}

/// Semihosting `SYS_EXIT` with `ADP_Stopped_ApplicationExit` and `code`:
/// QEMU ends with that exit status.
fn exit(code: u32) -> ! {
    let block: [u64; 2] = [0x20026, u64::from(code)];
    // SAFETY: the semihosting call reads the two words of `block`.
    unsafe {
        asm!("hlt #0xf000", in("x0") 0x18u64, in("x1") block.as_ptr(), options(nostack));
    }
    loop {
        // SAFETY: parks the core if QEMU did not end.
        unsafe { asm!("wfe", options(nomem, nostack)) };
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    print("zerodds-shm-region-two-cores: panic\n");
    exit(2);
}

/// Hands out a static heap upwards and never takes memory back: the
/// consumer's frames total a few MiB.
struct Bump;

const HEAP_BYTES: usize = 32 << 20;

#[repr(C, align(16))]
struct Heap([u8; HEAP_BYTES]);

static mut HEAP: Heap = Heap([0; HEAP_BYTES]);
static NEXT: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every block lies in `HEAP`, aligned as asked, and no two overlap
// (the offset moves by compare-and-swap).
unsafe impl GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let base = (&raw mut HEAP).cast::<u8>() as usize;
        let mut at = NEXT.load(Ordering::Relaxed);
        loop {
            let start = (base + at + layout.align() - 1) & !(layout.align() - 1);
            let end = start - base + layout.size();
            if end > HEAP_BYTES {
                return core::ptr::null_mut();
            }
            match NEXT.compare_exchange_weak(at, end, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => return start as *mut u8,
                Err(now) => at = now,
            }
        }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[global_allocator]
static ALLOCATOR: Bump = Bump;

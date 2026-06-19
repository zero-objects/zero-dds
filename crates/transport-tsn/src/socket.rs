// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Live AF_PACKET transport for RTPS-direct-on-Ethernet
//! (DDS-TSN 1.0 Annex A). Linux-only, feature `live`.
//!
//! Sends/receives RTPS messages directly in the Ethernet frame payload
//! (EtherType `0x88B5`, no IP/UDP stack) over an
//! `AF_PACKET`/`SOCK_RAW` socket bound to an interface. The
//! frame header is built via [`crate::ethernet_psm`] (incl.
//! an optional IEEE 802.1Q VLAN tag with PCP for TSN prioritization).
//!
//! # Privileges
//!
//! `AF_PACKET`/`SOCK_RAW` requires `CAP_NET_RAW`. Without this
//! capability, even [`TsnTransport::bind`] fails with `EPERM`.
//!
//! # Hardware validation
//!
//! The frame logic is unit-tested ([`crate::live_frame`]), but the
//! throughput/latency of this path and the TSN time guarantees are not yet
//! verified against a real TSN network for lack of
//! TSN-capable hardware — see the crate-doc section "Hardware validation".

#![cfg(all(feature = "live", target_os = "linux"))]

use std::io;
use std::os::fd::RawFd;
use std::sync::Mutex;
use std::time::Duration;

use zerodds_rtps::wire_types::{Locator, LocatorKind};
use zerodds_transport::{ReceivedDatagram, RecvError, SendError, Transport};

use crate::ethernet_psm::{ETHERTYPE_RTPS, EthernetFrameHeader};
use crate::live_frame::{build_frame, outbound_vlan, parse_mac_sysfs};
use crate::vlan_tag::Ieee802VlanTag;

/// Receive buffer size (max Ethernet jumbo + reserve).
const RECV_BUF: usize = 65_536;

/// Live TSN transport. See the module docs.
pub struct TsnTransport {
    fd: RawFd,
    ifindex: i32,
    local_mac: [u8; 6],
    /// Default VLAN VID for outbound frames (0 = untagged). Overridden by
    /// the destination locator if it carries a VID.
    vlan_vid: u16,
    /// PCP (Priority Code Point) for outbound tagged frames.
    pcp: u8,
    local_locator: Locator,
    recv_buf: Mutex<Vec<u8>>,
}

/// Closes the fd on an early-return error during `bind`.
struct FdGuard(RawFd);

impl Drop for FdGuard {
    fn drop(&mut self) {
        if self.0 >= 0 {
            // SAFETY: self.0 is a valid, not-yet-closed fd.
            unsafe {
                libc::close(self.0);
            }
        }
    }
}

impl FdGuard {
    /// Disarms the guard — the fd is no longer closed automatically
    /// (ownership passes to the `TsnTransport`).
    fn disarm(&mut self) {
        self.0 = -1;
    }
}

impl TsnTransport {
    /// Binds an `AF_PACKET`/`SOCK_RAW` socket to `ifname`, filtered
    /// on EtherType `0x88B5`. `vlan_vid`/`pcp` tag outbound frames
    /// (vid=0 → untagged). `recv_timeout` sets `SO_RCVTIMEO` so the
    /// recv loop can periodically check a stop flag.
    ///
    /// # Errors
    /// `io::Error` if socket/bind/setsockopt fail (e.g.
    /// `EPERM` without `CAP_NET_RAW`, or an unknown interface).
    pub fn bind(
        ifname: &str,
        vlan_vid: u16,
        pcp: u8,
        recv_timeout: Option<Duration>,
    ) -> io::Result<Self> {
        let proto: u16 = (ETHERTYPE_RTPS).to_be(); // htons(0x88B5)

        // SAFETY: socket() with constant, valid arguments; the return value
        // is checked immediately.
        let fd = unsafe { libc::socket(libc::AF_PACKET, libc::SOCK_RAW, i32::from(proto)) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut guard = FdGuard(fd);

        let cname = std::ffi::CString::new(ifname)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "ifname contains NUL"))?;
        // SAFETY: cname is a valid NUL-terminated C string.
        let ifindex = unsafe { libc::if_nametoindex(cname.as_ptr()) };
        if ifindex == 0 {
            return Err(io::Error::last_os_error());
        }
        let ifindex = ifindex as i32;

        // SAFETY: sockaddr_ll is POD; zeroed + field init is valid.
        let mut sll: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
        sll.sll_family = libc::AF_PACKET as u16;
        sll.sll_protocol = proto;
        sll.sll_ifindex = ifindex;
        // SAFETY: &sll points to a valid sockaddr_ll of matching length.
        let rc = unsafe {
            libc::bind(
                fd,
                std::ptr::addr_of!(sll).cast::<libc::sockaddr>(),
                std::mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
            )
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }

        if let Some(to) = recv_timeout {
            let tv = libc::timeval {
                tv_sec: to.as_secs() as libc::time_t,
                tv_usec: i64::from(to.subsec_micros()) as libc::suseconds_t,
            };
            // SAFETY: setsockopt with a valid timeval pointer + correct length.
            let rc = unsafe {
                libc::setsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_RCVTIMEO,
                    std::ptr::addr_of!(tv).cast::<libc::c_void>(),
                    std::mem::size_of::<libc::timeval>() as libc::socklen_t,
                )
            };
            if rc < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        // Enable `PACKET_AUXDATA`: NICs with HW VLAN offload strip the
        // 802.1Q tag from the frame on RX and deliver it instead as a
        // control message (`tpacket_auxdata.tp_vlan_tci` with
        // `TP_STATUS_VLAN_VALID`); `recv` reconstructs the VID from it.
        // On pure software links (veth) the kernel removes the tag but
        // does not report it in the auxdata — there the RX VID is not
        // reconstructible (the locator falls back to VLAN 0). See `recv`.
        let aux_on: libc::c_int = 1;
        // SAFETY: setsockopt with a valid int pointer + correct length.
        let rc = unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_PACKET,
                libc::PACKET_AUXDATA,
                std::ptr::addr_of!(aux_on).cast::<libc::c_void>(),
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            )
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }

        let local_mac = read_iface_mac(ifname)?;
        let local_locator = Locator::tsn(local_mac, vlan_vid);

        guard.disarm();
        Ok(Self {
            fd,
            ifindex,
            local_mac,
            vlan_vid,
            pcp,
            local_locator,
            recv_buf: Mutex::new(vec![0u8; RECV_BUF]),
        })
    }

    /// Local interface MAC (for tests/diagnostics).
    #[must_use]
    pub fn local_mac(&self) -> [u8; 6] {
        self.local_mac
    }

    fn build_vlan(&self, target_vid: u16) -> Result<Option<Ieee802VlanTag>, SendError> {
        outbound_vlan(self.vlan_vid, self.pcp, target_vid).map_err(|_| SendError::Io {
            message: "invalid VLAN tag (pcp/vid out of range)",
        })
    }
}

impl Transport for TsnTransport {
    fn send(&self, dest: &Locator, data: &[u8]) -> Result<(), SendError> {
        if dest.kind != LocatorKind::Tsn {
            return Err(SendError::UnsupportedLocator);
        }
        let mut dst_mac = [0u8; 6];
        dst_mac.copy_from_slice(&dest.address[..6]);
        let target_vid = u16::from_be_bytes([dest.address[6], dest.address[7]]);
        let vlan_tag = self.build_vlan(target_vid)?;
        let frame = build_frame(self.local_mac, dst_mac, vlan_tag, data);

        // SAFETY: sockaddr_ll is POD; zeroed + field init valid.
        let mut sll: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
        sll.sll_family = libc::AF_PACKET as u16;
        sll.sll_protocol = (ETHERTYPE_RTPS).to_be();
        sll.sll_ifindex = self.ifindex;
        sll.sll_halen = 6;
        sll.sll_addr[..6].copy_from_slice(&dst_mac);

        // SAFETY: frame pointer + length valid; sll correctly initialized
        // and of matching length.
        let sent = unsafe {
            libc::sendto(
                self.fd,
                frame.as_ptr().cast::<libc::c_void>(),
                frame.len(),
                0,
                std::ptr::addr_of!(sll).cast::<libc::sockaddr>(),
                std::mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
            )
        };
        if sent < 0 {
            return Err(SendError::Io {
                message: "AF_PACKET sendto failed",
            });
        }
        Ok(())
    }

    fn recv(&self) -> Result<ReceivedDatagram, RecvError> {
        let mut buf = self.recv_buf.lock().map_err(|_| RecvError::Io {
            message: "recv buffer poisoned",
        })?;
        loop {
            let (n, aux_vid) = recvmsg_with_vlan(self.fd, &mut buf)?;
            let frame = &buf[..n];
            let (hdr, consumed) = match EthernetFrameHeader::from_wire(frame) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if hdr.ethertype != ETHERTYPE_RTPS {
                continue;
            }
            // Ignore our own sent frames (some interfaces
            // mirror outbound back).
            if hdr.source.0 == self.local_mac {
                continue;
            }
            // VLAN VID: preferably from the inline tag; if the kernel moves
            // the tag into the auxdata (typical for AF_PACKET RX), from there.
            let vid = hdr.vlan_tag.map(|vt| vt.vid).or(aux_vid).unwrap_or(0);
            let source = Locator::tsn(hdr.source.0, vid);
            let data: std::sync::Arc<[u8]> = std::sync::Arc::from(&frame[consumed..]);
            return Ok(ReceivedDatagram { source, data });
        }
    }

    fn local_locator(&self) -> Locator {
        self.local_locator
    }
}

impl Drop for TsnTransport {
    fn drop(&mut self) {
        // SAFETY: self.fd is our own fd, closed exactly once.
        unsafe {
            libc::close(self.fd);
        }
    }
}

/// Receives a frame via `recvmsg` and extracts a VLAN VID possibly
/// stripped by the kernel into the `PACKET_AUXDATA` control message.
/// Returns `(bytes, Some(vid))` if a valid VLAN tag was in the
/// auxdata, otherwise `(bytes, None)`.
fn recvmsg_with_vlan(fd: RawFd, buf: &mut [u8]) -> Result<(usize, Option<u16>), RecvError> {
    let mut iov = libc::iovec {
        iov_base: buf.as_mut_ptr().cast::<libc::c_void>(),
        iov_len: buf.len(),
    };
    // 8-byte-aligned control buffer (cmsghdr alignment) for a
    // tpacket_auxdata control message (64 bytes are plenty).
    let mut cmsg_space = [0u64; 8];
    // SAFETY: msghdr is POD; zeroed + field init is valid.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = std::ptr::addr_of_mut!(iov);
    msg.msg_iovlen = 1;
    msg.msg_control = cmsg_space.as_mut_ptr().cast::<libc::c_void>();
    msg.msg_controllen = std::mem::size_of_val(&cmsg_space) as _;

    // SAFETY: msg, iov and the control buffer are valid and of matching length.
    let n = unsafe { libc::recvmsg(fd, std::ptr::addr_of_mut!(msg), 0) };
    if n < 0 {
        let e = io::Error::last_os_error();
        if matches!(
            e.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
        ) {
            return Err(RecvError::Timeout);
        }
        return Err(RecvError::Io {
            message: "AF_PACKET recvmsg failed",
        });
    }

    let mut aux_vid = None;
    // SAFETY: msg was filled by recvmsg; CMSG_*HDR return valid
    // pointers into the control buffer or null.
    let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(std::ptr::addr_of!(msg)) };
    while !cmsg.is_null() {
        // SAFETY: cmsg is non-null and points into the control buffer.
        let c = unsafe { &*cmsg };
        if c.cmsg_level == libc::SOL_PACKET && c.cmsg_type == libc::PACKET_AUXDATA {
            // SAFETY: for PACKET_AUXDATA, CMSG_DATA carries a
            // tpacket_auxdata; read unaligned, since the data offset in the
            // control buffer is not guaranteed to be aligned.
            let aux = unsafe {
                std::ptr::read_unaligned(libc::CMSG_DATA(cmsg).cast::<libc::tpacket_auxdata>())
            };
            if aux.tp_status & libc::TP_STATUS_VLAN_VALID != 0 {
                aux_vid = Some(aux.tp_vlan_tci & 0x0fff);
            }
        }
        // SAFETY: msg + cmsg are valid.
        cmsg = unsafe { libc::CMSG_NXTHDR(std::ptr::addr_of!(msg), cmsg) };
    }

    Ok((n as usize, aux_vid))
}

/// Reads an interface's MAC from `/sys/class/net/<if>/address`
/// (format `aa:bb:cc:dd:ee:ff`). Avoids the `SIOCGIFHWADDR` ioctl.
fn read_iface_mac(ifname: &str) -> io::Result<[u8; 6]> {
    let path = format!("/sys/class/net/{ifname}/address");
    let text = std::fs::read_to_string(&path)?;
    parse_mac_sysfs(&text).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid MAC"))
}

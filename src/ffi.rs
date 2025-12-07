//! FFI bindings to Zephyr C structures

use core::ffi::c_void;

/// Opaque pointer to Zephyr net_pkt structure
#[repr(C)]
pub struct NetPkt {
    _private: [u8; 0],
}

/// Opaque pointer to net_if structure  
#[repr(C)]
pub struct NetIf {
    _private: [u8; 0],
}

/// NAT configuration structure (exposed to C)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NatConfigC {
    pub internal_network: [u8; 4],
    pub internal_netmask: [u8; 4],
    pub external_ip: [u8; 4],
}

/// IPv4 header structure (matching Zephyr's net_ipv4_hdr)
#[repr(C, packed)]
#[derive(Clone, Copy)] // ← BURASI EKLENDİ
pub struct Ipv4Hdr {
    pub vhl: u8,         // Version + IHL
    pub tos: u8,         // Type of service
    pub len: [u8; 2],    // Total length (big-endian)
    pub id: [u8; 2],     // Identification
    pub offset: [u8; 2], // Flags + fragment offset
    pub ttl: u8,         // Time to live
    pub proto: u8,       // Protocol (TCP=6, UDP=17, ICMP=1)
    pub chksum: u16,     // Header checksum
    pub src: [u8; 4],    // Source IP
    pub dst: [u8; 4],    // Destination IP
}

/// TCP header structure
#[repr(C, packed)]
#[derive(Clone, Copy)] // ← BURASI EKLENDİ
pub struct TcpHdr {
    pub src_port: u16, // Source port (big-endian)
    pub dst_port: u16, // Destination port
    pub seq: u32,      // Sequence number
    pub ack: u32,      // Acknowledgment number
    pub offset: u8,    // Data offset + reserved
    pub flags: u8,     // TCP flags
    pub window: u16,   // Window size
    pub chksum: u16,   // Checksum
    pub urgent: u16,   // Urgent pointer
}

/// UDP header structure
#[repr(C, packed)]
#[derive(Clone, Copy)] // ← BURASI EKLENDİ
pub struct UdpHdr {
    pub src_port: u16, // Source port (big-endian)
    pub dst_port: u16, // Destination port
    pub len: u16,      // Length
    pub chksum: u16,   // Checksum
}

extern "C" {
    /// Get packet buffer pointer (direct access)
    pub fn net_pkt_get_buffer(pkt: *mut NetPkt) -> *mut u8;

    /// Mark packet as modified
    pub fn net_pkt_set_modified(pkt: *mut NetPkt);

    /// Get packet data pointer (with access structure)
    pub fn net_pkt_get_data(pkt: *mut NetPkt, access: *mut c_void) -> *mut u8;

    /// Get packet length
    pub fn net_pkt_get_len(pkt: *const NetPkt) -> usize;

    /// Get packet interface
    pub fn net_pkt_iface(pkt: *const NetPkt) -> *mut NetIf;

    /// Set packet data (after modification)
    pub fn net_pkt_set_data(pkt: *mut NetPkt, access: *mut c_void) -> i32;
}

// Helper to convert u16 between network and host byte order
#[inline]
pub fn ntohs(n: u16) -> u16 {
    u16::from_be(n)
}

#[inline]
pub fn htons(h: u16) -> u16 {
    h.to_be()
}

#[inline]
pub fn ntohl(n: u32) -> u32 {
    u32::from_be(n)
}

#[inline]
pub fn htonl(h: u32) -> u32 {
    h.to_be()
}

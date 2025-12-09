//! NAT table entry structure
use crate::nat::NetIf;

/// IP protocol types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Protocol {
    Tcp = 6,
    Udp = 17,
    Icmp = 1,
}

impl Protocol {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            6 => Some(Protocol::Tcp),
            17 => Some(Protocol::Udp),
            1 => Some(Protocol::Icmp),
            _ => None,
        }
    }
}

/// NAT connection entry
#[derive(Debug, Clone, Copy)]
pub struct NatEntry {
    /// Internal (LAN) IP address
    pub internal_ip: [u8; 4],

    /// Internal (LAN) port
    pub internal_port: u16,

    /// External (WAN) IP address (our public IP)
    pub external_ip: [u8; 4],

    /// External (WAN) port (mapped port)
    pub external_port: u16,

    /// Remote IP address (destination on internet)
    pub remote_ip: [u8; 4],

    /// Remote port
    pub remote_port: u16,

    /// Protocol (TCP/UDP/ICMP)
    pub protocol: Protocol,

    /// Last activity timestamp (in seconds since boot)
    pub last_activity: u32,

    /// Entry is in use
    pub in_use: bool,

    /// Internal (LAN) interface pointer
    pub internal_iface: *mut NetIf,

    /// External (WAN) interface pointer
    pub external_iface: *mut NetIf,
}

impl NatEntry {
    pub fn new() -> Self {
        Self {
            internal_ip: [0; 4],
            internal_port: 0,
            external_ip: [0; 4],
            external_port: 0,
            remote_ip: [0; 4],
            remote_port: 0,
            protocol: Protocol::Tcp,
            last_activity: 0,
            in_use: false,
            internal_iface: core::ptr::null_mut(),
            external_iface: core::ptr::null_mut(),
        }
    }

    /// Check if entry matches outbound packet
    pub fn matches_outbound(
        &self,
        src_ip: &[u8; 4],
        src_port: u16,
        dst_ip: &[u8; 4],
        dst_port: u16,
        proto: Protocol,
    ) -> bool {
        if !self.in_use || self.protocol != proto {
            return false;
        }

        // IP addresses must match
        if self.internal_ip != *src_ip || self.remote_ip != *dst_ip {
            return false;
        }

        // Port matching depends on protocol
        match proto {
            Protocol::Icmp => {
                // For ICMP: only match on ICMP ID (stored in src_port)
                // dst_port is ignored for ICMP (it's the sequence number)
                self.internal_port == src_port
            }
            Protocol::Tcp | Protocol::Udp => {
                // For TCP/UDP: match both ports
                self.internal_port == src_port && self.remote_port == dst_port
            }
        }
    }

    /// Check if entry matches inbound packet
    pub fn matches_inbound(
        &self,
        src_ip: &[u8; 4],
        src_port: u16,
        dst_port: u16,
        proto: Protocol,
    ) -> bool {
        if !self.in_use || self.protocol != proto {
            return false;
        }

        // Remote IP must match
        if self.remote_ip != *src_ip {
            return false;
        }

        // External port must match (this is what we're looking up by)
        if self.external_port != dst_port {
            return false;
        }

        // Port matching depends on protocol
        match proto {
            Protocol::Icmp => {
                // For ICMP replies: src_port is ICMP ID in the reply
                // We match by external_port (which maps to internal ICMP ID)
                // Remote port doesn't matter for ICMP
                true
            }
            Protocol::Tcp | Protocol::Udp => {
                // For TCP/UDP: also check remote port
                self.remote_port == src_port
            }
        }
    }

    /// Update last activity timestamp
    pub fn touch(&mut self, now: u32) {
        self.last_activity = now;
    }

    /// Check if entry is expired
    pub fn is_expired(&self, now: u32, timeout: u32) -> bool {
        self.in_use && (now - self.last_activity) > timeout
    }
}

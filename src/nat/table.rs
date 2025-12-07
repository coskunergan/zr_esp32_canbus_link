//! NAT connection table

use super::entry::{NatEntry, Protocol};
use crate::packet::PacketContext;
use heapless::Vec;

const MAX_NAT_ENTRIES: usize = 32;
const NAT_TIMEOUT_SEC: u32 = 300; // 5 minutes
const PORT_RANGE_START: u16 = 10000;
const PORT_RANGE_END: u16 = 65535;

/// NAT configuration
pub struct NatConfig {
    /// Internal (LAN) network - packets FROM this network will be NAT'd
    pub internal_network: [u8; 4],
    pub internal_netmask: [u8; 4],

    /// External (WAN) IP - our public-facing IP
    pub external_ip: [u8; 4],
}

impl Default for NatConfig {
    fn default() -> Self {
        Self {
            // LAN: 192.168.4.0/24 (AP subnet)
            internal_network: [192, 168, 4, 0],
            internal_netmask: [255, 255, 255, 0],

            // WAN: 192.168.1.77 (STA IP on main network)
            external_ip: [192, 168, 1, 77],
        }
    }
}

pub struct NatTable {
    entries: Vec<NatEntry, MAX_NAT_ENTRIES>,
    next_port: u16,
    config: NatConfig,
}

impl NatTable {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_port: PORT_RANGE_START,
            config: NatConfig::default(),
        }
    }

    /// Check if IP is in internal network (should be NAT'd)
    fn is_internal_ip(&self, ip: &[u8; 4]) -> bool {
        for i in 0..4 {
            if (ip[i] & self.config.internal_netmask[i])
                != (self.config.internal_network[i] & self.config.internal_netmask[i])
            {
                return false;
            }
        }
        log::info!(
            "should be NAT'd ip = {}.{}.{}.{}",
            ip[0],
            ip[1],
            ip[2],
            ip[3]
        );
        true
    }

    /// Check if IP is our external IP (WAN interface)
    fn is_external_ip(&self, ip: &[u8; 4]) -> bool {
        log::info!(
            "NAT ip = {}.{}.{}.{}, external_ip = {}.{}.{}.{}",
            ip[0],
            ip[1],
            ip[2],
            ip[3],
            self.config.external_ip[0],
            self.config.external_ip[1],
            self.config.external_ip[2],
            self.config.external_ip[3]
        );
        ip == &self.config.external_ip
    }

    /// Get current uptime (mock - should call Zephyr's k_uptime_get_32)
    fn get_uptime() -> u32 {
        // TODO: Call Zephyr k_uptime_get_32() via FFI
        0
    }

    /// Allocate a new external port
    fn allocate_port(&mut self) -> u16 {
        let port = self.next_port;
        self.next_port += 1;
        if self.next_port > PORT_RANGE_END {
            self.next_port = PORT_RANGE_START;
        }
        port
    }

    /// Find existing NAT entry for outbound packet
    fn find_outbound(
        &self,
        src_ip: &[u8; 4],
        src_port: u16,
        dst_ip: &[u8; 4],
        dst_port: u16,
        proto: Protocol,
    ) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.matches_outbound(src_ip, src_port, dst_ip, dst_port, proto))
    }

    /// Find existing NAT entry for inbound packet
    fn find_inbound(
        &self,
        src_ip: &[u8; 4],
        src_port: u16,
        dst_port: u16,
        proto: Protocol,
    ) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.matches_inbound(src_ip, src_port, dst_port, proto))
    }

    /// Clean up expired entries
    fn cleanup(&mut self) {
        let now = Self::get_uptime();
        self.entries.retain(|e| !e.is_expired(now, NAT_TIMEOUT_SEC));
    }

    /// Translate outbound packet (LAN -> WAN)
    /// Only translate if source is from internal network
    pub fn translate_outbound(&mut self, ctx: &mut PacketContext) -> Result<(), ()> {
        // *** POLICY CHECK: Only NAT internal traffic ***
        if !self.is_internal_ip(&ctx.ip_hdr.src) {
            // Not from internal network, don't NAT
            return Ok(());
        }

        // Don't NAT if destination is also internal
        if self.is_internal_ip(&ctx.ip_hdr.dst) {
            // Internal-to-internal traffic, don't NAT
            return Ok(());
        }

        log::info!(
            ">>>>>>>> Start NAT proccess ip src = {}.{}.{}.{} >> ip dst = {}.{}.{}.{} ",
            &ctx.ip_hdr.src[0],
            &ctx.ip_hdr.src[1],
            &ctx.ip_hdr.src[2],
            &ctx.ip_hdr.src[3],
            &ctx.ip_hdr.dst[0],
            &ctx.ip_hdr.dst[1],
            &ctx.ip_hdr.dst[2],
            &ctx.ip_hdr.dst[3]
        );

        let proto = Protocol::from_u8(ctx.ip_hdr.proto).ok_or(())?;

        // Check if we already have an entry
        if let Some(idx) = self.find_outbound(
            &ctx.ip_hdr.src,
            ctx.src_port,
            &ctx.ip_hdr.dst,
            ctx.dst_port,
            proto,
        ) {
            // Update existing entry
            let entry = &mut self.entries[idx];
            entry.touch(Self::get_uptime());

            // Translate source IP and port
            ctx.ip_hdr.src = entry.external_ip;
            ctx.src_port = entry.external_port;
            ctx.needs_update = true;

            return Ok(());
        }

        // Create new entry
        self.cleanup(); // Make room if needed

        let external_port = self.allocate_port();
        let external_ip = self.config.external_ip;

        let mut entry = NatEntry::new();
        entry.internal_ip = ctx.ip_hdr.src;
        entry.internal_port = ctx.src_port;
        entry.external_ip = external_ip;
        entry.external_port = external_port;
        entry.remote_ip = ctx.ip_hdr.dst;
        entry.remote_port = ctx.dst_port;
        entry.protocol = proto;
        entry.last_activity = Self::get_uptime();
        entry.in_use = true;

        // Add to table
        self.entries.push(entry).map_err(|_| ())?;

        // Translate packet
        ctx.ip_hdr.src = external_ip;
        ctx.src_port = external_port;
        ctx.needs_update = true;

        Ok(())
    }

    /// Translate inbound packet (WAN -> LAN)
    /// Only translate if destination is our external IP
    pub fn translate_inbound(&mut self, ctx: &mut PacketContext) -> Result<(), ()> {
        // *** POLICY CHECK: Only NAT if destined to our external IP ***
        if !self.is_external_ip(&ctx.ip_hdr.dst) {
            // Not for us, don't NAT
            return Ok(());
        }

        let proto = Protocol::from_u8(ctx.ip_hdr.proto).ok_or(())?;

        // Find matching entry
        let idx = self
            .find_inbound(&ctx.ip_hdr.src, ctx.src_port, ctx.dst_port, proto)
            .ok_or(())?;

        let entry = &mut self.entries[idx];
        entry.touch(Self::get_uptime());

        // Translate destination IP and port
        ctx.ip_hdr.dst = entry.internal_ip;
        ctx.dst_port = entry.internal_port;
        ctx.needs_update = true;

        Ok(())
    }

    /// Set NAT configuration (called from C)
    pub fn set_config(&mut self, config: NatConfig) {
        self.config = config;
    }
}
